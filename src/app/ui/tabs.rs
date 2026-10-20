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

/// Target jump query and 0-based match index within the document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchJump {
    /// Case-insensitive search query.
    pub query: String,
    /// 0-based index of the target match in the document.
    pub match_index: usize,
}

impl SearchJump {
    /// Purpose: Constructs a new `SearchJump` target.
    /// Inputs: `query` - search term, `match_index` - target match ordinal
    /// Outputs: `SearchJump` instance
    /// Purity: Pure
    pub fn new(query: impl Into<String>, match_index: usize) -> Self {
        Self {
            query: query.into(),
            match_index,
        }
    }
}

impl From<String> for SearchJump {
    fn from(query: String) -> Self {
        Self {
            query,
            match_index: 0,
        }
    }
}

impl From<&str> for SearchJump {
    fn from(query: &str) -> Self {
        Self {
            query: query.to_string(),
            match_index: 0,
        }
    }
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
    /// `egui::Id` at render time. Stored as a string so the
    /// manager is egui-independent.
    pub scroll_to_header_id: Option<String>,
    /// Pending search jump target query and match index. The center panel scrolls
    /// to the requested occurrence of this query within the document,
    /// then clears it.
    pub scroll_to_search: Option<SearchJump>,
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

    pub fn open_tab(&mut self, path: PathBuf) {
        if self.tabs_set.insert(path.clone()) {
            self.tabs.push(path);
            self.invalidate_tab_strip_cache();
        }
    }

    pub fn close_tab(&mut self, path: &PathBuf) {
        if self.tabs_set.remove(path) {
            self.tabs.retain(|p| p != path);
            self.invalidate_tab_strip_cache();
        }
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
        // The manager itself never touches egui::Id. Verify
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
        // Confirm the field is an egui-free Option<String>.
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
