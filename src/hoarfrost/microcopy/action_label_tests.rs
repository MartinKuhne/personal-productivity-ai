//! Tests for [`super::action_label`] — verb-first CTAs and the ellipsis
//! `iff` (`UIF-PRIN-007`).

use super::*;

fn prompt(label: &str) -> ActionLabel<'_> {
    ActionLabel {
        label,
        behaviour: PromptBehaviour::LaunchesPrompt,
    }
}

fn immediate(label: &str) -> ActionLabel<'_> {
    ActionLabel {
        label,
        behaviour: PromptBehaviour::Immediate,
    }
}

// ---------------------------------------------------------------------------
// Suffix detection
// ---------------------------------------------------------------------------

#[test]
fn test_ascii_suffix_detection() {
    assert!(ends_with_ascii_ellipsis("Rename..."));
    assert!(ends_with_ascii_ellipsis("Create Directory ..."));
    assert!(!ends_with_ascii_ellipsis("Rename"));
    assert!(!ends_with_ascii_ellipsis("Rename…"));
    assert!(!ends_with_ascii_ellipsis("Rename.."));
}

#[test]
fn test_unicode_suffix_detection() {
    assert!(ends_with_unicode_ellipsis("Title\u{2026}"));
    assert!(!ends_with_unicode_ellipsis("Title..."));
    assert!(!ends_with_unicode_ellipsis("Title"));
}

#[test]
fn test_spacing_detection() {
    assert!(!is_spaced_ellipsis("Rename..."));
    assert!(is_spaced_ellipsis("Create Directory ..."));
    assert!(is_spaced_ellipsis("Rename ..."));
    assert!(!is_spaced_ellipsis("Rename"), "no suffix, no spacing");
    assert!(!is_spaced_ellipsis("..."), "nothing before the suffix");
}

#[test]
fn test_the_two_ellipses_are_different_characters() {
    assert_ne!(ASCII_ACTION_ELLIPSIS.chars().last(), Some(UNICODE_ELLIPSIS));
    assert_eq!(UNICODE_ELLIPSIS as u32, 0x2026);
}

// ---------------------------------------------------------------------------
// The iff, all four quadrants
// ---------------------------------------------------------------------------

#[test]
fn test_quadrant_one_prompt_with_suffix_is_compliant() {
    let verdict = check_action_ellipsis(&prompt("Rename..."));
    assert!(verdict.is_compliant(), "{:?}", verdict.violation);
    assert!(verdict.violation.is_none());
}

#[test]
fn test_quadrant_two_prompt_without_suffix_is_a_violation() {
    let verdict = check_action_ellipsis(&prompt("Rename"));
    assert_eq!(
        verdict.violation,
        Some(EllipsisViolation::MissingActionEllipsis { label: "Rename" })
    );
    assert!(!verdict.is_compliant());
}

#[test]
fn test_quadrant_three_immediate_with_suffix_is_a_violation() {
    let verdict = check_action_ellipsis(&immediate("Save..."));
    assert_eq!(
        verdict.violation,
        Some(EllipsisViolation::SpuriousActionEllipsis { label: "Save..." })
    );
    assert!(!verdict.is_compliant());
}

#[test]
fn test_quadrant_four_immediate_without_suffix_is_compliant() {
    for label in ["Save", "Format Markdown", "System logs", "Close"] {
        let verdict = check_action_ellipsis(&immediate(label));
        assert!(
            verdict.is_compliant() && verdict.violation.is_none(),
            "'{label}' is immediate and carries no suffix; got {:?}",
            verdict.violation
        );
    }
}

