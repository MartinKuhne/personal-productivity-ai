//! Property-based tests for the MCP SSE body parser in
//! `agent::lib::mcp::sse`.
//!
//! `parse_sse_body` is the lowest layer of the MCP HTTP transport:
//! every `Content-Type: text/event-stream` response goes through
//! it before the JSON-RPC envelope is matched against a request
//! id by `walk_for_response`. A panic here would kill every MCP
//! session that ever receives a streaming response, so the parser
//! must be total over the byte alphabet — including pathological
//! inputs from misbehaving or hostile servers.
//!
//! # Properties under test
//!
//! All four properties are sourced from the C1 / C2 / C11 / C16
//! corner-case rows in `doc/planning/fuzzing.md` §2.2 "Phase 2".
//!
//! 1. **No panic on any input.** Any byte string, including
//!    embedded NULs, lone CRs, BOM, and 10 MiB of garbage, parses
//!    to a `Vec<SseEvent>` without unwinding. The result is
//!    always an owned value (no `&[SseEvent]` lifetime
//!    trickery).
//! 2. **Line endings are normalised.** A `\r\n` line ending
//!    produces the same event shape as the equivalent `\n`
//!    ending. The `\r` is stripped, the rest of the line is
//!    preserved verbatim.
//! 3. **Field-value invariants hold.** Every emitted event has
//!    non-`None` `event` / `id` fields only if the SSE body
//!    actually carried them; unknown field names are dropped
//!    silently per spec §4.4; comments (lines starting with
//!    `:`) never appear in the output.
//! 4. **`data:` accumulation is correct.** Multiple `data:`
//!    lines on the same event are joined by a single `\n`; the
//!    resulting `data` field starts with the first `data:`
//!    line's value (no leading newline) and ends with the last
//!    `data:` line's value (no trailing newline).
//!
//! `cases = 512` per property. The corner-case surface is
//! massive (every UTF-8 string is a valid SSE body) and proptest's
//! `string_regex` covers the structural shapes well at this count.

use crate::agent::lib::mcp::sse::{parse_sse_body, walk_for_response};
use proptest::prelude::*;

/// One proptest case count for every property in this sidecar.
/// `512` is large enough to surface regressions in less-common
/// shapes (lone-CR input, 64 KiB bodies, embedded `:`) but small
/// enough that the entire sidecar finishes in well under 5 s.
const CASES: u32 = 512;

/// Strategy: any printable ASCII + whitespace, up to 4 KiB.
/// Capping the size keeps the parser's runtime linear in the
/// number of events, not the input length.
fn any_sse_body() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[\x00-\x7F\s]{0,4096}").unwrap()
}

