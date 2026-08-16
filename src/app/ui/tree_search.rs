//! Directory-tree content search state — the search query, the active filter, and the set of files whose content matches.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Tracks the directory-tree search box (UI-050).
///
/// The query text and the applied filter are kept separate so the UI
/// can show a partial edit before the user commits it with Enter or
/// the magnifier button. While `is_filtering()` is `true`, the left
/// panel tree only shows files whose content matches
/// [`Self::matching_files`] and directories that contain such files.
pub struct TreeSearch {
    query: String,
    active_filter: Option<String>,
    matching_files: HashSet<PathBuf>,
}

impl Default for TreeSearch {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeSearch {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            active_filter: None,
            matching_files: HashSet::new(),
        }
    }

    /// The raw query text currently in the search box.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Mutable access to the search-box text, for the egui `TextEdit`.
    pub fn query_mut(&mut self) -> &mut String {
        &mut self.query
    }

    /// Returns `true` while a content filter is active (tree is narrowed).
    pub fn is_filtering(&self) -> bool {
        self.active_filter.is_some()
    }

    /// The committed search term, or `None` when no filter is active.
    pub fn active_filter(&self) -> Option<&str> {
        self.active_filter.as_deref()
    }

    /// Files whose content matched the active filter.
    pub fn matching_files(&self) -> &HashSet<PathBuf> {
        &self.matching_files
    }

    /// Commits the current query text as the active filter.
    ///
    /// A blank/whitespace query clears the filter (show all files).
    /// A non-blank query is matched case-insensitively against file
    /// content; unreadable files never match.
    pub fn apply(&mut self, all_files: &[PathBuf]) {
        let trimmed = self.query.trim();
        if trimmed.is_empty() {
            self.clear();
            return;
        }
        self.active_filter = Some(trimmed.to_string());
        let term_lower = trimmed.to_lowercase();
        self.matching_files = filter_files_by_content(all_files, &term_lower);
    }

    /// Clears the query text, the active filter, and the match set.
    pub fn clear(&mut self) {
        self.query.clear();
        self.active_filter = None;
        self.matching_files.clear();
    }
}

/// Returns the subset of `all_files` whose content contains `term_lower`.
///
/// Matching is case-insensitive and purely content-based: the file
/// name is not considered. Files that cannot be read (missing,
/// non-UTF-8, permission denied) are excluded from the result.
pub fn filter_files_by_content(all_files: &[PathBuf], term_lower: &str) -> HashSet<PathBuf> {
    all_files
        .iter()
        .filter(|path| file_contains_term(path, term_lower))
        .cloned()
        .collect()
}

/// Returns `true` when the file at `path` is readable UTF-8 text whose
/// lowercased content contains `term_lower`.
fn file_contains_term(path: &Path, term_lower: &str) -> bool {
    std::fs::read_to_string(path)
        .map(|content| content.to_lowercase().contains(term_lower))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    fn write_file(path: &Path, content: &str) {
        let mut file = fs::File::create(path).expect("test file should be created");
        file.write_all(content.as_bytes())
            .expect("test file should be written");
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fastmd_tree_search_{name}"));
        fs::create_dir_all(&dir).expect("temp dir should be created");
        dir
    }

    #[test]
    fn test_apply_filters_files_by_content() {
        let dir = temp_dir("apply");
        let matching = dir.join("match.md");
        let non_matching = dir.join("other.md");
        write_file(&matching, "This document mentions blueberries.");
        write_file(&non_matching, "Nothing here matches.");

        let mut search = TreeSearch::new();
        *search.query_mut() = "blueberries".to_string();
        search.apply(&[matching.clone(), non_matching.clone()]);

        assert!(search.is_filtering());
        assert_eq!(search.active_filter(), Some("blueberries"));
        assert!(search.matching_files().contains(&matching));
        assert!(!search.matching_files().contains(&non_matching));
    }

    #[test]
    fn test_apply_is_case_insensitive() {
        let dir = temp_dir("case");
        let file = dir.join("notes.md");
        write_file(&file, "Hello World");

        let mut search = TreeSearch::new();
        *search.query_mut() = "HELLO".to_string();
        search.apply(&[file.clone()]);

        assert!(search.matching_files().contains(&file));
    }

    #[test]
    fn test_apply_trims_query() {
        let dir = temp_dir("trim");
        let file = dir.join("notes.md");
        write_file(&file, "needle");

        let mut search = TreeSearch::new();
        *search.query_mut() = "  needle  ".to_string();
        search.apply(&[file.clone()]);

        assert_eq!(search.active_filter(), Some("needle"));
    }

    #[test]
    fn test_apply_with_empty_query_clears_filter() {
        let mut search = TreeSearch::new();
        *search.query_mut() = "needle".to_string();
        search.apply(&[]);
        assert!(search.is_filtering());

        *search.query_mut() = "   ".to_string();
        search.apply(&[]);
        assert!(!search.is_filtering());
        assert!(search.active_filter().is_none());
        assert!(search.matching_files().is_empty());
    }

    #[test]
    fn test_clear_resets_all_state() {
        let dir = temp_dir("clear");
        let file = dir.join("notes.md");
        write_file(&file, "needle");

        let mut search = TreeSearch::new();
        *search.query_mut() = "needle".to_string();
        search.apply(&[file.clone()]);
        assert!(search.is_filtering());

        search.clear();
        assert!(search.query().is_empty());
        assert!(!search.is_filtering());
        assert!(search.active_filter().is_none());
        assert!(search.matching_files().is_empty());
    }

    #[test]
    fn test_missing_file_never_matches() {
        let dir = temp_dir("missing");
        let ghost = dir.join("does_not_exist.md");
        let mut search = TreeSearch::new();
        *search.query_mut() = "anything".to_string();
        search.apply(&[ghost]);
        assert!(search.matching_files().is_empty());
        assert!(search.is_filtering());
    }
}