#[test]
fn test_the_iff_holds_in_both_directions_over_a_label_set() {
    // For every label tested, `has suffix` and `launches a prompt` must
    // agree — that is what makes this a biconditional rather than a
    // one-way requirement.
    let cases = [
        ("Rename...", PromptBehaviour::LaunchesPrompt, true),
        ("Rename", PromptBehaviour::LaunchesPrompt, false),
        ("Save...", PromptBehaviour::Immediate, false),
        ("Save", PromptBehaviour::Immediate, true),
    ];
    for (label, behaviour, expected_compliant) in cases {
        let verdict = check_action_ellipsis(&ActionLabel { label, behaviour });
        assert_eq!(
            verdict.is_compliant(),
            expected_compliant,
            "iff violated for '{label}' / {behaviour:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Spacing
// ---------------------------------------------------------------------------

#[test]
fn test_a_spaced_suffix_is_a_violation() {
    let verdict = check_action_ellipsis(&prompt("Rename ..."));
    assert_eq!(
        verdict.violation,
        Some(EllipsisViolation::SpacedActionEllipsis {
            label: "Rename ..."
        })
    );
    assert!(!verdict.is_compliant());
}

#[test]
fn test_the_pinned_spaced_suffix_is_exempt() {
    // 'Create Directory ...' is the one literal §1.1 exempts per UI-015.
    let verdict = check_action_ellipsis(&prompt("Create Directory ..."));
    assert!(
        matches!(
            verdict.violation,
            Some(EllipsisViolation::SpacedActionEllipsis { .. })
        ),
        "the spaced form must still be detected"
    );
    assert!(verdict.exempt, "and the pin must excuse it");
    assert!(verdict.is_compliant());
}

// ---------------------------------------------------------------------------
// The Unicode reservation
// ---------------------------------------------------------------------------

#[test]
fn test_unicode_ellipsis_as_action_suffix_is_always_a_violation() {
    for behaviour in [PromptBehaviour::Immediate, PromptBehaviour::LaunchesPrompt] {
        let verdict = check_action_ellipsis(&ActionLabel {
            label: "Rename\u{2026}",
            behaviour,
        });
        assert_eq!(
            verdict.violation,
            Some(EllipsisViolation::UnicodeEllipsisAsActionSuffix {
                label: "Rename\u{2026}"
            }),
            "U+2026 must never stand in for the action suffix ({behaviour:?})"
        );
    }
}

#[test]
fn test_the_unicode_check_precedes_the_missing_suffix_check() {
    // A prompt label ending in U+2026 is missing the ASCII suffix too,
    // but reporting that would send the reader to add the wrong thing.
    let verdict = check_action_ellipsis(&prompt("Move\u{2026}"));
    assert!(matches!(
        verdict.violation,
        Some(EllipsisViolation::UnicodeEllipsisAsActionSuffix { .. })
    ));
}

// ---------------------------------------------------------------------------
// Verb-first
// ---------------------------------------------------------------------------

#[test]
fn test_generic_ctas_are_flagged() {
    for label in ["ok", "OK", "Ok", "yes", "YES", "no", "NO", "Okay"] {
        assert!(is_generic_cta(label), "'{label}' is a generic CTA");
    }
}

#[test]
fn test_specific_imperatives_are_not_generic() {
    for label in [
        "Save changes",
        "Rename file",
        "Delete 3 items",
        "Run batch",
        "Apply",
        "Cancel",
    ] {
        assert!(
            !is_generic_cta(label),
            "'{label}' carries a specific imperative verb"
        );
    }
}

#[test]
fn test_generic_cta_trims_surrounding_whitespace() {
    assert!(is_generic_cta("  OK  "));
}

#[test]
fn test_pinned_ok_is_exempt_from_verb_first() {
    let verdict = check_verb_first("Ok");
    assert!(verdict.generic, "'Ok' is generic");
    assert!(verdict.exempt, "and pinned by UI-009");
    assert!(verdict.is_compliant());
}

#[test]
fn test_unpinned_ok_fails_verb_first() {
    let verdict = check_verb_first("OK");
    assert!(verdict.generic);
    assert!(
        !verdict.exempt,
        "'OK' is not one of the nine pinned literals"
    );
    assert!(!verdict.is_compliant());
}

#[test]
fn test_cancel_is_compliant_on_its_own_merits() {
    // The spec exempts 'Cancel' from the verb-first rule, but "cancel"
    // is not in the generic set §1.1 names, so nothing had to excuse it.
    let verdict = check_verb_first("Cancel");
    assert!(!verdict.generic);
    assert!(!verdict.exempt);
    assert!(verdict.is_compliant());
}

#[test]
fn test_yes_and_no_fail_verb_first() {
    for label in ["Yes", "No", "YES", "No "] {
        let verdict = check_verb_first(label);
        assert!(!verdict.is_compliant(), "'{label}' must not pass");
    }
}

// ---------------------------------------------------------------------------
// Combined
// ---------------------------------------------------------------------------

#[test]
fn test_check_action_returns_both_verdicts() {
    let (ellipsis, verb_first) = check_action(&prompt("Rename..."));
    assert!(ellipsis.is_compliant());
    assert!(verb_first.is_compliant());

    let (ellipsis, verb_first) = check_action(&immediate("OK"));
    assert!(ellipsis.is_compliant(), "'OK' has no suffix problem");
    assert!(!verb_first.is_compliant(), "'OK' fails verb-first");
}

#[test]
fn test_behaviour_predicate() {
    assert!(PromptBehaviour::LaunchesPrompt.requires_action_ellipsis());
    assert!(!PromptBehaviour::Immediate.requires_action_ellipsis());
}

#[test]
fn test_violations_render_readably() {
    let cases = [
        EllipsisViolation::MissingActionEllipsis { label: "Rename" },
        EllipsisViolation::SpuriousActionEllipsis { label: "Save..." },
        EllipsisViolation::SpacedActionEllipsis {
            label: "Rename ...",
        },
        EllipsisViolation::UnicodeEllipsisAsActionSuffix {
            label: "Rename\u{2026}",
        },
    ];
    for case in cases {
        let text = case.to_string();
        assert!(text.len() > 20, "message too thin: {text}");
    }
}

#[test]
fn test_behaviour_slugs_are_unique() {
    assert_ne!(
        PromptBehaviour::Immediate.slug(),
        PromptBehaviour::LaunchesPrompt.slug()
    );
}
