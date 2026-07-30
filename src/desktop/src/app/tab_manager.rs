//! Open-document tab manager — adding, closing, switching tabs, and tracking loaded markdown content, YAML front matter, and TOC.

use std::collections::HashSet;
use std::path::PathBuf;

use crate::markdown::toc_entry::ToCEntry;

pub struct TabManager {
    pub loaded_path: Option<PathBuf>,
    pub current_yaml: Option<serde_yml::Value>,
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
        }
    }

    pub fn open_tab(&mut self, path: PathBuf) {
        if self.tabs_set.insert(path.clone()) {
            self.tabs.push(path);
        }
    }

    pub fn close_tab(&mut self, path: &PathBuf) {
        if self.tabs_set.remove(path) {
            self.tabs.retain(|p| p != path);
        }
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