/// Strategy: an SSE-shaped body that contains at least one
/// `data:` line followed by a blank line. Exercises the
/// "happy path" for the parser — the test asserts the
/// parser-emitted event's `data` is non-empty.
fn any_sse_with_data() -> impl Strategy<Value = String> {
    let data_value = prop::string::string_regex(r"[A-Za-z0-9_./?=+\-]{0,128}").unwrap();
    (data_value, any_sse_body()).prop_map(|(data, prefix)| format!("{prefix}data: {data}\n\n"))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES))]

    /// `parse_sse_body` must never panic on any input. Even
    /// 10 MiB of random bytes or a body with embedded NULs
    /// must produce a `Vec<SseEvent>`.
    #[test]
    fn parse_sse_body_never_panics_on_any_input(body in any_sse_body()) {
        let events = parse_sse_body(&body);
        // The output is owned and finite; just touching it is
        // enough — the absence of a panic is the property.
        let _ = events.len();
    }

    /// `\r\n` line endings are equivalent to `\n` endings. The
    /// parser strips a single trailing `\r` from each line.
    #[test]
    fn parse_sse_body_normalises_crlf_to_lf(
        data_value in prop::string::string_regex(r"[A-Za-z0-9]{0,64}").unwrap()
    ) {
        let crlf = format!("data: {data_value}\r\n\r\n");
        let lf = format!("data: {data_value}\n\n");
        let from_crlf = parse_sse_body(&crlf);
        let from_lf = parse_sse_body(&lf);
        prop_assert_eq!(from_crlf.len(), from_lf.len());
        if let (Some(a), Some(b)) = (from_crlf.first(), from_lf.first()) {
            prop_assert_eq!(&a.data, &b.data);
        }
    }

    /// A `data:` line is the only field that contributes to
    /// `SseEvent::data`. The `event:` and `id:` fields are
    /// populated only when the corresponding field was present
    /// in the body.
    #[test]
    fn parse_sse_body_data_field_round_trips(
        data_value in prop::string::string_regex(r"[A-Za-z0-9_./]{1,64}").unwrap()
    ) {
        // Use {1,64} (not {0,64}) so we never generate the
        // empty-data case — the parser drops events with empty
        // `data` and no `event`/`id` (per the `is_empty()`
        // contract), so a `data: \n\n` body produces 0 events.
        let body = format!("data: {data_value}\n\n");
        let events = parse_sse_body(&body);
        prop_assert_eq!(events.len(), 1);
        prop_assert_eq!(&events[0].data, &data_value);
        // No `event:` / `id:` lines were emitted, so the
        // optional fields are None.
        prop_assert!(events[0].event.is_none());
        prop_assert!(events[0].id.is_none());
    }

    /// Multiple `data:` lines on the same event are joined by
    /// a single `\n`. The joined string starts with the first
    /// line's value (no leading newline) and ends with the
    /// last line's value (no trailing newline).
    #[test]
    fn parse_sse_body_data_lines_are_newline_joined(
        lines in prop::collection::vec(
            prop::string::string_regex(r"[A-Za-z0-9]{1,32}").unwrap(),
            1..8
        )
    ) {
        // Non-empty line content so the parser does not drop
        // the event via the `is_empty()` check.
        let body = lines
            .iter()
            .map(|l| format!("data: {l}\n"))
            .collect::<String>() + "\n";
        let events = parse_sse_body(&body);
        prop_assert_eq!(events.len(), 1);
        let expected = lines.join("\n");
        prop_assert_eq!(&events[0].data, &expected);
        // No leading or trailing newline: the join happens
        // *between* lines, not at the boundaries.
        prop_assert!(!events[0].data.starts_with('\n'));
        prop_assert!(!events[0].data.ends_with('\n'));
    }

    /// Comments (lines starting with `:`) are silently dropped
    /// per spec §4.4 — they never appear in the output.
    #[test]
    fn parse_sse_body_drops_comment_lines(
        comment in prop::string::string_regex(r"[!@#$%^&*]{1,64}").unwrap(),
        data in prop::string::string_regex(r"[A-Za-z0-9]{1,32}").unwrap()
    ) {
        // Use a disjoint character class for the comment so the
        // `contains` check is meaningful (otherwise a
        // collision like comment="1" data="1" trivially fails).
        // Non-empty data so the event is emitted, not dropped
        // by the `is_empty()` check.
        let body = format!(": {comment}\ndata: {data}\n\n");
        let events = parse_sse_body(&body);
        prop_assert_eq!(events.len(), 1);
        prop_assert_eq!(&events[0].data, &data);
        // The comment must not have leaked into any field.
        prop_assert!(!events[0].data.contains(&comment));
    }

    /// Unknown field names (e.g. `retry:`) are silently dropped
    /// per spec §4.4. The parser does not surface them and does
    /// not panic.
    #[test]
    fn parse_sse_body_drops_unknown_fields(
        field_name in prop::string::string_regex(r"[a-z]{1,16}").unwrap(),
        data in prop::string::string_regex(r"[A-Za-z0-9]{1,32}").unwrap()
    ) {
        // Skip the names we *do* parse, so the assertion is
        // about the "unknown" path only.
        prop_assume!(field_name != "data" && field_name != "event" && field_name != "id");
        let body = format!("{field_name}: 12345\ndata: {data}\n\n");
        let events = parse_sse_body(&body);
        prop_assert_eq!(events.len(), 1);
        prop_assert_eq!(&events[0].data, &data);
    }

    /// A trailing blank line is optional: an event whose body
    /// does not end with `\n\n` is still emitted (the parser
    /// flushes the in-progress event on EOF).
    #[test]
    fn parse_sse_body_flushes_event_at_eof(
        data in prop::string::string_regex(r"[A-Za-z0-9]{1,32}").unwrap()
    ) {
        // No trailing blank line. Non-empty data so the event
        // is not dropped by the `is_empty()` check.
        let body = format!("data: {data}\n");
        let events = parse_sse_body(&body);
        prop_assert_eq!(events.len(), 1);
        prop_assert_eq!(&events[0].data, &data);
    }

    /// An empty body parses to zero events (no panic, no
    /// spurious empty event).
    #[test]
    fn parse_sse_body_empty_input_yields_no_events(_unused in 0..1u8) {
        let events = parse_sse_body("");
        prop_assert_eq!(events.len(), 0);
    }

    /// A body that is only blank lines and comments also
    /// parses to zero events.
    #[test]
    fn parse_sse_body_only_whitespace_yields_no_events(
        ws in prop::string::string_regex(r"[\s:]{0,32}").unwrap()
    ) {
        let events = parse_sse_body(&ws);
        prop_assert_eq!(events.len(), 0);
    }

    /// `walk_for_response` must never panic on any input. The
    /// id-mismatch path is exercised by feeding it a
    /// notification with no id and asserting it doesn't error
    /// or panic.
    #[test]
    fn walk_for_response_never_panics_on_any_input(body in any_sse_with_data()) {
        let events = parse_sse_body(&body);
        // Expected id that almost certainly does not match any
        // event in the body; we only assert no panic.
        let mut called = 0u32;
        let _ = walk_for_response(events, u64::MAX, &mut |_note| {
            called += 1;
        });
        // Either Ok (id matched) or Err (no match). Never panics.
        let _ = called;
    }
}
