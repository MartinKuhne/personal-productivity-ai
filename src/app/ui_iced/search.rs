//! Iced content-search view model ([TASK-019], UI-050).
//!
//! The iced half of the workspace content search. This module owns the
//! **view-model** part of `src/app/ui/tree_search.rs` — the query text, the
//! committed filter, the result snapshot and the in-progress flag — as pure
//! state transitions an Elm `update` can replay. It deliberately does *not*
//! own the search *execution*: the filesystem scan stays in the existing
//! ripgrep-core engine, which is re-exported here unchanged instead of being
//! reimplemented ([RUST-051]).
//!
//! ## Reused, unchanged ([RUST-051])
//!
//! | Piece | Source |
//! | --- | --- |
//! | Result row type | [`SearchResultEntry`] (`src/app/ui/tree_search.rs:15-28`) — re-exported, never re-declared |
//! | Scoped engine | [`find_search_results_scoped`] (`src/app/ui/tree_search.rs:227-301`) — the same call the legacy UI panel made (`src/app/ui/panels/left.rs:252`) |
//! | Unscoped engine | [`find_search_results`] (`src/app/ui/tree_search.rs:308-314`) |
//! | Match-count label | [`strings::format_match_count`] (`src/app/ui/strings.rs:189-191`) |
//! | Header / empty state | [`strings::SEARCH_RESULTS_HEADER`], [`strings::SEARCH_NO_RESULTS`] |
//!
//! ## Why the scan is no longer synchronous
//!
//! The legacy UI build ran the scan inside the left panel's paint pass
//! (`src/app/ui/panels/left.rs:246-252` → `TreeSearch::apply_with_allowed_dirs`),
//! so a large workspace froze the UI for the whole duration — against
//! [FMD-060]/[FMD-061]. The iced port keeps the same engine but moves the
//! call off the UI thread:
//!
//! 1. [`SearchViewModel::submit`] commits the trimmed term, drops the stale
//!    result list and raises the in-progress flag, returning the term the
//!    reducer must scan for;
//! 2. the reducer runs the engine in a iced `Task` (`Task::perform` /
//!    `Task::run`, `iced_runtime-0.14.0/src/task.rs:48-69`) and reports
//!    back with [`SearchMessage::ResultsReady`](crate::ui_iced::message::SearchMessage::ResultsReady);
//! 3. [`SearchViewModel::apply_results`] snapshots whatever came back.
//!
//! Step 3 is **guarded against stale results**: a scan superseded by a newer
//! committed filter is dropped rather than painted, so a slow search for an
//! old term can never overwrite a fast one for a new term ([FMD-062]).
//!
//! ## Presentation logic ported as pure functions
//!
//! The legacy UI result card decided inline which lines to allocate
//! (`src/app/ui/panels/left.rs:299-323`). Those three predicates live here as
//! [`shows_match_count`], [`shows_relative_path`] and [`shows_snippet`], and
//! every user-facing string comes from [`crate::ui::strings`] ([GUD-002]) —
//! this module contains no literal UI text.
//!
//! Unit tests live in the sibling `search_tests.rs` sidecar.

use std::path::PathBuf;
use std::sync::Arc;

use crate::ui::strings;

/// The one result row type, reused unchanged ([RUST-051]) —
/// `src/app/ui/tree_search.rs:15`.
pub use crate::ui::tree_search::SearchResultEntry;

/// The search engines, reused unchanged ([RUST-051]) —
/// `src/app/ui/tree_search.rs:227` and `:308`.
///
/// The [TASK-022]/[TASK-023] reducer calls these from a `Task` and feeds
/// the returned vector back through
/// [`SearchViewModel::apply_results`]; nothing in the iced layer
/// re-implements scanning.
pub use crate::ui::tree_search::{find_search_results, find_search_results_scoped};

/// Workspace content-search view model ([TASK-019], UI-050).
///
/// Mirrors `TreeSearch` (`src/app/ui/tree_search.rs:37`) minus the
/// scan-on-commit behaviour: the query text stays separate from the
/// committed filter so a partial edit never re-scans the workspace, and the
/// results arrive from the outside rather than being produced in place.
#[derive(Debug, Clone)]
pub struct SearchViewModel {
    query: String,
    active_filter: Option<String>,
    /// Results behind an [`Arc`] so a panel can take a per-render
    /// [`Arc::clone`] (a pointer bump) instead of copying the vector —
    /// the same trick `TreeSearch::results_arc` uses.
    results: Arc<Vec<SearchResultEntry>>,
    in_progress: bool,
}

impl Default for SearchViewModel {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchViewModel {
    /// Creates a new, empty `SearchViewModel`.
    pub fn new() -> Self {
        Self {
            query: String::new(),
            active_filter: None,
            results: Arc::new(Vec::new()),
            in_progress: false,
        }
    }

    /// The raw query text currently in the search box.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// The committed search term, or `None` when no filter is active.
    pub fn active_filter(&self) -> Option<&str> {
        self.active_filter.as_deref()
    }

    /// The structured search results, one entry per matching file.
    pub fn results(&self) -> &[SearchResultEntry] {
        self.results.as_slice()
    }

    /// The search results as a cheap-to-clone [`Arc`] handle.
    pub fn results_arc(&self) -> Arc<Vec<SearchResultEntry>> {
        Arc::clone(&self.results)
    }

    /// Returns `true` while a content filter is active (results replace the
    /// tree). Alias of [`Self::is_filtering`], as in `TreeSearch`.
    pub fn is_searching(&self) -> bool {
        self.active_filter.is_some()
    }

