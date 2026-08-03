//! Tab item enum for representing both file and agent tabs in the tab bar.
//!
//! This enum is the foundation of the tab-based UI architecture, replacing the
//! previous PathBuf-only tab system with a unified type that can represent
//! both file tabs and the agent session tab.

use std::hash::Hash;
use std::path::PathBuf;

/// Represents a tab in the tab bar - either a file tab or an agent session tab.
///
/// This is the central type that enables the tab-based architecture by providing
/// a unified representation for both file and agent tabs. Each variant carries
/// the necessary data to identify and operate on the tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabItem {
    /// A file tab that contains a markdown document.
    File(PathBuf),
    /// The agent session tab that contains the FastMD Agent Session.
    Agent,
}

impl TabItem {
    /// Returns a user-friendly label for display in the tab bar.
    ///
    /// For file tabs, returns the file name. For the agent tab,
    /// returns the emoji and name "🤖 FastMD Agent Session".
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::PathBuf;
    /// use crate::ui::TabItem;
    ///
    /// let file_tab = TabItem::File(PathBuf::from("notes.md"));
    /// assert_eq!(file_tab.label(), "notes.md");
    ///
    /// let agent_tab = TabItem::Agent;
    /// assert_eq!(agent_tab.label(), "🤖 FastMD Agent Session");
    /// ```
    pub fn label(&self) -> String {
        match self {
            TabItem::File(path) => path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string(),
            TabItem::Agent => "🤖 FastMD Agent Session".to_string(),
        }
    }

    /// Checks if this tab is the agent session tab.
    ///
    /// Returns true if the tab is TabItem::Agent, false for File tabs.
    ///
    /// # Examples
    ///
    /// ```
    /// use crate::ui::TabItem;
    ///
    /// assert!(TabItem::Agent.is_agent());
    /// assert!(!TabItem::File(PathBuf::from("test.md")).is_agent());
    /// ```
    pub fn is_agent(&self) -> bool {
        matches!(self, TabItem::Agent)
    }

    /// Returns the file path for this tab if it's a file tab.
    ///
    /// Returns Some(PathBuf) for TabItem::File variants and None for
    /// TabItem::Agent variants since the agent tab has no associated file.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::PathBuf;
    /// use crate::ui::TabItem;
    ///
    /// assert!(TabItem::File(PathBuf::from("test.md")).file_path().is_some());
    /// assert!(TabItem::Agent.file_path().is_none());
    /// ```
    pub fn file_path(&self) -> Option<PathBuf> {
        match self {
            TabItem::File(path) => Some(path.clone()),
            TabItem::Agent => None,
        }
    }

    /// Computes a hash of the tab for use in caching.
    ///
    /// This is used by TabManager to compute the hash of the tabs
    /// vector to detect changes and invalidate the tab strip cache.
    #[allow(clippy::never_loop)]
    pub(crate) fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            TabItem::File(path) => path.hash(state),
            TabItem::Agent => {
                // Hash a constant to ensure all agent tabs hash the same
                state.write_u8(b'A');
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tab_item_agent_helpers() {
        let agent_tab = TabItem::Agent;

        // Test is_agent helper
        assert!(agent_tab.is_agent());

        // Test label helper
        assert_eq!(agent_tab.label(), "🤖 FastMD Agent Session");

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

        let path_without_name = TabItem::File(PathBuf::from("/home/user/"));
        assert_eq!(path_without_name.label(), "");
    }
}
