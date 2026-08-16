//! OAuth 2.1 types used by the MCP client authorization flow.
//!
//! Covers:
//! * Protected Resource Metadata (RFC 9728) — `ProtectedResourceMetadata`
//! * Authorization Server Metadata (RFC 8414) — `AuthorizationServerMetadata`
//! * Dynamic Client Registration request/response (RFC 7591)
//! * OAuth error envelope parsing
//!
//! Per the MCP 2025-11-25 spec, the client extracts the authorization
//! server URL(s) from the protected resource metadata, then fetches
//! the authorization server metadata using the well-known URL rules
//! in §4.3.1 of `doc/distill/mcp.md`.

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// Protected Resource Metadata (RFC 9728)
// ---------------------------------------------------------------------------

/// RFC 9728 Protected Resource Metadata document. The MCP server
/// advertises this document via a `WWW-Authenticate` header on a 401
/// or by the well-known URI `/.well-known/oauth-protected-resource[/<path>]`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProtectedResourceMetadata {
    /// The protected resource's canonical URI. Per RFC 9728 §2 this
    /// MUST be a URL (typically the MCP server URL itself).
    pub resource: String,

    /// URLs of the authorization servers that may issue tokens for
    /// this resource. The client tries each in order until one yields
    /// a valid metadata document.
    #[serde(default)]
    pub authorization_servers: Vec<String>,

    /// Scopes supported by the resource. The client may use this
    /// list as a fallback when the `WWW-Authenticate` `scope` parameter
    /// is absent (MCP spec §4.5).
    #[serde(default)]
    pub scopes_supported: Vec<String>,

    /// Methods clients can use to send a bearer token to the resource.
    /// Per RFC 9728. `"header"` is the only one we use today; the
    /// spec explicitly forbids URI query parameters.
    #[serde(default)]
    pub bearer_methods_supported: Vec<String>,

    /// Human-readable resource name.
    #[serde(default)]
    pub resource_name: Option<String>,

    /// Resource documentation URL.
    #[serde(default)]
    pub resource_documentation: Option<String>,

    /// Resource policy URL.
    #[serde(default)]
    pub resource_policy_uri: Option<String>,

    /// Resource terms-of-service URL.
    #[serde(default)]
    pub resource_tos_uri: Option<String>,
}

// ---------------------------------------------------------------------------
// Authorization Server Metadata (RFC 8414)
// ---------------------------------------------------------------------------

/// RFC 8414 Authorization Server Metadata document. Fetched from the
/// well-known URI per MCP spec §4.3.1.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct AuthorizationServerMetadata {
    /// Issuer identifier URL of the authorization server.
    pub issuer: String,

    /// Authorization endpoint. Must use HTTPS in production.
    pub authorization_endpoint: String,

    /// Token endpoint. Must use HTTPS in production.
    pub token_endpoint: String,

    /// Optional JWKS URI. Used when validating access tokens locally.
    #[serde(default)]
    pub jwks_uri: Option<String>,

    /// Optional registration endpoint. Presence indicates Dynamic
    /// Client Registration (RFC 7591) is supported.
    #[serde(default)]
    pub registration_endpoint: Option<String>,

    /// Scopes the server understands.
    #[serde(default)]
    pub scopes_supported: Vec<String>,

    /// Response types the server supports. We use `"code"`.
    #[serde(default)]
    pub response_types_supported: Vec<String>,

    /// PKCE code challenge methods the server supports. MCP spec
    /// §4.9 REQUIRES `S256`; the client verifies it's listed before
    /// proceeding.
    #[serde(default)]
    pub code_challenge_methods_supported: Vec<String>,

    /// Grant types the server supports. We use
    /// `authorization_code` and optionally `refresh_token`.
    #[serde(default)]
    pub grant_types_supported: Vec<String>,

    /// Token endpoint auth methods for confidential clients. We use
    /// `none` (public client) by default and `private_key_jwt` if
    /// the client hosts a Client ID Metadata Document.
    #[serde(default)]
    pub token_endpoint_auth_methods_supported: Vec<String>,

    /// Whether the server supports Client ID Metadata Documents
    /// (a recent draft; the server advertises support via
    /// `client_id_metadata_document_supported`).
    #[serde(default)]
    pub client_id_metadata_document_supported: bool,

    /// Service documentation URL.
    #[serde(default)]
    pub service_documentation: Option<String>,

    /// UI locales supported by the server.
    #[serde(default)]
    pub ui_locales_supported: Vec<String>,
}

