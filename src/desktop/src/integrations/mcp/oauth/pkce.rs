//! PKCE (RFC 7636) and `state` parameter generation for the MCP
//! client OAuth 2.1 authorization flow.
//!
//! The MCP spec §4.6 mandates PKCE with `S256`; the `state`
//! parameter is mandated by §4.9 (security) and MUST be verified
//! on the redirect callback.
//!
//! Verifier entropy is 32 bytes (256 bits), the same as the typical
//! `S256` recommendation. `state` entropy is 32 bytes too — the
//! spec doesn't pin this; we just want enough bits to make CSRF
//! guesses infeasible.
//!
//! We use `ring::rand` (already a transitive dep via `rustls`) for
//! CSPRNG entropy and `ring::digest::SHA256` for the S256 challenge.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ring::digest::{SHA256, digest};
use ring::rand::SystemRandom;

/// PKCE code-verifier / code-challenge pair.
#[derive(Debug, Clone)]
pub struct PkcePair {
    /// The plaintext verifier sent on the token request.
    pub verifier: String,
    /// The S256 challenge sent on the authorization request.
    pub challenge: String,
    /// The challenge method (`"S256"`).
    pub method: &'static str,
}

impl PkcePair {
    /// Generate a fresh PKCE pair using the system CSPRNG.
    pub fn generate() -> Self {
        Self::generate_with(&SystemRandom::new())
    }

    /// Generate a PKCE pair using the supplied CSPRNG. Used by tests
    /// to inject a deterministic source.
    pub fn generate_with(rng: &dyn ring::rand::SecureRandom) -> Self {
        // 32 random bytes, base64url-no-pad, 43 chars.
        let mut buf = [0u8; 32];
        rng.fill(&mut buf)
            .expect("system CSPRNG failed; cannot proceed with PKCE generation");
        let verifier = URL_SAFE_NO_PAD.encode(buf);
        let challenge = s256(&verifier);
        Self {
            verifier,
            challenge,
            method: "S256",
        }
    }
}

/// Random `state` value. Used to bind the authorization request
/// to the redirect callback so a malicious page can't swap in its
/// own code.
#[derive(Debug, Clone)]
pub struct State {
    value: String,
}

impl State {
    /// Generate a fresh `state` value with 32 bytes of entropy.
    pub fn generate() -> Self {
        Self::generate_with(&SystemRandom::new())
    }

    /// Generate a `state` value from the supplied CSPRNG.
    pub fn generate_with(rng: &dyn ring::rand::SecureRandom) -> Self {
        let mut buf = [0u8; 32];
        rng.fill(&mut buf)
            .expect("system CSPRNG failed; cannot proceed with state generation");
        Self {
            value: URL_SAFE_NO_PAD.encode(buf),
        }
    }

    /// Borrow the raw state value.
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

/// Compute the S256 PKCE challenge for a verifier per RFC 7636 §4.2.
pub fn s256(verifier: &str) -> String {
    let hash = digest(&SHA256, verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(hash.as_ref())
}

// ---------------------------------------------------------------------------
// Authorization URL builder
// ---------------------------------------------------------------------------

/// Inputs to the authorization request URL builder. Only the
/// `client_id`, `authorization_endpoint`, `redirect_uri`, and
/// `state` are strictly required. The remaining fields are the
/// standard OAuth 2.1 / MCP 2.1 parameters and SHOULD be populated
/// when the caller has them.
#[derive(Debug, Clone)]
pub struct AuthorizationUrlInputs<'a> {
    /// The AS metadata's `authorization_endpoint`.
    pub authorization_endpoint: &'a str,
    /// Client ID.
    pub client_id: &'a str,
    /// Loopback redirect URI (`http://127.0.0.1:<port>/callback`).
    pub redirect_uri: &'a str,
    /// `state` value to bind request and callback.
    pub state: &'a str,
    /// PKCE challenge + method.
    pub pkce: &'a PkcePair,
    /// Space-separated scope string. Spec §4.5: prefer the
    /// `scope` from the initial `WWW-Authenticate` header; else
    /// join all `scopes_supported` from the resource metadata.
    pub scope: Option<&'a str>,
    /// Canonical URI of the MCP server (RFC 8707 `resource` parameter).
    /// Spec §4.6 REQUIRES this.
    pub resource: &'a str,
}

/// Build the authorization request URL. Per RFC 6749 §3.1 the
/// parameters are query-encoded with the same rules as
/// `application/x-www-form-urlencoded`.
pub fn build_authorization_url(input: &AuthorizationUrlInputs<'_>) -> Result<String, String> {
    let mut url = input.authorization_endpoint.to_owned();
    // We always append our parameters; if the endpoint already has
    // a query string, we use '&', otherwise '?'. Per RFC 6749 §3.1
    // the standard form has no pre-existing query string, so this
    // is mostly future-proofing.
    let joiner = if url.contains('?') { '&' } else { '?' };
    url.push(joiner);
    let mut sep = "";
    for (k, v) in authorization_params(input) {
        url.push_str(sep);
        url.push_str(&percent_encode(k));
        url.push('=');
        url.push_str(&percent_encode(&v));
        sep = "&";
    }
    Ok(url)
}

fn authorization_params<'a>(input: &'a AuthorizationUrlInputs<'a>) -> Vec<(&'a str, String)> {
    let mut params: Vec<(&str, String)> = Vec::with_capacity(7);
    params.push(("response_type", "code".to_owned()));
    params.push(("client_id", input.client_id.to_owned()));
    params.push(("redirect_uri", input.redirect_uri.to_owned()));
    params.push(("state", input.state.to_owned()));
    params.push(("code_challenge", input.pkce.challenge.clone()));
    params.push(("code_challenge_method", input.pkce.method.to_owned()));
    params.push(("resource", input.resource.to_owned()));
    if let Some(scope) = input.scope
        && !scope.trim().is_empty()
    {
        params.push(("scope", scope.to_owned()));
    }
    params
}

