//! Unit tests for the iced ToC helpers in `right.rs` ([TASK-029],
//! [RUST-003]).
//!
//! Ported from the legacy UI `ui/panels/right_tests.rs` cases that exercised
//! the two helpers copied into [`super`], so coverage survives [TASK-029b]'s
//! deletion of the legacy UI panel. The legacy UI sidecar's snapshot / pixel-layout
//! tests are NOT ported — they drive `Ui` and have no iced-side
//! counterpart here.

use super::{resolve_anchor, should_show_panel};
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
