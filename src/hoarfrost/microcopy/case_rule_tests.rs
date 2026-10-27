//! Tests for [`super::case_rule`] — the sentence-case mandate
//! (`UIF-PRIN-007`).

use super::*;
use crate::microcopy::pins::HOST_PIN_TABLE;

// ---------------------------------------------------------------------------
// Word classification
// ---------------------------------------------------------------------------

#[test]
fn test_word_case_classifies_each_shape() {
    assert_eq!(WordCase::of("save"), WordCase::Lower);
    assert_eq!(WordCase::of("Save"), WordCase::Title);
    assert_eq!(WordCase::of("SAVE"), WordCase::Upper);
    assert_eq!(WordCase::of("PDF"), WordCase::Upper);
    assert_eq!(WordCase::of("FastMD"), WordCase::Mixed);
    assert_eq!(WordCase::of("OAuth"), WordCase::Mixed);
    assert_eq!(WordCase::of("myLabel"), WordCase::Mixed);
}

#[test]
fn test_word_case_ignores_surrounding_punctuation() {
    // The `...` of `Rename...` must not change the word's shape.
    assert_eq!(WordCase::of("Rename..."), WordCase::Title);
    assert_eq!(WordCase::of("save?"), WordCase::Lower);
    assert_eq!(WordCase::of("SAVE!"), WordCase::Upper);
}

#[test]
fn test_digit_and_symbol_tokens_are_uncased() {
    assert_eq!(WordCase::of("3"), WordCase::Uncased);
    assert_eq!(WordCase::of("..."), WordCase::Uncased);
    assert_eq!(WordCase::of("\u{2014}"), WordCase::Uncased);
}

#[test]
fn test_single_letter_words_classify() {
    assert_eq!(WordCase::of("A"), WordCase::Title);
    assert_eq!(WordCase::of("a"), WordCase::Lower);
}

#[test]
fn test_a_single_capital_letter_is_not_all_caps() {
    // The article "A" and the pronoun "I" are ordinary sentence-case
    // words. Treating a one-letter uppercase run as ALL-CAPS would make
    // the rule fire on correct copy.
    assert_eq!(classify("A"), Capitalisation::Sentence);
    assert_eq!(classify("I"), Capitalisation::Sentence);
    assert_eq!(classify("Add to I list"), Capitalisation::Sentence);
    assert!(check_label("A new note").is_compliant());
}

// ---------------------------------------------------------------------------
// Compliant labels
// ---------------------------------------------------------------------------

#[test]
fn test_sentence_case_labels_pass() {
    for label in [
        "Save changes",
        "Rename file",
        "Delete 3 items",
        "Run batch",
        "Apply",
        "System logs",
        "Format Markdown",
        "Run as prompt",
        "New document...",
        "Rename...",
    ] {
        let report = check_label(label);
        assert!(
            report.is_compliant() && !report.exempt,
            "'{label}' should be compliant on its own merits, got {report}"
        );
        assert_eq!(report.capitalisation, Capitalisation::Sentence);
    }
}

#[test]
fn test_licensed_acronyms_do_not_break_sentence_case() {
    for label in [
        "Export PDF",
        "Open with MCP",
        "Parse JSON",
        "Convert to Markdown",
        "Show UI",
        "Use the API",
    ] {
        let report = check_label(label);
        assert!(
            report.is_compliant() && !report.exempt,
            "'{label}' contains only licensed acronyms and must pass, got {report}"
        );
    }
}

#[test]
fn test_a_label_of_only_licensed_acronyms_is_compliant() {
    assert_eq!(classify("PDF"), Capitalisation::Sentence);
    assert_eq!(classify("PDF MCP JSON"), Capitalisation::Sentence);
}

#[test]
fn test_a_label_starting_with_a_digit_is_not_first_word_lowercase() {
    // Capitalising this to "3 Items selected" would be worse copy than
    // the rule is trying to prevent.
    assert_eq!(classify("3 items selected"), Capitalisation::Sentence);
    assert_eq!(classify("42 files found"), Capitalisation::Sentence);
}

#[test]
fn test_labels_with_no_cased_characters_are_uncasable() {
    assert_eq!(classify(""), Capitalisation::Uncasable);
    assert_eq!(classify("..."), Capitalisation::Uncasable);
    assert_eq!(classify("1234"), Capitalisation::Uncasable);
    assert!(classify("\u{2014}").is_compliant());
}

// ---------------------------------------------------------------------------
// Violations
// ---------------------------------------------------------------------------

#[test]
fn test_all_caps_is_detected() {
    for label in ["SAVE CHANGES", "DELETE", "OK", "RUN BATCH"] {
        assert_eq!(
            classify(label),
            Capitalisation::AllCaps,
            "'{label}' must classify as all-caps"
        );
    }
}

#[test]
fn test_all_caps_with_one_unlicensed_word_still_fails() {
    // Every word is uppercase and at least one is not a licensed
    // acronym, so the licence does not rescue the label.
    assert_eq!(classify("PDF EXPORT"), Capitalisation::AllCaps);
}

#[test]
fn test_title_case_is_detected() {
    for label in [
        "Save Changes",
        "Export The Report",
        "Close Window",
        "Copy Paths",
    ] {
        assert_eq!(
            classify(label),
            Capitalisation::TitleCase,
            "'{label}' must classify as title case"
        );
    }
}

#[test]
fn test_first_word_lowercase_is_detected() {
    assert_eq!(classify("save changes"), Capitalisation::FirstWordLowercase);
    assert_eq!(classify("rename"), Capitalisation::FirstWordLowercase);
    // Checked before title case, so the more actionable finding wins.
    assert_eq!(classify("save Changes"), Capitalisation::FirstWordLowercase);
}

