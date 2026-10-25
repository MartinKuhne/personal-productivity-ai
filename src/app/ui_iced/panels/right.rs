//! Right table-of-contents (ToC) helpers (UI-001).
//!
//! There is no `ui_iced/panels/right.rs` **view** — the ToC pane body is
//! rendered inline by [`view::toc_body`](crate::ui_iced::view) — so this
//! module owns the *pure* rules that pane is built from: the visibility
//! gate, the anchor → heading-id resolver and the per-level row indent.
//!
//! [`toc_indent_pixels`] lifts the indent arithmetic out of
//! `view::toc_body` so the rule is a named, testable function rather than
//! an inline expression.
//!
//! All three helpers are **framework-free by construction**: their
//! signatures mention only `bool`, `u32`, `f32`,
//! [`crate::markdown::ToCEntry`] and `String`. The two indent *constants*
//! are not framework-free — they are bound to `iced_pro` spacing tokens,
//! which is the point: the ToC's nesting pitch is now read off the
//! spacing scale instead of being a pair of numbers the pane invented.
//!
//! [`should_show_panel`] is re-exported from [`crate::ui_iced::layout`] so
//! [`view`](crate::ui_iced::view) and the pane-grid gate resolve through
//! `layout::should_show_panel`.
//!
//! Unit tests live in the sibling `right_tests.rs` sidecar.

#[cfg(test)]
#[path = "right_tests.rs"]
mod tests;

use iced_pro::token::space;

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
/// deliberately *not* [`tree_row::INDENT_STEP`](iced_pro::composite::tree_row::INDENT_STEP)
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
