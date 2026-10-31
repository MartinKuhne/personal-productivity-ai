//! Tests for PKCE, state generation and authorization-URL building (`agent::lib::mcp::oauth::pkce`).

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