// ---------------------------------------------------------------------------
// Dynamic Client Registration (RFC 7591) — request/response
// ---------------------------------------------------------------------------

/// RFC 7591 §3.1 Dynamic Client Registration request. Only the
/// fields the MCP client needs are modeled; unknown server-side
/// fields are tolerated via `serde(default)` on the response.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ClientRegistrationRequest {
    /// Array of redirection URIs used by the client (RFC 7591 §2).
    pub redirect_uris: Vec<String>,

    /// Human-readable client name. Optional but recommended.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_name: Option<String>,

    /// URI of the client's home page. Optional.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_uri: Option<String>,

    /// URL of the client's logo. Optional.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo_uri: Option<String>,

    /// Token endpoint authentication method. `"none"` for public
    /// clients (no client secret). We do not request `"client_secret_basic"`
    /// or `"client_secret_post"` because the desktop app cannot
    /// keep a long-lived secret.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_endpoint_auth_method: Option<String>,

    /// Grant types the client will use. We always include
    /// `authorization_code` and `refresh_token`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grant_types: Option<Vec<String>>,

    /// Response types the client will use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_types: Option<Vec<String>>,

    /// Scopes the client will request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,

    /// Software identifier / version. The client populates these with
    /// the values it sends in the `initialize` `clientInfo`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub software_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub software_version: Option<String>,
}

/// RFC 7591 §3.2 Dynamic Client Registration response. Only the
/// fields we use are required; everything else is optional so the
/// server can return additional metadata.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ClientRegistrationResponse {
    /// Assigned client identifier.
    pub client_id: String,

    /// Client secret (only for confidential clients). Public clients
    /// (`token_endpoint_auth_method = "none"`) do not get one.
    #[serde(default)]
    pub client_secret: Option<String>,

    /// Absolute lifetime of the client secret, in seconds, if issued.
    #[serde(default)]
    pub client_secret_expires_at: Option<i64>,

    /// Redirect URIs the server accepted (may be a subset of what
    /// we asked for).
    #[serde(default)]
    pub redirect_uris: Vec<String>,

    /// Token endpoint auth method the server assigned.
    #[serde(default)]
    pub token_endpoint_auth_method: Option<String>,

    /// Registration access token used to update the registration.
    /// Not used by us today; included for forward compatibility.
    #[serde(default)]
    pub registration_access_token: Option<String>,

    /// Registration client URI for updates.
    #[serde(default)]
    pub registration_client_uri: Option<String>,

    /// Lifetime in seconds after which the client SHOULD reregister.
    #[serde(default)]
    pub client_id_issued_at: Option<i64>,
}

// ---------------------------------------------------------------------------
// Token response
// ---------------------------------------------------------------------------

/// RFC 6749 §5.1 successful token response. RFC 6749 §5.1 also
/// defines an `error` envelope; we surface that via
/// [`OAuthError::OAuth`].
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct TokenResponse {
    /// The access token issued by the authorization server.
    pub access_token: String,

    /// Token type. We only ever act on `"Bearer"`.
    pub token_type: String,

    /// Lifetime in seconds. Used to schedule proactive refresh.
    #[serde(default)]
    pub expires_in: Option<u64>,

    /// Refresh token, if the server issued one.
    #[serde(default)]
    pub refresh_token: Option<String>,

    /// Scopes the token is scoped to (space-separated string per
    /// RFC 6749; some servers return a JSON array — we accept both).
    #[serde(default, deserialize_with = "deserialize_scope")]
    pub scope: Vec<String>,
}

