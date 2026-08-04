//! Centralised modal-dialog state — open/closed flags and temporary inputs for every dialog in the application.

use std::collections::HashMap;
use std::path::PathBuf;

/// Tracks the OAuth 2.1 authorization-flow state for a single MCP
/// server. Rendered in the Tools dialog to prevent double-spawning
/// and to show an in-progress label on the row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OAuthFlowStatus {
    /// A background thread is running the flow.
    InProgress,
}

/// Manager for all modal dialogs in the application.
///
/// Responsibilities:
/// - Owns all dialog state (open/closed flags, temporary inputs)
/// - Provides methods to show each modal type
/// - Handles dialog-specific logic and callbacks
///
/// This extraction reduces `FastMdApp` by ~10 fields and consolidates
/// modal interactions into a single, cohesive module.
pub struct DialogManager {
    // Move dialog
    pub move_dialog_open: bool,
    pub file_to_move: Option<PathBuf>,
    pub selected_move_folder: Option<PathBuf>,

    // Create directory dialog
    pub create_dir_dialog_open: bool,
    pub create_dir_parent: Option<PathBuf>,
    pub create_dir_name: String,

    // Create document dialog
    pub create_document_dialog_open: bool,
    pub create_document_parent: Option<PathBuf>,
    pub create_document_name: String,

    // Rename dialog
    pub rename_dialog_open: bool,
    pub file_to_rename: Option<PathBuf>,
    pub rename_new_name: String,

    // Batch processing dialog
    pub batch_dialog_open: bool,
    pub batch_dialog_config: crate::app::batch::types::BatchDialogConfig,
    pub batch_handle: Option<crate::app::batch::BatchHandle>,
    pub batch_cancel_flag: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,

    // Tools dialog (UI-051)
    pub tools_dialog_open: bool,

    // OAuth in-flight state (MCP-021)
    /// Per-server OAuth flow status. Present (InProgress) while the
    /// background thread running `McpClientManager::authenticate` is
    /// alive. Removed when the thread sends `McpAuthEvent::Completed`.
    pub oauth_status: HashMap<String, OAuthFlowStatus>,
}

impl DialogManager {
    /// Create a new, empty dialog manager.
    pub fn new() -> Self {
        Self {
            move_dialog_open: false,
            file_to_move: None,
            selected_move_folder: None,

            create_dir_dialog_open: false,
            create_dir_parent: None,
            create_dir_name: String::new(),

            create_document_dialog_open: false,
            create_document_parent: None,
            create_document_name: String::new(),

            rename_dialog_open: false,
            file_to_rename: None,
            rename_new_name: String::new(),

            batch_dialog_open: false,
            batch_dialog_config: crate::app::batch::types::BatchDialogConfig::default(),
            batch_handle: None,
            batch_cancel_flag: None,

            tools_dialog_open: false,

            oauth_status: HashMap::new(),
        }
    }

    /// Mark the OAuth flow for `server` as in-progress. Called just
    /// before the background thread is spawned.
    pub fn set_oauth_in_progress(&mut self, server: &str) {
        self.oauth_status
            .insert(server.to_owned(), OAuthFlowStatus::InProgress);
    }

    /// Clear the in-progress flag for `server`. Called when the
    /// background thread sends `McpAuthEvent::Completed`.
    pub fn set_oauth_idle(&mut self, server: &str) {
        self.oauth_status.remove(server);
    }

    /// Returns `true` while the OAuth background thread for `server`
    /// is still running.
    pub fn is_oauth_in_progress(&self, server: &str) -> bool {
        matches!(
            self.oauth_status.get(server),
            Some(OAuthFlowStatus::InProgress)
        )
    }
}

impl Default for DialogManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_dialog_manager_is_empty() {
        let dm = DialogManager::new();
        assert!(!dm.move_dialog_open);
        assert!(dm.file_to_move.is_none());
        assert!(!dm.create_dir_dialog_open);
        assert!(dm.create_dir_parent.is_none());
        assert!(!dm.create_document_dialog_open);
        assert!(dm.create_document_parent.is_none());
        assert!(dm.create_document_name.is_empty());
        assert!(!dm.rename_dialog_open);
        assert!(dm.file_to_rename.is_none());
        assert!(!dm.batch_dialog_open);
    }

    #[test]
    fn oauth_status_starts_idle() {
        let dm = DialogManager::new();
        assert!(!dm.is_oauth_in_progress("my-server"));
        assert!(dm.oauth_status.is_empty());
    }

    #[test]
    fn set_in_progress_and_back_to_idle() {
        let mut dm = DialogManager::new();
        dm.set_oauth_in_progress("srv");
        assert!(dm.is_oauth_in_progress("srv"));
        assert!(!dm.is_oauth_in_progress("other"));
        dm.set_oauth_idle("srv");
        assert!(!dm.is_oauth_in_progress("srv"));
        assert!(dm.oauth_status.is_empty());
    }
}
