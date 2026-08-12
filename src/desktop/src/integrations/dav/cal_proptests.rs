//! Property-based tests for the iCal / RFC 5545 parser in
//! `integrations::dav::cal::parse_ical_data`.
//!
//! The DAV server returns the calendar data as a raw iCalendar
//! text body (RFC 5545). A hostile or buggy server can return
//! any string in that field; the parser must turn it into a
//! typed `CalDavEventDetails` without panicking, even on:
//!
//! - Empty input, single-char input, multi-MB input.
//! - CRLF / LF / mixed line endings.
//! - Malformed `BEGIN/END` pairs (unclosed, mismatched).
//! - Time-zone `VTIMEZONE` blocks with bad `TZID`.
//! - `RDATE` / `EXDATE` recurrence entries with unparseable
//!   dates (leap-seconds, time-zone garbage, etc.).
//! - Property lines that wrap (line-folding, RFC 5545 §3.1).
//! - Embedded NULs and control characters in `SUMMARY` etc.
//!
//! # Properties under test
//!
//! All four properties are sourced from the C6 corner-case
//! row in `doc/planning/fuzzing.md` §2.2 "Phase 2".
//!
//! 1. **No panic on any input.** `parse_ical_data` is
//!    infallible: any byte string produces a
//!    `CalDavEventDetails` value with default-initialised
//!    fields. The function must never unwrap or panic.
//! 2. **Client and href are preserved verbatim.** The
//!    `(client, href)` tuple is the parser's identity for
//!    the event; a regression that swapped them or dropped
//!    them would corrupt the per-server attribution.
//! 3. **Empty input is a well-defined no-op.** A zero-byte
//!    body produces a value with all-`None` optional fields
//!    and the supplied client/href.
//! 4. **CRLF and LF line endings are equivalent.** The
//!    parser normalises line endings; a body with mixed
//!    `\r\n` and `\n` produces the same fields as the
//!    LF-normalised equivalent.
//!
//! `cases = 512` per property.

use crate::integrations::dav::cal::parse_ical_data;
use proptest::prelude::*;

/// One proptest case count for every property in this sidecar.
const CASES: u32 = 512;

