//! Single parallel Markdown content-search engine (ripgrep core) shared by the agent `search_notes` tool and the UI tree search.
//!
//! Requirements: TOOL-013 (Markdown-only search), TOOL-026c (paged results
//! sliced at the call site); the `.md`/`.markdown` filter also keeps PDF and
//! image files out of LLM file tools (REQ-451A, REQ-471A).
//!
//! Unit tests live in the sibling `search_tests.rs` sidecar.

use std::path::{Path, PathBuf};

use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use rayon::prelude::*;

/// One file's matching lines, in ascending line-number order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileLineMatches {
    /// Path of the searched file, as given in the input file list.
    pub path: PathBuf,
    /// `(1-based file line number, line text without terminator)` pairs.
    pub lines: Vec<(u64, String)>,
}

/// Returns `true` when `path` has a Markdown extension (`.md` or
/// `.markdown`), case-insensitively.
pub fn is_markdown_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("md" | "markdown")
    )
}

/// Enumerate every Markdown file under `root`.
///
/// This is the reusable file list: callers that already track workspace files
/// (e.g. the UI `FileEventProcessor::all_files`) pass their list straight to
/// [`search_files_parallel`]; callers without one (e.g. the agent tool)
/// build it once here and then fan out in parallel instead of
/// interleaving a sequential walk with per-line reads.
pub fn collect_markdown_files(root: &Path) -> Vec<PathBuf> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_file() && is_markdown_file(entry.path()))
        .map(|entry| entry.path().to_path_buf())
        .collect()
}

/// Build a case-insensitive literal matcher for `query`.
///
/// Returns `None` for a blank query or when the matcher cannot be built;
/// callers treat `None` as "no matches" rather than an error.
fn build_matcher(query: &str) -> Option<RegexMatcher> {
    if query.trim().is_empty() {
        return None;
    }
    let literal = regex::escape(query);
    RegexMatcherBuilder::new()
        .case_insensitive(true)
        .build(&literal)
        .ok()
}

/// Search one in-memory slice with an already-built matcher.
fn search_slice_with(matcher: &RegexMatcher, haystack: &[u8]) -> Vec<(u64, String)> {
    use grep_searcher::SearcherBuilder;
    use grep_searcher::sinks::UTF8;

    let mut searcher = SearcherBuilder::new().line_number(true).build();
    let mut matches = Vec::new();
    let sink = UTF8(|line_number: u64, line: &str| -> std::io::Result<bool> {
        matches.push((line_number, line.to_string()));
        Ok(true)
    });
    let _: std::io::Result<()> = searcher.search_slice(matcher, haystack, sink);
    matches
}

/// Search `content` for `query`, case-insensitively, as a literal substring.
///
/// When `strip_front_matter` is set, a valid YAML front-matter block is
/// excluded from the search and returned line numbers are offset past it
/// (the `search_notes` contract); otherwise the whole content is searched
/// (the tree-search contract). Returns `(1-based file line number, line
/// text)` pairs in ascending line order.
///
/// # Examples
///
/// ```
/// use fastmd_agent::tools::search::search_content;
///
/// let content = "---\ntags: [x]\n---\nHello world\n";
/// let matches = search_content("hello", content, true);
/// assert_eq!(matches.len(), 1);
/// assert_eq!(matches[0].0, 4);
/// ```
pub fn search_content(query: &str, content: &str, strip_front_matter: bool) -> Vec<(u64, String)> {
    let Some(matcher) = build_matcher(query) else {
        return Vec::new();
    };
    search_content_with(&matcher, content, strip_front_matter)
}

/// [`search_content`] with a pre-built matcher (used by the parallel fan-out
/// so the regex compiles once per search instead of once per file).
fn search_content_with(
    matcher: &RegexMatcher,
    content: &str,
    strip_front_matter: bool,
) -> Vec<(u64, String)> {
    if strip_front_matter
        && let Some(front_matter) = crate::utils::markdown::parse_front_matter(content)
    {
        let skipped = content
            .lines()
            .count()
            .saturating_sub(front_matter.body.lines().count()) as u64;
        return search_slice_with(matcher, front_matter.body.as_bytes())
            .into_iter()
            .map(|(line_number, text)| (line_number + skipped, text))
            .collect();
    }
    search_slice_with(matcher, content.as_bytes())
}

/// Search a reused file list in parallel with the shared ripgrep-core engine.
///
/// Only Markdown files are read; unreadable files never match. Returned
/// entries follow the input order (rayon preserves order) and contain only
/// files with at least one match, each file's lines in ascending order.
pub fn search_files_parallel<P>(
    files: &[P],
    query: &str,
    strip_front_matter: bool,
    read_file: impl Fn(&Path) -> std::io::Result<String> + Sync,
) -> Vec<FileLineMatches>
where
    P: AsRef<Path> + Sync,
{
    let Some(matcher) = build_matcher(query) else {
        return Vec::new();
    };
    files
        .par_iter()
        .filter(|path| is_markdown_file(path.as_ref()))
        .filter_map(|path| {
            let content = read_file(path.as_ref()).ok()?;
            let lines = search_content_with(&matcher, &content, strip_front_matter);
            if lines.is_empty() {
                None
            } else {
                Some(FileLineMatches {
                    path: path.as_ref().to_path_buf(),
                    lines,
                })
            }
        })
        .collect()
}

/// Count non-overlapping case-insensitive literal occurrences of `query` in
/// `haystack`.
pub fn count_occurrences(haystack: &str, query: &str) -> usize {
    let term_lower = query.to_lowercase();
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
        haystack.to_lowercase().matches(term_lower.as_str()).count()
    }
}

/// Truncate or window a matching line into a readable preview snippet.
pub fn extract_snippet(line: &str, query: &str) -> String {
    let term_lower = query.to_lowercase();
    let trimmed = line.trim();
    if trimmed.chars().count() <= 100 {
        return trimmed.to_string();
    }
    let lower = trimmed.to_lowercase();
    if let Some(byte_pos) = lower.find(term_lower.as_str()) {
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

// ---------------------------------------------------------------------------
// Tests live in the sibling `search_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
