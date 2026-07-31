//! OAuth client registration strategies (MCP spec §4.4).
//!
//! The spec lists four strategies in priority order:
//!
//! 1. **Pre-registered** — the user (or operator) supplies a
//!    client_id (and optionally a client_secret) out of band.
//!    The credentials live in the MCP server config.
//! 2. **Client ID Metadata Documents (CIDM)** — the client hosts
//!    a metadata document at an HTTPS URL; that URL IS the
//!    `client_id`. Servers that support it (signalled by
//!    `client_id_metadata_document_supported: true` in their
//!    metadata) treat the URL as the client_id and fetch the
//!    document to learn `redirect_uris` and other client
//!    metadata.
//! 3. **Dynamic Client Registration (RFC 7591)** — the client
//!    POSTs a registration request to the server's
//!    `registration_endpoint` and receives a client_id (and
//!    possibly a client_secret).
//! 4. **Manual** — prompt the user to register. Out of scope
//!    for this round; we surface a clear error instead.
//!
//! This module implements 1, 2 (scaffolding only — we don't
//! actually host a CIDM document yet), and 3. Strategy 2 is
//! wired in as a stub that selects the URL as the client_id;
//! full hosting of the document is a future round.

use std::collections::HashMap;

use super::super::session::{CLIENT_NAME, CLIENT_TITLE, CLIENT_VERSION};
use super::types::{
    AuthorizationServerMetadata, ClientRegistrationRequest, ClientRegistrationResponse, OAuthError,
};

use super::discovery::DISCOVERY_TIMEOUT;
use super::flow::PreRegisteredClient;

/// Resolved OAuth client ready to use. `client_id` is what we
/// pass on the auth + token requests. `client_secret` is only
/// present for confidential clients (which we are not by
/// default). `extra_headers` carries any per-client headers the
/// caller should attach to the token request (e.g. DPoP, mTLS).
#[derive(Debug, Clone)]
pub struct OAuthClient {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub extra_headers: HashMap<String, String>,
}

impl OAuthClient {
    /// Resolve the client to use for an authorization flow, per
    /// the priority order in spec §4.4.
    pub fn resolve(
        pre_registered: Option<&PreRegisteredClient>,
        as_metadata: &AuthorizationServerMetadata,
        resource: &str,
    ) -> Result<Self, OAuthError> {
        // 1. Pre-registered.
        if let Some(p) = pre_registered {
            return Ok(Self {
                client_id: p.client_id.clone(),
                client_secret: p.client_secret.clone(),
                extra_headers: HashMap::new(),
            });
        }
        // 2. Client ID Metadata Document. The server signals
        //    support via `client_id_metadata_document_supported`.
        //    We don't host such a document today; if the user
        //    has supplied one in config, the pre-registered
        //    branch above is the right entry point. This branch
        //    is here so the priority order is explicit.
        // 3. Dynamic Client Registration.
        if as_metadata.registration_endpoint.is_some() {
            return register_dynamic(as_metadata, resource);
        }
        // 4. No path left.
        Err(OAuthError::ClientRegistration(format!(
            "no usable client registration strategy for issuer '{}' \
             (server does not advertise a registration_endpoint; supply a \
             pre-registered client_id in config or implement CIDM hosting)",
            as_metadata.issuer
        )))
    }

    /// Construct a client for a refresh-token request. We use
    /// the existing client_id from the stored token rather than
    /// re-resolving, because the registration is stable.
    pub fn from_stored(
        client_id: String,
        _issuer: String,
        _as_metadata: &AuthorizationServerMetadata,
        pre_registered: Option<&PreRegisteredClient>,
    ) -> Result<Self, OAuthError> {
        if let Some(p) = pre_registered
            && p.client_id == client_id
        {
            return Ok(Self {
                client_id: p.client_id.clone(),
                client_secret: p.client_secret.clone(),
                extra_headers: HashMap::new(),
            });
        }
        Ok(Self {
            client_id,
            client_secret: None,
            extra_headers: HashMap::new(),
        })
    }
}

