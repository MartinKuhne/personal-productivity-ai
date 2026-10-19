//! Open-document tab manager — adding, closing, switching tabs, and tracking loaded markdown content, YAML front matter, and TOC.

use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;

use crate::markdown::model::ToCEntry;

/// Cached tab strip data to avoid rebuilding the tab bar every frame.
///
/// `paths` and `titles` are held behind [`Arc`] so the centre panel can take a
/// per-frame [`Arc::clone`] (a pointer bump) instead of cloning both vectors
/// (see `cached_tree_rows` for the established pattern). The cache is keyed by
/// a hash of the tab paths, so it self-invalidates even when `tabs` is mutated
/// directly (bypassing [`Tabs::invalidate_tab_strip_cache`]).
#[derive(Clone, Debug, Default)]
struct TabStripCache {
    /// Hash of the tabs vector to detect changes.
    tabs_hash: u64,
    /// Whether the cache holds a built snapshot.
    valid: bool,
    /// Cached tab paths (the source `tabs` vector).
    paths: Arc<Vec<PathBuf>>,
    /// Cached tab titles (file names).
    titles: Arc<Vec<String>>,
}

pub struct Tabs {
    pub loaded_path: Option<PathBuf>,
    pub current_yaml: Option<serde_norway::Value>,
    pub current_markdown: String,
    pub tabs: Vec<PathBuf>,
    tabs_set: HashSet<PathBuf>,
    pub toc: Vec<ToCEntry>,
    /// Pending scroll target as a stable string id (matches
    /// `ToCEntry::id`). The center panel converts it to an
    /// `Id` at render time. Stored as a string so the
    /// manager is framework-independent.
    pub scroll_to_header_id: Option<String>,
    /// Pending search jump target query. The center panel scrolls
    /// to the first occurrence of this query within the document,
    /// then clears it.
    pub scroll_to_search: Option<String>,
    /// Pending task checkbox toggles queued by `render_markdown`.
    /// Drained and applied to `current_markdown` after each frame.
    pub pending_task_toggles: Vec<(usize, bool)>,
    /// Cached tab strip to avoid per-frame rebuild.
    tab_strip_cache: TabStripCache,
    /// Pre-computed heading IDs with duplicate disambiguation.
    /// Keyed by a hash of the markdown content. Held behind [`Arc`] so the
    /// centre panel can take a per-frame [`Arc::clone`] instead of copying.
    heading_ids_cache: Option<(u64, Arc<Vec<String>>)>,
    /// Cached [`toc`](Self::toc) snapshot behind [`Arc`], keyed by a hash of the
    /// ToC entries, so the right panel can take a per-frame [`Arc::clone`].
    toc_cache: Option<(u64, Arc<Vec<ToCEntry>>)>,
}

impl Tabs {
    pub fn new() -> Self {
        Self {
            loaded_path: None,
            current_yaml: None,
            current_markdown: String::new(),
            tabs: Vec::new(),
            tabs_set: HashSet::new(),
            toc: Vec::new(),
            scroll_to_header_id: None,
            scroll_to_search: None,
            pending_task_toggles: Vec::new(),
            tab_strip_cache: TabStripCache::default(),
            heading_ids_cache: None,
            toc_cache: None,
        }
    }

    /// Opens the tab for `path` unless it is still open.
    ///
    /// `tabs_set` is the change detector the legacy mutations
    /// bypassed (`command_executor.rs` historically pushed into
    /// [`Tabs::tabs`](Self::tabs) directly), so `path` **already present in
    /// [`Tabs::tabs`](Self::tabs) is treated as open even when the shadow set
    /// has drifted** — the vector is the authority and the set is healed
    /// from it. Without that healing, a `CloseTab` mutation that left the
    /// path in the set (but not the vector) silently blocked every
    /// subsequent re-open of the same file.
    ///
    /// Purity: pure — mutates only this manager.
    pub fn open_tab(&mut self, path: PathBuf) {
        if self.tabs.contains(&path) {
            self.tabs_set.insert(path);
            return;
        }
        self.tabs_set.insert(path.clone());
        self.tabs.push(path);
        self.invalidate_tab_strip_cache();
    }

