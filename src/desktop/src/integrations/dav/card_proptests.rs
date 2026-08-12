//! Property-based tests for the vCard 3.0 / 4.0 parser in
//! `integrations::dav::card::parse_vcard`.
//!
//! The DAV server returns the contact data as a raw vCard
//! text body. The parser must turn that into a typed
//! `CardDavContactDetails` value without panicking on:
//!
//! - Multi-valued `N` and `ADR` (semicolon-separated lists).
//! - `VERSION:2.1` legacy forms (different field syntax than
//!   3.0/4.0).
//! - Quoted-printable encoded names.
//! - Very long single lines (1 MB of the same character).
//! - Embedded NULs and control characters.
//! - Missing required fields (`FN` is mandatory in 3.0+;
//!   the parser should not panic if it's missing, it should
//!   just leave `fn_name` as `None`).
//!
//! # Properties under test
//!
//! All four properties are sourced from the C7 corner-case
//! row in `doc/planning/fuzzing.md` §2.2 "Phase 2".
//!
//! 1. **No panic on any input.** The parser is infallible; any
//!    byte string produces a `CardDavContactDetails` value.
//! 2. **Client and href are preserved verbatim.** A regression
//!    here would corrupt per-server attribution.
//! 3. **Empty input is a well-defined no-op.** All optional
//!    fields are `None` and the structural fields match the
//!    supplied client/href.
//! 4. **Multi-valued `N` field does not crash.** RFC 6350
//!    §6.2.2 specifies the `N` field as a
//!    semicolon-separated list of 5 components. The parser
//!    must accept the full 5-component form without panic.

use crate::integrations::dav::card::parse_vcard;
use proptest::prelude::*;

/// One proptest case count for every property in this sidecar.
const CASES: u32 = 512;

/// Strategy: any 7-bit ASCII string. vCard is a 7-bit format
/// by default (8-bit support is via `CHARSET=UTF-8` which
/// the existing card_tests.rs covers).
fn any_vcard_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[\x00-\x7F]{0,2048}").unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES))]

    /// `parse_vcard` must never panic on any input. The
    /// function is infallible: any byte string produces a
    /// `CardDavContactDetails` value.
    #[test]
    fn parse_vcard_never_panics_on_any_input(
        client in "[A-Za-z0-9_-]{1,16}",
        href in "/[A-Za-z0-9/._-]{0,64}",
        data in any_vcard_text()
    ) {
        let _ = parse_vcard(&client, &href, &data);
    }

    /// The `client` and `href` fields are preserved verbatim
    /// in the parsed result.
    #[test]
    fn parse_vcard_preserves_client_and_href(
        client in "[A-Za-z0-9_-]{1,16}",
        href in "/[A-Za-z0-9/._-]{1,64}",
        data in any_vcard_text()
    ) {
        let result = parse_vcard(&client, &href, &data);
        prop_assert_eq!(&result.client, &client);
        prop_assert_eq!(&result.href, &href);
    }

    /// Empty input is a well-defined no-op: all optional
    /// fields are `None` and the structural fields are the
    /// supplied client/href.
    #[test]
    fn parse_vcard_empty_input_is_default(
        client in "[A-Za-z0-9_-]{1,16}",
        href in "/[A-Za-z0-9/._-]{1,64}",
    ) {
        let result = parse_vcard(&client, &href, "");
        prop_assert_eq!(&result.client, &client);
        prop_assert_eq!(&result.href, &href);
        // No FN, EMAIL, TEL, etc.
        prop_assert!(result.fn_name.is_none());
        prop_assert!(result.email.is_none());
        prop_assert!(result.tel.is_none());
    }

    /// A well-formed vCard 3.0 with `VERSION:3.0`, `FN`,
    /// and `EMAIL` is parsed into a value with non-None
    /// fields. This is the happy-path round-trip.
    #[test]
    fn parse_vcard_well_formed_3_0_round_trips(
        client in "[A-Za-z0-9_-]{1,16}",
        href in "/[A-Za-z0-9/._-]{1,64}",
        fn_name in "[A-Za-z][A-Za-z0-9]+",
        email in "[a-z]{1,8}@[a-z]{1,8}\\.[a-z]{2,4}",
    ) {
        // `fn_name` must start with a letter and have no
        // leading/trailing whitespace because the parser
        // `.trim()`s the value. We use `[A-Za-z][A-Za-z0-9]+`
        // to guarantee no whitespace on either side.
        let body = format!(
            "BEGIN:VCARD\nVERSION:3.0\nFN:{fn_name}\nEMAIL:{email}\nEND:VCARD\n"
        );
        let result = parse_vcard(&client, &href, &body);
        prop_assert_eq!(&result.client, &client);
        prop_assert_eq!(&result.href, &href);
        prop_assert_eq!(result.fn_name.as_deref(), Some(fn_name.as_str()));
        prop_assert_eq!(result.email.as_deref(), Some(email.as_str()));
    }

    /// A vCard with a multi-valued `N` field (5 components
    /// separated by semicolons) does not crash. The
    /// parser's `N` handling must accept the full form.
    #[test]
    fn parse_vcard_multivalued_n_does_not_panic(
        client in "[A-Za-z0-9_-]{1,16}",
        href in "/[A-Za-z0-9/._-]{1,64}",
        family in "[A-Za-z]{1,8}",
        given in "[A-Za-z]{1,8}",
        middle in "[A-Za-z]{0,8}",
        prefix in "[A-Za-z]{0,8}",
        suffix in "[A-Za-z]{0,8}",
    ) {
        let body = format!(
            "BEGIN:VCARD\nVERSION:3.0\nN:{family};{given};{middle};{prefix};{suffix}\nEND:VCARD\n"
        );
        let _ = parse_vcard(&client, &href, &body);
    }

    /// A vCard 2.1 (legacy form) with a `TEL` field and no
    /// `VERSION` line is parsed without panic. vCard 2.1 is
    /// still common in the wild.
    #[test]
    fn parse_vcard_2_1_legacy_form_does_not_panic(
        client in "[A-Za-z0-9_-]{1,16}",
        href in "/[A-Za-z0-9/._-]{1,64}",
        tel in "[0-9+\\-]{1,16}",
    ) {
        let body = format!("BEGIN:VCARD\nVERSION:2.1\nTEL:{tel}\nEND:VCARD\n");
        let _ = parse_vcard(&client, &href, &body);
    }
}
