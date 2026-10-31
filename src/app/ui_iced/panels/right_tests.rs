//! Unit tests for the iced ToC helpers in `right.rs` ([RUST-003]).
//!
//! The pure helpers are exercised table-first: every `resolve_anchor` rule,
//! every indent level and every `auto_fit_width` clamp is a row of a table
//! rather than its own process, and each row names itself in its failure
//! message so a regression still identifies the case that broke.

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

/// Every branch of the fuzzy rule: exact id, case-insensitivity, hyphen/space
/// equivalence in both directions, title matching, the heading-id fallback and
/// its precedence, and the verbatim fallback.
#[test]
fn test_resolve_anchor_applies_the_fuzzy_rule_in_both_directions() {
    let spaced_id = vec![ToCEntry::new("Getting Started", 1, "Getting Started")];
    let hyphenated_id = vec![ToCEntry::new("Getting-Started", 1, "Getting-Started")];
    // ToC ids may be disambiguated (`Intro#1`) while the title stays plain.
    let disambiguated = vec![ToCEntry::new("Intro", 1, "Intro#1")];
    let heading_ids = vec!["Body".to_string(), "Body#1".to_string()];
    // The ToC is searched first, so a ToC hit wins even when the rendered
    // heading ids also contain a match.
    let both_lists = vec!["Alpha".to_string()];
    let toc_over_headings = vec![ToCEntry::new("Alpha", 1, "Alpha#0")];

    let cases: Vec<(&str, &str, &str)> = vec![
        ("exact id match", "Getting Started", "Getting Started"),
        (
            "case-insensitive match",
            "getting started",
            "Getting Started",
        ),
        (
            "a hyphenated anchor matches a spaced id",
            "getting-started",
            "Getting Started",
        ),
    ];
    for (label, anchor, expected) in cases {
        assert_eq!(
            resolve_anchor(&spaced_id, &[], anchor),
            expected,
            "{label}: the anchor must resolve to the entry's own id"
        );
    }

    // And the reverse: a spaced anchor resolves against a hyphenated id.
    assert_eq!(
        resolve_anchor(&hyphenated_id, &[], "getting started"),
        "Getting-Started",
        "a spaced anchor must match a hyphenated id"
    );

    assert_eq!(
        resolve_anchor(&disambiguated, &[], "Intro"),
        "Intro#1",
        "title match: a title match must return the entry's disambiguated id"
    );

    assert_eq!(
        resolve_anchor(&[], &heading_ids, "body"),
        "Body",
        "heading-id fallback: an anchor with no ToC entry resolves against the rendered ids"
    );
    assert_eq!(
        resolve_anchor(&[], &heading_ids, "body#1"),
        "Body#1",
        "heading-id fallback: the duplicate-disambiguation suffix matches case-insensitively"
    );

    assert_eq!(
        resolve_anchor(&toc_over_headings, &both_lists, "alpha"),
        "Alpha#0",
        "precedence: ToC entries take precedence over the rendered heading-id list"
    );

    assert_eq!(
        resolve_anchor(&disambiguated, &[], "nowhere"),
        "nowhere",
        "unmatched: an unmatched anchor is returned unchanged so the scroll request still names a heading id"
    );
    assert_eq!(
        resolve_anchor(&[], &[], "nowhere"),
        "nowhere",
        "unmatched: an unmatched anchor against empty lists is returned unchanged"
    );

    assert_eq!(
        resolve_anchor(&[], &[], ""),
        "",
        "empty anchor: the empty anchor has nothing to match and must round-trip"
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
//
// The level rung is deliberately **not** the tree's: the ToC pane is the
// narrowest in the layout (`RIGHT_PANE_MIN_W` = 120px). At the tree's 16px
// step, six heading levels would spend 80px of that on indentation alone;
// at `space.md` they spend 60px. That narrower rung is the reason this
// constant is `md` and not `tree_row::INDENT_STEP`.
// ---------------------------------------------------------------------------

#[test]
fn the_toc_indent_constants_are_bound_to_the_spacing_scale() {
    assert_eq!(
        TOC_BASE_INDENT_PIXELS,
        hoarfrost::token::space::XS,
        "base indent: the ToC row's base pad reads off the spacing scale"
    );
    assert_eq!(
        TOC_LEVEL_INDENT_PIXELS,
        hoarfrost::token::space::MD,
        "level indent: one heading level of ToC indent reads off the spacing scale"
    );
    assert_ne!(
        TOC_LEVEL_INDENT_PIXELS,
        hoarfrost::composite::tree_row::INDENT_STEP,
        "level indent: the ToC indents one rung tighter than the tree on purpose: the pane \
         is half as wide"
    );

    let six_levels = toc_indent_pixels(6);
    let tree_at_six = hoarfrost::composite::tree_row::INDENT_STEP * 5.0;
    assert!(
        six_levels < tree_at_six,
        "pane budget: six ToC levels must cost less indent than six tree levels"
    );
    assert!(
        six_levels < 120.0,
        "pane budget: the deepest ordinary ToC level must stay inside the pane's minimum width"
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
fn test_toc_indent_pixels_follows_the_level_rule() {
    for level in 0u32..=6 {
        let expected = if level == 0 {
            TOC_BASE_INDENT_PIXELS
        } else {
            TOC_BASE_INDENT_PIXELS + (level as f32 - 1.0) * TOC_LEVEL_INDENT_PIXELS
        };
        assert_eq!(
            toc_indent_pixels(level),
            expected,
            "level {level}: must sit one step further in than the level above it"
        );
    }

    assert_eq!(
        toc_indent_pixels(1),
        TOC_BASE_INDENT_PIXELS,
        "level 1: a top-level heading gets the base pad and no level indent"
    );
    assert!(
        toc_indent_pixels(0) > 0.0,
        "level 0: a malformed level 0 saturates to the base pad and never goes negative"
    );

    // Strictly increasing across the whole heading-level range.
    for level in 2u32..=6 {
        assert!(
            toc_indent_pixels(level) > toc_indent_pixels(level - 1),
            "deeper levels must indent further, level {level} did not"
        );
    }

    // Above the heading range the rule is unclamped, so the only contract
    // left is that the arithmetic stays sane.
    for level in [7u32, 99, 1_000, u32::MAX] {
        let pad = toc_indent_pixels(level);
        assert!(
            pad.is_finite() && pad > 0.0,
            "level {level}: must produce a finite, positive pad, got {pad}"
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
fn toc_button_width_adds_both_pads_and_floors_at_the_icon_only_minimum() {
    assert_eq!(
        toc_button_width(50.0),
        50.0 + 2.0 * TOC_BUTTON_H_PAD_PIXELS,
        "padded label: a button pads its label left AND right"
    );
    // `# A` — a one-character heading measures a handful of pixels, but the
    // framework floors the box at `size.control.sm`.
    assert_eq!(
        toc_button_width(0.0),
        hoarfrost::widget::button::ICON_ONLY_MIN_WIDTH,
        "empty label: an empty label must still cost the icon-only floor the real button applies"
    );
    assert_eq!(
        toc_button_width(-100.0),
        hoarfrost::widget::button::ICON_ONLY_MIN_WIDTH,
        "negative measurement: a negative measurement is clamped before it can shrink the row"
    );
}

#[test]
fn entry_width_charges_the_furniture_and_grows_with_the_level() {
    let entry = ToCEntry::new("Intro", 1, "Intro");
    assert_eq!(
        entry_width(&entry, six_px_per_char),
        furniture(1) + 5.0 * 6.0,
        "formula: the row needs its indent, the padded label, its right pad and the gutter"
    );

    let shallow = ToCEntry::new("Intro", 1, "Intro");
    let deep = ToCEntry::new("Intro", 4, "Intro");
    assert_eq!(
        entry_width(&deep, six_px_per_char) - entry_width(&shallow, six_px_per_char),
        3.0 * TOC_LEVEL_INDENT_PIXELS,
        "level growth: three levels deeper cost three indent steps and nothing else"
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

/// The UI-016 clamps: the minimum floor, the content width between the
/// clamps, the 30% cap, and the degenerate branch where the minimum wins over
/// a cap that fell below it.
#[test]
fn auto_fit_width_solves_the_content_between_the_ui_016_clamps() {
    // 30 chars -> 180px label -> 222px row, inside [150, 300] at this viewport.
    let mid_label = "x".repeat(30);
    let wanted = entry_width(
        &ToCEntry::new(mid_label.as_str(), 1, "mid"),
        six_px_per_char,
    );
    // A 90-character heading needs far more than 30% of a 1000px viewport.
    let long_label = "x".repeat(90);
    let long_entry = ToCEntry::new(long_label.as_str(), 1, "long");

    // The fixtures have to land where the cases claim they do, or the case
    // below proves nothing.
    assert!(
        wanted > RIGHT_PANEL_MIN_WIDTH && wanted < 300.0,
        "fixture: the mid-length heading must fall between the clamps, needs {wanted}px"
    );
    assert!(
        entry_width(&long_entry, six_px_per_char) > 300.0,
        "fixture: the long heading must exceed the cap this case pins"
    );

    let cases: Vec<(&str, Vec<ToCEntry>, f32, f32)> = vec![
        (
            "short ToC on a roomy viewport",
            vec![ToCEntry::new("Intro", 1, "Intro")],
            1000.0,
            RIGHT_PANEL_MIN_WIDTH,
        ),
        (
            "content-sized ToC between the clamps",
            vec![ToCEntry::new(mid_label.as_str(), 1, "mid")],
            1000.0,
            wanted,
        ),
        (
            "a heading longer than the cap",
            vec![ToCEntry::new(long_label.as_str(), 1, "long")],
            1000.0,
            1000.0 * RIGHT_PANEL_MAX_WIDTH_FRACTION,
        ),
        (
            // 30% of 400px is 120px, BELOW the 150px minimum — the same
            // degenerate branch `clamped_width` implements for the left pane
            // and the drag.
            "minimum wins on a tiny viewport",
            vec![ToCEntry::new("Intro", 1, "Intro")],
            400.0,
            RIGHT_PANEL_MIN_WIDTH,
        ),
        ("an empty ToC", vec![], 1000.0, RIGHT_PANEL_MIN_WIDTH),
    ];

    for (label, entries, viewport, expected) in &cases {
        assert_eq!(
            auto_fit_width(entries, *viewport, six_px_per_char),
            *expected,
            "{label}: the solved width must honour the UI-016 floor and cap"
        );
    }

    assert!(
        auto_fit_width(&[], 0.0, six_px_per_char).is_finite(),
        "a zero viewport must not produce a non-finite width"
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