    /// Closes the tab for `path`. The vector is the authority — a path
    /// open in [`Tabs::tabs`](Self::tabs) is removed even if the shadow set
    /// has drifted, and the cache is invalidated only when something
    /// changed.
    pub fn close_tab(&mut self, path: &PathBuf) {
        self.tabs_set.remove(path);
        let len_before = self.tabs.len();
        self.tabs.retain(|p| p != path);
        if self.tabs.len() != len_before {
            self.invalidate_tab_strip_cache();
        }
    }

    /// Closes the tab at `index` (the order in [`Tabs::tabs`]), returning
    /// the closed path, or `None` when the index is out of range. Keeps the
    /// shadow set in sync (UI-049: `CloseTab`).
    #[must_use]
    pub fn close_tab_at(&mut self, index: usize) -> Option<PathBuf> {
        if index >= self.tabs.len() {
            return None;
        }
        let closed = self.tabs.remove(index);
        self.tabs_set.remove(&closed);
        self.invalidate_tab_strip_cache();
        Some(closed)
    }

    /// Closes the tab at `index`'s left, or `None` — every other tab
    /// (UI-049: `CloseOtherTabs`), keeping the set in sync.
    #[must_use]
    pub fn keep_only(&mut self, index: usize) -> Option<PathBuf> {
        let keep = self.tabs.get(index)?.clone();
        self.tabs.clear();
        self.tabs_set.clear();
        self.tabs.push(keep.clone());
        self.tabs_set.insert(keep.clone());
        self.invalidate_tab_strip_cache();
        Some(keep)
    }

    /// Closes every one of `paths` that is open, returning whether any tab
    /// was removed. Keeps the set in sync (UI-049: the file-removal
    /// close-out in `AppOrchestrator::close_tabs_for_removed_files`).
    pub fn close_tabs(&mut self, paths: &[PathBuf]) -> bool {
        let mut closed_any = false;
        for path in paths {
            self.tabs_set.remove(path);
            let len_before = self.tabs.len();
            self.tabs.retain(|p| p != path);
            closed_any |= self.tabs.len() != len_before;
        }
        if closed_any {
            self.invalidate_tab_strip_cache();
        }
        closed_any
    }

    /// Closes every tab (UI-049: `CloseAllTabs`), keeping the set in sync.
    pub fn close_all(&mut self) {
        if self.tabs.is_empty() && self.tabs_set.is_empty() {
            return;
        }
        self.tabs.clear();
        self.tabs_set.clear();
        self.invalidate_tab_strip_cache();
    }

    /// Invalidate the tab strip cache so it rebuilds on next frame.
    fn invalidate_tab_strip_cache(&mut self) {
        self.tab_strip_cache.valid = false;
    }

