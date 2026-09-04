//! Directory-tree content search state — search query, active filter, and search result list.
//!
//! Unit tests live in the sibling `tree_search_tests.rs` sidecar.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::config::ContentLibrary;

/// A single matching file in a content search result list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchResultEntry {
    /// Path to the matching file.
    pub path: PathBuf,
    /// File display name (e.g. "notes.md").
    pub file_name: String,
    /// Library-relative path (e.g. "Work / notes.md" or "notes.md").
    pub relative_path: String,
    /// Preview snippet of the first matching line.
    pub snippet: String,
    /// 1-based line number of the first match within the file.
    pub line_number: usize,
    /// Total number of occurrences of the term in this file.
    pub match_count: usize,
}

/// Tracks workspace content search state (UI-050).
///
/// The query text and the applied filter are kept separate so the UI
/// can show a partial edit before the user commits it with Enter or
/// the magnifier button. While `is_searching()` is `true`, the left
/// panel replaces the tree view with [`Self::results`], showing one
/// entry per matching file.
pub struct TreeSearch {
    query: String,
    active_filter: Option<String>,
    results: Vec<SearchResultEntry>,
    matching_files: HashSet<PathBuf>,
}

impl Default for TreeSearch {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeSearch {
    /// Creates a new, empty `TreeSearch`.
    pub fn new() -> Self {
        Self {
            query: String::new(),
            active_filter: None,
            results: Vec::new(),
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

    /// Returns `true` while a search filter is active (results replace tree).
    pub fn is_searching(&self) -> bool {
        self.active_filter.is_some()
    }

    /// Returns `true` while a content filter is active. Alias for [`Self::is_searching`].
    pub fn is_filtering(&self) -> bool {
        self.is_searching()
    }

    /// The committed search term, or `None` when no filter is active.
    pub fn active_filter(&self) -> Option<&str> {
        self.active_filter.as_deref()
    }

    /// The structured search results, one entry per matching file.
    pub fn results(&self) -> &[SearchResultEntry] {
        &self.results
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
    pub fn apply(&mut self, all_files: &[PathBuf], content_libraries: &[ContentLibrary]) {
        let trimmed = self.query.trim();
        if trimmed.is_empty() {
            self.clear();
            return;
        }
        self.active_filter = Some(trimmed.to_string());
        let term_lower = trimmed.to_lowercase();
        self.results = find_search_results(all_files, &term_lower, content_libraries);
        self.matching_files = self.results.iter().map(|r| r.path.clone()).collect();
    }

    /// Clears the query text, the active filter, and all search results.
    pub fn clear(&mut self) {
        self.query.clear();
        self.active_filter = None;
        self.results.clear();
        self.matching_files.clear();
    }
}

/// Derives the relative display path for a file against known content libraries.
pub fn compute_relative_path(path: &Path, content_libraries: &[ContentLibrary]) -> String {
    for lib in content_libraries {
        let lib_root = Path::new(&lib.root_folder);
        if let Ok(rel) = path.strip_prefix(lib_root) {
            let rel_str = rel.to_string_lossy();
            if rel_str.is_empty() {
                return lib.name.clone();
            }
            return format!("{}: {}", lib.name, rel_str.replace('\\', "/"));
        }
    }
    path.file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Truncates or windows a matching line into a readable preview snippet.
pub fn extract_snippet(line: &str, term_lower: &str) -> String {
    let trimmed = line.trim();
    if trimmed.chars().count() <= 100 {
        return trimmed.to_string();
    }
    let lower = trimmed.to_lowercase();
    if let Some(byte_pos) = lower.find(term_lower) {
        let char_idx = trimmed[..byte_pos].chars().count();
        let start = char_idx.saturating_sub(30);
        let snippet_chars: String = trimmed.chars().skip(start).take(90).collect();
        let prefix = if start > 0 { "…" } else { "" };
        let suffix = if trimmed.chars().count() > start + 90 {
            "…"
        } else {
            ""
        };
        format!("{prefix}{snippet_chars}{suffix}")
    } else {
        let snippet_chars: String = trimmed.chars().take(90).collect();
        format!("{snippet_chars}…")
    }
}

/// Returns `true` if `path` begins with `root`, matching case-insensitively on Windows.
pub fn path_starts_with_root(path: &Path, root: &Path) -> bool {
    if path.starts_with(root) {
        return true;
    }
    #[cfg(windows)]
    {
        use std::path::Component;
        let mut path_comps = path.components();
        for root_comp in root.components() {
            match (path_comps.next(), root_comp) {
                (Some(Component::Prefix(p_pref)), Component::Prefix(r_pref)) => {
                    if p_pref.as_os_str().to_string_lossy().to_lowercase()
                        != r_pref.as_os_str().to_string_lossy().to_lowercase()
                    {
                        return false;
                    }
                }
                (Some(p_c), r_c) => {
                    if p_c.as_os_str().to_string_lossy().to_lowercase()
                        != r_c.as_os_str().to_string_lossy().to_lowercase()
                    {
                        return false;
                    }
                }
                (None, _) => return false,
            }
        }
        true
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Returns `true` if `path` is within at least one of the configured content libraries.
pub fn is_in_content_library(path: &Path, content_libraries: &[ContentLibrary]) -> bool {
    if content_libraries.is_empty() {
        return false;
    }
    content_libraries.iter().any(|lib| {
        let lib_root = Path::new(&lib.root_folder);
        path_starts_with_root(path, lib_root)
    })
}

/// Returns `true` if `path` has a markdown extension (`.md` or `.markdown`), case-insensitively.
pub fn is_markdown_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("md" | "markdown")
    )
}

/// Returns `true` if `haystack` contains `term_lower` case-insensitively.
fn contains_ignore_case(haystack: &str, term_lower: &str) -> bool {
    if term_lower.is_empty() {
        return true;
    }
    if term_lower.is_ascii() {
        let term_bytes = term_lower.as_bytes();
        haystack
            .as_bytes()
            .windows(term_bytes.len())
            .any(|window| window.eq_ignore_ascii_case(term_bytes))
    } else {
        haystack.to_lowercase().contains(term_lower)
    }
}

/// Counts non-overlapping case-insensitive occurrences of `term_lower` in `haystack`.
fn count_matches_ignore_case(haystack: &str, term_lower: &str) -> usize {
    if term_lower.is_empty() {
        return 0;
    }
    if term_lower.is_ascii() {
        let term_bytes = term_lower.as_bytes();
        let term_len = term_bytes.len();
        let haystack_bytes = haystack.as_bytes();
        let mut count = 0;
        let mut i = 0;
        while i + term_len <= haystack_bytes.len() {
            if haystack_bytes[i..i + term_len].eq_ignore_ascii_case(term_bytes) {
                count += 1;
                i += term_len;
            } else {
                i += 1;
            }
        }
        count
    } else {
        haystack.to_lowercase().matches(term_lower).count()
    }
}

/// Scans a single markdown file for occurrences of `term_lower`.
/// Returns `Some(SearchResultEntry)` if at least one match is found.
fn extract_file_result(
    path: &Path,
    term_lower: &str,
    content_libraries: &[ContentLibrary],
) -> Option<SearchResultEntry> {
    if !is_markdown_file(path) {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    if !contains_ignore_case(&content, term_lower) {
        return None;
    }

    let mut match_count = 0usize;
    let mut first_line_num = 0usize;
    let mut first_snippet = String::new();

    for (idx, line) in content.lines().enumerate() {
        let count_on_line = count_matches_ignore_case(line, term_lower);
        if count_on_line > 0 {
            if match_count == 0 {
                first_line_num = idx + 1;
                first_snippet = extract_snippet(line, term_lower);
            }
            match_count += count_on_line;
        }
    }

    if match_count > 0 {
        let file_name = path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default();
        let relative_path = compute_relative_path(path, content_libraries);
        Some(SearchResultEntry {
            path: path.to_path_buf(),
            file_name,
            relative_path,
            snippet: first_snippet,
            line_number: first_line_num,
            match_count,
        })
    } else {
        None
    }
}

/// Returns structured search results across `all_files` for `term_lower`.
///
/// Only directories containing markdown files within configured content libraries
/// are searched, and only markdown files within those directories are scanned.
/// One entry per matching file.
pub fn find_search_results(
    all_files: &[PathBuf],
    term_lower: &str,
    content_libraries: &[ContentLibrary],
) -> Vec<SearchResultEntry> {
    let markdown_dirs: HashSet<&Path> = all_files
        .iter()
        .filter(|path| is_markdown_file(path))
        .filter_map(|path| path.parent())
        .filter(|dir| content_libraries.is_empty() || is_in_content_library(dir, content_libraries))
        .collect();

    let mut results: Vec<SearchResultEntry> = all_files
        .iter()
        .filter(|path| {
            is_markdown_file(path)
                && (content_libraries.is_empty() || is_in_content_library(path, content_libraries))
                && path
                    .parent()
                    .is_some_and(|parent| markdown_dirs.contains(parent))
        })
        .filter_map(|path| extract_file_result(path, term_lower, content_libraries))
        .collect();

    // Sort alphabetically by file name, then relative path
    results.sort_by(|a, b| {
        a.file_name
            .cmp(&b.file_name)
            .then(a.relative_path.cmp(&b.relative_path))
    });
    results
}

/// Returns the subset of `all_files` whose content contains `term_lower`.
///
/// Only directories containing markdown files and markdown files themselves are considered.
pub fn filter_files_by_content(all_files: &[PathBuf], term_lower: &str) -> HashSet<PathBuf> {
    let markdown_dirs: HashSet<&Path> = all_files
        .iter()
        .filter(|path| is_markdown_file(path))
        .filter_map(|path| path.parent())
        .collect();

    all_files
        .iter()
        .filter(|path| {
            is_markdown_file(path)
                && path
                    .parent()
                    .is_some_and(|parent| markdown_dirs.contains(parent))
                && file_contains_term(path, term_lower)
        })
        .cloned()
        .collect()
}

/// Returns `true` when the file at `path` is readable UTF-8 text whose
/// content contains `term_lower` case-insensitively.
fn file_contains_term(path: &Path, term_lower: &str) -> bool {
    std::fs::read_to_string(path)
        .map(|content| contains_ignore_case(&content, term_lower))
        .unwrap_or(false)
}

#[cfg(test)]
#[path = "tree_search_tests.rs"]
mod tests;
