//! Tests for [`TabItem`] (`ui::tab_item`).

use super::*;

#[test]
fn test_tab_item_agent_helpers() {
    let agent_tab = TabItem::Agent;

    // Test is_agent helper
    assert!(agent_tab.is_agent());

    // Test label helper
    assert_eq!(agent_tab.label(), crate::ui::strings::AGENT_SESSION_HEADER);

    // Test file_path helper (should return None for Agent)
    assert!(agent_tab.file_path().is_none());
}

#[test]
fn test_tab_item_file_helpers() {
    let file_tab = TabItem::File(PathBuf::from("notes.md"));
    let empty_file_tab = TabItem::File(PathBuf::from(""));

    // Test is_agent helper (should return false for File)
    assert!(!file_tab.is_agent());
    assert!(!empty_file_tab.is_agent());

    // Test label helper (should return file name)
    assert_eq!(file_tab.label(), "notes.md");
    assert_eq!(empty_file_tab.label(), "");

    // Test file_path helper (should return Some(path) for File)
    assert_eq!(file_tab.file_path(), Some(PathBuf::from("notes.md")));
    assert_eq!(empty_file_tab.file_path(), Some(PathBuf::from("")));
}

#[test]
fn test_tab_item_file_path_edge_cases() {
    // Test with PathBuf from different OS path formats
    let unix_path = TabItem::File(PathBuf::from("/home/user/docs/note.md"));
    assert_eq!(unix_path.label(), "note.md");

    let windows_path = TabItem::File(PathBuf::from("C:\\Users\\user\\docs\\note.md"));
    assert_eq!(windows_path.label(), "note.md");

    let path_without_name = TabItem::File(PathBuf::from("/"));
    assert_eq!(path_without_name.label(), "");
}
