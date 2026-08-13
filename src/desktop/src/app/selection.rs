//! File-tree selection state — single-file selection, multi-file selection, selected directory, and expanded directories.

use std::collections::HashSet;
use std::path::PathBuf;

pub struct FileSelection {
    pub selected_file: Option<PathBuf>,
    pub selected_files: HashSet<PathBuf>,
    pub selected_dir: Option<PathBuf>,
    pub expanded_dirs: HashSet<PathBuf>,
    pub tree_dirty: bool,
}

impl Default for FileSelection {
    fn default() -> Self {
        Self::new()
    }
}

impl FileSelection {
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
    /// File-level context (the active file and the multi-selected
    /// files) is only handed over when at least one document tab is
    /// open (AGENT-026). The directory context is always derived via
    /// [`Self::prompt_dir`]: the parent of the file selected in the
    /// tabs panel when one is selected, otherwise the last directory
    /// the user selected from the directory tree (AGENT-014).
    pub fn agent_context(
        &self,
        open_tabs: &[PathBuf],
    ) -> (Option<PathBuf>, Option<PathBuf>, HashSet<PathBuf>) {
        let active_file = if open_tabs.is_empty() {
            None
        } else {
            self.selected_file.clone()
        };
        let selected_files = if open_tabs.is_empty() {
            HashSet::new()
        } else {
            self.selected_files.clone()
        };
        (active_file, self.prompt_dir(open_tabs), selected_files)
    }

    /// Derive the directory context for the LLM prompt and the
    /// bottom-panel prompt prefix: the parent of the file selected in
    /// the tabs panel when one is active, otherwise the last
    /// directory the user selected from the directory tree.
    ///
    /// A file counts as "selected in the tabs panel" only if it is
    /// among `open_tabs`; a tree-selected file that was never opened
    /// as a tab (e.g. a shift-click multi-select) does not qualify,
    /// so it falls through to the tree-selected directory.
    ///
    /// This is the single source of truth shared by the agent-session
    /// dispatch (`agent_context`/`orchestrator.rs`) and the bottom
    /// panel's prompt prefix (`ui/panels/bottom.rs`), so the
    /// displayed prefix always matches the directory handed to the
    /// LLM (AGENT-015).
    pub fn prompt_dir(&self, open_tabs: &[PathBuf]) -> Option<PathBuf> {
        if let Some(file) = self.selected_file.as_ref()
            && open_tabs.contains(file)
        {
            return file.parent().map(|p| p.to_path_buf());
        }
        self.selected_dir.clone()
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
        let mut manager = FileSelection::new();
        let path = PathBuf::from("test.md");
        manager.select_file(path.clone());
        assert_eq!(manager.selected_file(), Some(&path));
    }

    #[test]
    fn test_toggle_file_adds() {
        let mut manager = FileSelection::new();
        let path = PathBuf::from("test.md");
        manager.toggle_file(path.clone());
        assert!(manager.is_selected(&path));
    }

    #[test]
    fn test_toggle_file_removes() {
        let mut manager = FileSelection::new();
        let path = PathBuf::from("test.md");
        manager.toggle_file(path.clone());
        manager.toggle_file(path.clone());
        assert!(!manager.is_selected(&path));
    }

    #[test]
    fn test_toggle_expanded() {
        let mut manager = FileSelection::new();
        let path = PathBuf::from("dir");
        manager.toggle_expanded(path.clone());
        assert!(manager.is_expanded(&path));
        manager.toggle_expanded(path.clone());
        assert!(!manager.is_expanded(&path));
    }

    #[test]
    fn test_clear() {
        let mut manager = FileSelection::new();
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
        let mut manager = FileSelection::new();
        let path = PathBuf::from("test.md");
        assert!(!manager.is_selected(&path));
        manager.toggle_file(path.clone());
        assert!(manager.is_selected(&path));
    }

