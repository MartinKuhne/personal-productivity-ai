//! OAuth 2.1 metadata discovery for MCP servers.
//!
//! Implements the two discovery steps from the MCP 2025-11-25
//! specification §4.3:
//!
//! 1. **Protected Resource Metadata (RFC 9728)** — discover the
//!    `authorization_servers` list. Triggered by a 401 carrying a
//!    `WWW-Authenticate` header, or by probing the well-known URI
//!    `/.well-known/oauth-protected-resource[/<mcp-path>]`.
//! 2. **Authorization Server Metadata (RFC 8414)** — for each
//!    authorization server URL, try the well-known URI variants
//!    defined in spec §4.3.1:
//!    * URLs with a path component, e.g. `https://auth.example.com/tenant1`:
//!      1. `https://auth.example.com/.well-known/oauth-authorization-server/tenant1`
//!      2. `https://auth.example.com/.well-known/openid-configuration/tenant1`
//!      3. `https://auth.example.com/tenant1/.well-known/openid-configuration`
//!    * URLs without a path, e.g. `https://auth.example.com`:
//!      1. `https://auth.example.com/.well-known/oauth-authorization-server`
//!      2. `https://auth.example.com/.well-known/openid-configuration`
//!
//! The probe order in the spec is the order we try them. The first
//! URL that returns a parseable `application/json` body is used.

use super::types::{
    AuthorizationServerMetadata, OAuthError, ProtectedResourceMetadata,
    WwwAuthenticateChallenge,
};
use std::time::Duration;

/// Default timeout for metadata HTTP fetches. Spec is silent on
/// this; the OAuth metadata documents are tiny, so 10s is plenty.
pub const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(10);

/// Discover the Protected Resource Metadata for an MCP server.
///
/// Tries, in order:
///
/// 1. The `resource_metadata` URL from the `WWW-Authenticate` header
///    (if `www_authenticate` is provided and carries one).
/// 2. `https://<mcp-host>/.well-known/oauth-protected-resource/<mcp-path>`
///    where `<mcp-path>` is the path component of the MCP server URL.
/// 3. `https://<mcp-host>/.well-known/oauth-protected-resource`
///    (host-only, RFC 9728 §3.1).
///
/// The MCP server URL's scheme/host are preserved; only the path
/// is rewritten. (We do not silently rewrite `http://` to `https://`.)
pub fn discover_resource_metadata(
    mcp_server_url: &str,
    www_authenticate: Option<&WwwAuthenticateChallenge>,
) -> Result<ProtectedResourceMetadata, OAuthError> {
    let mut tried: Vec<String> = Vec::new();
    if let Some(challenge) = www_authenticate
        && let Some(rm_url) = challenge.get("resource_metadata")
    {
        tried.push(rm_url.to_owned());
        if let Some(doc) = fetch_resource_metadata(rm_url) {
            return Ok(doc);
        }
    }
    // The two well-known probes per spec §4.3 (the "no header present" fallback).
    let candidates = well_known_resource_metadata_candidates(mcp_server_url);
    for url in &candidates {
        if tried.iter().any(|t| t == url) {
            continue;
        }
        tried.push(url.clone());
        if let Some(doc) = fetch_resource_metadata(url) {
            return Ok(doc);
        }
    }
    Err(OAuthError::ResourceMetadata(format!(
        "could not fetch Protected Resource Metadata for server '{mcp_server_url}'; \
         tried: {}",
        tried.join(", ")
    )))
}

