//! Open-document tab manager - adding, closing, switching tabs, and tracking loaded markdown content, YAML front matter, and TOC.
//!
//! Unit tests live in the sibling `tabs_tests.rs` sidecar.

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

    /// Opens the tab for `path` unless it is already open.
    ///
    /// `tabs_set` is the change detector, and mutations may push into
    /// [`Tabs::tabs`](Self::tabs) directly without touching it, so a `path`
    /// **already present in [`Tabs::tabs`](Self::tabs) is treated as open
    /// even when the shadow set has drifted** — the vector is the authority
    /// and the set is healed from it. Without that healing, a `CloseTab`
    /// mutation that left the path in the set (but not the vector) would
    /// silently block every subsequent re-open of the same file.
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
#[path = "tabs_tests.rs"]
mod tests;
