//! Unit tests for the iced content-search view model.

use std::path::PathBuf;
use std::sync::Arc;

use tempfile::TempDir;

use super::{
    SearchResultEntry, SearchViewModel, find_search_results, find_search_results_scoped,
    format_result_row, shows_match_count, shows_relative_path, shows_snippet,
};
use crate::config::ContentLibrary;
use crate::ui::strings;

fn entry(
    file_name: &str,
    relative_path: &str,
    snippet: &str,
    match_count: usize,
) -> SearchResultEntry {
    SearchResultEntry {
        path: PathBuf::from(relative_path).join(file_name),
        file_name: file_name.to_string(),
        relative_path: relative_path.to_string(),
        snippet: snippet.to_string(),
        line_number: 1,
        match_count,
    }
}

fn library(root: &TempDir, name: &str) -> ContentLibrary {
    ContentLibrary {
        root_folder: root.path().to_string_lossy().to_string(),
        name: name.to_string(),
        kind: "text".to_string(),
        readonly: false,
        priority: 0,
    }
}

#[test]
fn test_new_view_model_is_empty_and_idle() {
    let model = SearchViewModel::new();
    assert!(model.query().is_empty());
    assert!(model.active_filter().is_none());
    assert!(model.results().is_empty());
    assert!(!model.is_searching());
    assert!(!model.is_filtering());
    assert!(!model.is_in_progress());
    assert_eq!(model.result_count(), 0);
    assert_eq!(model.total_matches(), 0);
}

#[test]
fn test_default_matches_new() {
    let default = SearchViewModel::default();
    assert_eq!(default.query(), SearchViewModel::new().query());
    assert!(!default.is_searching());
    assert!(!default.is_in_progress());
}

#[test]
fn test_set_query_reports_whether_it_changed() {
    let mut model = SearchViewModel::new();
    assert!(model.set_query("note".to_string()));
    assert_eq!(model.query(), "note");
    assert!(
        !model.set_query("note".to_string()),
        "an identical query must not report a change"
    );
    assert!(
        !model.is_searching(),
        "typing must not commit a filter or start a scan"
    );
}

#[test]
fn test_submit_commits_trimmed_term_and_raises_in_progress() {
    let mut model = SearchViewModel::new();
    model.set_query("  blueberries  ".to_string());

    assert_eq!(model.submit().as_deref(), Some("blueberries"));
    assert_eq!(model.active_filter(), Some("blueberries"));
    assert!(model.is_searching());
    assert!(
        model.is_in_progress(),
        "a committed search is in progress until results arrive"
    );
    assert_eq!(
        model.query(),
        "  blueberries  ",
        "the box keeps the raw text; only the filter is trimmed"
    );
}

#[test]
fn test_submit_blank_query_clears_instead() {
    let mut model = SearchViewModel::new();
    model.set_query("keep".to_string());
    assert!(model.submit().is_some());
    assert!(model.is_searching());

    model.set_query("   ".to_string());
    assert_eq!(
        model.submit(),
        None,
        "a whitespace-only submission is a clear, not a scan"
    );
    assert!(!model.is_searching());
    assert!(model.active_filter().is_none());
    assert!(model.results().is_empty());
    assert!(!model.is_in_progress());
}

#[test]
fn test_submit_drops_the_previous_result_snapshot() {
    let mut model = SearchViewModel::new();
    model.set_query("first".to_string());
    let term = model.submit().expect("committed term");
    assert!(model.apply_results(&term, vec![entry("a.md", "Lib", "hit", 2)]));
    assert_eq!(model.result_count(), 1);

    model.set_query("second".to_string());
    model.submit().expect("committed term");
    assert!(
        model.results().is_empty(),
        "last scan's rows must not be shown under a new term"
    );
}

#[test]
fn test_apply_results_snapshots_and_lowers_the_in_progress_flag() {
    let mut model = SearchViewModel::new();
    model.set_query("beta".to_string());
    let term = model.submit().expect("committed term");
    assert!(model.is_in_progress());

    let applied = model.apply_results(&term, vec![entry("a.md", "Lib", "hit", 3)]);
    assert!(applied);
    assert!(!model.is_in_progress());
    assert_eq!(model.result_count(), 1);
    assert_eq!(model.results()[0].file_name, "a.md");
}

