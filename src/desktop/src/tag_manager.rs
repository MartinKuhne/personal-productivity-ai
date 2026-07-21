use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Global tag manager that owns the tag index and prompt-path tracking.
///
/// Responsibilities:
/// - Maintains `file_tags` (path → list of tags) and the derived `all_tags` set.
/// - Tracks which files are prompts (tagged with `prompt`, case-insensitive).
/// - Provides incremental updates during the initial scan (`add_tags`) and
///   full rebuilds (`rebuild`) after modifications or removals.
pub struct TagManager {
    file_tags: BTreeMap<PathBuf, Vec<String>>,
    all_tags: BTreeSet<String>,
    prompt_paths: BTreeSet<PathBuf>,
}

impl TagManager {
    pub fn new() -> Self {
        Self {
            file_tags: BTreeMap::new(),
            all_tags: BTreeSet::new(),
            prompt_paths: BTreeSet::new(),
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
}

impl Default for TagManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_is_empty() {
        let tm = TagManager::new();
        assert!(tm.file_tags().is_empty());
        assert!(tm.all_tags().is_empty());
        assert!(tm.prompt_paths().is_empty());
    }

    #[test]
    fn test_add_tags_tracks_tags_and_prompts() {
        let mut tm = TagManager::new();
        let p1 = PathBuf::from("doc.md");
        let p2 = PathBuf::from("prompt.md");

        tm.add_tags(p1.clone(), vec!["rust".into(), "ui".into()]);
        tm.add_tags(p2.clone(), vec!["prompt".into()]);

        assert_eq!(tm.file_tags().len(), 2);
        assert_eq!(tm.all_tags().len(), 3);
        assert!(tm.all_tags().contains("rust"));
        assert!(tm.all_tags().contains("ui"));
        assert!(tm.all_tags().contains("prompt"));

        assert!(tm.prompt_paths().contains(&p2));
        assert!(!tm.prompt_paths().contains(&p1));
    }

    #[test]
    fn test_add_tags_prompt_case_insensitive() {
        let mut tm = TagManager::new();
        let p = PathBuf::from("p.md");
        tm.add_tags(p.clone(), vec!["PROMPT".into()]);
        assert!(tm.prompt_paths().contains(&p));
    }

    #[test]
    fn test_add_tags_removes_old_prompt_status() {
        let mut tm = TagManager::new();
        let p = PathBuf::from("was_prompt.md");

        tm.add_tags(p.clone(), vec!["prompt".into()]);
        assert!(tm.prompt_paths().contains(&p));

        // File no longer has prompt tag
        tm.add_tags(p.clone(), vec!["general".into()]);
        assert!(!tm.prompt_paths().contains(&p));
    }

    #[test]
    fn test_remove_file_removes_from_prompt_paths() {
        let mut tm = TagManager::new();
        let p = PathBuf::from("prompt.md");
        tm.add_tags(p.clone(), vec!["prompt".into()]);
        assert!(tm.prompt_paths().contains(&p));

        tm.remove_file(&p);
        assert!(!tm.file_tags().contains_key(&p));
        assert!(!tm.prompt_paths().contains(&p));
    }

    #[test]
    fn test_rebuild_evicts_stale_tags() {
        let mut tm = TagManager::new();
        let p1 = PathBuf::from("a.md");
        let p2 = PathBuf::from("b.md");

        tm.add_tags(p1.clone(), vec!["rust".into(), "ui".into()]);
        tm.add_tags(p2.clone(), vec!["rust".into(), "testing".into()]);
        assert_eq!(tm.all_tags().len(), 3);

        // Remove p2 and rebuild — the "testing" tag must be evicted.
        tm.remove_file(&p2);
        tm.rebuild();
        assert_eq!(tm.all_tags().len(), 2);
        assert!(tm.all_tags().contains("rust"));
        assert!(tm.all_tags().contains("ui"));
    }

    #[test]
    fn test_rebuild_rebuilds_prompt_paths() {
        let mut tm = TagManager::new();
        let p1 = PathBuf::from("prompt.md");
        let p2 = PathBuf::from("regular.md");

        tm.add_tags(p1.clone(), vec!["prompt".into()]);
        tm.add_tags(p2.clone(), vec!["work".into()]);

        // Simulate: add_tags was called before rebuild was needed.
        // Directly manipulate file_tags to simulate stale state.
        tm.file_tags.insert(p2.clone(), vec!["prompt".into()]);
        tm.rebuild();

        assert!(tm.prompt_paths().contains(&p1));
        assert!(tm.prompt_paths().contains(&p2));
    }

    #[test]
    fn test_add_tags_incremental_does_not_evict_stale() {
        // Match current behavior: add_tags is incremental and does NOT
        // remove tags from all_tags that old tag lists no longer carry.
        // The caller must call rebuild() for eviction.
        let mut tm = TagManager::new();
        let p = PathBuf::from("doc.md");

        tm.add_tags(p.clone(), vec!["rust".into(), "ui".into()]);
        assert_eq!(tm.all_tags().len(), 2);

        // Re-add with different tags — old tag "ui" should remain
        // until rebuild() is called.
        tm.add_tags(p.clone(), vec!["rust".into()]);
        assert!(
            tm.all_tags().contains("ui"),
            "incremental add_tags should not evict stale tags"
        );

        tm.rebuild();
        assert!(!tm.all_tags().contains("ui"));
    }
}
