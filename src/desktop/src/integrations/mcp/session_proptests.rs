//! Property-based tests for the JSON-RPC response parser in
//! `integrations::mcp::session::extract_result`.
//!
//! Every MCP response goes through `extract_result` before the
//! LLM sees the result. The function is a pure JSON-shape
//! check, not an HTTP handler, so it's the cheapest place to
//! catch the JSON-RPC fuzz regressions (id mismatch, missing
//! `result`, error envelope instead of result, deeply-nested
//! objects, etc.).
//!
//! # Properties under test
//!
//! All four properties are sourced from the C5 corner-case
//! row in `doc/planning/fuzzing.md` §2.2 "Phase 2".
//!
//! 1. **No panic on any input.** `extract_result` accepts an
//!    owned `serde_json::Value`; even the most adversarial
//!    shape (deeply nested, huge arrays, unicode escapes) must
//!    not unwrap. The return type is `Result<Value, McpError>`;
//!    `Err` is the only acceptable outcome for malformed input.
//! 2. **Missing `jsonrpc` discriminator is rejected.** The
//!    spec mandates `"jsonrpc":"2.0"` on every response; a
//!    value that omits or has a wrong-type discriminator
//!    returns an `Err` and never panics.
//! 3. **`error` envelope takes precedence over `result`.** A
//!    response carrying both is well-formed but must surface
//!    the `error` (the LLM should see the error, not the
//!    stale result). The parser never panics when both are
//!    present.
//! 4. **Missing both `result` and `error` is rejected.** The
//!    spec says exactly one of those is required; a response
//!    with neither is an `Err`.
//!
//! `cases = 512` per property. The JSON-RPC spec surface is
//! small; the proptest value is in adversarial-shape coverage.

use crate::integrations::mcp::session::McpClientSession;
use proptest::prelude::*;

/// One proptest case count for every property in this sidecar.
const CASES: u32 = 512;

/// Strategy: any `serde_json::Value` with bounded depth. The
/// JSON-RPC envelope is shallow in practice, but a hostile
/// server can return a deeply nested `params` payload that
/// the parser must accept.
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

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES))]

    /// `extract_result` must never panic on any input. The
    /// function returns `Result<serde_json::Value, McpError>`;
    /// any input is either Ok or Err, never unwinding.
    #[test]
    fn extract_result_never_panics_on_any_input(value in json_value_strategy()) {
        let _ = McpClientSession::extract_result("server", "method", value);
    }

    /// A response missing the `"jsonrpc":"2.0"` discriminator
    /// must be rejected. The discriminator is required by
    /// spec; a missing or wrong-type value surfaces as
    /// `Err`, never a panic.
    #[test]
    fn extract_result_rejects_missing_jsonrpc_discriminator(
        result in json_value_strategy()
    ) {
        // Build a response that has a `result` but no
        // `jsonrpc` field. The parser must reject.
        let value = serde_json::json!({ "id": 1, "result": result });
        let outcome = McpClientSession::extract_result("server", "method", value);
        prop_assert!(outcome.is_err(), "missing 'jsonrpc' must be rejected");
    }

    /// A response with a wrong-type `jsonrpc` field (e.g.
    /// an integer instead of the string `"2.0"`) is
    /// rejected.
    #[test]
    fn extract_result_rejects_wrong_type_jsonrpc(
        wrong in prop_oneof![
            any::<i64>().prop_map(serde_json::Value::from),
            any::<bool>().prop_map(serde_json::Value::Bool),
        ]
    ) {
        let value = serde_json::json!({
            "jsonrpc": wrong,
            "id": 1,
            "result": { "ok": true }
        });
        let outcome = McpClientSession::extract_result("server", "method", value);
        prop_assert!(outcome.is_err(), "wrong-type 'jsonrpc' must be rejected");
    }

    /// A response with both `result` and `error` is
    /// well-formed but the spec says the error must take
    /// precedence (the LLM should see the error, not the
    /// stale result). The parser must surface an `Err` and
    /// not panic.
    #[test]
    fn extract_result_error_takes_precedence_over_result(
        result in json_value_strategy(),
        error_code in -32_000i32..0i32,
        error_message in prop::string::string_regex(r"[A-Za-z0-9 _]{1,64}").unwrap(),
    ) {
        let value = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": result,
            "error": { "code": error_code, "message": error_message }
        });
        let outcome = McpClientSession::extract_result("server", "method", value);
        prop_assert!(outcome.is_err(), "error envelope must take precedence");
    }

    /// A response with neither `result` nor `error` is
    /// rejected (the spec mandates exactly one).
    #[test]
    fn extract_result_rejects_missing_both_result_and_error(_unused in 0..1u8) {
        let value = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1
        });
        let outcome = McpClientSession::extract_result("server", "method", value);
        prop_assert!(
            outcome.is_err(),
            "response with neither result nor error must be rejected"
        );
    }

    /// A well-formed `{"jsonrpc":"2.0","id":N,"result":X}` is
    /// accepted and the `result` value is returned verbatim
    /// (cloned). The `id` field is not part of the return
    /// value — only the result body.
    #[test]
    fn extract_result_well_formed_returns_result(
        id in any::<u64>(),
        result in json_value_strategy()
    ) {
        let value = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result.clone()
        });
        let outcome = McpClientSession::extract_result("server", "method", value);
        prop_assert!(outcome.is_ok());
        let returned = outcome.unwrap();
        prop_assert_eq!(&returned, &result);
    }
}