#[test]
fn test_apply_results_drops_a_stale_query() {
    let mut model = SearchViewModel::new();
    model.set_query("old".to_string());
    let stale = model.submit().expect("committed term");

    model.set_query("new".to_string());
    let fresh = model.submit().expect("committed term");
    assert_ne!(stale, fresh);

    assert!(
        !model.apply_results(&stale, vec![entry("old.md", "Lib", "stale hit", 1)]),
        "a superseded scan must not be applied"
    );
    assert!(model.results().is_empty());
    assert!(
        model.is_in_progress(),
        "the dropped scan must not clear the flag the newer scan still owns"
    );

    assert!(model.apply_results(&fresh, vec![entry("new.md", "Lib", "fresh", 1)]));
    assert_eq!(model.results()[0].file_name, "new.md");
}

#[test]
fn test_apply_results_after_clear_is_stale() {
    let mut model = SearchViewModel::new();
    model.set_query("term".to_string());
    let term = model.submit().expect("committed term");
    model.clear();

    assert!(
        !model.apply_results(&term, vec![entry("a.md", "Lib", "hit", 1)]),
        "results arriving after a clear have no filter to belong to"
    );
    assert!(model.results().is_empty());
}

#[test]
fn test_finish_scan_lowers_the_flag_only_once() {
    let mut model = SearchViewModel::new();
    assert!(
        !model.finish_scan(),
        "nothing is in progress, so nothing changes"
    );

    model.set_query("term".to_string());
    model.submit().expect("committed term");
    assert!(model.finish_scan());
    assert!(!model.is_in_progress());
    assert!(!model.finish_scan());
    assert!(
        model.is_searching(),
        "finishing a scan without results keeps the filter active"
    );
}

#[test]
fn test_clear_resets_every_field() {
    let mut model = SearchViewModel::new();
    model.set_query("term".to_string());
    let term = model.submit().expect("committed term");
    model.apply_results(&term, vec![entry("a.md", "Lib", "hit", 1)]);

    model.clear();
    assert!(model.query().is_empty());
    assert!(model.active_filter().is_none());
    assert!(model.results().is_empty());
    assert!(!model.is_searching());
    assert!(!model.is_in_progress());
}

#[test]
fn test_counts_and_paths_over_a_snapshot() {
    let mut model = SearchViewModel::new();
    model.set_query("term".to_string());
    let term = model.submit().expect("committed term");
    model.apply_results(
        &term,
        vec![
            entry("a.md", "Lib", "one", 3),
            entry("b.md", "Lib/inner", "two", 2),
        ],
    );

    assert_eq!(model.result_count(), 2);
    assert_eq!(model.total_matches(), 5);
    assert_eq!(
        model.matching_paths(),
        vec![
            PathBuf::from("Lib").join("a.md"),
            PathBuf::from("Lib/inner").join("b.md")
        ]
    );
}

#[test]
fn test_results_arc_is_a_pointer_bump() {
    let mut model = SearchViewModel::new();
    model.set_query("term".to_string());
    let term = model.submit().expect("committed term");
    model.apply_results(&term, vec![entry("a.md", "Lib", "hit", 1)]);

    let first = model.results_arc();
    let second = model.results_arc();
    assert!(
        Arc::ptr_eq(&first, &second),
        "repeated reads must not copy the vector"
    );

    model.clear();
    let third = model.results_arc();
    assert!(!Arc::ptr_eq(&first, &third));
    assert!(third.is_empty());
}

#[test]
fn test_labels_come_from_the_shared_strings_module() {
    let model = SearchViewModel::new();
    assert_eq!(model.header_label(), strings::SEARCH_RESULTS_HEADER);
    assert_eq!(model.empty_label(), strings::SEARCH_NO_RESULTS);
    assert_eq!(
        SearchViewModel::match_count_label(7),
        strings::format_match_count(7)
    );
    assert_eq!(SearchViewModel::match_count_label(7), "(7)");
}

