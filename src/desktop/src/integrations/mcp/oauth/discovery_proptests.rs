//! Property-based tests for the OAuth 2.1 metadata parsers in
//! `integrations::mcp::oauth::discovery`.
//!
//! The two discovery functions in `discovery.rs` —
//! `discover_resource_metadata` and
//! `discover_authorization_server_metadata` — sit at the trust
//! boundary between the MCP client and the remote OAuth server.
//! A hostile or buggy server can return arbitrary JSON; the
//! parsers must turn that into typed
//! `ProtectedResourceMetadata` / `AuthorizationServerMetadata`
//! values without panicking, even on:
//!
//! - Missing required fields (the parsers must surface the
//!   error, not unwrap).
//! - Wrong types (a `String` where the spec mandates a URL
//!   string is fine; an `i64` for a `Vec<String>` is not —
//!   the parser must return an error).
//! - Very deep or very wide JSON.
//! - Embedded NULs and control characters in string fields.
//! - The wildcard `*` CORS / OpenID flavour where every field
//!   is an array of strings.
//!
//! `cases = 512` per property. The serde deserialiser's own
//! coverage already handles the common shapes; the
//! proptest-added value is in the adversarial-shape corner
//! cases.

use crate::integrations::mcp::oauth::types::{
    AuthorizationServerMetadata, ProtectedResourceMetadata,
};
use proptest::prelude::*;

/// One proptest case count for every property in this sidecar.
const CASES: u32 = 512;

/// Strategy: any `serde_json::Value` with bounded depth. The
/// MCP metadata documents are tiny in practice; we cap the
/// depth so a single deep tree doesn't dominate the runtime.
fn json_value_strategy() -> impl Strategy<Value = serde_json::Value> {
    let leaf = prop_oneof![
        Just(serde_json::Value::Null),
        any::<bool>().prop_map(serde_json::Value::Bool),
        any::<i64>().prop_map(serde_json::Value::from),
        any::<String>().prop_map(serde_json::Value::String),
    ];
    leaf.prop_recursive(5, 64, 5, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..4).prop_map(serde_json::Value::Array),
            prop::collection::hash_map(any::<String>(), inner, 0..4)
                .prop_map(|m| serde_json::Value::Object(m.into_iter().collect())),
        ]
    })
}

/// Strategy: a JSON value that looks like a valid
/// `ProtectedResourceMetadata` document. Used to verify the
/// happy-path round-trip.
fn valid_resource_metadata_value() -> impl Strategy<Value = serde_json::Value> {
    (
        "https://[a-z]{2,16}\\.[a-z]{2,8}/[a-z0-9/]{0,32}",
        prop::collection::vec("https://[a-z]{2,16}\\.[a-z]{2,8}", 0..4),
    )
        .prop_map(|(resource, authorization_servers)| {
            serde_json::json!({
                "resource": resource,
                "authorization_servers": authorization_servers,
            })
        })
}

