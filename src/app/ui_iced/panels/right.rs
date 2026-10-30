//! Right table-of-contents (ToC) helpers (UI-001).
//!
//! There is no `ui_iced/panels/right.rs` **view** — the ToC pane body is
//! rendered inline by [`view::toc_body`](crate::ui_iced::view) — so this
//! module owns the *pure* rules that pane is built from: the visibility
//! gate, the anchor → heading-id resolver, the per-level row indent and the
//! content-width auto-fit.
//!
//! [`toc_indent_pixels`] lifts the indent arithmetic out of
//! `view::toc_body` so the rule is a named, testable function rather than
//! an inline expression.
//!
//! # The content-sized width (defect #14)
//!
//! The right split used to be a bare `ratio: 0.5`, so the pane opened at
//! roughly 37.5% of the viewport whether its longest entry was "Intro" or a
//! 90-character heading. [`auto_fit_width`] is the ToC's counterpart of the
//! left pane's [`auto_fit_width`](crate::ui_iced::panels::left::auto_fit_width):
//! it measures every entry the way the row is actually built — level indent,
//! the hoarfrost button's own padding, the row container's right pad and the
//! permanently reserved scrollbar gutter — and clamps the widest of them
//! through [`clamped_width`], the same UI-016 rule the pane resize and the
//! tree auto-fit share.
//!
//! The measurement is **injected**, exactly as the tree's is: iced only hands
//! a text renderer to `Widget::layout`, so neither `view` nor the reducer can
//! measure from inside the widget tree. The reducer passes
//! [`fira_sans_width`](crate::ui_iced::table_width::fira_sans_width), the
//! production measurer that shapes against the same Fira Sans bytes iced
//! renders with.
//!
//! The *furniture* terms are not invented here either: they are the same
//! hoarfrost constants the row composite is built from
//! ([`button::HORIZONTAL_PADDING`], [`button::ICON_ONLY_MIN_WIDTH`],
//! [`space::XS`] and [`scroll_view::TRACK_WIDTH_RESERVED`]), so the solver
//! and [`view::toc_body`](crate::ui_iced::view) cannot disagree about what a
//! row costs ([RUST-021]).
//!
//! The two indent *constants* are not framework-free — they are bound to
//! `hoarfrost` spacing tokens, which is the point: the ToC's nesting pitch is
//! now read off the spacing scale instead of being a pair of numbers the pane
//! invented.
//!
//! [`should_show_panel`] is re-exported from [`crate::ui_iced::layout`] so
//! [`view`](crate::ui_iced::view) and the pane-grid gate resolve through
//! `layout::should_show_panel`.
//!
//! Unit tests live in the sibling `right_tests.rs` sidecar.

#[cfg(test)]
#[path = "right_tests.rs"]
mod tests;

use crate::markdown::ToCEntry;
use crate::ui::panel_layout::{RIGHT_PANEL_MAX_WIDTH_FRACTION, RIGHT_PANEL_MIN_WIDTH};
use crate::ui_iced::layout::clamped_width;
use hoarfrost::composite::scroll_view;
use hoarfrost::token::space;
use hoarfrost::widget::button;

/// Determines if the right panel should be shown based on application state.
///
/// Precondition: None.
/// Postcondition: Returns true if there is a non-empty TOC and a selected file.
/// Purity: Pure function.
#[must_use]
pub fn should_show_panel(has_toc: bool, has_selected_file: bool) -> bool {
    has_toc && has_selected_file
}

/// Resolves a markdown link anchor (e.g. `#Getting-Started`) to the concrete
/// heading id used by the renderer.
///
/// Applies the same fuzzy matching in both directions — case-insensitive, and
/// treating `-` and space as interchangeable — first against the ToC entries
/// (by id or title) and then against the rendered heading ids. Falls back to
/// `anchor` verbatim when nothing matches, so the scroll request still names a
/// heading id.
///
/// Extracted from `FastMdApp::handle_open_link` so the matching rule is a pure,
/// testable function ([RUST-023]).
#[must_use]
pub fn resolve_anchor(
    toc: &[crate::markdown::ToCEntry],
    heading_ids: &[String],
    anchor: &str,
) -> String {
    let anchor_lower = anchor.to_lowercase();
    let hyphenated = anchor_lower.replace(' ', "-");
    let spaced = anchor_lower.replace('-', " ");

    let matches = |candidate: &str| {
        let candidate_lower = candidate.to_lowercase();
        candidate.eq_ignore_ascii_case(anchor)
            || candidate_lower.replace(' ', "-") == hyphenated
            || spaced == candidate_lower
    };

    toc.iter()
        .find(|entry| matches(&entry.id) || matches(&entry.title))
        .map(|entry| entry.id.clone())
        .or_else(|| heading_ids.iter().find(|id| matches(id)).cloned())
        .unwrap_or_else(|| anchor.to_owned())
}

