//! Open-document tab manager — adding, closing, switching tabs, and tracking loaded markdown content, YAML front matter, and TOC.

use crate::ui::app::ToCEntry;
use eframe::egui;
use std::path::PathBuf;

pub struct TabManager {
    pub loaded_path: Option<PathBuf>,
    pub current_yaml: Option<serde_yaml::Value>,
    pub current_markdown: String,
    pub tabs: Vec<PathBuf>,
    pub toc: Vec<ToCEntry>,
    pub scroll_to_header_id: Option<egui::Id>,
}

impl TabManager {
    pub fn new() -> Self {
        Self {
            loaded_path: None,
            current_yaml: None,
            current_markdown: String::new(),
            tabs: Vec::new(),
            toc: Vec::new(),
            scroll_to_header_id: None,
        }
    }

    pub fn open_tab(&mut self, path: PathBuf) {
        if !self.tabs.contains(&path) {
            self.tabs.push(path);
        }
    }

    pub fn close_tab(&mut self, path: &PathBuf) {
        self.tabs.retain(|p| p != path);
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
}
