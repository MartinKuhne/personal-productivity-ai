//! Tests for [`super::pins`] — the closed Host `SPEC.md` Literal Pin
//! Table (`UIF-PRIN-007`).

use super::*;

/// The nine literals §1.1 line 34 lists, transcribed independently of
/// the table under test.
const SPEC_LITERALS: [&str; 9] = [
    "Ok",
    "Cancel",
    "Open in Editor",
    "Show in File Explorer",
    "Create Directory ...",
    "Close Others",
    "Close All",
    "Copy Path",
    "Windows...",
];

#[test]
fn test_the_table_carries_exactly_nine_literals() {
    assert_eq!(
        HOST_PIN_TABLE.len(),
        9,
        "§1.1 pins exactly nine literals; the table is closed"
    );
}

#[test]
fn test_every_spec_literal_is_present_verbatim() {
    for literal in SPEC_LITERALS {
        assert!(
            HOST_PIN_TABLE.iter().any(|pin| pin.literal == literal),
            "'{literal}' is pinned by the spec but missing from the table"
        );
    }
}

#[test]
fn test_no_unpinned_literal_sneaked_into_the_table() {
    for pin in HOST_PIN_TABLE {
        assert!(
            SPEC_LITERALS.contains(&pin.literal),
            "'{}' is not one of §1.1's nine pinned literals",
            pin.literal
        );
    }
}

#[test]
fn test_lookup_is_exact_match() {
    assert!(is_pinned("Copy Path"));
    assert_eq!(pin_for("Copy Path").map(|p| p.literal), Some("Copy Path"));
}

#[test]
fn test_the_unpinned_near_miss_is_not_pinned() {
    // §4.9 says `Copy path` on tree targets is UNPINNED and follows
    // UIF-PRIN-007. A fuzzy lookup would silently exempt it.
    assert!(
        !is_pinned("Copy path"),
        "'Copy path' is explicitly unpinned and must not match 'Copy Path'"
    );
    assert_eq!(pin_for("Copy path"), None);
}

#[test]
fn test_case_variants_are_not_pinned() {
    for variant in [
        "OK",
        "ok",
        "CANCEL",
        "cancel",
        "OPEN IN EDITOR",
        "close all",
    ] {
        assert!(
            !is_pinned(variant),
            "'{variant}' is a case variant of a pinned literal and must not match"
        );
    }
}

#[test]
fn test_whitespace_variants_are_not_pinned() {
    for variant in [" Ok", "Ok ", "Close  Others", "Create  Directory ..."] {
        assert!(!is_pinned(variant), "'{variant}' must not match");
    }
}

#[test]
fn test_ok_and_cancel_are_exempt_from_verb_first() {
    assert!(is_exempt("Ok", PinExemption::VerbFirst));
    assert!(is_exempt("Cancel", PinExemption::VerbFirst));
}

#[test]
fn test_ok_and_cancel_are_not_exempt_from_the_ellipsis_rule() {
    // The §4.5 block exempts them from the verb-first CTA rule only.
    assert!(!is_exempt("Ok", PinExemption::Ellipsis));
    assert!(!is_exempt("Cancel", PinExemption::Ellipsis));
    assert!(!is_exempt("Ok", PinExemption::SentenceCase));
    assert!(!is_exempt("Cancel", PinExemption::SentenceCase));
}

#[test]
fn test_chrome_rows_are_exempt_from_sentence_case_and_ellipsis() {
    for literal in [
        "Open in Editor",
        "Show in File Explorer",
        "Create Directory ...",
        "Close Others",
        "Close All",
        "Copy Path",
        "Windows...",
    ] {
        assert!(
            is_exempt(literal, PinExemption::SentenceCase),
            "'{literal}' must be exempt from sentence case"
        );
        assert!(
            is_exempt(literal, PinExemption::Ellipsis),
            "'{literal}' must be exempt from the ellipsis rules"
        );
    }
}

#[test]
fn test_every_pin_carries_a_citation() {
    for pin in HOST_PIN_TABLE {
        assert!(
            pin.pinned_by.contains("UI-"),
            "'{}' carries no host requirement citation: {:?}",
            pin.literal,
            pin.pinned_by
        );
        assert!(
            !pin.exempts.is_empty(),
            "'{}' is pinned but exempts nothing, which makes the pin meaningless",
            pin.literal
        );
    }
}

#[test]
fn test_specific_citations_are_recorded_where_the_spec_names_an_id() {
    assert_eq!(pin_for("Ok").map(|p| p.pinned_by), Some(PIN_INPUT_DIALOG));
    assert_eq!(
        pin_for("Create Directory ...").map(|p| p.pinned_by),
        Some(PIN_CREATE_DIRECTORY)
    );
    assert_eq!(
        pin_for("Windows...").map(|p| p.pinned_by),
        Some(PIN_WINDOWS)
    );
    assert_eq!(
        pin_for("Close All").map(|p| p.pinned_by),
        Some(PIN_CONTEXT_MENU)
    );
}

#[test]
fn test_the_broad_citation_is_the_specs_own_range() {
    assert_eq!(HOST_PIN_REQUIREMENTS, "UI-009..UI-034, UI-062");
}

#[test]
fn test_an_arbitrary_label_is_exempt_from_nothing() {
    for exemption in [
        PinExemption::SentenceCase,
        PinExemption::VerbFirst,
        PinExemption::Ellipsis,
    ] {
        assert!(!is_exempt("Save changes", exemption));
        assert!(!is_exempt("", exemption));
    }
}
