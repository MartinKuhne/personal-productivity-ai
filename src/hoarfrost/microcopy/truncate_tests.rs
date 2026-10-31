//! Tests for [`super::truncate`] — end / middle truncation and the
//! tooltip pairing (`UIF-PRIN-007`).

use super::*;

/// A path long enough that every budget in the sweep actually truncates.
const PATH: &str = "src/app/ui_iced/app.rs";
/// Prose with no segment separator at all.
const PROSE: &str = "A very long document title that will not fit";
/// A multi-byte string, to prove the walk is over `char`s.
const CJK: &str = "日本語/テキスト.txt";

// ---------------------------------------------------------------------------
// End truncation
// ---------------------------------------------------------------------------

#[test]
fn test_short_strings_pass_through_untouched() {
    let result = truncate_end("Save", 10);
    assert!(!result.was_truncated);
    assert_eq!(result.text, "Save");
    assert!(result.head_kept);
    assert!(result.tail_preserved);
}

#[test]
fn test_exact_budget_passes_through_untouched() {
    let result = truncate_end("Save", 4);
    assert!(!result.was_truncated, "fitting exactly is not truncation");
    assert_eq!(result.text, "Save");
}

#[test]
fn test_end_truncation_keeps_the_head_and_marks_the_cut() {
    let result = truncate_end("Save changes", 8);
    assert!(result.was_truncated);
    assert_eq!(result.text, "Save ch…");
    assert!(result.head_kept);
    assert!(!result.tail_preserved);
}

#[test]
fn test_end_truncation_emits_u2026_not_three_dots() {
    let result = truncate_end("Save changes", 8);
    assert!(result.text.ends_with('\u{2026}'));
    assert!(
        !result.text.ends_with("..."),
        "truncation must never emit the action suffix: {}",
        result.text
    );
}

#[test]
fn test_end_truncation_with_budget_one_is_just_the_mark() {
    let result = truncate_end("Save changes", 1);
    assert_eq!(result.text, "\u{2026}");
    assert!(!result.head_kept);
}

#[test]
fn test_end_truncation_with_zero_budget_produces_nothing() {
    let result = truncate_end("Save changes", 0);
    assert!(result.was_truncated);
    assert_eq!(result.text, "");
    assert!(!result.head_kept);
    assert!(!result.tail_preserved);
}

#[test]
fn test_end_truncation_of_an_empty_string_is_not_truncation() {
    let result = truncate_end("", 0);
    assert!(!result.was_truncated);
    assert_eq!(result.text, "");
}

// ---------------------------------------------------------------------------
// Middle truncation
// ---------------------------------------------------------------------------

#[test]
fn test_middle_truncation_produces_the_specs_own_shape() {
    // §1.1 writes the required shape as `src/app/…/mod.rs`: the mark
    // sits between separators, the basename survives.
    let result = truncate_middle("src/app/ui_iced/mod.rs", 16);
    assert_eq!(result.text, "src/app/\u{2026}/mod.rs");
    assert!(result.head_kept);
    assert!(result.tail_preserved);
}

#[test]
fn test_middle_truncation_preserves_the_extension() {
    let result = truncate_middle(PATH, 12);
    assert!(
        result.text.ends_with(".rs"),
        "the extension must survive: {}",
        result.text
    );
    assert!(result.tail_preserved);
}

#[test]
fn test_middle_truncation_keeps_root_context_and_basename() {
    let result = truncate_middle(PATH, 14);
    assert!(result.head_kept, "root context lost: {}", result.text);
    assert!(result.tail_preserved, "basename lost: {}", result.text);
    assert!(result.text.starts_with("src/"));
    assert!(result.text.ends_with("app.rs"));
}

#[test]
fn test_middle_truncation_emits_u2026_not_three_dots() {
    let result = truncate_middle(PATH, 12);
    assert!(result.text.contains('\u{2026}'));
    assert!(
        !result.text.contains("..."),
        "truncation must never emit the action suffix: {}",
        result.text
    );
}

