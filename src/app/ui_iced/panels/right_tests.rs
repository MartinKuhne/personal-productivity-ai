//! Unit tests for the iced ToC helpers in `right.rs` ([RUST-003]).

use super::{
    TOC_BASE_INDENT_PIXELS, TOC_LEVEL_INDENT_PIXELS, resolve_anchor, should_show_panel,
    toc_indent_pixels,
};
use crate::markdown::ToCEntry;

// ---------------------------------------------------------------------------
// `should_show_panel` — the right-pane visibility gate (UI-009)
// ---------------------------------------------------------------------------

#[test]
fn test_should_show_panel_requires_both_toc_and_selected_file() {
    assert!(should_show_panel(true, true));
    assert!(!should_show_panel(false, true));
    assert!(!should_show_panel(true, false));
    assert!(!should_show_panel(false, false));
}

// ---------------------------------------------------------------------------
// `resolve_anchor` — the fuzzy anchor → heading-id rule
// ---------------------------------------------------------------------------

#[test]
fn test_resolve_anchor_exact_id_match() {
    let toc = vec![ToCEntry::new("Getting Started", 1, "Getting Started")];
    assert_eq!(
        resolve_anchor(&toc, &[], "Getting Started"),
        "Getting Started",
        "an exact anchor must resolve to the entry's own id"
    );
}

#[test]
fn test_resolve_anchor_is_case_insensitive() {
    let toc = vec![ToCEntry::new("Getting Started", 1, "Getting Started")];
    assert_eq!(
        resolve_anchor(&toc, &[], "getting started"),
        "Getting Started",
        "matching ignores case"
    );
}

#[test]
fn test_resolve_anchor_treats_hyphen_and_space_as_equivalent() {
    let toc = vec![ToCEntry::new("Getting Started", 1, "Getting Started")];
    // GitHub-style `#getting-started` anchor resolves to the spaced id.
    assert_eq!(
        resolve_anchor(&toc, &[], "getting-started"),
        "Getting Started",
        "a hyphenated anchor must match a spaced id"
    );
    // And the reverse: a spaced anchor resolves against a hyphenated id.
    let hyphenated = vec![ToCEntry::new("Getting-Started", 1, "Getting-Started")];
    assert_eq!(
        resolve_anchor(&hyphenated, &[], "getting started"),
        "Getting-Started",
        "a spaced anchor must match a hyphenated id"
    );
}

#[test]
fn test_resolve_anchor_matches_title_when_id_differs() {
    // ToC ids may be disambiguated (`Intro#1`) while the title stays plain.
    let toc = vec![ToCEntry::new("Intro", 1, "Intro#1")];
    assert_eq!(
        resolve_anchor(&toc, &[], "Intro"),
        "Intro#1",
        "a title match must return the entry's disambiguated id"
    );
}

#[test]
fn test_resolve_anchor_falls_back_to_heading_ids() {
    let heading_ids = vec!["Body".to_string(), "Body#1".to_string()];
    assert_eq!(resolve_anchor(&[], &heading_ids, "body"), "Body");
    // The duplicate-disambiguation suffix matches case-insensitively.
    assert_eq!(resolve_anchor(&[], &heading_ids, "body#1"), "Body#1");
}

#[test]
fn test_resolve_anchor_prefers_toc_over_heading_ids() {
    // The ToC is searched first, so a ToC hit wins even when the rendered
    // heading ids also contain a match.
    let toc = vec![ToCEntry::new("Alpha", 1, "Alpha#0")];
    let heading_ids = vec!["Alpha".to_string()];
    assert_eq!(
        resolve_anchor(&toc, &heading_ids, "alpha"),
        "Alpha#0",
        "ToC entries take precedence over the rendered heading-id list"
    );
}

#[test]
fn test_resolve_anchor_unmatched_returns_anchor_verbatim() {
    let toc = vec![ToCEntry::new("Intro", 1, "Intro#1")];
    assert_eq!(
        resolve_anchor(&toc, &[], "nowhere"),
        "nowhere",
        "an unmatched anchor is returned unchanged so the scroll request still names a heading id"
    );
    assert_eq!(resolve_anchor(&[], &[], "nowhere"), "nowhere");
}

#[test]
fn test_resolve_anchor_on_empty_inputs_is_empty_anchor() {
    assert_eq!(
        resolve_anchor(&[], &[], ""),
        "",
        "the empty anchor has nothing to match and must round-trip"
    );
}

// ---------------------------------------------------------------------------
// `toc_indent_pixels` — the per-level ToC row indent (UI-001)
//
// The rule is `TOC_BASE_INDENT_PIXELS + (level - 1) * TOC_LEVEL_INDENT_PIXELS`
// (base 4 px, 12 px per level beyond the first), saturated at the base for
// `level == 0` and unclamped above it.
// ---------------------------------------------------------------------------

#[test]
fn test_toc_indent_pixels_level_one_is_the_base_pad() {
    assert_eq!(
        toc_indent_pixels(1),
        TOC_BASE_INDENT_PIXELS,
        "a top-level heading gets the base pad and no level indent"
    );
}

#[test]
fn test_toc_indent_pixels_grows_by_one_step_per_level() {
    for level in 1u32..=6 {
        let expected = TOC_BASE_INDENT_PIXELS + (level as f32 - 1.0) * TOC_LEVEL_INDENT_PIXELS;
        assert_eq!(
            toc_indent_pixels(level),
            expected,
            "level {level} must sit one step further in than the level above it"
        );
    }
    // Strictly increasing across the whole heading-level range.
    for level in 2u32..=6 {
        assert!(
            toc_indent_pixels(level) > toc_indent_pixels(level - 1),
            "deeper levels must indent further, level {level} did not"
        );
    }
}

#[test]
fn test_toc_indent_pixels_clamps_level_zero_to_the_base() {
    assert_eq!(
        toc_indent_pixels(0),
        TOC_BASE_INDENT_PIXELS,
        "a malformed level 0 saturates to the base pad and never goes negative"
    );
    assert!(toc_indent_pixels(0) > 0.0);
}

#[test]
fn test_toc_indent_pixels_stays_finite_for_absurd_levels() {
    for level in [7u32, 99, 1_000, u32::MAX] {
        let pad = toc_indent_pixels(level);
        assert!(
            pad.is_finite() && pad > 0.0,
            "level {level} must produce a finite, positive pad, got {pad}"
        );
    }
}
