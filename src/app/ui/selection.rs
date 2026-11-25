//! File-tree selection state - single-file selection, multi-file selection, selected directory, and expanded directories.
//!
//! Unit tests live in the sibling `selection_tests.rs` sidecar.

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
#[path = "selection_tests.rs"]
mod tests;
