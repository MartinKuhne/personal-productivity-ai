//! Unit tests for the iced ToC helpers in `right.rs` ([RUST-003]).

use super::{
    RIGHT_PANEL_MAX_WIDTH_FRACTION, RIGHT_PANEL_MIN_WIDTH, TOC_BASE_INDENT_PIXELS,
    TOC_BUTTON_H_PAD_PIXELS, TOC_LEVEL_INDENT_PIXELS, TOC_ROW_RIGHT_PAD_PIXELS,
    TOC_ROW_VERTICAL_PAD_PIXELS, TOC_SCROLLBAR_GUTTER_PIXELS, auto_fit_width, entry_width,
    longest_entry_width, resolve_anchor, should_show_panel, toc_button_width, toc_indent_pixels,
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
// INT-7b: the two ToC indent constants are bound to spacing tokens
//
// Both bindings are **exact** — `4.0` *is* `space.xs` and `12.0` *is*
// `space.md` — so unlike the other INT-7b pins there is no retired
// literal to `assert_ne!` against: the number did not move, the *source*
// did. What these pins hold is the binding itself, so a retune of the
// spacing rung moves the ToC with every other `xs`/`md` inset instead of
// leaving this pane behind, and a retune of the ToC alone cannot silently
// take it off the grid.
// ---------------------------------------------------------------------------

#[test]
fn the_toc_base_indent_is_bound_to_space_xs() {
    assert_eq!(
        TOC_BASE_INDENT_PIXELS,
        hoarfrost::token::space::XS,
        "the ToC row's base pad reads off the spacing scale"
    );
}

#[test]
fn the_toc_level_indent_is_bound_to_space_md() {
    assert_eq!(
        TOC_LEVEL_INDENT_PIXELS,
        hoarfrost::token::space::MD,
        "one heading level of ToC indent reads off the spacing scale"
    );
}

#[test]
fn the_toc_level_indent_is_deliberately_not_the_tree_indent_step() {
    // The ToC pane is the narrowest in the layout (RIGHT_PANE_MIN_W =
    // 120px). At the tree's 16px step, six heading levels would spend
    // 80px of that on indentation alone; at `space.md` they spend 60px.
    // The narrower rung is the reason this constant is `md` and not
    // `tree_row::INDENT_STEP`.
    assert_ne!(
        TOC_LEVEL_INDENT_PIXELS,
        hoarfrost::composite::tree_row::INDENT_STEP,
        "the ToC indents one rung tighter than the tree on purpose: the pane \
         is half as wide"
    );
    let six_levels = toc_indent_pixels(6);
    let tree_at_six = hoarfrost::composite::tree_row::INDENT_STEP * 5.0;
    assert!(
        six_levels < tree_at_six,
        "six ToC levels must cost less indent than six tree levels"
    );
    assert!(
        six_levels < 120.0,
        "the deepest ordinary ToC level must stay inside the pane's minimum width"
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

// ---------------------------------------------------------------------------
// Defect #14 — the content-sized width
//
// "The table of contents takes up more horizontal space than it needs to
// display its contents." The pane used to be a bare `ratio: 0.5`; these tests
// pin the solver that replaces it. A deterministic `chars * 6px` measurer is
// used so the arithmetic is exact, plus one case against the real
// `fira_sans_width` measurer the reducer actually injects.
// ---------------------------------------------------------------------------

/// A deterministic measurer: 6 logical pixels per character.
///
/// Exactness matters here — the point of these cases is the **formula**, so
/// the measurement must not be the thing under test.
fn six_px_per_char(text: &str) -> f32 {
    text.chars().count() as f32 * 6.0
}

/// The furniture every row costs, minus the label: indent + button padding +
/// right pad + reserved scrollbar gutter.
fn furniture(level: u32) -> f32 {
    toc_indent_pixels(level)
        + 2.0 * TOC_BUTTON_H_PAD_PIXELS
        + TOC_ROW_RIGHT_PAD_PIXELS
        + TOC_SCROLLBAR_GUTTER_PIXELS
}

#[test]
fn the_row_furniture_constants_are_bound_to_framework_tokens() {
    assert_eq!(TOC_ROW_RIGHT_PAD_PIXELS, hoarfrost::token::space::XS);
    assert_eq!(TOC_ROW_VERTICAL_PAD_PIXELS, hoarfrost::token::space::XXS);
    assert_eq!(
        TOC_BUTTON_H_PAD_PIXELS,
        hoarfrost::widget::button::HORIZONTAL_PADDING,
        "the solver must charge the button padding the row actually carries"
    );
    assert_eq!(
        TOC_SCROLLBAR_GUTTER_PIXELS,
        hoarfrost::composite::scroll_view::TRACK_WIDTH_RESERVED,
        "the reserved ScrollView gutter is the framework's, not a number this pane invented"
    );
    assert_eq!(
        TOC_SCROLLBAR_GUTTER_PIXELS,
        hoarfrost::token::size::SPLITTER_GRAB
    );
}

#[test]
fn toc_button_width_adds_the_padding_on_both_sides() {
    assert_eq!(
        toc_button_width(50.0),
        50.0 + 2.0 * TOC_BUTTON_H_PAD_PIXELS,
        "a button pads its label left AND right"
    );
}

#[test]
fn toc_button_width_floors_at_the_icon_only_minimum() {
    // `# A` — a one-character heading measures a handful of pixels, but the
    // framework floors the box at `size.control.sm`.
    assert_eq!(
        toc_button_width(0.0),
        hoarfrost::widget::button::ICON_ONLY_MIN_WIDTH,
        "an empty label must still cost the icon-only floor the real button applies"
    );
    assert_eq!(
        toc_button_width(-100.0),
        hoarfrost::widget::button::ICON_ONLY_MIN_WIDTH,
        "a negative measurement is clamped before it can shrink the row"
    );
}

#[test]
fn entry_width_is_indent_plus_button_plus_right_pad_plus_scrollbar_gutter() {
    let entry = ToCEntry::new("Intro", 1, "Intro");
    assert_eq!(
        entry_width(&entry, six_px_per_char),
        furniture(1) + 5.0 * 6.0,
        "the row needs its indent, the padded label, its right pad and the gutter"
    );
}

#[test]
fn entry_width_grows_with_the_heading_level() {
    let shallow = ToCEntry::new("Intro", 1, "Intro");
    let deep = ToCEntry::new("Intro", 4, "Intro");
    assert_eq!(
        entry_width(&deep, six_px_per_char) - entry_width(&shallow, six_px_per_char),
        3.0 * TOC_LEVEL_INDENT_PIXELS,
        "three levels deeper cost three indent steps and nothing else"
    );
}

#[test]
fn longest_entry_width_is_the_widest_row_and_zero_when_empty() {
    assert_eq!(longest_entry_width([], six_px_per_char), 0.0);
    let entries = [
        ToCEntry::new("Intro", 1, "Intro"),
        ToCEntry::new("A much longer heading than the first", 1, "long"),
        ToCEntry::new("Mid", 3, "Mid"),
    ];
    assert_eq!(
        longest_entry_width(&entries, six_px_per_char),
        entry_width(&entries[1], six_px_per_char),
        "the solve is a max over entries, not a sum and not the first row"
    );
}

#[test]
fn auto_fit_width_lands_a_short_toc_on_the_panel_minimum() {
    let entries = [ToCEntry::new("Intro", 1, "Intro")];
    assert_eq!(
        auto_fit_width(&entries, 1000.0, six_px_per_char),
        RIGHT_PANEL_MIN_WIDTH,
        "a ToC that needs {}px still gets the UI-016 floor",
        entry_width(&entries[0], six_px_per_char),
    );
}

#[test]
fn auto_fit_width_grows_to_the_content_between_the_clamps() {
    // 30 chars -> 180px label -> 222px row, inside [150, 300] at this viewport.
    let entries = [ToCEntry::new("x".repeat(30).as_str(), 1, "mid")];
    let wanted = entry_width(&entries[0], six_px_per_char);
    assert!(wanted > RIGHT_PANEL_MIN_WIDTH && wanted < 300.0);
    assert_eq!(auto_fit_width(&entries, 1000.0, six_px_per_char), wanted);
}

#[test]
fn auto_fit_width_respects_the_thirty_percent_cap() {
    // A 90-character heading needs far more than 30% of a 1000px viewport.
    let entries = [ToCEntry::new("x".repeat(90).as_str(), 1, "long")];
    assert!(entry_width(&entries[0], six_px_per_char) > 300.0);
    assert_eq!(
        auto_fit_width(&entries, 1000.0, six_px_per_char),
        1000.0 * RIGHT_PANEL_MAX_WIDTH_FRACTION,
        "the UI-016 cap is not renegotiated by a long heading"
    );
}

#[test]
fn auto_fit_width_lets_the_minimum_win_on_a_tiny_viewport() {
    // 30% of 400px is 120px, BELOW the 150px minimum — the same degenerate
    // branch `clamped_width` implements for the left pane and the drag.
    let entries = [ToCEntry::new("Intro", 1, "Intro")];
    assert_eq!(
        auto_fit_width(&entries, 400.0, six_px_per_char),
        RIGHT_PANEL_MIN_WIDTH
    );
}

#[test]
fn auto_fit_width_measures_real_glyphs_not_a_character_count() {
    use crate::ui_iced::table_width::fira_sans_width;
    let measure = |text: &str| fira_sans_width(text, iced::Font::DEFAULT, 13.0);

    // Long enough that neither string lands on the 150px floor nor on the
    // 600px cap of this viewport, so the difference in glyphs is what shows.
    let narrow = [ToCEntry::new("i".repeat(48).as_str(), 1, "narrow")];
    let wide = [ToCEntry::new("M".repeat(48).as_str(), 1, "wide")];
    let narrow_w = auto_fit_width(&narrow, 2000.0, measure);
    let wide_w = auto_fit_width(&wide, 2000.0, measure);

    assert!(
        narrow_w > RIGHT_PANEL_MIN_WIDTH && wide_w < 2000.0 * RIGHT_PANEL_MAX_WIDTH_FRACTION,
        "both cases must sit between the clamps to prove anything \
         (narrow {narrow_w}, wide {wide_w})"
    );
    assert!(
        narrow_w < wide_w,
        "48 narrow glyphs must cost less pane than 48 wide ones; a flat \
         chars-per-em model cannot tell them apart ({narrow_w} vs {wide_w})"
    );
}

#[test]
fn auto_fit_width_is_total_on_an_empty_toc_and_a_zero_viewport() {
    assert_eq!(
        auto_fit_width(&[], 1000.0, six_px_per_char),
        RIGHT_PANEL_MIN_WIDTH
    );
    assert!(auto_fit_width(&[], 0.0, six_px_per_char).is_finite());
}
