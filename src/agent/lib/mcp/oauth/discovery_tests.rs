//! Tests for OAuth protected-resource and authorization-server discovery (`agent::lib::mcp::oauth::discovery`).

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
    assert_eq!(
        v[0],
        "https://mcp.example.com:8443/.well-known/oauth-protected-resource/mcp"
    );
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
    assert_eq!(
        origin_of(&p).as_deref(),
        Some("https://mcp.example.com:8443")
    );
}

#[test]
fn parse_url_rejects_missing_scheme() {
    assert!(url_parse("mcp.example.com").is_err());
}

#[test]
fn parse_url_rejects_empty() {
    assert!(url_parse("").is_err());
}

#[test]
fn parse_url_trims_whitespace_and_lowercases_the_scheme() {
    let p = url_parse("  HTTPS://MCP.Example.com/mcp  ").unwrap();
    assert_eq!(p.scheme, "https", "the scheme is normalised");
    assert_eq!(p.host, "MCP.Example.com", "the host is left as written");
    assert_eq!(p.path, "/mcp");
}

#[test]
fn parse_url_defaults_to_root_path() {
    let p = url_parse("https://mcp.example.com").unwrap();
    assert_eq!(p.path, "/", "a URL without a path gets the root path");
    assert_eq!(p.port, None);
}

#[test]
fn parse_url_treats_a_non_numeric_port_as_part_of_the_host() {
    // A userinfo-style colon must not be mistaken for a port separator.
    let p = url_parse("http://user:pass@mcp.example.com/mcp").unwrap();
    assert_eq!(p.host, "user:pass@mcp.example.com");
    assert_eq!(p.port, None);
    assert_eq!(p.path, "/mcp");
}

#[test]
fn origin_of_drops_the_default_port() {
    let https = url_parse("https://auth.example.com:443/tenant1").unwrap();
    assert_eq!(
        origin_of(&https).as_deref(),
        Some("https://auth.example.com")
    );

    let http = url_parse("http://127.0.0.1:80/mcp").unwrap();
    assert_eq!(origin_of(&http).as_deref(), Some("http://127.0.0.1"));
}

#[test]
fn origin_of_keeps_a_non_default_port() {
    let p = url_parse("http://127.0.0.1:8765/mcp").unwrap();
    assert_eq!(origin_of(&p).as_deref(), Some("http://127.0.0.1:8765"));
}

#[test]
fn origin_of_keeps_the_port_for_an_unknown_scheme() {
    let p = url_parse("ftp://host.example:21/doc").unwrap();
    assert_eq!(origin_of(&p).as_deref(), Some("ftp://host.example:21"));
}

#[test]
fn origin_of_requires_a_host() {
    let p = url_parse("https:///mcp").unwrap();
    assert_eq!(p.host, "");
    assert_eq!(origin_of(&p), None, "an empty host has no origin");
}

#[test]
fn candidate_helpers_return_nothing_for_unusable_urls() {
    for url in ["", "not a url", "https:///mcp"] {
        assert!(
            well_known_resource_metadata_candidates(url).is_empty(),
            "resource candidates for {url:?}"
        );
        assert!(
            well_known_as_metadata_candidates(url).is_empty(),
            "authorization server candidates for {url:?}"
        );
    }
}

#[test]
fn as_candidates_trim_surrounding_slashes() {
    let v = well_known_as_metadata_candidates("https://auth.example.com/tenant1/");
    assert_eq!(
        v[0],
        "https://auth.example.com/.well-known/oauth-authorization-server/tenant1"
    );
}

#[test]
fn discovery_reports_every_candidate_it_tried() {
    // An unparseable URL produces no candidates at all, so the error
    // message shows an empty list and no last error.
    let err = discover_authorization_server_metadata("not-a-url").expect_err("nothing to probe");
    match err {
        OAuthError::AuthorizationServerMetadata(msg) => {
            assert!(msg.contains("tried: ;"), "got: {msg}");
            assert!(msg.contains("last_error: <none>"), "got: {msg}");
        }
        other => panic!("expected AuthorizationServerMetadata, got {other:?}"),
    }
}

#[test]
fn resource_metadata_discovery_reports_no_candidates() {
    let err = discover_resource_metadata("not-a-url", None).expect_err("nothing to probe");
    match err {
        OAuthError::ResourceMetadata(msg) => {
            assert!(
                msg.contains("could not fetch Protected Resource Metadata"),
                "got: {msg}"
            );
            assert!(msg.contains("tried: "), "got: {msg}");
        }
        other => panic!("expected ResourceMetadata, got {other:?}"),
    }
}

#[test]
fn resource_metadata_discovery_survives_an_unusable_challenge_url() {
    // A challenge whose `resource_metadata` cannot be fetched (here: not even
    // a URL) falls through to the well-known probes.
    let challenge = WwwAuthenticateChallenge {
        scheme: "Bearer".to_owned(),
        params: vec![("resource_metadata".to_owned(), "not-a-url".to_owned())],
    };
    let err = discover_resource_metadata("not-a-url-either", Some(&challenge))
        .expect_err("no metadata is reachable");
    assert!(
        matches!(err, OAuthError::ResourceMetadata(_)),
        "got: {err:?}"
    );
}