/// Strategy: a JSON value that looks like a valid
/// `AuthorizationServerMetadata` document. The required fields
/// are `issuer`, `authorization_endpoint`, `token_endpoint`.
fn valid_as_metadata_value() -> impl Strategy<Value = serde_json::Value> {
    (
        "https://[a-z]{2,16}\\.[a-z]{2,8}",
        "https://[a-z]{2,16}\\.[a-z]{2,8}/authorize",
        "https://[a-z]{2,16}\\.[a-z]{2,8}/token",
    )
        .prop_map(|(issuer, authorization_endpoint, token_endpoint)| {
            serde_json::json!({
                "issuer": issuer,
                "authorization_endpoint": authorization_endpoint,
                "token_endpoint": token_endpoint,
            })
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES))]

    /// The `ProtectedResourceMetadata` deserialiser must never
    /// panic on any JSON value. A `Result::Err` is the only
    /// acceptable outcome for malformed input.
    #[test]
    fn protected_resource_metadata_never_panics_on_any_json(value in json_value_strategy()) {
        let body = value.to_string();
        let _ = serde_json::from_str::<ProtectedResourceMetadata>(&body);
    }

    /// The `AuthorizationServerMetadata` deserialiser must
    /// never panic on any JSON value.
    #[test]
    fn authorization_server_metadata_never_panics_on_any_json(value in json_value_strategy()) {
        let body = value.to_string();
        let _ = serde_json::from_str::<AuthorizationServerMetadata>(&body);
    }

    /// A well-formed `ProtectedResourceMetadata` document
    /// round-trips through `serde_json`. The deserialised
    /// struct's `resource` field matches the input.
    #[test]
    fn protected_resource_metadata_round_trips(value in valid_resource_metadata_value()) {
        let body = value.to_string();
        let doc: ProtectedResourceMetadata = serde_json::from_str(&body)
            .expect("valid resource metadata should deserialise");
        let expected_resource = value.get("resource").and_then(|v| v.as_str()).unwrap();
        prop_assert_eq!(&doc.resource, expected_resource);
    }

    /// A well-formed `AuthorizationServerMetadata` document
    /// round-trips through `serde_json`. The required fields
    /// (`issuer`, `authorization_endpoint`, `token_endpoint`)
    /// are preserved.
    #[test]
    fn authorization_server_metadata_round_trips(value in valid_as_metadata_value()) {
        let body = value.to_string();
        let doc: AuthorizationServerMetadata = serde_json::from_str(&body)
            .expect("valid AS metadata should deserialise");
        let expected_issuer = value.get("issuer").and_then(|v| v.as_str()).unwrap();
        let expected_authz = value
            .get("authorization_endpoint")
            .and_then(|v| v.as_str())
            .unwrap();
        let expected_token = value
            .get("token_endpoint")
            .and_then(|v| v.as_str())
            .unwrap();
        prop_assert_eq!(&doc.issuer, expected_issuer);
        prop_assert_eq!(&doc.authorization_endpoint, expected_authz);
        prop_assert_eq!(&doc.token_endpoint, expected_token);
    }

    /// `ProtectedResourceMetadata` rejects input where
    /// `resource` is the wrong type (e.g. an object, an array,
    /// a number). The deserialiser returns Err, never panics.
    #[test]
    fn protected_resource_metadata_rejects_wrong_type_for_resource(
        wrong in prop_oneof![
            any::<i64>().prop_map(serde_json::Value::from),
            any::<bool>().prop_map(serde_json::Value::Bool),
            prop::collection::vec(any::<String>(), 0..3)
                .prop_map(|v| serde_json::Value::Array(v.into_iter().map(serde_json::Value::String).collect())),
        ]
    ) {
        let value = serde_json::json!({ "resource": wrong });
        let body = value.to_string();
        let result = serde_json::from_str::<ProtectedResourceMetadata>(&body);
        prop_assert!(result.is_err(), "wrong-type 'resource' must be rejected");
    }

    /// `AuthorizationServerMetadata` rejects input where
    /// `issuer` is the wrong type. Required fields are not
    /// `Option`, so a missing or wrong-type value surfaces as
    /// a deserialiser error, not a panic.
    #[test]
    fn authorization_server_metadata_rejects_wrong_type_for_issuer(
        wrong in prop_oneof![
            any::<i64>().prop_map(serde_json::Value::from),
            any::<bool>().prop_map(serde_json::Value::Bool),
        ]
    ) {
        let value = serde_json::json!({
            "issuer": wrong,
            "authorization_endpoint": "https://example.com/auth",
            "token_endpoint": "https://example.com/token",
        });
        let body = value.to_string();
        let result = serde_json::from_str::<AuthorizationServerMetadata>(&body);
        prop_assert!(result.is_err(), "wrong-type 'issuer' must be rejected");
    }

    /// A `ProtectedResourceMetadata` document with a missing
    /// `resource` field is rejected (the field is required,
    /// not `Option`).
    #[test]
    fn protected_resource_metadata_rejects_missing_resource(_unused in 0..1u8) {
        let body = "{}";
        let result = serde_json::from_str::<ProtectedResourceMetadata>(body);
        prop_assert!(result.is_err(), "missing 'resource' must be rejected");
    }

    /// An `AuthorizationServerMetadata` document with a
    /// missing `issuer` is rejected.
    #[test]
    fn authorization_server_metadata_rejects_missing_issuer(_unused in 0..1u8) {
        let body = "{}";
        let result = serde_json::from_str::<AuthorizationServerMetadata>(body);
        prop_assert!(result.is_err(), "missing 'issuer' must be rejected");
    }

    /// `protected_resource_metadata_authorization_servers`
    /// defaults to an empty Vec when absent — the field is
    /// `#[serde(default)]`. A document without it must
    /// deserialise, and the field must be empty.
    #[test]
    fn protected_resource_metadata_authorization_servers_defaults_to_empty(
        resource in "https://[a-z]{2,16}\\.[a-z]{2,8}/[a-z0-9/]{0,16}",
    ) {
        let body = format!(r#"{{"resource":"{resource}"}}"#);
        let doc: ProtectedResourceMetadata = serde_json::from_str(&body)
            .expect("missing optional field should default");
        prop_assert!(doc.authorization_servers.is_empty());
    }
}