#[test]
fn test_mid_word_capital_is_detected() {
    assert_eq!(classify("saveChanges"), Capitalisation::MidWordCapital);
    assert_eq!(classify("myLabel here"), Capitalisation::MidWordCapital);
}

#[test]
fn test_a_capital_at_a_word_head_is_not_a_mid_word_capital() {
    // "my Label" has no capital *inside* a word — the capital is
    // word-initial, so the finding is the lowercase opening, not a
    // mid-word capital.
    assert_eq!(classify("my Label"), Capitalisation::FirstWordLowercase);
}

#[test]
fn test_licensed_mixed_case_words_do_not_trigger_mid_word_capital() {
    assert_eq!(classify("Open FastMD"), Capitalisation::Sentence);
    assert_eq!(classify("Sign in with OAuth"), Capitalisation::Sentence);
}

// ---------------------------------------------------------------------------
// The pin table's effect
// ---------------------------------------------------------------------------

#[test]
fn test_every_pinned_literal_is_exempt() {
    for pin in HOST_PIN_TABLE {
        let report = check_label(pin.literal);
        assert!(
            report.is_compliant(),
            "'{}' is pinned and must be compliant",
            pin.literal
        );
    }
}

#[test]
fn test_a_title_case_pinned_literal_is_reported_as_exempt_not_clean() {
    let report = check_label("Close Others");
    assert_eq!(report.capitalisation, Capitalisation::TitleCase);
    assert!(report.exempt, "the pin must be visible in the report");
    assert!(report.is_compliant());
    assert!(report.pinned_by.is_some());
}

#[test]
fn test_the_unpinned_near_miss_is_not_exempt() {
    // §4.9: `Copy path` on tree targets is UNPINNED and follows
    // UIF-PRIN-007. It is compliant because it is genuinely sentence
    // case — not because anything excused it.
    let report = check_label("Copy path");
    assert_eq!(report.capitalisation, Capitalisation::Sentence);
    assert!(!report.exempt, "'Copy path' must not inherit the pin");
    assert!(report.pinned_by.is_none());
    assert!(report.is_compliant());
}

#[test]
fn test_a_title_case_near_miss_of_a_pin_fails() {
    let report = check_label("Copy Paths");
    assert_eq!(report.capitalisation, Capitalisation::TitleCase);
    assert!(!report.exempt);
    assert!(!report.is_compliant());
}

#[test]
fn test_pinned_compliant_literal_is_not_marked_exempt() {
    // 'Ok' is sentence case on its own merits, so nothing had to excuse
    // it — the exemption machinery must not claim credit.
    let report = check_label("Ok");
    assert_eq!(report.capitalisation, Capitalisation::Sentence);
    assert!(!report.exempt);
    assert!(report.is_compliant());
}

// ---------------------------------------------------------------------------
// The licensed set is data
// ---------------------------------------------------------------------------

#[test]
fn test_the_four_spec_named_words_are_licensed_by_default() {
    for word in SPEC_NAMED_LICENSED {
        assert!(
            DEFAULT_LICENSED.contains(&word),
            "'{word}' is named by §1.1 but missing from DEFAULT_LICENSED"
        );
    }
}

#[test]
fn test_a_custom_licensed_set_changes_the_verdict() {
    assert_eq!(
        classify_with("Frobinate Widgets", &[]),
        Capitalisation::TitleCase
    );
    assert_eq!(
        classify_with("Frobinate Widgets", &["Widgets"]),
        Capitalisation::Sentence,
        "licensing a word must be enough to make the label pass"
    );
}

#[test]
fn test_check_label_with_uses_the_supplied_set() {
    let report = check_label_with("Frobinate Widgets", &["Widgets"]);
    assert!(report.is_compliant());
    let report = check_label_with("Frobinate Widgets", &[]);
    assert!(!report.is_compliant());
}

#[test]
fn test_licensing_is_case_sensitive() {
    assert_eq!(
        classify_with("Export widgets", &["Widgets"]),
        Capitalisation::Sentence
    );
    // "WIDGETS" is not the licensed spelling, so nothing rescues it.
    // It lands on Title Case rather than All Caps because "Export" is
    // Title, and the All Caps shape needs *every* word uppercased.
    assert_eq!(
        classify_with("Export WIDGETS", &["Widgets"]),
        Capitalisation::TitleCase,
        "'WIDGETS' is not the licensed spelling"
    );
    assert!(!check_label_with("Export WIDGETS", &["Widgets"]).is_compliant());
}

// ---------------------------------------------------------------------------
// Reporting
// ---------------------------------------------------------------------------

#[test]
fn test_reports_render_readably() {
    assert!(
        check_label("Save changes")
            .to_string()
            .contains("is sentence")
    );
    let violating = check_label("SAVE CHANGES").to_string();
    assert!(violating.contains("all-caps"), "{violating}");
    assert!(violating.contains("requires sentence case"), "{violating}");
    let exempt = check_label("Close All").to_string();
    assert!(exempt.contains("pinned by"), "{exempt}");
}

#[test]
fn test_capitalisation_slugs_are_unique() {
    let all = [
        Capitalisation::Uncasable,
        Capitalisation::Sentence,
        Capitalisation::AllCaps,
        Capitalisation::MidWordCapital,
        Capitalisation::FirstWordLowercase,
        Capitalisation::TitleCase,
    ];
    let slugs: Vec<&str> = all.iter().map(|c| c.slug()).collect();
    let unique: std::collections::BTreeSet<&str> = slugs.iter().copied().collect();
    assert_eq!(slugs.len(), unique.len(), "slugs collide: {slugs:?}");
}