/// Left padding every ToC row gets before its level indent is applied, in
/// logical pixels (UI-001) — `space.xs` (`UIF-TOK-001`).
///
/// **Movement: none.** The retired literal `4.0` *is* `space.xs`, so the
/// constant is re-expressed against the token rather than retuned. The
/// value is now load-bearing in the sense that matters: a retune of the
/// spacing rung moves the ToC's base pad with every other `xs` inset in
/// the application instead of leaving this one behind.
pub const TOC_BASE_INDENT_PIXELS: f32 = space::XS;

/// Extra left padding one heading level beyond the first costs, in logical
/// pixels (UI-001) — `space.md` (`UIF-TOK-001`).
///
/// **Movement: none.** `12.0` is exactly `space.md`. Note this is
/// deliberately *not* [`tree_row::INDENT_STEP`](hoarfrost::composite::tree_row::INDENT_STEP)
/// (`space.lg`, 16): the ToC pane is the narrowest in the layout
/// (`RIGHT_PANE_MIN_W = 120px`) and six heading levels at 16px would
/// spend 80px of it on indentation alone. `space.md` is the nearest rung
/// that keeps level 6 inside the pane's minimum width, and it is the
/// value the retired literal already used.
pub const TOC_LEVEL_INDENT_PIXELS: f32 = space::MD;

/// Left padding of the ToC row for a heading of `level` (1-based).
///
/// The ToC pane renders one button per heading and uses the row's left
/// padding as its only nesting cue. Extracted from the inline arithmetic in
/// [`view::toc_body`](crate::ui_iced::view) so the rule is a pure, testable
/// function ([RUST-023]).
///
/// Levels below 1 saturate to the base indent, so a malformed level can
/// never produce a negative pad.
///
/// Precondition: None.
/// Postcondition: `TOC_BASE_INDENT_PIXELS + (level - 1) * TOC_LEVEL_INDENT_PIXELS`,
/// floored at the base for `level == 0`.
/// Purity: Pure function.
#[must_use]
pub fn toc_indent_pixels(level: u32) -> f32 {
    TOC_BASE_INDENT_PIXELS + level.saturating_sub(1) as f32 * TOC_LEVEL_INDENT_PIXELS
}

/// Right padding every ToC row gets after its label, in logical pixels
/// (UI-001, defect #14) — `space.xs` (`UIF-TOK-001`).
///
/// **Movement: none.** The retired literal `4.0` *is* `space.xs`. It is named
/// here rather than left inline in
/// [`view::toc_body`](crate::ui_iced::view) because the width solver
/// below has to charge the same padding to the row it is measuring: a row
/// that pads its right edge by 4px needs 4px more pane than its label alone
/// would ([RUST-021]).
pub const TOC_ROW_RIGHT_PAD_PIXELS: f32 = space::XS;

/// Vertical padding above and below every ToC row, in logical pixels
/// (UI-001) — `space.2xs` (`UIF-TOK-001`).
///
/// **Movement: none.** The retired literal `2.0` *is* `space.2xs`. Named for
/// the same reason as [`TOC_ROW_RIGHT_PAD_PIXELS`]: the row's furniture has
/// one home, even though only the vertical axis reads this one.
pub const TOC_ROW_VERTICAL_PAD_PIXELS: f32 = space::XXS;

/// Horizontal padding the entry's
/// [`Button`](hoarfrost::widget::button_widget::Button) costs on **each** side, in
/// logical pixels — `button.horizontal_padding` (`UIF-TOK-002`,
/// `hoarfrost::widget::button::HORIZONTAL_PADDING`).
///
/// The ToC row's label is not painted bare: `view::toc_entry_button` wraps it
/// in a hoarfrost ghost button, and `Button::new`
/// (`button_widget.rs:535`) gives every button
/// [`button::HORIZONTAL_PADDING`] on both sides. A solver that measured only
/// the label would under-size every row by twice this much and truncate the
/// heading it was supposed to be sizing for.
pub const TOC_BUTTON_H_PAD_PIXELS: f32 = button::HORIZONTAL_PADDING;

