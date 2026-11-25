//! High-level OAuth 2.1 flow driver for the MCP client.
//!
//! Glue module: takes a request like "get me a fresh access token
//! for this MCP server" and runs the full authorization code flow
//! (with PKCE) end-to-end:
//!
//! 1. **Discovery**: Protected Resource Metadata (RFC 9728) and
//!    Authorization Server Metadata (RFC 8414).
//! 2. **Registration**: in priority order — pre-registered, Client
//!    ID Metadata Document, Dynamic Client Registration (RFC 7591),
//!    manual. Today we implement pre-registered (from config) and
//!    DCR; CIDM and manual prompts are out of scope for this round.
//! 3. **PKCE + state generation** (S256, 32 bytes each).
//! 4. **Loopback callback server** + browser launch.
//! 5. **Authorization code → token exchange**.
//! 6. **Cache the token** in the [`TokenStore`] for next time.
//!
//! Step-up authorization (MCP §4.7): if the resource server returns
//! 403 with `error="insufficient_scope"`, the driver can be called
//! again with `extra_scopes` to obtain a new token with additional
//! scopes, while keeping the same redirect URI / client_id.
//! Unit tests live in the sibling `flow_tests.rs` sidecar.

use std::sync::Arc;
use std::time::Duration;

use super::client::OAuthClient;
use super::discovery::{
    DISCOVERY_TIMEOUT, discover_authorization_server_metadata, discover_resource_metadata,
};
use super::pkce::{
    AuthorizationUrlInputs, PkcePair, State, build_authorization_url, percent_encode,
};
use super::redirect::{LoopbackServer, open_browser, start};
use super::store::{StoredToken, TokenStore};
use super::types::{
    AuthorizationServerMetadata, OAuthError, ProtectedResourceMetadata, TokenResponse,
    WwwAuthenticateChallenge,
};

/// Inputs to one OAuth flow run. Built by the caller (the session)
/// from the per-request context: MCP server URL, optional static
/// headers, and a pre-emptive `WWW-Authenticate` challenge if the
/// caller already saw a 401.
pub struct OAuthFlowInputs {
    /// Canonical MCP server URL. This becomes the `resource`
    /// parameter (RFC 8707) and the token store key.
    pub mcp_server_url: String,
    /// Optional pre-emptive `WWW-Authenticate` challenge from a 401
    /// that triggered this flow. If absent, the driver falls back
    /// to the well-known probe.
    pub www_authenticate: Option<WwwAuthenticateChallenge>,
    /// Additional scopes to request. The driver combines these
    /// with the server's advertised scopes per spec §4.5; if the
    /// caller wants to expand (step-up) they pass the new scopes
    /// here and the driver requests exactly the union.
    pub extra_scopes: Vec<String>,
    /// Per-flow deadline. The browser + token exchange together
    /// must complete within this. Default 2 minutes.
    pub timeout: Option<Duration>,
    /// Optional pre-registered client id + (optional) secret. If
    /// set, the driver skips dynamic registration. The secret is
    /// only used if the server's metadata lists
    /// `token_endpoint_auth_methods_supported` containing
    /// `client_secret_basic` / `client_secret_post`.
    pub pre_registered_client: Option<PreRegisteredClient>,
    /// Test seam: if set, the driver uses this loopback server
    /// instead of starting its own. Used by integration tests to
    /// inject a controlled callback.
    pub loopback_override: Option<LoopbackServer>,
    /// Test seam: if set, the driver calls this closure in place
    /// of `webbrowser::open` for the authorization step. Production
    /// code leaves this `None` and pops a real browser; tests
    /// substitute an HTTP-redirect-following client (e.g.
    /// `reqwest::blocking::Client::get`) pointed at a wiremock
    /// that 302-redirects to the loopback with `code` + `state`.
    /// The closure must return `Ok(())` on success and an
    /// `OAuthError` on failure (matching the production contract).
    pub browser_override: Option<BrowserOverride>,
}

/// Test-seam closure for the OAuth browser step. See
/// [`OAuthFlowInputs::browser_override`]. `Send + Sync` so the
/// flow can hold it in the same struct as the loopback server
/// without lifetime gymnastics.
pub type BrowserOverride = Arc<dyn Fn(&str) -> Result<(), OAuthError> + Send + Sync>;