#[test]
fn test_a_path_with_no_separator_splits_evenly() {
    // No structure to preserve, so the budget is shared rather than
    // silently degrading to end truncation.
    let result = truncate_middle(PROSE, 20);
    assert!(result.head_kept);
    assert!(!result.tail_preserved);
    assert!(result.text.contains('\u{2026}'));
    let head_len = result.text.chars().take_while(|c| *c != '\u{2026}').count();
    let tail_len = result.text.chars().count() - head_len - 1;
    assert!(head_len >= tail_len, "head should get the extra char");
    assert!(head_len > 0 && tail_len > 0, "both halves must be present");
}

#[test]
fn test_leading_separator_paths_are_handled() {
    let result = truncate_middle("/etc/passwd", 7);
    assert_eq!(result.text.chars().count(), 7);
    assert!(result.text.ends_with("passwd"));
}

#[test]
fn test_backslash_paths_truncate_like_posix_paths() {
    let result = truncate_middle("C:\\project\\src\\main.rs", 14);
    assert_eq!(result.text.chars().count(), 14);
    assert!(
        result.text.ends_with("main.rs"),
        "basename must survive: {}",
        result.text
    );
    assert!(result.tail_preserved);
}

#[test]
fn test_uri_middle_truncation_keeps_scheme_and_last_segment() {
    let result = truncate_middle("https://example.com/a/b/c/report.pdf", 20);
    assert_eq!(result.text.chars().count(), 20);
    assert!(result.text.starts_with("https://"));
    assert!(result.text.ends_with("/report.pdf"));
}

#[test]
fn test_git_ref_middle_truncation() {
    let result = truncate_middle("feature/deep/nested/branch:src/lib.rs", 18);
    assert_eq!(result.text.chars().count(), 18);
    assert!(result.text.ends_with("lib.rs"), "{}", result.text);
}

#[test]
fn test_a_bare_git_ref_has_no_structure_and_degrades_honestly() {
    // "HEAD" has no separator, so nothing can promise a preserved root.
    let result = truncate_middle("HEAD", 3);
    assert_eq!(result.text.chars().count(), 3);
    assert!(!result.tail_preserved);
}

// ---------------------------------------------------------------------------
// Degenerate budgets
// ---------------------------------------------------------------------------

#[test]
fn test_middle_truncation_with_one_char_budget_is_just_the_mark() {
    let result = truncate_middle(PATH, 1);
    assert_eq!(result.text, "\u{2026}");
    assert!(!result.head_kept);
    assert!(!result.tail_preserved);
}

#[test]
fn test_middle_truncation_with_zero_budget_produces_nothing() {
    let result = truncate_middle(PATH, 0);
    assert!(result.was_truncated);
    assert_eq!(result.text, "");
}

#[test]
fn test_middle_truncation_of_an_empty_string_is_not_truncation() {
    let result = truncate_middle("", 0);
    assert!(!result.was_truncated);
    assert_eq!(result.text, "");
}

#[test]
fn test_a_budget_too_small_for_both_ends_reports_the_loss() {
    // The budget cannot hold the root and the basename. The function must
    // not panic and must say which half it dropped.
    let result = truncate_middle(PATH, 5);
    assert_eq!(result.text.chars().count(), 5);
    assert!(
        !(result.head_kept && result.tail_preserved),
        "a 5-char budget cannot preserve both halves of a 22-char path"
    );
}

#[test]
fn test_neither_mode_ever_exceeds_the_budget() {
    let inputs = [
        "",
        "a",
        "ab",
        PATH,
        PROSE,
        CJK,
        "/a",
        "a/",
        "//",
        "https://example.com/a/b/c/report.pdf",
        "feature/deep/nested/branch:src/lib.rs",
    ];
    for input in inputs {
        for budget in 0..=24 {
            for mode in [TruncationMode::End, TruncationMode::Middle] {
                let result = truncate(input, budget, mode);
                assert!(
                    result.text.chars().count() <= budget,
                    "{mode:?} of {input:?} at budget {budget} produced {:?} ({} chars)",
                    result.text,
                    result.text.chars().count(),
                );
            }
        }
    }
}

