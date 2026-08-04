//! Open-document tab manager — adding, closing, switching tabs, and tracking loaded markdown content, YAML front matter, and TOC.

use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use crate::markdown::toc_entry::ToCEntry;

/// Cached tab strip data to avoid rebuilding the tab bar every frame.
#[derive(Clone, Debug, Default)]
struct TabStripCache {
    /// Hash of the tabs vector to detect changes.
    tabs_hash: u64,
    /// Cached tab titles (file names).
    titles: Vec<String>,
}

pub struct TabManager {
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
    /// Pending task checkbox toggles queued by `render_markdown`.
    /// Drained and applied to `current_markdown` after each frame.
    pub pending_task_toggles: Vec<(usize, bool)>,
    /// Cached tab strip to avoid per-frame rebuild.
    tab_strip_cache: TabStripCache,
    /// Pre-computed heading IDs with duplicate disambiguation.
    /// Keyed by a hash of the markdown content.
    heading_ids_cache: Option<(u64, Vec<String>)>,
}

impl TabManager {
    pub fn new() -> Self {
        Self {
            loaded_path: None,
            current_yaml: None,
            current_markdown: String::new(),
            tabs: Vec::new(),
            tabs_set: HashSet::new(),
            toc: Vec::new(),
            scroll_to_header_id: None,
            pending_task_toggles: Vec::new(),
            tab_strip_cache: TabStripCache::default(),
            heading_ids_cache: None,
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
        self.tab_strip_cache.tabs_hash = 0;
        self.tab_strip_cache.titles.clear();
    }

    /// Invalidate the heading IDs cache so it rebuilds on next frame.
    pub fn invalidate_heading_ids_cache(&mut self) {
        self.heading_ids_cache = None;
    }

    /// Get or build the cached tab titles.
    pub fn tab_titles(&mut self) -> &[String] {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.tabs.len().hash(&mut hasher);
        for tab in &self.tabs {
            tab.hash(&mut hasher);
        }
        let tabs_hash = hasher.finish();

        if self.tab_strip_cache.tabs_hash == tabs_hash && !self.tab_strip_cache.titles.is_empty() {
            return &self.tab_strip_cache.titles;
        }

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
        self.tab_strip_cache.titles = titles;
        &self.tab_strip_cache.titles
    }

    /// Get or compute heading IDs for the current markdown content.
    /// Returns a vector of stable heading IDs (with duplicate disambiguation).
    pub fn heading_ids(&mut self) -> &[String] {
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

            self.heading_ids_cache = Some((content_hash, heading_ids));
        }

        // Return reference to cached data (cache is guaranteed valid now)
        self.heading_ids_cache.as_ref().unwrap().1.as_slice()
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
        self.heading_ids_cache = None;
    }
}

impl Default for TabManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let manager = TabManager::new();
        assert!(manager.loaded_path.is_none());
        assert!(manager.current_yaml.is_none());
        assert_eq!(manager.current_markdown, "");
        assert!(manager.tabs.is_empty());
        assert!(manager.toc.is_empty());
        assert!(manager.scroll_to_header_id.is_none());
    }

    #[test]
    fn test_open_tab() {
        let mut manager = TabManager::new();
        let path = PathBuf::from("test.md");
        manager.open_tab(path.clone());
        assert!(manager.tabs.contains(&path));
    }

    #[test]
    fn test_open_tab_no_duplicates() {
        let mut manager = TabManager::new();
        let path = PathBuf::from("test.md");
        manager.open_tab(path.clone());
        manager.open_tab(path.clone());
        assert_eq!(manager.tabs.len(), 1);
    }

    #[test]
    fn test_close_tab() {
        let mut manager = TabManager::new();
        let path = PathBuf::from("test.md");
        manager.open_tab(path.clone());
        manager.close_tab(&path);
        assert!(!manager.tabs.contains(&path));
    }

    #[test]
    fn test_clear_content() {
        let mut manager = TabManager::new();
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
        let mut m = TabManager::new();
        assert_eq!(m.scroll_to_header_id, None);
        m.scroll_to_header_id = Some("Intro".to_string());
        assert_eq!(m.scroll_to_header_id.as_deref(), Some("Intro"));
        m.scroll_to_header_id = None;
        assert_eq!(m.scroll_to_header_id, None);
    }
}
