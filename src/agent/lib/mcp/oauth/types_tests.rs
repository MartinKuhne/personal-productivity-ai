//! Tests for the OAuth protocol DTOs and [`OAuthError`] (`agent::lib::mcp::oauth::types`).

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

// ─────────────────────────────────────────────────────────────────────────────
// WWW-Authenticate rendering (`Display`)
// ─────────────────────────────────────────────────────────────────────────────

/// Token-shaped values render bare; anything with separators is quoted and
/// backslashes/quotes inside the value are escaped.
#[test]
fn www_challenge_display_quotes_and_escapes_values() {
    let challenge = WwwAuthenticateChallenge {
        scheme: "Bearer".to_owned(),
        params: vec![
            ("error".to_owned(), "invalid_token".to_owned()),
            ("error_description".to_owned(), "say \"hi\"".to_owned()),
            ("scope".to_owned(), "read write".to_owned()),
            ("path".to_owned(), "a\\b".to_owned()),
        ],
    };

    assert_eq!(
        challenge.to_string(),
        "Bearer error=invalid_token, \
         error_description=\"say \\\"hi\\\"\", \
         scope=\"read write\", \
         path=\"a\\\\b\""
    );
}

#[test]
fn www_challenge_display_renders_scheme_only_without_params() {
    let challenge = WwwAuthenticateChallenge {
        scheme: "Bearer".to_owned(),
        params: Vec::new(),
    };
    assert_eq!(
        challenge.to_string(),
        "Bearer ",
        "the scheme is always rendered with its trailing space"
    );
}

/// A rendered challenge parses back to the same parameter list.
#[test]
fn www_challenge_display_round_trips_through_parser() {
    let challenge = WwwAuthenticateChallenge {
        scheme: "Bearer".to_owned(),
        params: vec![
            ("error".to_owned(), "invalid_token".to_owned()),
            (
                "error_description".to_owned(),
                "the token \"expiring\" soon".to_owned(),
            ),
            (
                "resource_metadata".to_owned(),
                "https://mcp.example.com/.well-known/oauth-protected-resource".to_owned(),
            ),
        ],
    };

    let rendered = challenge.to_string();
    let parsed = parse_bearer_challenge(&rendered).expect("rendered header must parse");
    assert_eq!(parsed.scheme, "Bearer");
    assert_eq!(parsed.params, challenge.params, "rendered as: {rendered}");
}

// ─────────────────────────────────────────────────────────────────────────────
// WWW-Authenticate parsing — remaining branches
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn parse_bearer_challenge_requires_bearer_token_prefix() {
    assert!(parse_bearer_challenge("Basic realm=\"api\"").is_none());
    assert!(
        parse_bearer_challenge("Bearer").is_none(),
        "the scheme must be followed by a space"
    );
}

/// A comma followed by a bare scheme name starts a new challenge, so the
/// parameters of the Bearer challenge stop there.
#[test]
fn parse_bearer_challenge_caps_params_at_next_challenge() {
    let ch =
        parse_bearer_challenge("Bearer realm=\"mcp\", Basic realm=\"other\"").expect("must parse");
    assert_eq!(ch.params.len(), 1);
    assert_eq!(ch.get("realm"), Some("mcp"));
    assert_eq!(
        ch.get("Basic"),
        None,
        "the trailing challenge must not leak into the parameters"
    );
}