/// Scrollbar gutter the ToC pane must reserve, in logical pixels —
/// `size.splitter.grab` via [`scroll_view::TRACK_WIDTH_RESERVED`]
/// (`UIF-COMP-004`).
///
/// The pane is a `scrollable`
/// ([`TOC_SCROLLABLE_ID`](crate::ui_iced::view::TOC_SCROLLABLE_ID)), and §3B.4
/// requires the gutter to be **permanently reserved** at the widest thumb
/// phase so a hover that expands the thumb causes `0px` content reflow. A
/// width solve that ignores it produces a heading that fits at rest and is
/// clipped the moment the pointer enters the pane.
pub const TOC_SCROLLBAR_GUTTER_PIXELS: f32 = scroll_view::TRACK_WIDTH_RESERVED;

/// Width the entry's button needs around a label of `label_width`.
///
/// This is `Button::outer_size`'s width clause
/// (`hoarfrost/widget/button_widget.rs:808-811`) restated for a solver that
/// has no renderer: label + [`TOC_BUTTON_H_PAD_PIXELS`] on each side, floored
/// at the icon-only minimum `button::ICON_ONLY_MIN_WIDTH` that the same
/// function applies. A one-character heading (`# A`) therefore still costs the
/// 28px floor instead of collapsing to the glyph.
///
/// Purity: pure.
#[must_use]
pub fn toc_button_width(label_width: f32) -> f32 {
    (label_width.max(0.0) + 2.0 * TOC_BUTTON_H_PAD_PIXELS).max(button::ICON_ONLY_MIN_WIDTH)
}

/// Width a single ToC row needs: `level indent + button + right pad +
/// reserved scrollbar gutter` (defect #14).
///
/// `measure` is the injected label measurer — see the module header for why
/// measurement cannot come from inside the view. It is called with the entry's
/// title at [`TOC_TEXT_RUNG`](crate::ui_iced::view::TOC_TEXT_RUNG)'s point
/// size by the reducer, which is the rung `view::toc_entry_label` paints at.
///
/// Purity: pure for a pure `measure`.
#[must_use]
pub fn entry_width(entry: &ToCEntry, measure: impl Fn(&str) -> f32) -> f32 {
    toc_indent_pixels(entry.level)
        + toc_button_width(measure(&entry.title))
        + TOC_ROW_RIGHT_PAD_PIXELS
        + TOC_SCROLLBAR_GUTTER_PIXELS
}

/// Width of the widest row in `entries`; `0.0` for an empty ToC.
///
/// Purity: pure for a pure `measure`.
#[must_use]
pub fn longest_entry_width<'a>(
    entries: impl IntoIterator<Item = &'a ToCEntry>,
    measure: impl Fn(&str) -> f32,
) -> f32 {
    entries
        .into_iter()
        .map(|entry| entry_width(entry, &measure))
        .fold(0.0_f32, f32::max)
}

/// UI-016 auto-fit for the right pane: the width that fits the longest ToC
/// entry, clamped to
/// `[RIGHT_PANEL_MIN_WIDTH, viewport_width × RIGHT_PANEL_MAX_WIDTH_FRACTION]`
/// (defect #14).
///
/// Shares [`clamped_width`] with the pane-grid resize reduction and the tree
/// auto-fit, so the degenerate tiny-viewport case behaves identically in all
/// three paths: when the 30% cap falls *below* the minimum, the minimum wins.
///
/// **The 30% cap can win.** A heading long enough to need more than
/// `viewport × 0.30` is truncated rather than given more room, because the
/// clamp is the UI-016 contract this solve is not allowed to renegotiate. The
/// pane then scrolls horizontally-free and clips; that is the clamp doing
/// what it says, not a sizing bug.
///
/// Purity: pure for a pure `measure`.
#[must_use]
pub fn auto_fit_width(
    entries: &[ToCEntry],
    viewport_width: f32,
    measure: impl Fn(&str) -> f32,
) -> f32 {
    clamped_width(
        longest_entry_width(entries, &measure),
        RIGHT_PANEL_MIN_WIDTH,
        viewport_width * RIGHT_PANEL_MAX_WIDTH_FRACTION,
    )
}
