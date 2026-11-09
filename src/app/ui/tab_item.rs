//! Tab item enum for representing both file and agent tabs in the tab bar.
//!
//! This enum is the foundation of the tab-based UI architecture, replacing the
//! previous PathBuf-only tab system with a unified type that can represent
//! both file tabs and the agent session tab.
//!
//! Unit tests live in the sibling `tab_item_tests.rs` sidecar.

use std::hash::Hash;
use std::path::PathBuf;

/// Represents a tab in the tab bar - either a file tab or an agent session tab.
///
/// This is the central type that enables the tab-based architecture by providing
/// a unified representation for both file and agent tabs. Each variant carries
/// the necessary data to identify and operate on the tab.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
                .map(|name| name.rsplit(['/', '\\']).next().unwrap_or(name))
                .unwrap_or("")
                .to_string(),
            TabItem::Agent => crate::ui::strings::AGENT_SESSION_HEADER.to_string(),
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
}

#[cfg(test)]
#[path = "tab_item_tests.rs"]
mod tests;