impl TokenResponse {
    /// `true` if the response token type is the bearer kind we use.
    pub fn is_bearer(&self) -> bool {
        self.token_type.eq_ignore_ascii_case("bearer")
    }
}

fn deserialize_scope<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Null => Ok(Vec::new()),
        serde_json::Value::String(s) => Ok(s.split_whitespace().map(str::to_owned).collect()),
        serde_json::Value::Array(arr) => arr
            .into_iter()
            .map(|v| match v {
                serde_json::Value::String(s) => Ok(s),
                other => Err(D::Error::custom(format!(
                    "scope array must contain strings, got {other}"
                ))),
            })
            .collect(),
        other => Err(D::Error::custom(format!(
            "scope must be string or array, got {other}"
        ))),
    }
}

// ---------------------------------------------------------------------------
// WWW-Authenticate parser (RFC 6750 §3 + RFC 7235)
// ---------------------------------------------------------------------------

/// Parsed form of an HTTP `WWW-Authenticate: Bearer …` header. Per
/// the MCP spec §4.3 the client extracts `resource_metadata` from
/// the challenge and (in §4.5) `scope` and `error`. RFC 6750 §3
/// also defines `error="invalid_token"` (caller-side token problem)
/// and `error="insufficient_scope"` (step-up trigger).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WwwAuthenticateChallenge {
    /// The auth scheme (`"Bearer"`, etc).
    pub scheme: String,
    /// All `key="value"` parameters as name/value pairs. Quotes are
    /// stripped and case is preserved.
    pub params: Vec<(String, String)>,
}

impl WwwAuthenticateChallenge {
    /// Get the value of a named parameter (case-insensitive on the
    /// key, per RFC 7235 §2.2).
    pub fn get(&self, key: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }
}

impl fmt::Display for WwwAuthenticateChallenge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ", self.scheme)?;
        for (i, (k, v)) in self.params.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            if Self::is_token(v) {
                write!(f, "{k}={v}")?;
            } else {
                write!(
                    f,
                    "{k}=\"{}\"",
                    v.replace('\\', r"\\").replace('"', r#"\""#)
                )?;
            }
        }
        Ok(())
    }
}

impl WwwAuthenticateChallenge {
    /// Per RFC 7235, a parameter value that matches the `token`
    /// production (no separators) does not need to be quoted. We
    /// keep this permissive — anything containing a comma, semicolon,
    /// space, or quote gets quoted.
    fn is_token(s: &str) -> bool {
        !s.is_empty()
            && s.bytes()
                .all(|b| !matches!(b, b' ' | b',' | b';' | b'\\' | b'"' | b'(' | b')'))
    }
}

/// Parse an HTTP `WWW-Authenticate` header value. The header may
/// contain multiple comma-separated challenges (RFC 7235 §2.1);
/// this function returns the first `Bearer` challenge. Returns
/// `None` if no `Bearer` challenge is present or the header cannot
/// be parsed.
pub fn parse_bearer_challenge(header: &str) -> Option<WwwAuthenticateChallenge> {
    // Find the 'Bearer' challenge in the WWW-Authenticate header.
    // Multiple challenges may be present (e.g., 'Basic realm="...", Bearer realm="..."').
    let lower_header = header.to_ascii_lowercase();
    let idx = lower_header.find("bearer ")?;
    let rest = &header[idx + 7..];

    // If another challenge follows (e.g. ', Basic '), cap rest at that comma.
    // However, commas inside parameters (e.g. scope="a, b") or separating parameters
    // (e.g. error="...", scope="...") do NOT start a new challenge unless followed by a scheme name.
    let params_str = if let Some(end_idx) = find_next_challenge_start(rest) {
        &rest[..end_idx]
    } else {
        rest
    };

    let params = parse_auth_params(params_str);
    Some(WwwAuthenticateChallenge {
        scheme: "Bearer".to_owned(),
        params,
    })
}

