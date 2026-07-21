use std::collections::HashSet;
use std::path::PathBuf;

pub struct SelectionManager {
    pub selected_file: Option<PathBuf>,
    pub selected_files: HashSet<PathBuf>,
    pub selected_dir: Option<PathBuf>,
    pub expanded_dirs: HashSet<PathBuf>,
}

impl SelectionManager {
    pub fn new() -> Self {
        Self {
            selected_file: None,
            selected_files: HashSet::new(),
            selected_dir: None,
            expanded_dirs: HashSet::new(),
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
}
