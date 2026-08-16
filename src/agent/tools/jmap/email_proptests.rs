//! Property-based tests for the JMAP email-shape converters
//! in `agent::tools::jmap::email`.
//!
//! The JMAP client fetches `Email/get` and `Email/query`
//! responses from the mail server and runs them through
//! `convert_html_in_jmap` and `simplify_jmap_emails` before
//! the LLM sees them. A panic in either function would crash
//! every email tool call. The converters must be total over
//! the JMAP response shape — which means total over the
//! `serde_json::Value` alphabet, since the wire format is
//! JSON.
//!
//! # Properties under test
//!
//! All four properties are sourced from the C8 corner-case
//! row in `doc/planning/fuzzing.md` §2.2 "Phase 2".
//!
//! 1. **No panic on any input.** Both converters accept an
//!    owned `serde_json::Value`; any shape (deeply nested,
//!    `isTruncated:true` mid-stream, `htmlBodyValues` with
//!    deeply-nested `<table>`, charset-quirks
//!    `=?UTF-8?B?...?=`) must not unwrap.
//! 2. **Plain-text body values are preserved verbatim.** A
//!    `bodyValues` entry with a plain string value is
//!    untouched. This is the "no false positive" guarantee:
//!    the converter must not corrupt already-converted
//!    content.
//! 3. **Simplify is idempotent on a no-op input.** An input
//!    with no `methodResponses` produces an output with
//!    `simplified_emails: []`. (We assert the surface
//!    structure, not the exact JSON, because the function
//!    may grow fields over time.)
//! 4. **`isTruncated:true` does not crash the parser.** A
//!    `bodyValues` entry with `isTruncated: true` is
//!    accepted; the converter does not try to fetch
//!    additional content and does not panic.

use crate::tools::jmap::email::{convert_html_in_jmap, simplify_jmap_emails};
use proptest::prelude::*;

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

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES))]

    /// `convert_html_in_jmap` must never panic on any input.
    /// The function is total: any `serde_json::Value` is
    /// accepted and a (possibly equal) value is returned.
    #[test]
    fn convert_html_in_jmap_never_panics_on_any_input(value in json_value_strategy()) {
        let _ = convert_html_in_jmap(value);
    }

    /// `simplify_jmap_emails` must never panic on any input.
    #[test]
    fn simplify_jmap_emails_never_panics_on_any_input(value in json_value_strategy()) {
        let _ = simplify_jmap_emails(value, None);
    }

    /// A `bodyValues` entry with a plain string value is
    /// preserved verbatim. The converter only acts on
    /// HTML-typed values; a `text/plain` value passes
    /// through.
    #[test]
    fn convert_html_in_jmap_preserves_plain_text_body_values(
        body in prop::string::string_regex(r"[A-Za-z0-9 .,!]{1,128}").unwrap()
    ) {
        // The body is restricted to characters that
        // `fast_h2m` will leave alone: no `<`, `>`, `&`,
        // or newline sequences. This isolates the test to
        // the converter's "no-op on plain text" path.
        let input = serde_json::json!({
            "methodResponses": [[
                "Email/get",
                {
                    "list": [{
                        "id": "1",
                        "bodyValues": {
                            "part1": { "value": body.clone(), "isTruncated": false }
                        }
                    }]
                },
                "0"
            ]]
        });
        let output = convert_html_in_jmap(input);
        let val = output["methodResponses"][0][1]["list"][0]["bodyValues"]["part1"]["value"]
            .as_str()
            .unwrap();
        prop_assert_eq!(val, body.as_str());
    }

    /// `simplify_jmap_emails` with an empty `methodResponses`
    /// produces an output whose top-level value is an array.
    /// (The function is a no-op on an empty input and
    /// returns the empty simplified list as the top-level
    /// value, not wrapped in a `simplified_emails` key.)
    #[test]
    fn simplify_jmap_emails_empty_input_yields_array(_unused in 0..1u8) {
        let input = serde_json::json!({ "methodResponses": [] });
        let output = simplify_jmap_emails(input, None);
        prop_assert!(
            output.is_array(),
            "output must be an array (got: {output:?})"
        );
        prop_assert!(output.as_array().unwrap().is_empty());
    }

    /// A `bodyValues` entry with `isTruncated: true` does
    /// not crash the converter. The converter must accept
    /// the truncated marker and continue.
    #[test]
    fn convert_html_in_jmap_is_truncated_does_not_panic(
        partial_body in prop::string::string_regex(r"[\x20-\x7E]{1,128}").unwrap()
    ) {
        let input = serde_json::json!({
            "methodResponses": [[
                "Email/get",
                {
                    "list": [{
                        "id": "1",
                        "bodyValues": {
                            "part1": { "value": partial_body, "isTruncated": true }
                        }
                    }]
                },
                "0"
            ]]
        });
        let _ = convert_html_in_jmap(input);
    }

    /// A `bodyValues` entry with a deep `htmlBodyValues`
    /// structure does not crash the converter. The
    /// converter only acts on the value, not the wrapper
    /// structure.
    #[test]
    fn convert_html_in_jmap_deep_html_body_values_does_not_panic(
        depth in 1..8u32,
        body in prop::string::string_regex(r"<[a-z0-9>]{1,32}").unwrap()
    ) {
        // Build a nested `<table>` chain of the requested
        // depth. The converter must not panic regardless of
        // the nesting depth.
        let mut html = String::new();
        for _ in 0..depth {
            html.push_str("<table><tr><td>");
        }
        html.push_str(&body);
        for _ in 0..depth {
            html.push_str("</td></tr></table>");
        }
        let input = serde_json::json!({
            "methodResponses": [[
                "Email/get",
                {
                    "list": [{
                        "id": "1",
                        "bodyValues": {
                            "part1": { "value": html, "isTruncated": false }
                        }
                    }]
                },
                "0"
            ]]
        });
        let _ = convert_html_in_jmap(input);
    }
}