fn find_next_challenge_start(s: &str) -> Option<usize> {
    let mut in_quotes = false;
    let mut escape = false;
    for (i, c) in s.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if c == '\\' && in_quotes {
            escape = true;
            continue;
        }
        if c == '"' {
            in_quotes = !in_quotes;
            continue;
        }
        if c == ',' && !in_quotes {
            let after = s[i + 1..].trim_start();
            // A new challenge starts with a scheme name followed by space/equals, e.g. "Basic " or "Digest "
            if let Some(space_idx) = after.find(char::is_whitespace) {
                let token = &after[..space_idx];
                if !token.contains('=')
                    && token
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
                {
                    return Some(i);
                }
            }
        }
    }
    None
}

/// Parse comma-separated `key=value` parameters after the scheme name.
/// Values may be quoted; quotes are stripped.
fn parse_auth_params(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut in_quotes = false;
    let mut escape = false;
    for c in s.chars() {
        if escape {
            buf.push(c);
            escape = false;
            continue;
        }
        if c == '\\' && in_quotes {
            escape = true;
            continue;
        }
        if c == '"' {
            in_quotes = !in_quotes;
            continue;
        }
        if c == ',' && !in_quotes {
            push_param(&mut out, &buf);
            buf.clear();
            continue;
        }
        buf.push(c);
    }
    push_param(&mut out, &buf);
    out
}

fn push_param(out: &mut Vec<(String, String)>, buf: &str) {
    let s = buf.trim();
    if s.is_empty() {
        return;
    }
    if let Some((k, v)) = s.split_once('=') {
        let val = v.trim();
        let unquoted = if val.starts_with('"') && val.ends_with('"') && val.len() >= 2 {
            &val[1..val.len() - 1]
        } else {
            val
        };
        out.push((k.trim().to_owned(), unquoted.to_owned()));
    } else {
        out.push((String::new(), s.to_owned()));
    }
}

// ---------------------------------------------------------------------------
// OAuth error envelope (RFC 6749 §5.2 / §4.1.2.1)
// ---------------------------------------------------------------------------

/// Parsed OAuth 2.0 / 2.1 error envelope. Returned by the token
/// endpoint (RFC 6749 §5.2), the authorization endpoint
/// (RFC 6749 §4.1.2.1), or the registration endpoint (RFC 7591 §3.2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthErrorBody {
    /// Error code. RFC 6749 enumerates: `invalid_request`,
    /// `invalid_client`, `invalid_grant`, `unauthorized_client`,
    /// `unsupported_grant_type`, `invalid_scope`, `server_error`,
    /// `temporarily_unavailable`.
    pub error: String,
    /// Optional human-readable description.
    pub error_description: Option<String>,
    /// Optional URI for the error in the server documentation.
    pub error_uri: Option<String>,
}

impl OAuthErrorBody {
    /// Parse from a JSON object, accepting any case for the keys.
    pub fn from_json(value: &serde_json::Value) -> Option<Self> {
        let obj = value.as_object()?;
        let error = obj
            .get("error")
            .and_then(|v| v.as_str())
            .map(str::to_owned)?;
        let error_description = obj
            .get("error_description")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        let error_uri = obj
            .get("error_uri")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        Some(Self {
            error,
            error_description,
            error_uri,
        })
    }
}

// ---------------------------------------------------------------------------
// OAuthError — the public error type for the OAuth module
// ---------------------------------------------------------------------------