/// After a comma, a token that contains `=` or an illegal character is a
/// parameter, not a new scheme, so parsing continues past it.
#[test]
fn parse_bearer_challenge_keeps_non_scheme_tokens() {
    // `a=b` contains '=' → parameter separator, not a new challenge.
    let ch = parse_bearer_challenge("Bearer x=1, a=b c").expect("must parse");
    assert_eq!(ch.params.len(), 2);
    assert_eq!(ch.params[0], ("x".to_owned(), "1".to_owned()));

    // `a*b` has an illegal scheme character → also not a new challenge, and
    // the entry has no '=' so it is stored under an empty key.
    let ch = parse_bearer_challenge("Bearer x=1, a*b c").expect("must parse");
    assert_eq!(ch.params.len(), 2);
    assert_eq!(
        ch.params[1],
        (String::new(), "a*b c".to_owned()),
        "a value without '=' is stored with an empty key"
    );

    // No whitespace after the comma at all → never a new challenge.
    let ch = parse_bearer_challenge("Bearer x=1,abc").expect("must parse");
    assert_eq!(ch.params.len(), 2);
    assert_eq!(ch.params[1], (String::new(), "abc".to_owned()));
}

/// Empty buffers between separators are dropped rather than stored.
#[test]
fn parse_bearer_challenge_drops_empty_entries() {
    let ch = parse_bearer_challenge("Bearer x=1,,y=2,").expect("must parse");
    assert_eq!(
        ch.params,
        vec![
            ("x".to_owned(), "1".to_owned()),
            ("y".to_owned(), "2".to_owned()),
        ],
        "empty segments must not produce parameters"
    );
}

/// `get` matches parameter names case-insensitively (RFC 7235 §2.2).
#[test]
fn www_challenge_get_is_case_insensitive() {
    let ch = parse_bearer_challenge("Bearer REALM=\"api\", Scope=\"read\"").expect("must parse");
    assert_eq!(ch.get("realm"), Some("api"));
    assert_eq!(ch.get("SCOPE"), Some("read"));
    assert_eq!(ch.get("missing"), None);
}

/// Direct coverage for `push_param`'s unquoting rules, including the
/// single-quote edge case that the parser itself cannot produce.
#[test]
fn push_param_trims_and_unquotes() {
    let mut out = Vec::new();
    push_param(&mut out, "   key = value   ");
    assert_eq!(
        out,
        vec![("key".to_owned(), "value".to_owned())],
        "surrounding whitespace is trimmed from both halves"
    );

    out.clear();
    push_param(&mut out, "key=\"quoted\"");
    assert_eq!(out, vec![("key".to_owned(), "quoted".to_owned())]);

    out.clear();
    push_param(&mut out, "key=\"");
    assert_eq!(
        out,
        vec![("key".to_owned(), "\"".to_owned())],
        "a lone quote is too short to be a quoted string"
    );

    out.clear();
    push_param(&mut out, "   ");
    assert!(out.is_empty(), "blank buffers are ignored");
}

// ─────────────────────────────────────────────────────────────────────────────
// Scope deserialization — remaining branches
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn token_response_scope_accepts_null_and_both_shapes() {
    let null_scope: TokenResponse = serde_json::from_value(serde_json::json!({
        "access_token": "abc",
        "token_type": "Bearer",
        "scope": null,
    }))
    .expect("an explicit null scope is empty");
    assert!(null_scope.scope.is_empty());

    let scalar: TokenResponse = serde_json::from_value(serde_json::json!({
        "access_token": "abc",
        "token_type": "Bearer",
        "scope": "  read   write  ",
    }))
    .expect("scalar scope");
    assert_eq!(scalar.scope, vec!["read".to_owned(), "write".to_owned()]);

    let array: TokenResponse = serde_json::from_value(serde_json::json!({
        "access_token": "abc",
        "token_type": "Bearer",
        "scope": [],
    }))
    .expect("empty array scope");
    assert!(array.scope.is_empty());
}

#[test]
fn token_response_scope_rejects_non_string_element() {
    let err = serde_json::from_value::<TokenResponse>(serde_json::json!({
        "access_token": "abc",
        "token_type": "Bearer",
        "scope": ["read", 42],
    }))
    .expect_err("a numeric scope element must be rejected");
    assert!(
        err.to_string()
            .contains("scope array must contain strings, got 42"),
        "unexpected message: {err}"
    );
}

