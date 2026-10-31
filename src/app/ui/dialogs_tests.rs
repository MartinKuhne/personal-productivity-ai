//! Tests for [`Dialogs`] and [`OAuthFlowStatus`] (`ui::dialogs`).

use super::*;

#[test]
fn test_new_dialogs_is_empty() {
    let dm = Dialogs::new();
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
    assert!(!dm.about_dialog_open);
}

#[test]
fn oauth_status_starts_idle() {
    let dm = Dialogs::new();
    assert!(!dm.is_oauth_in_progress("my-server"));
    assert!(dm.oauth_status.is_empty());
}

#[test]
fn set_in_progress_and_back_to_idle() {
    let mut dm = Dialogs::new();
    dm.set_oauth_in_progress("srv");
    assert!(dm.is_oauth_in_progress("srv"));
    assert!(!dm.is_oauth_in_progress("other"));
    dm.set_oauth_idle("srv");
    assert!(!dm.is_oauth_in_progress("srv"));
    assert!(dm.oauth_status.is_empty());
}