    /// Hash of the current tab paths (cheap change detector).
    fn tabs_hash(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.tabs.len().hash(&mut hasher);
        for tab in &self.tabs {
            tab.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Returns the cached `(paths, titles)` tab-strip snapshot, rebuilding it
    /// only when the tab set changed.
    ///
    /// The returned [`Arc`]s are cheap to clone, so the centre panel takes them
    /// once per frame instead of cloning both vectors (RUST-023).
    pub fn tab_strip(&mut self) -> (Arc<Vec<PathBuf>>, Arc<Vec<String>>) {
        let tabs_hash = self.tabs_hash();
        if !self.tab_strip_cache.valid || self.tab_strip_cache.tabs_hash != tabs_hash {
            let titles: Vec<String> = self
                .tabs
                .iter()
                .map(|p| {
                    p.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect();
            self.tab_strip_cache.tabs_hash = tabs_hash;
            self.tab_strip_cache.paths = Arc::new(self.tabs.clone());
            self.tab_strip_cache.titles = Arc::new(titles);
            self.tab_strip_cache.valid = true;
        }
        (
            Arc::clone(&self.tab_strip_cache.paths),
            Arc::clone(&self.tab_strip_cache.titles),
        )
    }

    /// Invalidate the heading IDs cache so it rebuilds on next frame.
    pub fn invalidate_heading_ids_cache(&mut self) {
        self.heading_ids_cache = None;
    }

    /// Get or build the cached tab titles.
    pub fn tab_titles(&mut self) -> &[String] {
        let _ = self.tab_strip();
        self.tab_strip_cache.titles.as_slice()
    }

    /// Get or compute heading IDs for the current markdown content, as a
    /// cheap-to-clone [`Arc`] handle.
    ///
    /// Returns a vector of stable heading IDs (with duplicate disambiguation).
    /// The centre panel uses this handle to pass the ids to the renderer
    /// without copying the vector every frame.
    pub fn heading_ids_arc(&mut self) -> Arc<Vec<String>> {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.current_markdown.hash(&mut hasher);
        let content_hash = hasher.finish();

        // Check if cached version is valid
        let cache_valid = matches!(&self.heading_ids_cache, Some((cached_hash, _)) if *cached_hash == content_hash);

        if !cache_valid {
            // Build heading IDs with duplicate disambiguation (same logic as render_markdown).
            use std::collections::HashMap;
            let mut heading_seen: HashMap<String, usize> = HashMap::new();
            let mut heading_ids = Vec::new();

            // Parse markdown to find headings - reuse the parser.
            let events = crate::markdown::parse_markdown_to_events(&self.current_markdown);
            for event in &events {
                if let crate::markdown::RenderEvent::Heading { level: _, elems } = event {
                    let text = crate::markdown::heading_plain_text(elems);
                    let trimmed = text.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let occurrence = heading_seen.entry(trimmed.to_string()).or_insert(0);
                    let id = if *occurrence == 0 {
                        trimmed.to_string()
                    } else {
                        format!("{}#{}", trimmed, *occurrence)
                    };
                    *occurrence += 1;
                    heading_ids.push(id);
                }
            }

            self.heading_ids_cache = Some((content_hash, Arc::new(heading_ids)));
        }

        Arc::clone(&self.heading_ids_cache.as_ref().unwrap().1)
    }

    /// Get or compute heading IDs for the current markdown content.
    /// Returns a slice of stable heading IDs (with duplicate disambiguation).
    pub fn heading_ids(&mut self) -> &[String] {
        let _ = self.heading_ids_arc();
        self.heading_ids_cache
            .as_ref()
            .map(|(_, ids)| ids.as_slice())
            .unwrap_or(&[])
    }

    /// Get or compute the ToC snapshot as a cheap-to-clone [`Arc`] handle.
    ///
    /// Keyed by a hash of the entries so a change to `toc` (including direct
    /// field writes) is picked up without an explicit invalidation hook.
    pub fn toc_arc(&mut self) -> Arc<Vec<ToCEntry>> {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.toc.len().hash(&mut hasher);
        for entry in &self.toc {
            entry.id.hash(&mut hasher);
            entry.title.hash(&mut hasher);
            entry.level.hash(&mut hasher);
        }
        let toc_hash = hasher.finish();

        if self.toc_cache.as_ref().map(|(h, _)| *h) != Some(toc_hash) {
            self.toc_cache = Some((toc_hash, Arc::new(self.toc.clone())));
        }
        Arc::clone(&self.toc_cache.as_ref().unwrap().1)
    }

    pub fn has_tabs(&self) -> bool {
        !self.tabs.is_empty()
    }

    pub fn clear_content(&mut self) {
        self.loaded_path = None;
        self.current_yaml = None;
        self.current_markdown = String::new();
        self.toc.clear();
        self.scroll_to_header_id = None;
        self.scroll_to_search = None;
        self.heading_ids_cache = None;
    }
}

impl Default for Tabs {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let manager = Tabs::new();
        assert!(manager.loaded_path.is_none());
        assert!(manager.current_yaml.is_none());
        assert_eq!(manager.current_markdown, "");
        assert!(manager.tabs.is_empty());
        assert!(manager.toc.is_empty());
        assert!(manager.scroll_to_header_id.is_none());
        assert!(manager.scroll_to_search.is_none());
    }

    #[test]
    fn test_open_tab() {
        let mut manager = Tabs::new();
        let path = PathBuf::from("test.md");
        manager.open_tab(path.clone());
        assert!(manager.tabs.contains(&path));
    }

    #[test]
    fn test_open_tab_no_duplicates() {
        let mut manager = Tabs::new();
        let path = PathBuf::from("test.md");
        manager.open_tab(path.clone());
        manager.open_tab(path.clone());
        assert_eq!(manager.tabs.len(), 1);
    }

    #[test]
    fn test_close_tab() {
        let mut manager = Tabs::new();
        let path = PathBuf::from("test.md");
        manager.open_tab(path.clone());
        manager.close_tab(&path);
        assert!(!manager.tabs.contains(&path));
    }

    #[test]
    fn test_clear_content() {
        let mut manager = Tabs::new();
        manager.loaded_path = Some(PathBuf::from("test.md"));
        manager.current_markdown = "# Test".to_string();
        manager.clear_content();
        assert!(manager.loaded_path.is_none());
        assert_eq!(manager.current_markdown, "");
    }

    #[test]
    fn test_toc_entry_id_is_stable_string() {
        // The manager itself never touches Id. Verify
        // the contract: ToCEntry.id is a plain String.
        let e = ToCEntry::new("Intro", 1, "Intro");
        assert_eq!(e.id, "Intro");
        assert_eq!(e.title, "Intro");
        assert_eq!(e.level, 1);

        // The second occurrence gets a unique id.
        let dup = ToCEntry::new("Intro", 1, "Intro#1");
        assert_ne!(e.id, dup.id);
    }

    #[test]
    fn test_scroll_to_header_id_is_optional_string() {
        // Confirm the field is an framework-free Option<String>.
        let mut m = Tabs::new();
        assert_eq!(m.scroll_to_header_id, None);
        m.scroll_to_header_id = Some("Intro".to_string());
        assert_eq!(m.scroll_to_header_id.as_deref(), Some("Intro"));
        m.scroll_to_header_id = None;
        assert_eq!(m.scroll_to_header_id, None);
    }

    #[test]
    fn test_close_tab_missing_path_is_noop() {
        // Closing a tab that was never opened must be a silent no-op
        // (no panic, no spurious removal of other tabs).
        let mut manager = Tabs::new();
        let open = PathBuf::from("a.md");
        manager.open_tab(open.clone());
        manager.close_tab(&PathBuf::from("never-opened.md"));
        assert_eq!(manager.tabs.len(), 1);
        assert!(manager.tabs.contains(&open));
    }

    #[test]
    fn test_tab_titles_path_without_file_name_uses_empty() {
        // A path with no file name (e.g. a root "/") falls back to an
        // empty title via `unwrap_or_default`.
        let mut manager = Tabs::new();
        manager.open_tab(PathBuf::from("/"));
        let titles = manager.tab_titles().to_vec();
        assert_eq!(titles.len(), 1);
        assert_eq!(titles[0], "");
    }

    #[test]
    fn test_tab_titles_mixed_and_cached() {
        let mut manager = Tabs::new();
        manager.open_tab(PathBuf::from("/"));
        manager.open_tab(PathBuf::from("dir/notes.md"));
        let titles = manager.tab_titles().to_vec();
        assert_eq!(titles, vec!["".to_string(), "notes.md".to_string()]);

        // A second call returns the cached slice with the same values.
        let again = manager.tab_titles().to_vec();
        assert_eq!(titles, again);
    }

    #[test]
    fn test_clear_content_invalidates_heading_ids_cache() {
        let mut manager = Tabs::new();
        manager.current_markdown = "# Intro\n\n# Intro\n\n# Body".to_string();
        let first = manager.heading_ids().to_vec();
        assert_eq!(first.len(), 3);
        assert_eq!(first[1], "Intro#1");

        // clear_content resets the markdown and invalidates the cache.
        manager.clear_content();
        assert!(manager.heading_ids().is_empty());
    }

    #[test]
    fn test_tab_strip_reuses_arc_until_tabs_change() {
        let mut manager = Tabs::new();
        manager.open_tab(PathBuf::from("a.md"));
        manager.open_tab(PathBuf::from("dir/b.md"));

        let (paths, titles) = manager.tab_strip();
        assert_eq!(paths.as_slice(), &manager.tabs);
        assert_eq!(titles.as_slice(), ["a.md".to_string(), "b.md".to_string()]);

        // A second call returns the *same* Arc allocations (pointer bump).
        let (paths2, titles2) = manager.tab_strip();
        assert!(Arc::ptr_eq(&paths, &paths2));
        assert!(Arc::ptr_eq(&titles, &titles2));

        // Mutating `tabs` directly (bypassing invalidation) rebuilds.
        manager.tabs.push(PathBuf::from("c.md"));
        let (paths3, _) = manager.tab_strip();
        assert!(!Arc::ptr_eq(&paths, &paths3));
        assert_eq!(paths3.len(), 3);
    }

    /// Regression: a removed-opened-close cycle must re-open the tab. The
    /// pre-fix `close_tab_at` mutation left the path in `tabs_set` while
    /// dropping it from `tabs`, so `open_tab`'s set guard silently refused
    /// to re-push it — "the markdown tab does not open".
    #[test]
    fn open_after_close_tab_at_reopens() {
        let mut manager = Tabs::new();
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");
        manager.open_tab(a.clone());
        manager.open_tab(b.clone());

        assert_eq!(manager.close_tab_at(0), Some(a.clone()));
        assert!(!manager.tabs.contains(&a));

        // The set must no longer block the re-open.
        manager.open_tab(a.clone());
        assert!(manager.tabs.contains(&a));
        assert_eq!(manager.tabs.len(), 2);
    }

    /// Regression: `CloseOtherTabs` must keep the kept tab openable and drop
    /// the closed ones from the shadow set.
    #[test]
    fn keep_only_syncs_set_and_selection_order() {
        let mut manager = Tabs::new();
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");
        let c = PathBuf::from("c.md");
        manager.open_tab(a.clone());
        manager.open_tab(b.clone());
        manager.open_tab(c.clone());

        assert_eq!(manager.keep_only(1), Some(b.clone()));
        assert_eq!(manager.tabs, vec![b.clone()]);
        assert!(manager.tabs_set.contains(&b));

        // The closed paths can be re-opened immediately.
        manager.open_tab(a.clone());
        assert_eq!(manager.tabs, vec![b.clone(), a.clone()]);
    }

    /// Regression: out-of-range indices are no-ops, not panics.
    #[test]
    fn keep_only_and_close_tab_at_out_of_range_are_noops() {
        let mut manager = Tabs::new();
        manager.open_tab(PathBuf::from("a.md"));
        assert_eq!(manager.keep_only(5), None);
        assert_eq!(manager.close_tab_at(5), None);
        assert_eq!(manager.tabs.len(), 1);
    }

    /// Regression: `CloseAllTabs` must clear the shadow set too, otherwise a
    /// whole workspace becomes permanently un-openable until restart.
    #[test]
    fn close_all_and_reopen() {
        let mut manager = Tabs::new();
        let a = PathBuf::from("a.md");
        manager.open_tab(a.clone());
        manager.close_all();
        assert!(manager.tabs.is_empty());
        manager.open_tab(a.clone());
        assert_eq!(manager.tabs, vec![a.clone()]);
        // A second open is still a no-op duplicate guard.
        manager.open_tab(a.clone());
        assert_eq!(manager.tabs.len(), 1);
    }

    /// Regression: `close_tabs` reports whether anything was closed so the
    /// orchestrator's selection retarget only runs on real removals.
    #[test]
    fn close_tabs_reports_closed_any_and_reopens() {
        let mut manager = Tabs::new();
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");
        manager.open_tab(a.clone());
        manager.open_tab(b.clone());

        // Paths that are not open must not count as closed.
        let never_opened = PathBuf::from("never-opened.md");
        assert!(!manager.close_tabs(&[never_opened, PathBuf::from("/")]));
        assert_eq!(manager.tabs.len(), 2);

        assert!(manager.close_tabs(std::slice::from_ref(&a)));
        assert_eq!(manager.tabs, vec![b.clone()]);
        manager.open_tab(a.clone());
        assert_eq!(manager.tabs, vec![b.clone(), a.clone()]);
    }

    /// Healing: a manager whose set/vector disagree after a legacy direct
    /// mutation still opens (no duplicate) and closes (vector and set in
    /// step again).
    #[test]
    fn open_and_close_heal_set_desync() {
        let mut manager = Tabs::new();
        let a = PathBuf::from("a.md");
        let b = PathBuf::from("b.md");
        manager.open_tab(a.clone());
        manager.open_tab(b.clone());

        // Desync half 1: the set lost `a` but the vector still has it —
        // re-opening must not create a duplicate.
        assert!(manager.tabs_set.remove(&a));
        manager.open_tab(a.clone());
        assert_eq!(
            manager.tabs.iter().position(|p| p == &a),
            Some(0),
            "open must not duplicate"
        );
        assert_eq!(manager.tabs.len(), 2);

        // Desync half 2: closing by index removes the row from *both*
        // bookkeeping sides, so re-opening pushes it again instead of being
        // swallowed by a stale set guard.
        assert_eq!(manager.close_tab_at(0), Some(a.clone()));
        assert!(!manager.tabs_set.contains(&a));
        manager.open_tab(a.clone());
        assert_eq!(manager.tabs, vec![b.clone(), a.clone()]);

        // Closing once removes both bookkeeping sides.
        manager.close_tab(&a);
        assert!(!manager.tabs.contains(&a));
        assert!(!manager.tabs_set.contains(&a));
        assert_eq!(manager.tabs, vec![b.clone()]);
    }

    #[test]
    fn test_heading_ids_arc_reuses_arc_until_content_changes() {
        let mut manager = Tabs::new();
        manager.current_markdown = "# Intro".to_string();
        let first = manager.heading_ids_arc();
        assert_eq!(first.as_slice(), ["Intro".to_string()]);

        let second = manager.heading_ids_arc();
        assert!(
            Arc::ptr_eq(&first, &second),
            "unchanged content must reuse the Arc allocation"
        );

        manager.current_markdown = "# Intro\n\n# Two".to_string();
        let third = manager.heading_ids_arc();
        assert!(!Arc::ptr_eq(&first, &third));
        assert_eq!(third.len(), 2);
    }

    #[test]
    fn test_toc_arc_reuses_arc_until_toc_changes() {
        let mut manager = Tabs::new();
        manager.toc = vec![ToCEntry::new("Intro", 1, "Intro")];
        let first = manager.toc_arc();
        assert_eq!(first.len(), 1);

        let second = manager.toc_arc();
        assert!(
            Arc::ptr_eq(&first, &second),
            "unchanged ToC must reuse the Arc allocation"
        );

        manager.toc.push(ToCEntry::new("Body", 1, "Body"));
        let third = manager.toc_arc();
        assert!(!Arc::ptr_eq(&first, &third));
        assert_eq!(third.len(), 2);
    }
}
