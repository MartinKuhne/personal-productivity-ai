//! Property-based tests for the query-string parser and
//! resource-URI builder in `integrations::mcp::oauth::redirect`
//! and `integrations::mcp::oauth::flow`.
//!
//! The redirect callback parser is the entry point for
//! attacker-controlled input: the user's browser navigates to
//! `http://127.0.0.1:<port>/callback?code=…&state=…` after the
//! authorization server finishes, and a bug in the parser is a
//! CSRF / state-confusion vector.
//!
//! The full `run_flow` is not testable here (it needs a real
//! loopback HTTP server + browser). What we *can* fuzz are the
//! two pure helpers it relies on:
//!
//! - `parse_query` (in `redirect.rs`) — splits a redirect URL's
//!   query string into a `HashMap<String, String>`. The full
//!   `CallbackParams` builder uses this internally.
//! - `build_resource_uri` (in `flow.rs`) — strips the
//!   fragment and query string from an MCP server URL per
//!   RFC 8707.
//!
//! # Properties under test
//!
//! All four properties are sourced from the C4 corner-case
//! row in `doc/planning/fuzzing.md` §2.2 "Phase 2".
//!
//! 1. **`parse_query` never panics.** Any byte string is a
//!    valid query (it just produces an empty `HashMap`).
//! 2. **`state=`-repeated handling.** If the body carries
//!    `state=A&state=B`, the parser must keep the first (or
//!    last — we don't care which) value but must NOT panic
//!    and must produce exactly one entry for `state` in the
//!    resulting map. (The flow's state-check rejects *either*
//!    value vs. the freshly-generated one.)
//! 3. **Embedded `\n` in a value does not split the line.** A
//!    value like `abc%0Adef` is decoded to `abc\ndef` and
//!    stored as a single value. A bug that treated `\n` as a
//    key/value boundary would let an attacker smuggle extra
//!    query parameters.
//! 4. **`build_resource_uri` strips fragment and query
//!    string but preserves the rest of the URL verbatim.** A
//!    regression here would change the resource parameter
//!    sent to the token endpoint, breaking the
//!    RFC-8707-bound token.
//!
//! `cases = 512` per property.

use crate::integrations::mcp::oauth::flow::build_resource_uri;
use proptest::prelude::*;

/// One proptest case count for every property in this sidecar.
const CASES: u32 = 512;

/// Strategy: any printable ASCII string. The redirect URI's
/// query string is RFC 3986 unreserved + a few percent-encoded
/// specials; the parser is intentionally permissive so we use
/// the same wider alphabet as a hostile server might.
fn any_query_string() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[\x20-\x7E]{0,512}").unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES))]

    /// `parse_query` never panics on any input. Any byte
    /// string is a valid query; the parser produces an empty
    /// `HashMap` for an empty input and a fully-populated
    /// `HashMap` for a well-formed one.
    #[test]
    fn parse_query_does_not_panic_on_any_input(q in any_query_string()) {
        use crate::integrations::mcp::oauth::redirect::parse_query;
        let out = parse_query(&q);
        let _ = out.len();
    }

    /// Repeated `state=` keys must produce a single `state`
    /// entry (the HashMap dedupes). The original is
    /// CSRF-vulnerable either way, but a panic would be
    /// worse.
    #[test]
    fn parse_query_repeated_state_does_not_panic(
        state_a in "[A-Za-z0-9]{1,32}",
        state_b in "[A-Za-z0-9]{1,32}",
    ) {
        prop_assume!(state_a != state_b);
        use crate::integrations::mcp::oauth::redirect::parse_query;
        let q = format!("state={state_a}&state={state_b}");
        let out = parse_query(&q);
        let state = out.get("state");
        prop_assert!(state.is_some());
        let v = state.unwrap();
        prop_assert!(v == &state_a || v == &state_b);
    }

    /// Embedded `\n` (encoded as `%0A` in the URL) decodes
    /// to a single literal newline inside the value. It does
    /// NOT split the value into a new key/value pair.
    #[test]
    fn parse_query_percent_encoded_newline_does_not_split(
        prefix in "[A-Za-z0-9]{1,16}",
        suffix in "[A-Za-z0-9]{1,16}",
    ) {
        use crate::integrations::mcp::oauth::redirect::parse_query;
        let q = format!("k={prefix}%0A{suffix}");
        let out = parse_query(&q);
        let v = out.get("k").expect("key `k` must be present");
        prop_assert_eq!(v, &format!("{prefix}\n{suffix}"));
        // The parser must not have produced a phantom second
        // key from the embedded newline.
        prop_assert_eq!(out.len(), 1);
    }

    /// `parse_query` with `code` and `&` inside the code
    /// value (encoded as `%26`) decodes the value to a
    /// single string that contains a literal `&`. A
    /// regression that treated unencoded `&` as the only
    /// separator would split the value into two keys.
    #[test]
    fn parse_query_percent_encoded_ampersand_does_not_split(
        prefix in "[A-Za-z0-9]{1,16}",
        suffix in "[A-Za-z0-9]{1,16}",
    ) {
        use crate::integrations::mcp::oauth::redirect::parse_query;
        let q = format!("code={prefix}%26{suffix}");
        let out = parse_query(&q);
        let v = out.get("code").expect("key `code` must be present");
        prop_assert_eq!(v, &format!("{prefix}&{suffix}"));
    }

    /// `build_resource_uri` strips the fragment and query
    /// string from the URL, preserving the rest verbatim.
    /// The result must equal the input with everything after
    /// the first `?` and `#` removed.
    #[test]
    fn build_resource_uri_strips_query_and_fragment(
        scheme in "https?",
        host in "[a-z]{2,16}\\.[a-z]{2,8}",
        path in "/[a-z0-9/]{0,32}",
        query in "[a-z0-9=&]{0,32}",
        fragment in "[a-z0-9]{0,16}",
    ) {
        let mut url = format!("{scheme}://{host}{path}");
        if !query.is_empty() {
            url.push('?');
            url.push_str(&query);
        }
        if !fragment.is_empty() {
            url.push('#');
            url.push_str(&fragment);
        }
        let result = build_resource_uri(&url).expect("valid URL should parse");
        let expected_base = format!("{scheme}://{host}{path}");
        prop_assert_eq!(&result, &expected_base);
    }

    /// `build_resource_uri` rejects non-http(s) schemes. The
    /// resource parameter requires https (or http for
    /// loopback).
    #[test]
    fn build_resource_uri_rejects_non_http_schemes(
        scheme in "[a-z]{2,6}",
    ) {
        prop_assume!(scheme != "http" && scheme != "https");
        let url = format!("{scheme}://example.com/foo");
        let result = build_resource_uri(&url);
        prop_assert!(result.is_err(), "non-http(s) scheme must be rejected");
    }
}
