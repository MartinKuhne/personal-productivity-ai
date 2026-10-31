//! Tests for [`FileSelection`] (`ui::selection`).

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