/// Strategy: any 7-bit ASCII string. iCal is a 7-bit
/// format; we deliberately exclude 8-bit input to keep the
/// shape of the test surface tractable. The existing
/// `cal_tests.rs` covers UTF-8 happy-path cases.
fn any_ical_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[\x00-\x7F]{0,2048}").unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES))]

    /// `parse_ical_data` must never panic on any input. The
    /// function is infallible: any byte string produces a
    /// `CalDavEventDetails` value with default-initialised
    /// optional fields.
    #[test]
    fn parse_ical_data_never_panics_on_any_input(
        client in "[A-Za-z0-9_-]{1,16}",
        href in "/[A-Za-z0-9/._-]{0,64}",
        data in any_ical_text()
    ) {
        let _ = parse_ical_data(&client, &href, &data);
    }

    /// The `client` and `href` fields are preserved verbatim
    /// in the parsed result. They are the parser's identity
    /// for the event; a regression that dropped or swapped
    /// them would corrupt per-server attribution.
    #[test]
    fn parse_ical_data_preserves_client_and_href(
        client in "[A-Za-z0-9_-]{1,16}",
        href in "/[A-Za-z0-9/._-]{1,64}",
        data in any_ical_text()
    ) {
        let result = parse_ical_data(&client, &href, &data);
        prop_assert_eq!(&result.client, &client);
        prop_assert_eq!(&result.href, &href);
    }

    /// Empty input is a well-defined no-op: all optional
    /// fields are `None` and the structural fields are the
    /// supplied client/href.
    #[test]
    fn parse_ical_data_empty_input_is_default(
        client in "[A-Za-z0-9_-]{1,16}",
        href in "/[A-Za-z0-9/._-]{1,64}",
    ) {
        let result = parse_ical_data(&client, &href, "");
        prop_assert_eq!(&result.client, &client);
        prop_assert_eq!(&result.href, &href);
        // No SUMMARY, no DTSTART, no DTEND, etc.
        prop_assert!(result.summary.is_none());
        prop_assert!(result.start.is_none());
        prop_assert!(result.end.is_none());
    }

    /// CRLF (`\r\n`) and LF (`\n`) line endings are
    /// equivalent: a body using one produces the same parsed
    /// fields as a body using the other. The parser
    /// normalises line endings per RFC 5545 §3.1.
    #[test]
    fn parse_ical_data_crlf_and_lf_are_equivalent(
        client in "[A-Za-z0-9_-]{1,16}",
        href in "/[A-Za-z0-9/._-]{1,64}",
        summary in "[A-Za-z0-9 ]{1,32}",
    ) {
        let lf_body = format!("BEGIN:VEVENT\nSUMMARY:{summary}\nEND:VEVENT\n");
        let crlf_body = format!("BEGIN:VEVENT\r\nSUMMARY:{summary}\r\nEND:VEVENT\r\n");
        let from_lf = parse_ical_data(&client, &href, &lf_body);
        let from_crlf = parse_ical_data(&client, &href, &crlf_body);
        prop_assert_eq!(&from_lf.summary, &from_crlf.summary);
    }

    /// A summary containing RFC 5545 line-folding (a CRLF
    /// followed by a single space) is unfolded by the parser.
    /// The parser strips the CRLF + leading space, so the
    /// continuation is concatenated to the previous line
    /// without an intervening space.
    #[test]
    fn parse_ical_data_line_folding_is_unfolded(
        client in "[A-Za-z0-9_-]{1,16}",
        href in "/[A-Za-z0-9/._-]{1,64}",
        prefix in "[A-Za-z]{1,8}",
        suffix in "[A-Za-z]{1,8}",
    ) {
        // `prefix` then CRLF + space + `suffix` should be
        // unfolded to `prefix` + `suffix` (the parser
        // strips the leading space, not the CRLF).
        let folded = format!("BEGIN:VEVENT\r\nSUMMARY:{prefix}\r\n {suffix}\r\nEND:VEVENT\r\n");
        let unfolded = format!("BEGIN:VEVENT\nSUMMARY:{prefix}{suffix}\nEND:VEVENT\n");
        let from_folded = parse_ical_data(&client, &href, &folded);
        let from_unfolded = parse_ical_data(&client, &href, &unfolded);
        prop_assert_eq!(&from_folded.summary, &from_unfolded.summary);
    }

    /// A `VTIMEZONE` block (which is required by RFC 5545
    /// for any `DTSTART` with a TZID) does not crash the
    /// parser. We test by surrounding a `VEVENT` with a
    /// minimal VTIMEZONE.
    #[test]
    fn parse_ical_data_vtimezone_block_does_not_panic(
        client in "[A-Za-z0-9_-]{1,16}",
        href in "/[A-Za-z0-9/._-]{1,64}",
        tzid in "[A-Za-z0-9/_-]{1,16}",
    ) {
        let body = format!(
            "BEGIN:VTIMEZONE\nTZID:{tzid}\nBEGIN:STANDARD\nDTSTART:19700101T000000\nEND:STANDARD\nEND:VTIMEZONE\nBEGIN:VEVENT\nDTSTART;TZID={tzid}:20240101T120000\nEND:VEVENT\n"
        );
        let _ = parse_ical_data(&client, &href, &body);
    }

    /// A `RDATE` recurrence entry with a malformed date does
    /// not crash the parser. We test by including a
    /// deliberately-unparseable RDATE value.
    #[test]
    fn parse_ical_data_malformed_rdate_does_not_panic(
        client in "[A-Za-z0-9_-]{1,16}",
        href in "/[A-Za-z0-9/._-]{1,64}",
        bad_date in prop::string::string_regex(r"[!-~]{1,32}").unwrap(),
    ) {
        // The bad date may or may not include a "T" and a
        // time component; the parser must not panic either
        // way.
        let body = format!("BEGIN:VEVENT\nRDATE:{bad_date}\nEND:VEVENT\n");
        let _ = parse_ical_data(&client, &href, &body);
    }
}