    /// Returns `true` while a content filter is active.
    pub fn is_filtering(&self) -> bool {
        self.is_searching()
    }

    /// Returns `true` between [`Self::submit`] and the matching
    /// [`Self::apply_results`] / [`Self::finish_scan`].
    pub fn is_in_progress(&self) -> bool {
        self.in_progress
    }

    /// Number of matching files in the current snapshot.
    pub fn result_count(&self) -> usize {
        self.results.len()
    }

    /// Total occurrences across every result row.
    pub fn total_matches(&self) -> usize {
        self.results.iter().map(|entry| entry.match_count).sum()
    }

    /// Sets the search-box text without committing it ([`Self::submit`]).
    ///
    /// Returns whether the text actually changed, so a reducer can skip the
    /// redraw iced would otherwise be asked for.
    pub fn set_query(&mut self, text: String) -> bool {
        if self.query == text {
            return false;
        }
        self.query = text;
        true
    }

    /// Commits the current query text as the active filter.
    ///
    /// A blank or whitespace-only query clears everything instead, matching
    /// `TreeSearch::apply_with_allowed_dirs`
    /// (`src/app/ui/tree_search.rs:114-118`). A committed term drops the
    /// previous result list — showing last scan's rows under a new term
    /// would be a lie — and raises the in-progress flag.
    ///
    /// Returns the trimmed term the reducer must scan for, or `None` when
    /// the submission was a clear.
    pub fn submit(&mut self) -> Option<String> {
        let trimmed = self.query.trim();
        if trimmed.is_empty() {
            self.clear();
            return None;
        }

        let term = trimmed.to_string();
        self.active_filter = Some(term.clone());
        self.results = Arc::new(Vec::new());
        self.in_progress = true;
        Some(term)
    }

    /// Snapshots results produced by the reused engine for `query`.
    ///
    /// Results whose query is not the currently committed filter are
    /// **stale** — a newer submission superseded the scan — and are dropped
    /// without touching any state ([FMD-062]).
    ///
    /// Returns whether the snapshot was applied.
    pub fn apply_results(&mut self, query: &str, results: Vec<SearchResultEntry>) -> bool {
        if self.active_filter.as_deref() != Some(query) {
            return false;
        }
        self.results = Arc::new(results);
        self.in_progress = false;
        true
    }

    /// Lowers the in-progress flag without changing the result list, for a
    /// scan that finished with nothing to report.
    ///
    /// Returns whether the flag changed.
    pub fn finish_scan(&mut self) -> bool {
        if !self.in_progress {
            return false;
        }
        self.in_progress = false;
        true
    }

    /// Clears the query text, the active filter, the results and the
    /// in-progress flag.
    pub fn clear(&mut self) {
        self.query.clear();
        self.active_filter = None;
        self.results = Arc::new(Vec::new());
        self.in_progress = false;
    }

    /// The paths of every file whose content matched, in result order.
    pub fn matching_paths(&self) -> Vec<PathBuf> {
        self.results
            .iter()
            .map(|entry| entry.path.clone())
            .collect()
    }

    /// The header above the result list ([`strings::SEARCH_RESULTS_HEADER`]).
    pub fn header_label(&self) -> &'static str {
        strings::SEARCH_RESULTS_HEADER
    }

    /// The empty-state line ([`strings::SEARCH_NO_RESULTS`]).
    pub fn empty_label(&self) -> &'static str {
        strings::SEARCH_NO_RESULTS
    }

    /// The per-file match counter, e.g. `(3)` ([`strings::format_match_count`]).
    #[must_use]
    pub fn match_count_label(match_count: usize) -> String {
        strings::format_match_count(match_count)
    }
}

/// Returns `true` when a result row should show its match counter.
///
/// the legacy UI gated the counter on `match_count > 1`
/// (`src/app/ui/panels/left.rs:301`): a single match adds nothing next to
/// the file name, so only repeated hits are labelled.
#[must_use]
pub fn shows_match_count(entry: &SearchResultEntry) -> bool {
    entry.match_count > 1
}

/// Returns `true` when a result row should show its library-relative path.
///
/// the legacy UI skipped the line when it was empty or identical to the file name
/// (`src/app/ui/panels/left.rs:309`) — a duplicate line is noise.
#[must_use]
pub fn shows_relative_path(entry: &SearchResultEntry) -> bool {
    !entry.relative_path.is_empty() && entry.relative_path != entry.file_name
}

/// Returns `true` when a result row should show its preview snippet.
///
/// the legacy UI skipped an empty snippet (`src/app/ui/panels/left.rs:316`).
#[must_use]
pub fn shows_snippet(entry: &SearchResultEntry) -> bool {
    !entry.snippet.is_empty()
}

/// Formats one result row for a plain-text surface: `file_name (n) — path`.
///
/// The single-source-of-truth label behind any non-widget presentation of a
/// result (tooltips, logs, a11y text). Every piece is user-facing text and
/// therefore comes from [`crate::ui::strings`] ([GUD-002]); the separator is
/// the only literal here and it is punctuation, not copy.
#[must_use]
pub fn format_result_row(entry: &SearchResultEntry) -> String {
    let mut label = entry.file_name.clone();
    if shows_match_count(entry) {
        label.push(' ');
        label.push_str(&strings::format_match_count(entry.match_count));
    }
    if shows_relative_path(entry) {
        label.push_str(" — ");
        label.push_str(&entry.relative_path);
    }
    label
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