#[test]
fn token_response_scope_rejects_other_json_types() {
    let err = serde_json::from_value::<TokenResponse>(serde_json::json!({
        "access_token": "abc",
        "token_type": "Bearer",
        "scope": 42,
    }))
    .expect_err("a numeric scope must be rejected");
    assert!(
        err.to_string()
            .contains("scope must be string or array, got 42"),
        "unexpected message: {err}"
    );
}

#[test]
fn token_response_is_bearer_ignores_case() {
    let make = |ty: &str| TokenResponse {
        access_token: "a".to_owned(),
        token_type: ty.to_owned(),
        expires_in: None,
        refresh_token: None,
        scope: Vec::new(),
    };
    assert!(make("Bearer").is_bearer());
    assert!(make("bearer").is_bearer());
    assert!(!make("MAC").is_bearer());
    assert!(!make("").is_bearer());
}

// ─────────────────────────────────────────────────────────────────────────────
// OAuthErrorBody::from_json — remaining branches
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn oauth_error_body_from_json_rejects_non_objects() {
    assert!(OAuthErrorBody::from_json(&serde_json::json!([])).is_none());
    assert!(OAuthErrorBody::from_json(&serde_json::json!("invalid_grant")).is_none());
    assert!(OAuthErrorBody::from_json(&serde_json::Value::Null).is_none());
}

#[test]
fn oauth_error_body_from_json_requires_string_error() {
    assert!(OAuthErrorBody::from_json(&serde_json::json!({ "error": 42 })).is_none());
    assert!(OAuthErrorBody::from_json(&serde_json::json!({ "error": null })).is_none());
    assert!(OAuthErrorBody::from_json(&serde_json::json!({})).is_none());
}

/// Optional fields are ignored when they are not strings, and `error_uri` is
/// picked up when present.
#[test]
fn oauth_error_body_from_json_reads_optional_fields() {
    let body = OAuthErrorBody::from_json(&serde_json::json!({
        "error": "invalid_grant",
        "error_description": "expired",
        "error_uri": "https://auth.example.com/errors#invalid_grant",
    }))
    .expect("full envelope");
    assert_eq!(body.error, "invalid_grant");
    assert_eq!(body.error_description.as_deref(), Some("expired"));
    assert_eq!(
        body.error_uri.as_deref(),
        Some("https://auth.example.com/errors#invalid_grant")
    );

    let body = OAuthErrorBody::from_json(&serde_json::json!({
        "error": "invalid_grant",
        "error_description": 404,
        "error_uri": [],
    }))
    .expect("non-string optionals are tolerated");
    assert_eq!(body.error_description, None);
    assert_eq!(body.error_uri, None);
}

// ─────────────────────────────────────────────────────────────────────────────
// DTO serde round-trips
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn protected_resource_metadata_round_trips() {
    let raw = r#"{
        "resource": "https://mcp.example.com/mcp",
        "authorization_servers": ["https://auth.example.com"],
        "scopes_supported": ["mcp:tools"],
        "bearer_methods_supported": ["header"],
        "resource_name": "Example MCP",
        "resource_documentation": "https://docs.example.com",
        "resource_policy_uri": "https://example.com/policy",
        "resource_tos_uri": "https://example.com/tos"
    }"#;
    let parsed: ProtectedResourceMetadata = serde_json::from_str(raw).expect("parse");
    let json = serde_json::to_value(&parsed).expect("serialise");
    let back: ProtectedResourceMetadata = serde_json::from_value(json).expect("re-parse");
    assert_eq!(back, parsed);
    assert_eq!(parsed.resource, "https://mcp.example.com/mcp");
    assert_eq!(parsed.authorization_servers.len(), 1);
}