    #[test]
    fn test_select_dir() {
        let mut manager = FileSelection::new();
        let path = PathBuf::from("dir");
        manager.select_dir(path.clone());
        assert_eq!(manager.selected_dir(), Some(&path));
    }

    /// AGENT-026: closing all tabs must suppress the file-level
    /// context (active file and selected files), even when stale
    /// tree-selection state is still present. The directory context
    /// is still handed over: when no file is selected in the tabs
    /// panel, the last tree-selected directory is used (AGENT-014).
    #[test]
    fn test_agent_context_suppresses_file_context_when_no_tabs() {
        let mut manager = FileSelection::new();
        manager.select_file(PathBuf::from("a.md"));
        manager.select_dir(PathBuf::from("stale_dir"));
        manager.toggle_file(PathBuf::from("b.md"));

        let (file, dir, files) = manager.agent_context(&[]);
        assert!(file.is_none(), "no active file when tabs are closed");
        assert_eq!(
            dir,
            Some(PathBuf::from("stale_dir")),
            "directory context falls back to the last tree-selected directory"
        );
        assert!(files.is_empty(), "no selected files when tabs are closed");
    }

    /// With at least one tab open, the active file and selected files
    /// are handed over unchanged, and the directory context is the
    /// active tab file's parent (AGENT-013/AGENT-014).
    #[test]
    fn test_agent_context_returned_when_tabs_open() {
        let mut manager = FileSelection::new();
        let file = PathBuf::from("C:/notes/folder/a.md");
        let multi = PathBuf::from("C:/notes/other/b.md");
        manager.select_file(file.clone());
        manager.select_dir(PathBuf::from("stale_tree_dir"));
        manager.toggle_file(multi.clone());

        let (got_file, got_dir, got_files) = manager.agent_context(std::slice::from_ref(&file));
        assert_eq!(got_file, Some(file));
        assert_eq!(
            got_dir,
            Some(PathBuf::from("C:/notes/folder")),
            "with an active tab, the directory context is the tab file's parent"
        );
        assert!(got_files.contains(&multi));
    }

    /// AGENT-014: when a file is selected in the tabs panel, the
    /// directory context is that file's parent, not the last
    /// tree-selected directory.
    #[test]
    fn test_prompt_dir_uses_active_tab_file_parent() {
        let mut manager = FileSelection::new();
        manager.select_dir(PathBuf::from("stale_tree_dir"));
        let tab = PathBuf::from("C:/notes/folder/file.md");
        manager.select_file(tab.clone());
        let expected = Some(PathBuf::from("C:/notes/folder"));
        assert_eq!(manager.prompt_dir(std::slice::from_ref(&tab)), expected);
    }

    /// AGENT-014: a tree-selected file that was never opened as a tab
    /// (e.g. a shift-click multi-select) does not change the
    /// directory context; the last tree-selected directory is used.
    #[test]
    fn test_prompt_dir_ignores_non_tab_selected_file() {
        let mut manager = FileSelection::new();
        manager.select_dir(PathBuf::from("tree_dir"));
        manager.select_file(PathBuf::from("C:/other/selected.md"));
        assert_eq!(
            manager.prompt_dir(&[]),
            Some(PathBuf::from("tree_dir")),
            "a selected file not open in a tab must not override the tree-selected directory"
        );
    }

    /// AGENT-014: with no active tab file, the directory context is
    /// the last directory selected from the directory tree.
    #[test]
    fn test_prompt_dir_falls_back_to_tree_selected_dir() {
        let mut manager = FileSelection::new();
        manager.select_dir(PathBuf::from("tree_dir"));
        assert_eq!(manager.prompt_dir(&[]), Some(PathBuf::from("tree_dir")));
    }

    /// AGENT-014: when neither a tab file nor a tree-selected
    /// directory exists, the directory context is `None`.
    #[test]
    fn test_prompt_dir_none_when_no_context() {
        let manager = FileSelection::new();
        assert_eq!(manager.prompt_dir(&[]), None);
    }
}