/// Top-level OAuth error. Implements `Display` and `std::error::Error`
/// for ergonomic use across the rest of the codebase. Categorized so
/// callers (e.g. the session) can branch on the failure mode.
#[derive(Debug, Clone)]
pub enum OAuthError {
    /// A network-level failure (DNS, TCP, TLS, etc).
    Transport(String),
    /// The server returned an HTTP status the client doesn't know
    /// how to handle, or returned malformed JSON.
    Protocol(String),
    /// Protected Resource Metadata (RFC 9728) discovery failed.
    /// The string carries the detail.
    ResourceMetadata(String),
    /// Authorization Server Metadata (RFC 8414) discovery failed.
    AuthorizationServerMetadata(String),
    /// Dynamic Client Registration (RFC 7591) failed.
    ClientRegistration(String),
    /// The authorization server returned an `error` envelope on
    /// the authorization or token endpoint.
    OAuth {
        /// Which endpoint returned the error.
        endpoint: &'static str,
        /// Parsed OAuth error body.
        body: OAuthErrorBody,
    },
    /// The user denied the authorization request, or the redirect
    /// callback carried an `error` parameter.
    AuthorizationDenied(String),
    /// The redirect callback's `state` did not match what we sent.
    /// Per spec §4.9, this is a hard error.
    StateMismatch,
    /// The user closed the loopback browser before completing the
    /// flow (no callback ever arrived).
    CallbackTimeout(String),
    /// The user declined to open the browser (e.g. headless environment
    /// with no UI).
    UserCancelled,
    /// Spec violation: something the spec REQUIRES but we couldn't
    /// obtain (e.g. an HTTPS-only redirect URI when the configured
    /// one is `http://`, or a server that doesn't list `S256`).
    SpecViolation(String),
    /// The server's authorization server metadata does not list
    /// `S256` in `code_challenge_methods_supported`. Spec §4.9
    /// says the client MUST verify PKCE support and refuse to
    /// proceed otherwise.
    PkceNotSupported,
    /// Refreshing the access token failed (e.g. refresh token
    /// revoked, server returned `invalid_grant`).
    RefreshFailed(String),
    /// Generic catch-all for unexpected internal failures.
    Internal(String),
}

impl fmt::Display for OAuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OAuthError::Transport(s) => write!(f, "OAuth transport error: {s}"),
            OAuthError::Protocol(s) => write!(f, "OAuth protocol error: {s}"),
            OAuthError::ResourceMetadata(s) => {
                write!(f, "Protected Resource Metadata discovery failed: {s}")
            }
            OAuthError::AuthorizationServerMetadata(s) => {
                write!(f, "Authorization Server Metadata discovery failed: {s}")
            }
            OAuthError::ClientRegistration(s) => {
                write!(f, "Dynamic Client Registration failed: {s}")
            }
            OAuthError::OAuth { endpoint, body } => {
                write!(f, "{endpoint} returned OAuth error '{}'", body.error)?;
                if let Some(desc) = &body.error_description {
                    write!(f, ": {desc}")?;
                }
                Ok(())
            }
            OAuthError::AuthorizationDenied(s) => write!(f, "authorization denied: {s}"),
            OAuthError::StateMismatch => write!(
                f,
                "OAuth state mismatch (redirect callback did not match sent state)"
            ),
            OAuthError::CallbackTimeout(s) => write!(f, "OAuth callback timeout: {s}"),
            OAuthError::UserCancelled => write!(f, "OAuth flow cancelled by user"),
            OAuthError::SpecViolation(s) => write!(f, "OAuth spec violation: {s}"),
            OAuthError::PkceNotSupported => write!(
                f,
                "server does not advertise PKCE S256 in code_challenge_methods_supported"
            ),
            OAuthError::RefreshFailed(s) => write!(f, "OAuth refresh failed: {s}"),
            OAuthError::Internal(s) => write!(f, "OAuth internal error: {s}"),
        }
    }
}

impl std::error::Error for OAuthError {}