#[test]
fn test_multi_byte_input_is_never_split_mid_sequence() {
    for budget in 0..=CJK.chars().count() {
        for mode in [TruncationMode::End, TruncationMode::Middle] {
            let result = truncate(CJK, budget, mode);
            // A split sequence would not round-trip through UTF-8.
            assert_eq!(
                result.text,
                String::from_utf8(result.text.as_bytes().to_vec()).unwrap()
            );
        }
    }
}

#[test]
fn test_truncate_dispatches_on_mode() {
    assert_eq!(
        truncate(PATH, 8, TruncationMode::End).mode,
        TruncationMode::End
    );
    assert_eq!(
        truncate(PATH, 8, TruncationMode::Middle).mode,
        TruncationMode::Middle
    );
}

// ---------------------------------------------------------------------------
// Truncation => tooltip pairing
// ---------------------------------------------------------------------------

#[test]
fn test_a_label_that_fits_needs_no_tooltip() {
    let slot = LabelSlot {
        id: "tab",
        text: "notes.md",
        tooltip: None,
    };
    assert!(check_truncation_tooltip(&slot, TruncationMode::End, 32).is_empty());
}

#[test]
fn test_a_truncated_label_without_a_tooltip_is_a_violation() {
    let slot = LabelSlot {
        id: "tab",
        text: "a-very-long-document-title.md",
        tooltip: None,
    };
    let violations = check_truncation_tooltip(&slot, TruncationMode::End, 12);
    assert_eq!(
        violations,
        vec![TruncationTooltipViolation::MissingTooltip {
            id: "tab",
            full: "a-very-long-document-title.md",
        }]
    );
}

#[test]
fn test_a_truncated_label_with_a_shortened_tooltip_is_a_violation() {
    // The tooltip must expose the FULL string. A tooltip that is itself
    // truncated does not satisfy the rule.
    let slot = LabelSlot {
        id: "cell",
        text: "quarterly-report-final.md",
        tooltip: Some("quarterly…"),
    };
    let violations = check_truncation_tooltip(&slot, TruncationMode::End, 10);
    assert_eq!(
        violations,
        vec![TruncationTooltipViolation::TooltipNotFullText {
            id: "cell",
            full: "quarterly-report-final.md",
            tooltip: "quarterly…",
        }]
    );
}

#[test]
fn test_a_truncated_label_with_the_full_string_as_tooltip_satisfies_the_rule() {
    let full = "quarterly-report-final.md";
    let slot = LabelSlot {
        id: "cell",
        text: full,
        tooltip: Some(full),
    };
    assert!(check_truncation_tooltip(&slot, TruncationMode::End, 10).is_empty());
}

#[test]
fn test_the_pairing_rule_applies_to_middle_truncation_too() {
    let slot = LabelSlot {
        id: "path",
        text: PATH,
        tooltip: None,
    };
    let violations = check_truncation_tooltip(&slot, TruncationMode::Middle, 12);
    assert_eq!(violations.len(), 1);
    assert!(matches!(
        violations[0],
        TruncationTooltipViolation::MissingTooltip {
            id: "path",
            full: PATH
        }
    ));
}

#[test]
fn test_violations_render_readably() {
    let slot = LabelSlot {
        id: "tab",
        text: "a-very-long-document-title.md",
        tooltip: Some("short"),
    };
    let violations = check_truncation_tooltip(&slot, TruncationMode::End, 8);
    let text = violations[0].to_string();
    assert!(text.contains("tab"), "{text}");
    assert!(text.contains("a-very-long-document-title.md"), "{text}");
}

#[test]
fn test_truncation_mode_slugs_are_unique() {
    assert_ne!(TruncationMode::End.slug(), TruncationMode::Middle.slug());
}