// Manual `Debug` impl: the derived form would try to format
// `Arc<dyn Fn>`, which isn't `Debug`. In practice the
// `browser_override` is a test-only field; we print its
// presence/absence and the rest of the struct as usual.
impl std::fmt::Debug for OAuthFlowInputs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuthFlowInputs")
            .field("mcp_server_url", &self.mcp_server_url)
            .field("www_authenticate", &self.www_authenticate)
            .field("extra_scopes", &self.extra_scopes)
            .field("timeout", &self.timeout)
            .field("pre_registered_client", &self.pre_registered_client)
            .field("loopback_override", &self.loopback_override)
            .field(
                "browser_override",
                &self.browser_override.as_ref().map(|_| "<fn>"),
            )
            .finish()
    }
}

impl Clone for OAuthFlowInputs {
    fn clone(&self) -> Self {
        Self {
            mcp_server_url: self.mcp_server_url.clone(),
            www_authenticate: self.www_authenticate.clone(),
            extra_scopes: self.extra_scopes.clone(),
            timeout: self.timeout,
            pre_registered_client: self.pre_registered_client.clone(),
            loopback_override: self.loopback_override.clone(),
            browser_override: self.browser_override.clone(),
        }
    }
}

/// Pre-registered OAuth client. Comes from the MCP server's
/// `headers: { Authorization: "Bearer <client_id>:<client_secret>" }`
/// convention, or from out-of-band setup.
#[derive(Debug, Clone)]
pub struct PreRegisteredClient {
    pub client_id: String,
    pub client_secret: Option<String>,
}

/// Output of one OAuth flow run. Includes the freshly minted token
/// and the client/AS pair the driver used (so the caller can store
/// them for refresh).
#[derive(Debug, Clone)]
pub struct OAuthFlowOutput {
    /// The new token.
    pub token: StoredToken,
    /// The client id used (may be newly registered).
    pub client_id: String,
    /// The AS issuer URL the token came from.
    pub issuer: String,
    /// The authorization server's metadata, cached so the next
    /// refresh doesn't re-fetch.
    pub as_metadata: AuthorizationServerMetadata,
    /// The resource server's metadata, cached the same way.
    pub resource_metadata: ProtectedResourceMetadata,
}