#[test]
fn test_match_count_line_shows_only_for_repeated_hits() {
    assert!(!shows_match_count(&entry("a.md", "Lib", "hit", 0)));
    assert!(
        !shows_match_count(&entry("a.md", "Lib", "hit", 1)),
        "a single match adds nothing next to the file name (legacy parity)"
    );
    assert!(shows_match_count(&entry("a.md", "Lib", "hit", 2)));
}

#[test]
fn test_relative_path_line_hides_when_redundant() {
    assert!(
        !shows_relative_path(&entry("a.md", "", "hit", 1)),
        "an empty relative path shows nothing"
    );
    assert!(
        !shows_relative_path(&entry("a.md", "a.md", "hit", 1)),
        "a relative path identical to the file name is a duplicate line"
    );
    assert!(shows_relative_path(&entry("a.md", "Lib: x/a.md", "hit", 1)));
}

#[test]
fn test_snippet_line_hides_when_empty() {
    assert!(!shows_snippet(&entry("a.md", "Lib", "", 1)));
    assert!(shows_snippet(&entry("a.md", "Lib", "…context…", 1)));
}

#[test]
fn test_format_result_row_composes_only_the_visible_parts() {
    let bare = entry("a.md", "a.md", "", 1);
    assert_eq!(
        format_result_row(&bare),
        "a.md",
        "no count (single match), no path (redundant)"
    );

    let full = entry("note.md", "Work: notes/note.md", "…matched text…", 4);
    assert_eq!(
        format_result_row(&full),
        "note.md (4) — Work: notes/note.md"
    );
}

#[test]
fn test_reused_engine_feeds_the_view_model() {
    let dir = TempDir::new().expect("isolated temp dir");
    let matching = dir.path().join("match.md");
    let other = dir.path().join("other.md");
    std::fs::write(&matching, "alpha beta gamma\nbeta again").expect("write fixture");
    std::fs::write(&other, "nothing here matches").expect("write fixture");

    let results = find_search_results(
        &[matching.clone(), other.clone()],
        "beta",
        &[library(&dir, "Docs")],
    );

    let mut model = SearchViewModel::new();
    model.set_query(" beta ".to_string());
    let term = model.submit().expect("committed term");
    assert_eq!(term, "beta");
    assert!(model.apply_results(&term, results));

    assert_eq!(model.result_count(), 1);
    assert_eq!(model.results()[0].path, matching);
    assert_eq!(model.results()[0].file_name, "match.md");
    assert_eq!(
        model.total_matches(),
        2,
        "the engine counts both occurrences of the term"
    );
    assert!(!model.is_in_progress());
}

#[test]
fn test_scoped_engine_restrictes_to_allowed_dirs() {
    let inside = TempDir::new().expect("isolated temp dir");
    let outside = TempDir::new().expect("isolated temp dir");
    let inside_file = inside.path().join("note.md");
    let outside_file = outside.path().join("note.md");
    std::fs::write(&inside_file, "shared keyword").expect("write fixture");
    std::fs::write(&outside_file, "shared keyword").expect("write fixture");

    let allowed: std::collections::HashSet<PathBuf> =
        [inside.path().to_path_buf()].into_iter().collect();
    let results = find_search_results_scoped(
        &[inside_file.clone(), outside_file],
        "keyword",
        &[],
        Some(&allowed),
    );

    let mut model = SearchViewModel::new();
    model.set_query("keyword".to_string());
    let term = model.submit().expect("committed term");
    assert!(model.apply_results(&term, results));
    assert_eq!(
        model.results(),
        &[SearchResultEntry {
            path: inside_file.clone(),
            file_name: "note.md".to_string(),
            relative_path: "note.md".to_string(),
            snippet: "shared keyword".to_string(),
            line_number: 1,
            match_count: 1,
        }]
    );
}

#[test]
fn test_unreadable_file_never_matches() {
    let dir = TempDir::new().expect("isolated temp dir");
    let ghost = dir.path().join("does_not_exist.md");

    let results = find_search_results(std::slice::from_ref(&ghost), "anything", &[]);
    let mut model = SearchViewModel::new();
    model.set_query("anything".to_string());
    let term = model.submit().expect("committed term");
    assert!(model.apply_results(&term, results));
    assert!(model.results().is_empty());
    assert!(model.is_searching());
}
