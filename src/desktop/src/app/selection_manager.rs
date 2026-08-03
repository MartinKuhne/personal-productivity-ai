//! File-tree selection state — single-file selection, multi-file selection, selected directory, and expanded directories.

use std::collections::HashSet;
use std::path::PathBuf;

pub struct SelectionManager {
    pub selected_file: Option<PathBuf>,
    pub selected_files: HashSet<PathBuf>,
    pub selected_dir: Option<PathBuf>,
    pub expanded_dirs: HashSet<PathBuf>,
    pub tree_dirty: bool,
}

impl Default for SelectionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SelectionManager {
    pub fn new() -> Self {
        Self {
            selected_file: None,
            selected_files: HashSet::new(),
            selected_dir: None,
            expanded_dirs: HashSet::new(),
            tree_dirty: true,
        }
    }

    pub fn select_file(&mut self, path: PathBuf) {
        self.selected_file = Some(path);
    }

    pub fn toggle_file(&mut self, path: PathBuf) {
        if self.selected_files.contains(&path) {
            self.selected_files.remove(&path);
        } else {
            self.selected_files.insert(path);
        }
    }

    pub fn select_dir(&mut self, path: PathBuf) {
        self.selected_dir = Some(path);
    }

    pub fn toggle_expanded(&mut self, path: PathBuf) {
        if self.expanded_dirs.contains(&path) {
            self.expanded_dirs.remove(&path);
        } else {
            self.expanded_dirs.insert(path);
        }
        self.tree_dirty = true;
    }

    pub fn clear(&mut self) {
        self.selected_file = None;
        self.selected_files.clear();
        self.selected_dir = None;
        self.expanded_dirs.clear();
    }

    pub fn is_selected(&self, path: &PathBuf) -> bool {
        self.selected_files.contains(path)
    }

    pub fn is_expanded(&self, path: &PathBuf) -> bool {
        self.expanded_dirs.contains(path)
    }

    pub fn selected_file(&self) -> Option<&PathBuf> {
        self.selected_file.as_ref()
    }

    pub fn selected_files(&self) -> &HashSet<PathBuf> {
        &self.selected_files
    }

    pub fn selected_dir(&self) -> Option<&PathBuf> {
        self.selected_dir.as_ref()
    }

    pub fn expanded_dirs(&self) -> &HashSet<PathBuf> {
        &self.expanded_dirs
    }

    pub fn selected_file_mut(&mut self) -> &mut Option<PathBuf> {
        &mut self.selected_file
    }

    pub fn selected_files_mut(&mut self) -> &mut HashSet<PathBuf> {
        &mut self.selected_files
    }

    pub fn selected_dir_mut(&mut self) -> &mut Option<PathBuf> {
        &mut self.selected_dir
    }

    pub fn expanded_dirs_mut(&mut self) -> &mut HashSet<PathBuf> {
        &mut self.expanded_dirs
    }

    /// Compute the file/directory context to hand to the LLM for a
    /// new agent prompt: `(active file, active directory, selected
    /// files)`.
    ///
    /// When no document tabs are open there is no working context,
    /// so every component is suppressed — the LLM is handed no file
    /// or directory context (AGENT-026). When at least one tab is
    /// open, the current selection (single file, directory, and any
    /// multi-selected files) is returned. Open tabs are the source
    /// of truth because closing all tabs must clear stale directory
    /// and multi-selection state left behind by tree navigation.
    pub fn agent_context(
        &self,
        open_tabs: &[PathBuf],
    ) -> (Option<PathBuf>, Option<PathBuf>, HashSet<PathBuf>) {
        if open_tabs.is_empty() {
            (None, None, HashSet::new())
        } else {
            (
                self.selected_file.clone(),
                self.selected_dir.clone(),
                self.selected_files.clone(),
            )
        }
    }

    pub fn tree_dirty(&self) -> bool {
        self.tree_dirty
    }

    pub fn tree_dirty_mut(&mut self) -> &mut bool {
        &mut self.tree_dirty
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_file() {
        let mut manager = SelectionManager::new();
        let path = PathBuf::from("test.md");
        manager.select_file(path.clone());
        assert_eq!(manager.selected_file(), Some(&path));
    }

    #[test]
    fn test_toggle_file_adds() {
        let mut manager = SelectionManager::new();
        let path = PathBuf::from("test.md");
        manager.toggle_file(path.clone());
        assert!(manager.is_selected(&path));
    }

    #[test]
    fn test_toggle_file_removes() {
        let mut manager = SelectionManager::new();
        let path = PathBuf::from("test.md");
        manager.toggle_file(path.clone());
        manager.toggle_file(path.clone());
        assert!(!manager.is_selected(&path));
    }

    #[test]
    fn test_toggle_expanded() {
        let mut manager = SelectionManager::new();
        let path = PathBuf::from("dir");
        manager.toggle_expanded(path.clone());
        assert!(manager.is_expanded(&path));
        manager.toggle_expanded(path.clone());
        assert!(!manager.is_expanded(&path));
    }

    #[test]
    fn test_clear() {
        let mut manager = SelectionManager::new();
        let file = PathBuf::from("test.md");
        let dir = PathBuf::from("dir");
        manager.select_file(file.clone());
        manager.toggle_file(file.clone());
        manager.select_dir(dir.clone());
        manager.toggle_expanded(dir.clone());
        manager.clear();
        assert!(manager.selected_file().is_none());
        assert!(manager.selected_files().is_empty());
        assert!(manager.selected_dir().is_none());
        assert!(manager.expanded_dirs().is_empty());
    }

    #[test]
    fn test_is_selected() {
        let mut manager = SelectionManager::new();
        let path = PathBuf::from("test.md");
        assert!(!manager.is_selected(&path));
        manager.toggle_file(path.clone());
        assert!(manager.is_selected(&path));
    }

    #[test]
    fn test_select_dir() {
        let mut manager = SelectionManager::new();
        let path = PathBuf::from("dir");
        manager.select_dir(path.clone());
        assert_eq!(manager.selected_dir(), Some(&path));
    }

    /// AGENT-026: closing all tabs must suppress every piece of
    /// file/directory context, even when stale tree-selection state
    /// (directory, multi-select) is still present.
    #[test]
    fn test_agent_context_suppressed_when_no_tabs() {
        let mut manager = SelectionManager::new();
        manager.select_file(PathBuf::from("a.md"));
        manager.select_dir(PathBuf::from("stale_dir"));
        manager.toggle_file(PathBuf::from("b.md"));

        let (file, dir, files) = manager.agent_context(&[]);
        assert!(file.is_none(), "no active file when tabs are closed");
        assert!(dir.is_none(), "no directory context when tabs are closed");
        assert!(files.is_empty(), "no selected files when tabs are closed");
    }

    /// With at least one tab open, the selection is handed over
    /// unchanged (AGENT-013/AGENT-014).
    #[test]
    fn test_agent_context_returned_when_tabs_open() {
        let mut manager = SelectionManager::new();
        let file = PathBuf::from("a.md");
        let dir = PathBuf::from("lib");
        let multi = PathBuf::from("b.md");
        manager.select_file(file.clone());
        manager.select_dir(dir.clone());
        manager.toggle_file(multi.clone());

        let (got_file, got_dir, got_files) = manager.agent_context(std::slice::from_ref(&file));
        assert_eq!(got_file, Some(file));
        assert_eq!(got_dir, Some(dir));
        assert!(got_files.contains(&multi));
    }
}
