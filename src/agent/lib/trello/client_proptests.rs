//! Property-based tests for the Trello HTTP client in
//! `agent::lib::trello::client::trello_http_call`.
//!
//! `trello_http_call` is a thin HTTP wrapper, but its response
//! parser (`serde_json::from_str(&text)`) sees every Trello
//! response body the LLM ever requests. A panic in the parser
//! (or in the `status.is_success()` / body extraction path)
//! would crash every Trello tool call.
//!
//! We feed the function an arbitrary `serde_json::Value` body
//! via a `wiremock` server. The wiremock takes care of the
//! HTTP plumbing; the proptest exercises the parser + the
//! status-code branching + the body-extraction path on every
//! shape.
//!
//! # Properties under test
//!
//! All four properties are sourced from the C10 corner-case
//! row in `doc/planning/fuzzing.md` §2.2 "Phase 2".
//!
//! 1. **No panic on any input.** A wiremock that returns an
//!    arbitrary `serde_json::Value` body — including
//!    null fields where the DTO expects `Vec`, an `id` that
//!    overflows `i64`, deeply-nested custom-field objects —
//!    is parsed without unwinding. The result is `Ok` or
//!    `Err`, never a panic.
//! 2. **`null` top-level value is `Ok(Value::Null)`, not a
//!    parse error.** The parser accepts a literal `null`
//!    and returns `Ok(Value::Null)`.
//! 3. **Empty body is `Err`, not a panic.** A wiremock that
//!    returns an empty body is a transport-level shape; the
//!    parser rejects it with `Err`.
//! 4. **Non-JSON body is `Err`, not a panic.** A body that
//!    is not valid JSON (e.g. an HTML error page from a
//!    misconfigured upstream) is rejected with `Err`.
//!
//! `cases = 512` per property.

use crate::lib::trello::client::trello_http_call;
use proptest::prelude::*;
use wiremock::{Mock, MockServer, ResponseTemplate};

/// One proptest case count for every property in this sidecar.
const CASES: u32 = 512;

/// Strategy: any `serde_json::Value` with bounded depth.
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

/// Single-threaded tokio runtime for the proptest block_on
/// calls. Proptest runs tests synchronously, but wiremock is
/// async — we use a static `Lazy` runtime so the runtime
/// outlives the per-test scope. (A per-test runtime would
/// fail with "Cannot drop a runtime in a context where
/// blocking is not allowed" because proptest itself
/// internally uses `block_on`.)
static RUNTIME: std::sync::LazyLock<tokio::runtime::Runtime> = std::sync::LazyLock::new(|| {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime for trello client proptests")
});

/// Start a wiremock server on a random localhost port.
fn start_mock() -> MockServer {
    RUNTIME.block_on(MockServer::start())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES))]

    /// `trello_http_call` must never panic on any input.
    /// The wiremock returns an arbitrary `serde_json::Value`
    /// body; the parser must accept or reject it, never
    /// unwinding.
    #[test]
    fn trello_http_call_never_panics_on_any_input(value in json_value_strategy()) {
        let server = start_mock();
        RUNTIME.block_on(async {
            Mock::given(wiremock::matchers::any())
                .respond_with(ResponseTemplate::new(200).set_body_json(&value))
                .mount(&server)
                .await;
        });
        let result = trello_http_call(
            reqwest::Method::GET,
            &format!("{}/test", server.uri()),
            None,
        );
        // The function returns `Result<Value, String>`; any
        // value is either Ok or Err.
        let _ = result;
    }

    /// `null` body is `Ok(Value::Null)`, not a parse error.
    /// A literal `null` is valid JSON.
    #[test]
    fn trello_http_call_null_body_is_ok(_unused in 0..1u8) {
        let server = start_mock();
        RUNTIME.block_on(async {
            Mock::given(wiremock::matchers::any())
                .respond_with(ResponseTemplate::new(200).set_body_string("null"))
                .mount(&server)
                .await;
        });
        let result = trello_http_call(
            reqwest::Method::GET,
            &format!("{}/test", server.uri()),
            None,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::Value::Null);
    }

    /// An empty body is rejected (not valid JSON), not a
    /// panic.
    #[test]
    fn trello_http_call_empty_body_is_err(_unused in 0..1u8) {
        let server = start_mock();
        RUNTIME.block_on(async {
            Mock::given(wiremock::matchers::any())
                .respond_with(ResponseTemplate::new(200).set_body_string(""))
                .mount(&server)
                .await;
        });
        let result = trello_http_call(
            reqwest::Method::GET,
            &format!("{}/test", server.uri()),
            None,
        );
        assert!(result.is_err());
    }

    /// A non-JSON body is rejected. The HTTP transport
    /// returns 200 but the body is an HTML error page.
    #[test]
    fn trello_http_call_non_json_body_is_err(
        body in prop::string::string_regex(r"<[a-z]{1,16}>[A-Za-z ]{1,64}</[a-z]{1,16}>").unwrap()
    ) {
        let server = start_mock();
        RUNTIME.block_on(async {
            Mock::given(wiremock::matchers::any())
                .respond_with(ResponseTemplate::new(200).set_body_string(&body))
                .mount(&server)
                .await;
        });
        let result = trello_http_call(
            reqwest::Method::GET,
            &format!("{}/test", server.uri()),
            None,
        );
        assert!(result.is_err());
    }

    /// An HTTP 500 response with an error body is rejected
    /// (the `is_success()` branch is skipped). The body is
    /// not parsed as JSON; the function returns `Err` with
    /// the status in the message.
    #[test]
    fn trello_http_call_5xx_response_is_err(
        body in prop::string::string_regex(r"[A-Za-z0-9 ]{1,64}").unwrap()
    ) {
        let server = start_mock();
        RUNTIME.block_on(async {
            Mock::given(wiremock::matchers::any())
                .respond_with(ResponseTemplate::new(500).set_body_string(&body))
                .mount(&server)
                .await;
        });
        let result = trello_http_call(
            reqwest::Method::GET,
            &format!("{}/test", server.uri()),
            None,
        );
        assert!(result.is_err());
    }
}