impl From<OAuthError> for String {
    fn from(err: OAuthError) -> Self {
        err.to_string()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bearer_challenge_with_resource_metadata() {
        let header = r#"Bearer realm="example", resource_metadata="https://mcp.example.com/.well-known/oauth-protected-resource""#;
        let ch = parse_bearer_challenge(header).expect("must parse");
        assert_eq!(ch.scheme, "Bearer");
        assert_eq!(ch.get("realm"), Some("example"));
        assert_eq!(
            ch.get("resource_metadata"),
            Some("https://mcp.example.com/.well-known/oauth-protected-resource")
        );
    }

    #[test]
    fn parses_challenge_with_scope_and_error() {
        // Step-up scenario (MCP spec §4.7).
        let header = r#"Bearer error="insufficient_scope", scope="read write", resource_metadata="https://mcp.example.com/.well-known/oauth-protected-resource""#;
        let ch = parse_bearer_challenge(header).expect("must parse");
        assert_eq!(ch.get("error"), Some("insufficient_scope"));
        assert_eq!(ch.get("scope"), Some("read write"));
        assert!(ch.get("resource_metadata").is_some());
    }

    #[test]
    fn parses_bare_token_value() {
        let header = r#"Bearer realm=api, scope=read""#;
        let ch = parse_bearer_challenge(header).expect("must parse");
        assert_eq!(ch.get("realm"), Some("api"));
        assert_eq!(ch.get("scope"), Some("read"));
    }

    #[test]
    fn ignores_non_bearer_challenges() {
        let header = r#"Basic realm="api", Bearer realm="mcp", scope="tools/read""#;
        let ch = parse_bearer_challenge(header).expect("must parse");
        assert_eq!(ch.scheme, "Bearer");
        assert_eq!(ch.get("scope"), Some("tools/read"));
    }

    #[test]
    fn handles_value_with_comma_in_quotes() {
        let header = r#"Bearer error="invalid_token", error_description="a, b, c""#;
        let ch = parse_bearer_challenge(header).expect("must parse");
        assert_eq!(ch.get("error"), Some("invalid_token"));
        assert_eq!(ch.get("error_description"), Some("a, b, c"));
    }

    #[test]
    fn returns_none_when_no_bearer_challenge() {
        assert!(parse_bearer_challenge(r#"Basic realm="api""#).is_none());
    }

    #[test]
    fn empty_header_returns_none() {
        assert!(parse_bearer_challenge("").is_none());
        assert!(parse_bearer_challenge("   ").is_none());
    }

    #[test]
    fn token_response_accepts_string_scope() {
        let v = serde_json::json!({
            "access_token": "abc",
            "token_type": "Bearer",
            "expires_in": 3600,
            "refresh_token": "rt",
            "scope": "read write tools/call",
        });
        let t: TokenResponse = serde_json::from_value(v).expect("parse");
        assert_eq!(t.access_token, "abc");
        assert!(t.is_bearer());
        assert_eq!(t.expires_in, Some(3600));
        assert_eq!(t.refresh_token.as_deref(), Some("rt"));
        assert_eq!(t.scope, vec!["read", "write", "tools/call"]);
    }

    #[test]
    fn token_response_accepts_array_scope() {
        let v = serde_json::json!({
            "access_token": "abc",
            "token_type": "Bearer",
            "scope": ["read", "write"],
        });
        let t: TokenResponse = serde_json::from_value(v).expect("parse");
        assert_eq!(t.scope, vec!["read", "write"]);
    }

    #[test]
    fn oauth_error_body_from_json() {
        let v = serde_json::json!({
            "error": "invalid_grant",
            "error_description": "refresh token expired",
        });
        let body = OAuthErrorBody::from_json(&v).expect("parse");
        assert_eq!(body.error, "invalid_grant");
        assert_eq!(
            body.error_description.as_deref(),
            Some("refresh token expired")
        );
        assert_eq!(body.error_uri, None);
    }

    #[test]
    fn oauth_error_body_requires_error_field() {
        let v = serde_json::json!({ "error_description": "no error code" });
        assert!(OAuthErrorBody::from_json(&v).is_none());
    }

    #[test]
    fn display_includes_error_description() {
        let err = OAuthError::OAuth {
            endpoint: "token",
            body: OAuthErrorBody {
                error: "invalid_grant".to_owned(),
                error_description: Some("expired".to_owned()),
                error_uri: None,
            },
        };
        let s = err.to_string();
        assert!(s.contains("invalid_grant"));
        assert!(s.contains("expired"));
    }
}
