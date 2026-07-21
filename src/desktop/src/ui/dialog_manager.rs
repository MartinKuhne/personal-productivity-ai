use std::path::PathBuf;

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

    // Rename dialog
    pub rename_dialog_open: bool,
    pub file_to_rename: Option<PathBuf>,
    pub rename_new_name: String,

    // Batch processing dialog
    pub batch_dialog_open: bool,
    pub batch_dialog_config: crate::batch::types::BatchDialogConfig,
    pub batch_handle: Option<crate::batch::BatchHandle>,
    pub batch_cancel_flag: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
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

            rename_dialog_open: false,
            file_to_rename: None,
            rename_new_name: String::new(),

            batch_dialog_open: false,
            batch_dialog_config: crate::batch::types::BatchDialogConfig::default(),
            batch_handle: None,
            batch_cancel_flag: None,
        }
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
        assert!(!dm.rename_dialog_open);
        assert!(dm.file_to_rename.is_none());
        assert!(!dm.batch_dialog_open);
    }
}