/// Build the two well-known Protected Resource Metadata candidate
/// URLs from an MCP server URL.
pub fn well_known_resource_metadata_candidates(mcp_server_url: &str) -> Vec<String> {
    let parsed = match url_parse(mcp_server_url) {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    let origin = match origin_of(&parsed) {
        Some(o) => o,
        None => return Vec::new(),
    };
    let path = parsed.path.trim_end_matches('/');
    if path.is_empty() {
        vec![format!("{origin}/.well-known/oauth-protected-resource")]
    } else {
        vec![
            format!("{origin}/.well-known/oauth-protected-resource{path}"),
            format!("{origin}/.well-known/oauth-protected-resource"),
        ]
    }
}

/// Discover Authorization Server Metadata for the given issuer URL.
/// Tries the well-known URL order defined in spec §4.3.1.
pub fn discover_authorization_server_metadata(
    issuer_url: &str,
) -> Result<AuthorizationServerMetadata, OAuthError> {
    let candidates = well_known_as_metadata_candidates(issuer_url);
    let mut last_error: Option<String> = None;
    for url in &candidates {
        match fetch_authorization_server_metadata(url) {
            Ok(Some(doc)) => return Ok(doc),
            Ok(None) => { /* 404 or non-JSON; keep trying */ }
            Err(e) => last_error = Some(e.to_string()),
        }
    }
    Err(OAuthError::AuthorizationServerMetadata(format!(
        "no well-known metadata document reachable for issuer '{issuer_url}'; \
         tried: {}; last_error: {}",
        candidates.join(", "),
        last_error.unwrap_or_else(|| "<none>".to_owned())
    )))
}

/// Build the well-known Authorization Server Metadata candidate
/// URLs for the given issuer URL. Mirrors spec §4.3.1.
pub fn well_known_as_metadata_candidates(issuer_url: &str) -> Vec<String> {
    let parsed = match url_parse(issuer_url) {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    let origin = match origin_of(&parsed) {
        Some(o) => o,
        None => return Vec::new(),
    };
    let path = parsed.path.trim_matches('/');
    if path.is_empty() {
        vec![
            format!("{origin}/.well-known/oauth-authorization-server"),
            format!("{origin}/.well-known/openid-configuration"),
        ]
    } else {
        // Spec §4.3.1: URLs with a path component try these three in order.
        // We normalize the issuer's path-segments: e.g. for
        //   https://auth.example.com/tenant1
        // the leading path is `tenant1`, so we get:
        //   https://auth.example.com/.well-known/oauth-authorization-server/tenant1
        //   https://auth.example.com/.well-known/openid-configuration/tenant1
        //   https://auth.example.com/tenant1/.well-known/openid-configuration
        vec![
            format!("{origin}/.well-known/oauth-authorization-server/{path}"),
            format!("{origin}/.well-known/openid-configuration/{path}"),
            format!("{origin}/{path}/.well-known/openid-configuration"),
        ]
    }
}

fn fetch_resource_metadata(url: &str) -> Option<ProtectedResourceMetadata> {
    let body = http_get_json(url).ok()?;
    serde_json::from_str::<ProtectedResourceMetadata>(&body).ok()
}

fn fetch_authorization_server_metadata(url: &str) -> Result<Option<AuthorizationServerMetadata>, OAuthError> {
    match http_get_json(url) {
        Ok(body) => match serde_json::from_str::<AuthorizationServerMetadata>(&body) {
            Ok(doc) => Ok(Some(doc)),
            Err(e) => Err(OAuthError::Protocol(format!(
                "invalid Authorization Server Metadata at {url}: {e}"
            ))),
        },
        Err(HttpError::NotFound) => Ok(None),
        Err(e) => Err(OAuthError::Transport(e.to_string())),
    }
}

// ---------------------------------------------------------------------------
// Lightweight HTTP GET (JSON body) — the codebase already uses `ureq`
// everywhere; we re-use it for the metadata fetches so the project
// stays on a single HTTP stack.
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum HttpError {
    NotFound,
    Other(String),
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpError::NotFound => f.write_str("404 Not Found"),
            HttpError::Other(s) => f.write_str(s),
        }
    }
}

fn http_get_json(url: &str) -> Result<String, HttpError> {
    let req = ureq::get(url)
        .set("Accept", "application/json")
        .timeout(DISCOVERY_TIMEOUT);
    match req.call() {
        Ok(resp) => {
            let ct = resp
                .header("Content-Type")
                .unwrap_or("")
                .to_ascii_lowercase();
            if !ct.contains("application/json") {
                return Err(HttpError::Other(format!(
                    "metadata at {url} returned unexpected Content-Type '{ct}'"
                )));
            }
            resp.into_string().map_err(|e| HttpError::Other(e.to_string()))
        }
        Err(ureq::Error::Status(404, _)) => Err(HttpError::NotFound),
        Err(ureq::Error::Status(code, resp)) => {
            let body = resp.into_string().unwrap_or_default();
            Err(HttpError::Other(format!("HTTP {code}: {body}")))
        }
        Err(e) => Err(HttpError::Other(e.to_string())),
    }
}

// ---------------------------------------------------------------------------
// URL parsing helpers — we deliberately don't pull in the `url` crate
// just for these two operations. The parser is intentionally permissive:
// it splits the URL into scheme, host:port, and path, and rebuilds an
// `origin` URL. Anything we don't understand yields an empty path and
// the caller falls back to the host-only well-known URL.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
struct ParsedUrl {
    scheme: String,
    host: String,
    port: Option<u16>,
    path: String,
}

fn url_parse(s: &str) -> Result<ParsedUrl, OAuthError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(OAuthError::Protocol("empty URL".to_owned()));
    }
    // Split scheme://rest
    let (scheme, rest) = match s.split_once("://") {
        Some(parts) => parts,
        None => {
            return Err(OAuthError::Protocol(format!(
                "URL '{s}' missing scheme"
            )))
        }
    };
    let (authority, path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, "/"),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => {
            // Reject port values that aren't digits (e.g. userinfo-style colons).
            if let Ok(port) = p.parse::<u16>() {
                (h.to_owned(), Some(port))
            } else {
                (authority.to_owned(), None)
            }
        }
        None => (authority.to_owned(), None),
    };
    Ok(ParsedUrl {
        scheme: scheme.to_ascii_lowercase(),
        host,
        port,
        path: path.to_owned(),
    })
}

