//! Global tag index — per-file tag map, derived set of all tags, and prompt-tagged file tracking.
//!
//! Unit tests live in the sibling `tags_tests.rs` sidecar.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Global tag manager that owns the tag index and prompt-path tracking.
///
/// Responsibilities:
/// - Maintains `file_tags` (path → list of tags) and the derived `all_tags` set.
/// - Tracks which files are prompts (tagged with `prompt`, case-insensitive).
/// - Provides incremental updates during the initial scan (`add_tags`) and
///   full rebuilds (`rebuild`) after modifications or removals.
pub struct Tags {
    file_tags: BTreeMap<PathBuf, Vec<String>>,
    all_tags: BTreeSet<String>,
    prompt_paths: BTreeSet<PathBuf>,
    pub selected_tag: Option<String>,
}

impl Tags {
    pub fn new() -> Self {
        Self {
            file_tags: BTreeMap::new(),
            all_tags: BTreeSet::new(),
            prompt_paths: BTreeSet::new(),
            selected_tag: None,
        }
    }

    pub fn file_tags(&self) -> &BTreeMap<PathBuf, Vec<String>> {
        &self.file_tags
    }

    pub fn all_tags(&self) -> &BTreeSet<String> {
        &self.all_tags
    }

    /// Paths of files whose tags include `prompt` (case-insensitive).
    pub fn prompt_paths(&self) -> &BTreeSet<PathBuf> {
        &self.prompt_paths
    }

    /// Add or update tags for a file.
    ///
    /// During the initial scan (`FileParsed`) this incrementally updates
    /// `all_tags`. After modifications (`FileModified`) the caller should
    /// also call `rebuild()` to evict stale tags.
    pub fn add_tags(&mut self, path: PathBuf, tags: Vec<String>) {
        let is_prompt = tags.iter().any(|t| t.eq_ignore_ascii_case("prompt"));

        self.file_tags.insert(path.clone(), tags);

        for tag in &self.file_tags[&path] {
            self.all_tags.insert(tag.clone());
        }

        if is_prompt {
            self.prompt_paths.insert(path);
        } else {
            self.prompt_paths.remove(&path);
        }
    }

    /// Remove a file from the tag index entirely.
    ///
    /// Does NOT update `all_tags` — the caller should call `rebuild()` so
    /// stale tags are evicted (matching the existing contract for
    /// `FileDeleted` and `Removed` events).
    pub fn remove_file(&mut self, path: &Path) {
        self.file_tags.remove(path);
        self.prompt_paths.remove(path);
    }

    /// Fully rebuild `all_tags` and `prompt_paths` from `file_tags`.
    ///
    /// Call this after removals or batch modifications to evict stale
    /// entries from the derived sets.
    pub fn rebuild(&mut self) {
        self.all_tags.clear();
        self.prompt_paths.clear();
        for (path, tags) in &self.file_tags {
            for tag in tags {
                self.all_tags.insert(tag.clone());
            }
            if tags.iter().any(|t| t.eq_ignore_ascii_case("prompt")) {
                self.prompt_paths.insert(path.clone());
            }
        }
    }

    /// Replace the tag index with a full background resynchronization snapshot.
    pub fn replace_all(&mut self, files: Vec<(PathBuf, Vec<String>)>) {
        self.file_tags = files.into_iter().collect();
        self.rebuild();
    }
}

impl Default for Tags {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "tags_tests.rs"]
mod tests;