#[test]
fn protected_resource_metadata_minimal_document_uses_defaults() {
    let parsed: ProtectedResourceMetadata =
        serde_json::from_str(r#"{"resource":"https://mcp.example.com"}"#).expect("parse");
    assert!(parsed.authorization_servers.is_empty());
    assert!(parsed.scopes_supported.is_empty());
    assert!(parsed.bearer_methods_supported.is_empty());
    assert_eq!(parsed.resource_name, None);
    assert_eq!(parsed.resource_policy_uri, None);
    assert_eq!(parsed.resource_tos_uri, None);
}

#[test]
fn authorization_server_metadata_round_trips() {
    let raw = r#"{
        "issuer": "https://auth.example.com",
        "authorization_endpoint": "https://auth.example.com/authorize",
        "token_endpoint": "https://auth.example.com/token",
        "jwks_uri": "https://auth.example.com/jwks.json",
        "registration_endpoint": "https://auth.example.com/register",
        "scopes_supported": ["mcp:tools"],
        "response_types_supported": ["code"],
        "code_challenge_methods_supported": ["S256"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "token_endpoint_auth_methods_supported": ["client_secret_basic"],
        "client_id_metadata_document_supported": true,
        "service_documentation": "https://docs.example.com",
        "ui_locales_supported": ["en-GB"]
    }"#;
    let parsed: AuthorizationServerMetadata = serde_json::from_str(raw).expect("parse");
    let json = serde_json::to_value(&parsed).expect("serialise");
    let back: AuthorizationServerMetadata = serde_json::from_value(json).expect("re-parse");
    assert_eq!(back, parsed);
    assert!(parsed.client_id_metadata_document_supported);
    assert_eq!(
        parsed.code_challenge_methods_supported,
        vec!["S256".to_owned()]
    );
}

#[test]
fn authorization_server_metadata_minimal_document_uses_defaults() {
    let parsed: AuthorizationServerMetadata = serde_json::from_str(
        r#"{
            "issuer": "https://auth.example.com",
            "authorization_endpoint": "https://auth.example.com/authorize",
            "token_endpoint": "https://auth.example.com/token"
        }"#,
    )
    .expect("parse");
    assert_eq!(parsed.registration_endpoint, None);
    assert_eq!(parsed.jwks_uri, None);
    assert!(!parsed.client_id_metadata_document_supported);
    assert!(parsed.scopes_supported.is_empty());
    assert!(parsed.ui_locales_supported.is_empty());
}

#[test]
fn client_registration_request_round_trips_and_skips_absent_optionals() {
    let raw = r#"{
        "redirect_uris": ["http://127.0.0.1:8765/callback"],
        "client_name": "fastmd",
        "client_uri": "https://example.com",
        "logo_uri": "https://example.com/logo.png",
        "token_endpoint_auth_method": "none",
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "scope": "mcp:tools",
        "software_id": "fastmd",
        "software_version": "1.2.3"
    }"#;
    let parsed: ClientRegistrationRequest = serde_json::from_str(raw).expect("parse");
    let json = serde_json::to_value(&parsed).expect("serialise");
    let back: ClientRegistrationRequest = serde_json::from_value(json).expect("re-parse");
    assert_eq!(back, parsed);

    let minimal: ClientRegistrationRequest =
        serde_json::from_str(r#"{"redirect_uris":["http://127.0.0.1/cb"]}"#).expect("parse");
    let json = serde_json::to_value(&minimal).expect("serialise");
    assert_eq!(
        json.as_object().expect("object").len(),
        1,
        "absent optionals must be skipped: {json}"
    );
    assert_eq!(json["redirect_uris"].as_array().expect("array").len(), 1);
}