fn origin_of(p: &ParsedUrl) -> Option<String> {
    if p.host.is_empty() {
        return None;
    }
    let default_port = match p.scheme.as_str() {
        "https" => Some(443),
        "http" => Some(80),
        _ => None,
    };
    let port_part = match (p.port, default_port) {
        (Some(port), Some(default)) if port == default => String::new(),
        (Some(port), _) => format!(":{port}"),
        (None, _) => String::new(),
    };
    Some(format!("{}://{}{}", p.scheme, p.host, port_part))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn well_known_resource_metadata_candidates_with_path() {
        let v = well_known_resource_metadata_candidates("https://mcp.example.com/mcp");
        assert_eq!(
            v,
            vec![
                "https://mcp.example.com/.well-known/oauth-protected-resource/mcp".to_owned(),
                "https://mcp.example.com/.well-known/oauth-protected-resource".to_owned(),
            ]
        );
    }

    #[test]
    fn well_known_resource_metadata_candidates_host_only() {
        let v = well_known_resource_metadata_candidates("https://mcp.example.com");
        assert_eq!(
            v,
            vec!["https://mcp.example.com/.well-known/oauth-protected-resource".to_owned()]
        );
    }

    #[test]
    fn well_known_resource_metadata_candidates_strips_trailing_slash() {
        let v = well_known_resource_metadata_candidates("https://mcp.example.com/mcp/");
        // Trailing slash should be trimmed so the resulting candidate is "/mcp", not "//mcp".
        assert_eq!(
            v,
            vec![
                "https://mcp.example.com/.well-known/oauth-protected-resource/mcp".to_owned(),
                "https://mcp.example.com/.well-known/oauth-protected-resource".to_owned(),
            ]
        );
    }

    #[test]
    fn well_known_resource_metadata_candidates_with_port() {
        let v = well_known_resource_metadata_candidates("https://mcp.example.com:8443/mcp");
        assert_eq!(v[0], "https://mcp.example.com:8443/.well-known/oauth-protected-resource/mcp");
    }

    #[test]
    fn well_known_as_metadata_candidates_with_path() {
        let v = well_known_as_metadata_candidates("https://auth.example.com/tenant1");
        assert_eq!(
            v,
            vec![
                "https://auth.example.com/.well-known/oauth-authorization-server/tenant1".to_owned(),
                "https://auth.example.com/.well-known/openid-configuration/tenant1".to_owned(),
                "https://auth.example.com/tenant1/.well-known/openid-configuration".to_owned(),
            ]
        );
    }

    #[test]
    fn well_known_as_metadata_candidates_host_only() {
        let v = well_known_as_metadata_candidates("https://auth.example.com");
        assert_eq!(
            v,
            vec![
                "https://auth.example.com/.well-known/oauth-authorization-server".to_owned(),
                "https://auth.example.com/.well-known/openid-configuration".to_owned(),
            ]
        );
    }

    #[test]
    fn well_known_as_metadata_candidates_with_port() {
        let v = well_known_as_metadata_candidates("https://auth.example.com:8443/tenant1");
        // Non-default port must be preserved on all three candidates.
        for c in &v {
            assert!(c.starts_with("https://auth.example.com:8443/"), "bad: {c}");
        }
    }

    #[test]
    fn well_known_as_metadata_candidates_nested_path() {
        // /tenants/a/b — the "first path segment" is "tenants"; spec
        // §4.3.1 only shows one path segment but the rule is
        // "leading path components", so we treat the whole path
        // (minus leading/trailing slashes) as the suffix.
        let v = well_known_as_metadata_candidates("https://auth.example.com/tenants/a");
        assert_eq!(
            v,
            vec![
                "https://auth.example.com/.well-known/oauth-authorization-server/tenants/a".to_owned(),
                "https://auth.example.com/.well-known/openid-configuration/tenants/a".to_owned(),
                "https://auth.example.com/tenants/a/.well-known/openid-configuration".to_owned(),
            ]
        );
    }

    #[test]
    fn parse_url_basic() {
        let p = url_parse("https://mcp.example.com/mcp").unwrap();
        assert_eq!(p.scheme, "https");
        assert_eq!(p.host, "mcp.example.com");
        assert_eq!(p.path, "/mcp");
        assert_eq!(origin_of(&p).as_deref(), Some("https://mcp.example.com"));
    }

    #[test]
    fn parse_url_with_port() {
        let p = url_parse("https://mcp.example.com:8443/mcp").unwrap();
        assert_eq!(origin_of(&p).as_deref(), Some("https://mcp.example.com:8443"));
    }

    #[test]
    fn parse_url_rejects_missing_scheme() {
        assert!(url_parse("mcp.example.com").is_err());
    }

    #[test]
    fn parse_url_rejects_empty() {
        assert!(url_parse("").is_err());
    }
}
