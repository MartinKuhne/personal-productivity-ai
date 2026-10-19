//! Iced right table-of-contents (ToC) helpers ([TASK-029], UI-009).
//!
//! The iced build has no `ui_iced/panels/right.rs` **view** yet — the ToC
//! pane body is rendered inline by [`view::toc_body`](crate::ui_iced::view),
//! so this module owns only the two *pure* rules the legacy UI ToC panel owned.
//! They are copied here (not shared) so `ui_iced` no longer depends on the
//! the legacy UI `ui/panels/right.rs` file that [TASK-029b] deletes
//! ([TASK-029] self-containment).
//!
//! Both helpers are **framework-free by construction**: their signatures mention
//! only `bool`, [`crate::markdown::ToCEntry`] and `String`, so the copy is
//! byte-identical to the legacy UI original apart from this header. The legacy UI
//! panel keeps its own copy until Phase 5 removes it; the transient
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