/// Run one full OAuth flow. Returns the minted token (also cached
/// in `store` on success). The driver is synchronous; the loopback
/// server runs on a background thread but the flow itself blocks
/// until the user completes (or fails) the browser step.
pub fn run_flow(
    inputs: &OAuthFlowInputs,
    store: &TokenStore,
) -> Result<OAuthFlowOutput, OAuthError> {
    // Step 1: discovery.
    let resource = build_resource_uri(&inputs.mcp_server_url)?;
    let resource_metadata =
        discover_resource_metadata(&resource, inputs.www_authenticate.as_ref())?;
    let as_url = resource_metadata
        .authorization_servers
        .first()
        .ok_or_else(|| {
            OAuthError::ResourceMetadata(format!(
                "resource metadata for {resource} has no authorization_servers"
            ))
        })?
        .clone();
    let as_metadata = discover_authorization_server_metadata(&as_url)?;
    // Spec §4.9: client MUST verify PKCE support before proceeding.
    if !as_metadata
        .code_challenge_methods_supported
        .iter()
        .any(|m| m.eq_ignore_ascii_case("S256"))
    {
        return Err(OAuthError::PkceNotSupported);
    }
    // Spec §4.9: "Use only `localhost` or HTTPS redirect URIs."
    if !as_metadata.authorization_endpoint.starts_with("https://")
        && !as_metadata
            .authorization_endpoint
            .starts_with("http://127.0.0.1:")
        && !as_metadata
            .authorization_endpoint
            .starts_with("http://localhost:")
    {
        return Err(OAuthError::SpecViolation(format!(
            "authorization endpoint {} is not https://",
            as_metadata.authorization_endpoint
        )));
    }
    if !as_metadata.token_endpoint.starts_with("https://")
        && !as_metadata.token_endpoint.starts_with("http://127.0.0.1:")
        && !as_metadata.token_endpoint.starts_with("http://localhost:")
    {
        return Err(OAuthError::SpecViolation(format!(
            "token endpoint {} is not https://",
            as_metadata.token_endpoint
        )));
    }

    // Step 2: registration.
    let client = OAuthClient::resolve(
        inputs.pre_registered_client.as_ref(),
        &as_metadata,
        &resource,
    )?;

    // Step 3: scope selection per spec §4.5.
    let scope = pick_scope(
        inputs.www_authenticate.as_ref(),
        &resource_metadata,
        &inputs.extra_scopes,
    );

    // Step 4: PKCE + state.
    let pkce = PkcePair::generate();
    let state = State::generate();

    // Step 5: start the loopback server.
    let server = match inputs.loopback_override.clone() {
        Some(s) => s,
        None => start(None, inputs.timeout)?,
    };
    let redirect_uri = server.redirect_uri.clone();

    tracing::info!(
        mcp_server = %resource,
        redirect_uri = %redirect_uri,
        authorization_endpoint = %as_metadata.authorization_endpoint,
        "loopback server started; will use this redirect URI for OAuth"
    );

    // Step 6: build the authorization URL.
    let auth_url = build_authorization_url(&AuthorizationUrlInputs {
        authorization_endpoint: &as_metadata.authorization_endpoint,
        client_id: &client.client_id,
        redirect_uri: &redirect_uri,
        state: state.as_str(),
        pkce: &pkce,
        scope: scope.as_deref(),
        resource: &resource,
    })
    .map_err(|e| OAuthError::Internal(format!("build authorization url: {e}")))?;

    tracing::info!(
        mcp_server = %resource,
        issuer = %as_metadata.issuer,
        client_id = %client.client_id,
        authorization_url = %auth_url,
        redirect_uri = %redirect_uri,
        "starting OAuth authorization flow — open this URL in browser if auto-open fails"
    );

    // Step 7: open the browser. Best-effort: if the browser fails
    // to launch, we still let the caller print the URL themselves.
    // The `browser_override` seam lets integration tests substitute
    // a non-browser path (typically an HTTP-redirect-following
    // client pointed at a wiremock) without touching the
    // production code path.
    if let Some(override_fn) = &inputs.browser_override {
        if let Err(e) = override_fn(&auth_url) {
            tracing::warn!(
                error = %e,
                authorization_url = %auth_url,
                "browser override returned an error"
            );
        }
    } else if let Err(e) = open_browser(&auth_url) {
        tracing::warn!(
            error = %e,
            authorization_url = %auth_url,
            "failed to open browser; user must navigate manually"
        );
    }

    // Step 8: wait for the callback.
    let params = server.wait_for_code()?;
    if params.state != state.as_str() {
        return Err(OAuthError::StateMismatch);
    }

    // Step 9: exchange the code for a token.
    let token_response = exchange_code(
        &as_metadata,
        &client,
        &redirect_uri,
        &params.code,
        &pkce.verifier,
        &resource,
    )?;
    if !token_response.is_bearer() {
        return Err(OAuthError::Protocol(format!(
            "token endpoint returned token_type '{}', expected 'Bearer'",
            token_response.token_type
        )));
    }

    let now = std::time::SystemTime::now();
    let stored = StoredToken::from_response(
        &token_response,
        now,
        Some(client.client_id.clone()),
        Some(as_metadata.issuer.clone()),
    );
    store.put(&resource, stored.clone());

    Ok(OAuthFlowOutput {
        token: stored,
        client_id: client.client_id,
        issuer: as_metadata.issuer.clone(),
        as_metadata,
        resource_metadata,
    })
}

