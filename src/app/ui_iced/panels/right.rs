//! Iced right table-of-contents (ToC) helpers ([TASK-029], UI-001).
//!
//! The iced build has no `ui_iced/panels/right.rs` **view** yet — the ToC
//! pane body is rendered inline by [`view::toc_body`](crate::ui_iced::view),
//! so this module owns the *pure* rules that pane is built from: the
//! visibility gate, the anchor → heading-id resolver and the per-level row
//! indent. They are copied here (not shared) so `ui_iced` no longer depends
//! on the the legacy UI `ui/panels/right.rs` file that [TASK-029b] deletes
//! ([TASK-029] self-containment).
//!
//! [`toc_indent_pixels`] was lifted out of the inline arithmetic in
//! `view::toc_body` by [TASK-027]: the deleted legacy UI sidecar asserted
//! that rule (`calculate_indent`) and no iced test covered it, so the rule
//! is now a function with a sidecar assertion instead of a magic expression.
//!
//! All three helpers are **framework-free by construction**: their signatures mention
//! only `bool`, `u32`, `f32`, [`crate::markdown::ToCEntry`] and `String`, so the
//! copy is byte-identical to the legacy UI original apart from this header. The
//! legacy UI panel keeps its own copy until Phase 5 removes it; the transient
//! duplication is intentional and dies with the legacy UI tree.
//!
//! [`should_show_panel`] is re-exported from [`crate::ui_iced::layout`] to
//! preserve the public path [TASK-010] established, so
//! [`view`](crate::ui_iced::view) and the pane-grid gate keep resolving
//! through `layout::should_show_panel` unchanged.
//!
//! Unit tests live in the sibling `right_tests.rs` sidecar.

#[cfg(test)]
#[path = "right_tests.rs"]
mod tests;

/// Determines if the right panel should be shown based on application state.
///
/// Copied from the legacy UI ToC panel for the framework-free migration
/// ([TASK-029]); the rule is unchanged.
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
/// Copied from the legacy UI ToC panel for the framework-free migration ([TASK-029]);
/// the matching rule is unchanged.
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
/// logical pixels ([TASK-027], UI-001).
pub const TOC_BASE_INDENT_PIXELS: f32 = 4.0;

/// Extra left padding one heading level beyond the first costs, in logical
/// pixels ([TASK-027], UI-001).
pub const TOC_LEVEL_INDENT_PIXELS: f32 = 12.0;

/// Left padding of the ToC row for a heading of `level` (1-based).
///
/// The iced ToC pane renders one button per heading and uses the row's left
/// padding as its only nesting cue. Extracted from the inline arithmetic in
/// [`view::toc_body`](crate::ui_iced::view) so the rule is a pure, testable
/// function ([RUST-023]) — the deleted legacy UI sidecar pinned this rule as
/// `calculate_indent` and no iced test covered the iced formula ([TASK-027]).
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