/// `application/x-www-form-urlencoded` percent-encoding, preserving
/// the unreserved character set from RFC 3986. Spaces become `+`
/// (the form encoding) rather than `%20`.
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match *b {
            // Unreserved: A-Z / a-z / 0-9 / `-` / `_` / `.` / `~`
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push(hex_nibble(b >> 4));
                out.push(hex_nibble(b & 0x0f));
            }
        }
    }
    out
}

fn hex_nibble(b: u8) -> char {
    match b & 0x0f {
        0..=9 => (b'0' + (b & 0x0f)) as char,
        10..=15 => (b'A' + ((b & 0x0f) - 10)) as char,
        _ => '0',
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_pair_is_43_char_verifier_and_challenge() {
        let p = PkcePair::generate();
        assert_eq!(p.method, "S256");
        // 32 bytes base64url-no-pad = 43 chars.
        assert_eq!(p.verifier.len(), 43);
        // S256 of 32-byte input is 32 bytes, still 43 chars.
        assert_eq!(p.challenge.len(), 43);
    }

    #[test]
    fn pkce_pair_generates_unique_pairs() {
        let p1 = PkcePair::generate();
        let p2 = PkcePair::generate();
        assert_ne!(p1.verifier, p2.verifier);
        assert_ne!(p1.challenge, p2.challenge);
    }

    #[test]
    fn s256_matches_rfc7636_test_vector() {
        // RFC 7636 Appendix B test vector:
        //   verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
        //   challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = s256(verifier);
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn state_is_43_chars() {
        let s = State::generate();
        assert_eq!(s.as_str().len(), 43);
    }

    #[test]
    fn state_generates_unique_values() {
        let s1 = State::generate();
        let s2 = State::generate();
        assert_ne!(s1.as_str(), s2.as_str());
    }

    #[test]
    fn percent_encode_unreserved() {
        assert_eq!(percent_encode("abcXYZ012-_.~"), "abcXYZ012-_.~");
    }

    #[test]
    fn percent_encode_special_chars() {
        assert_eq!(percent_encode("a b"), "a+b");
        assert_eq!(percent_encode("a/b"), "a%2Fb");
        assert_eq!(percent_encode("a&b=c"), "a%26b%3Dc");
    }

    #[test]
    fn authorization_url_contains_required_params() {
        let pkce = PkcePair::generate();
        let url = build_authorization_url(&AuthorizationUrlInputs {
            authorization_endpoint: "https://auth.example.com/authorize",
            client_id: "my-client",
            redirect_uri: "http://127.0.0.1:54321/callback",
            state: "abc",
            pkce: &pkce,
            scope: Some("read write"),
            resource: "https://mcp.example.com/mcp",
        })
        .expect("url must build");
        assert!(url.starts_with("https://auth.example.com/authorize?"));
        for required in &[
            "response_type=code",
            "client_id=my-client",
            "redirect_uri=http%3A%2F%2F127.0.0.1%3A54321%2Fcallback",
            "state=abc",
            "code_challenge=",
            "code_challenge_method=S256",
            "scope=read+write",
            "resource=https%3A%2F%2Fmcp.example.com%2Fmcp",
        ] {
            assert!(url.contains(required), "missing {required} in {url}");
        }
    }

    #[test]
    fn authorization_url_omits_blank_scope() {
        let pkce = PkcePair::generate();
        let url = build_authorization_url(&AuthorizationUrlInputs {
            authorization_endpoint: "https://auth.example.com/authorize",
            client_id: "c",
            redirect_uri: "http://127.0.0.1:1/cb",
            state: "s",
            pkce: &pkce,
            scope: Some("   "),
            resource: "https://mcp.example.com",
        })
        .expect("url must build");
        assert!(!url.contains("scope="));
    }

    #[test]
    fn authorization_url_preserves_existing_query() {
        let pkce = PkcePair::generate();
        let url = build_authorization_url(&AuthorizationUrlInputs {
            authorization_endpoint: "https://auth.example.com/authorize?foo=bar",
            client_id: "c",
            redirect_uri: "http://127.0.0.1:1/cb",
            state: "s",
            pkce: &pkce,
            scope: None,
            resource: "https://mcp.example.com",
        })
        .expect("url must build");
        assert!(url.contains("foo=bar&"));
        assert!(url.contains("client_id=c&") || url.contains("client_id=c"));
    }
}