/// Refresh an existing token using its refresh token. The caller
/// must have already determined that the access token is no longer
/// fresh (or that the resource server returned 401/403).
pub fn refresh(
    inputs: &OAuthFlowInputs,
    store: &TokenStore,
    existing: &StoredToken,
) -> Result<OAuthFlowOutput, OAuthError> {
    let refresh_token = existing
        .refresh_token
        .as_ref()
        .ok_or_else(|| OAuthError::RefreshFailed("stored token has no refresh_token".to_owned()))?;
    let resource = build_resource_uri(&inputs.mcp_server_url)?;
    let resource_metadata =
        discover_resource_metadata(&resource, inputs.www_authenticate.as_ref())?;
    let as_url = resource_metadata
        .authorization_servers
        .first()
        .ok_or_else(|| {
            OAuthError::ResourceMetadata(format!(
                "resource metadata for {resource} has no authorization_servers"
            ))
        })?
        .clone();
    let as_metadata = discover_authorization_server_metadata(&as_url)?;
    let client = OAuthClient::from_stored(
        existing
            .client_id
            .clone()
            .ok_or_else(|| OAuthError::RefreshFailed("no client_id on stored token".to_owned()))?,
        existing
            .issuer
            .clone()
            .ok_or_else(|| OAuthError::RefreshFailed("no issuer on stored token".to_owned()))?,
        &as_metadata,
        inputs.pre_registered_client.as_ref(),
    )?;

    let token_response = refresh_with_token(
        &as_metadata,
        &client,
        refresh_token,
        &resource,
        &inputs.extra_scopes,
    )?;
    let now = std::time::SystemTime::now();
    let stored = StoredToken::from_response(
        &token_response,
        now,
        Some(client.client_id.clone()),
        Some(as_metadata.issuer.clone()),
    );
    store.put(&resource, stored.clone());
    Ok(OAuthFlowOutput {
        token: stored,
        client_id: client.client_id,
        issuer: as_metadata.issuer.clone(),
        as_metadata,
        resource_metadata,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Reduce the MCP server URL to a canonical "resource" URI per
/// RFC 8707. The spec shows examples like
/// `https://mcp.example.com/mcp` or `https://mcp.example.com`.
/// We strip the fragment, drop the query string, and otherwise
/// leave the URL alone.
pub fn build_resource_uri(mcp_server_url: &str) -> Result<String, OAuthError> {
    let trimmed = mcp_server_url.trim();
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return Err(OAuthError::SpecViolation(format!(
            "MCP server URL '{trimmed}' is not http(s); resource parameter requires https (or http for loopback)"
        )));
    }
    // Drop the fragment and query string.
    let without_fragment = trimmed.split('#').next().unwrap_or(trimmed);
    let without_query = without_fragment
        .split('?')
        .next()
        .unwrap_or(without_fragment);
    Ok(without_query.to_owned())
}

/// Scope selection per spec §4.5.
fn pick_scope(
    www_authenticate: Option<&WwwAuthenticateChallenge>,
    resource_metadata: &ProtectedResourceMetadata,
    extra_scopes: &[String],
) -> Option<String> {
    // 1. The `scope` parameter from the initial WWW-Authenticate
    //    header (if present).
    if let Some(ch) = www_authenticate
        && let Some(scope) = ch.get("scope")
        && !scope.trim().is_empty()
    {
        let mut all: Vec<String> = scope.split_whitespace().map(str::to_owned).collect();
        for s in extra_scopes {
            if !all.iter().any(|x| x == s) {
                all.push(s.clone());
            }
        }
        return Some(all.join(" "));
    }
    // 2. All scopes_supported from the resource metadata, plus
    //    any extra_scopes (deduplicated).
    let mut all: Vec<String> = resource_metadata.scopes_supported.clone();
    for s in extra_scopes {
        if !all.iter().any(|x| x == s) {
            all.push(s.clone());
        }
    }
    if all.is_empty() {
        None
    } else {
        Some(all.join(" "))
    }
}

/// Exchange an authorization code for a token.
fn exchange_code(
    as_metadata: &AuthorizationServerMetadata,
    client: &OAuthClient,
    redirect_uri: &str,
    code: &str,
    code_verifier: &str,
    resource: &str,
) -> Result<TokenResponse, OAuthError> {
    let mut form: Vec<(&'static str, String)> = vec![
        ("grant_type", "authorization_code".to_owned()),
        ("code", code.to_owned()),
        ("redirect_uri", redirect_uri.to_owned()),
        ("code_verifier", code_verifier.to_owned()),
        ("resource", resource.to_owned()),
    ];
    if let Some(secret) = &client.client_secret {
        // Confidential clients send the secret in the form body.
        form.push(("client_id", client.client_id.clone()));
        form.push(("client_secret", secret.clone()));
    } else {
        form.push(("client_id", client.client_id.clone()));
    }
    let body = encode_form(&form);
    // The shared `token_client()` below applies `DISCOVERY_TIMEOUT`
    // globally on the blocking client. We add the per-request
    // headers (Accept, Content-Type, and any per-client extras)
    // inside `send_token_request`.
    send_token_request(
        &token_client(),
        &as_metadata.token_endpoint,
        &client.extra_headers,
        &body,
        "authorization_code",
    )
}

fn refresh_with_token(
    as_metadata: &AuthorizationServerMetadata,
    client: &OAuthClient,
    refresh_token: &str,
    resource: &str,
    extra_scopes: &[String],
) -> Result<TokenResponse, OAuthError> {
    let mut form: Vec<(&'static str, String)> = vec![
        ("grant_type", "refresh_token".to_owned()),
        ("refresh_token", refresh_token.to_owned()),
        ("resource", resource.to_owned()),
    ];
    if !extra_scopes.is_empty() {
        form.push(("scope", extra_scopes.join(" ")));
    }
    if let Some(secret) = &client.client_secret {
        form.push(("client_id", client.client_id.clone()));
        form.push(("client_secret", secret.clone()));
    } else {
        form.push(("client_id", client.client_id.clone()));
    }
    let body = encode_form(&form);
    send_token_request(
        &token_client(),
        &as_metadata.token_endpoint,
        &client.extra_headers,
        &body,
        "refresh_token",
    )
}

fn encode_form(form: &[(impl AsRef<str>, String)]) -> String {
    form.iter()
        .map(|(k, v)| format!("{}={}", percent_encode(k.as_ref()), percent_encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

/// Build the shared `reqwest::blocking::Client` used for OAuth
/// token requests.
///
/// * Default 4xx/5xx handling — reqwest's blocking client does not
///   raise on 4xx/5xx by default, so the response body is available
///   for parsing the OAuth error envelope.
/// * `DISCOVERY_TIMEOUT` — global request timeout. The same client
///   serves both the `authorization_code` and `refresh_token` grants.
fn token_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(DISCOVERY_TIMEOUT)
        .build()
        .expect("reqwest::blocking::Client builder should not fail with default configuration")
}

fn send_token_request(
    client: &reqwest::blocking::Client,
    url: &str,
    extra_headers: &std::collections::HashMap<String, String>,
    body: &str,
    grant: &str,
) -> Result<TokenResponse, OAuthError> {
    let mut req = client
        .post(url)
        .header("Accept", "application/json")
        .header("Content-Type", "application/x-www-form-urlencoded");
    for (k, v) in extra_headers {
        req = req.header(k.as_str(), v.as_str());
    }
    let resp = req.body(body.to_string()).send();
    match resp {
        Ok(r) => {
            if r.status().as_u16() >= 400 {
                let code = r.status().as_u16();
                let err_body = r.text().unwrap_or_default();
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&err_body)
                    && let Some(err) = super::types::OAuthErrorBody::from_json(&value)
                {
                    return Err(OAuthError::OAuth {
                        endpoint: "token",
                        body: err,
                    });
                }
                return Err(OAuthError::Transport(format!(
                    "token endpoint returned HTTP {code}: {err_body}; grant={grant}"
                )));
            }
            let body = r
                .text()
                .map_err(|e| OAuthError::Transport(format!("token response read: {e}")))?;
            let value: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
                OAuthError::Protocol(format!("token response json: {e}; body: {body}"))
            })?;
            if let Some(err) = super::types::OAuthErrorBody::from_json(&value) {
                return Err(OAuthError::OAuth {
                    endpoint: "token",
                    body: err,
                });
            }
            serde_json::from_value::<TokenResponse>(value)
                .map_err(|e| OAuthError::Protocol(format!("token response shape: {e}")))
        }
        Err(e) => Err(OAuthError::Transport(format!("token request: {e}"))),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "flow_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "flow_proptests.rs"]
mod flow_proptests;