#[test]
fn client_registration_response_round_trips() {
    let raw = r#"{
        "client_id": "client-1",
        "client_secret": "secret-1",
        "client_secret_expires_at": 1800000000,
        "redirect_uris": ["http://127.0.0.1:8765/callback"],
        "token_endpoint_auth_method": "client_secret_basic",
        "registration_access_token": "rat",
        "registration_client_uri": "https://auth.example.com/register/client-1",
        "client_id_issued_at": 1700000000,
        "unknown_future_field": true
    }"#;
    let parsed: ClientRegistrationResponse = serde_json::from_str(raw).expect("parse");
    let json = serde_json::to_value(&parsed).expect("serialise");
    let back: ClientRegistrationResponse = serde_json::from_value(json).expect("re-parse");
    assert_eq!(back, parsed);
    assert_eq!(parsed.client_secret.as_deref(), Some("secret-1"));
    assert_eq!(parsed.client_id_issued_at, Some(1_700_000_000));

    let minimal: ClientRegistrationResponse =
        serde_json::from_str(r#"{"client_id":"anon"}"#).expect("parse");
    assert_eq!(minimal.client_secret, None);
    assert_eq!(minimal.client_secret_expires_at, None);
    assert!(minimal.redirect_uris.is_empty());
    assert_eq!(minimal.client_id_issued_at, None);
}

#[test]
fn token_response_round_trips() {
    let token = TokenResponse {
        access_token: "abc".to_owned(),
        token_type: "Bearer".to_owned(),
        expires_in: Some(3600),
        refresh_token: Some("rt".to_owned()),
        scope: vec!["read".to_owned(), "write".to_owned()],
    };
    let json = serde_json::to_value(&token).expect("serialise");
    let back: TokenResponse = serde_json::from_value(json).expect("re-parse");
    assert_eq!(back, token);
}

// ─────────────────────────────────────────────────────────────────────────────
// OAuthError display surface
// ─────────────────────────────────────────────────────────────────────────────

/// Every variant renders a user-facing message: these strings are what the
/// agent surfaces when authentication fails.
#[test]
fn oauth_error_display_covers_every_variant() {
    let cases: Vec<(OAuthError, &str)> = vec![
        (
            OAuthError::Transport("dns failure".to_owned()),
            "OAuth transport error: dns failure",
        ),
        (
            OAuthError::Protocol("malformed json".to_owned()),
            "OAuth protocol error: malformed json",
        ),
        (
            OAuthError::ResourceMetadata("404".to_owned()),
            "Protected Resource Metadata discovery failed: 404",
        ),
        (
            OAuthError::AuthorizationServerMetadata("no issuer".to_owned()),
            "Authorization Server Metadata discovery failed: no issuer",
        ),
        (
            OAuthError::ClientRegistration("403".to_owned()),
            "Dynamic Client Registration failed: 403",
        ),
        (
            OAuthError::AuthorizationDenied("user said no".to_owned()),
            "authorization denied: user said no",
        ),
        (
            OAuthError::StateMismatch,
            "OAuth state mismatch (redirect callback did not match sent state)",
        ),
        (
            OAuthError::CallbackTimeout("no callback within 120s".to_owned()),
            "OAuth callback timeout: no callback within 120s",
        ),
        (OAuthError::UserCancelled, "OAuth flow cancelled by user"),
        (
            OAuthError::SpecViolation("http redirect uri".to_owned()),
            "OAuth spec violation: http redirect uri",
        ),
        (
            OAuthError::PkceNotSupported,
            "server does not advertise PKCE S256 in code_challenge_methods_supported",
        ),
        (
            OAuthError::RefreshFailed("revoked".to_owned()),
            "OAuth refresh failed: revoked",
        ),
        (
            OAuthError::Internal("boom".to_owned()),
            "OAuth internal error: boom",
        ),
        (
            OAuthError::OAuth {
                endpoint: "token",
                body: OAuthErrorBody {
                    error: "invalid_grant".to_owned(),
                    error_description: None,
                    error_uri: None,
                },
            },
            "token returned OAuth error 'invalid_grant'",
        ),
    ];

    for (err, expected) in cases {
        assert_eq!(err.to_string(), expected, "wrong message for {err:?}");
    }
}

#[test]
fn oauth_error_converts_to_string_for_storage() {
    let message = String::from(OAuthError::StateMismatch);
    assert_eq!(
        message,
        "OAuth state mismatch (redirect callback did not match sent state)"
    );
}