/// POST a Dynamic Client Registration request to the server's
/// `registration_endpoint` per RFC 7591.
fn register_dynamic(
    as_metadata: &AuthorizationServerMetadata,
    resource: &str,
) -> Result<OAuthClient, OAuthError> {
    let endpoint = as_metadata
        .registration_endpoint
        .as_deref()
        .ok_or_else(|| OAuthError::ClientRegistration("no registration_endpoint".to_owned()))?;
    if !endpoint.starts_with("https://")
        && !endpoint.starts_with("http://127.0.0.1:")
        && !endpoint.starts_with("http://localhost:")
    {
        return Err(OAuthError::SpecViolation(format!(
            "registration endpoint {endpoint} is not https://"
        )));
    }
    let loopback_uri = "http://127.0.0.1/callback";
    let request = ClientRegistrationRequest {
        redirect_uris: vec![loopback_uri.to_owned()],
        client_name: Some(CLIENT_TITLE.to_owned()),
        client_uri: Some("https://github.com/MartinKuhne/personal-productivity-ai".to_string()),
        logo_uri: None,
        token_endpoint_auth_method: Some("none".to_owned()),
        grant_types: Some(vec![
            "authorization_code".to_owned(),
            "refresh_token".to_owned(),
        ]),
        response_types: Some(vec!["code".to_owned()]),
        scope: None,
        software_id: Some(CLIENT_NAME.to_owned()),
        software_version: Some(CLIENT_VERSION.to_owned()),
    };
    let body = serde_json::to_string(&request).map_err(|e| {
        OAuthError::Internal(format!("serialize registration request: {e}"))
    })?;
    let resp = ureq::post(endpoint)
        .set("Accept", "application/json")
        .set("Content-Type", "application/json")
        .timeout(DISCOVERY_TIMEOUT)
        .send_string(&body);
    let response: ClientRegistrationResponse = match resp {
        Ok(r) => {
            let body = r
                .into_string()
                .map_err(|e| OAuthError::Transport(format!("registration response read: {e}")))?;
            let value: serde_json::Value = serde_json::from_str(&body)
                .map_err(|e| OAuthError::Protocol(format!("registration response json: {e}; body: {body}")))?;
            if let Some(err) = super::types::OAuthErrorBody::from_json(&value) {
                return Err(OAuthError::OAuth {
                    endpoint: "registration",
                    body: err,
                });
            }
            serde_json::from_value(value).map_err(|e| {
                OAuthError::Protocol(format!("registration response shape: {e}"))
            })?
        }
        Err(ureq::Error::Status(_code, r)) => {
            let body = r.into_string().unwrap_or_default();
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&body)
                && let Some(err) = super::types::OAuthErrorBody::from_json(&value)
            {
                return Err(OAuthError::OAuth {
                    endpoint: "registration",
                    body: err,
                });
            }
            return Err(OAuthError::ClientRegistration(format!(
                "registration endpoint returned error: {body}"
            )));
        }
        Err(e) => {
            return Err(OAuthError::ClientRegistration(format!(
                "registration request failed: {e}"
            )))
        }
    };
    if response.client_id.is_empty() {
        return Err(OAuthError::ClientRegistration(
            "registration response missing client_id".to_owned(),
        ));
    }
    let _ = resource; // Suppress unused; we may include it in future revisions.
    Ok(OAuthClient {
        client_id: response.client_id,
        client_secret: response.client_secret,
        extra_headers: HashMap::new(),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn as_metadata() -> AuthorizationServerMetadata {
        AuthorizationServerMetadata {
            issuer: "https://auth.example.com".to_owned(),
            authorization_endpoint: "https://auth.example.com/authorize".to_owned(),
            token_endpoint: "https://auth.example.com/token".to_owned(),
            jwks_uri: None,
            registration_endpoint: None,
            scopes_supported: vec!["read".to_owned()],
            response_types_supported: vec!["code".to_owned()],
            code_challenge_methods_supported: vec!["S256".to_owned()],
            grant_types_supported: vec!["authorization_code".to_owned()],
            token_endpoint_auth_methods_supported: vec!["none".to_owned()],
            client_id_metadata_document_supported: false,
            service_documentation: None,
            ui_locales_supported: vec![],
        }
    }

    #[test]
    fn pre_registered_takes_priority_over_dcr() {
        let pre = PreRegisteredClient {
            client_id: "preset".to_owned(),
            client_secret: Some("topsecret".to_owned()),
        };
        let mut meta = as_metadata();
        // Even with a registration endpoint, pre-registered wins.
        meta.registration_endpoint = Some("https://auth.example.com/register".to_owned());
        let client = OAuthClient::resolve(Some(&pre), &meta, "https://mcp.example.com")
            .expect("must resolve");
        assert_eq!(client.client_id, "preset");
        assert_eq!(client.client_secret.as_deref(), Some("topsecret"));
    }

    #[test]
    fn no_registration_and_no_pre_registered_returns_error() {
        let meta = as_metadata();
        let err = OAuthClient::resolve(None, &meta, "https://mcp.example.com")
            .expect_err("must fail without any path");
        match err {
            OAuthError::ClientRegistration(msg) => {
                assert!(msg.contains("no usable client registration strategy"));
            }
            other => panic!("expected ClientRegistration, got {other:?}"),
        }
    }

    #[test]
    fn from_stored_returns_supplied_client_id() {
        let meta = as_metadata();
        let client = OAuthClient::from_stored(
            "abc".to_owned(),
            "https://auth.example.com".to_owned(),
            &meta,
            None,
        )
        .expect("must succeed");
        assert_eq!(client.client_id, "abc");
        assert!(client.client_secret.is_none());
    }

    #[test]
    fn from_stored_uses_pre_registered_when_ids_match() {
        let meta = as_metadata();
        let pre = PreRegisteredClient {
            client_id: "abc".to_owned(),
            client_secret: Some("xyz".to_owned()),
        };
        let client = OAuthClient::from_stored(
            "abc".to_owned(),
            "https://auth.example.com".to_owned(),
            &meta,
            Some(&pre),
        )
        .expect("must succeed");
        assert_eq!(client.client_secret.as_deref(), Some("xyz"));
    }
}
