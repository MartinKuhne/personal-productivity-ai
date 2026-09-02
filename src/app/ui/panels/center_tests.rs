//! Tests for `panels/center.rs`.

use super::*;
use crate::bus::events::user_command::UserCommand;
use std::path::PathBuf;

#[test]
fn test_apply_tab_close_click() {
    assert_eq!(apply_tab_close_click(1), UserCommand::CloseTab(1));
}

#[test]
fn test_apply_tab_close_others_click() {
    assert_eq!(
        apply_tab_close_others_click(2),
        UserCommand::CloseOtherTabs(2)
    );
}

#[test]
fn test_apply_tab_close_all_click() {
    assert_eq!(apply_tab_close_all_click(), UserCommand::CloseAllTabs);
}

#[test]
fn test_tab_select_via_bus_sets_selection() {
    let mut app = crate::ui::app::FastMdApp::empty_state(crate::config::AppConfig::default());
    let path = PathBuf::from("/tmp/doc.md");
    // Create a tab entry so selection has context
    app.orchestrator.tabs.tabs.push(path.clone());
    let reader = app.orchestrator.user_command_bus.subscribe();
    // Simulate tab label click publishing SelectFile
    app.orchestrator
        .user_command_bus
        .publish(UserCommand::SelectFile {
            path: path.clone(),
            multi: false,
        });
    // Drain bus and apply
    let cmd = reader.try_recv_exposing_lag().unwrap();
    app.orchestrator.apply_user_command(cmd);
    assert_eq!(app.orchestrator.selection.selected_file, Some(path));
    assert_eq!(app.orchestrator.tabs.loaded_path, None);
}

#[test]
fn test_tab_copy_path_published_via_bus() {
    let app = crate::ui::app::FastMdApp::empty_state(crate::config::AppConfig::default());
    let reader = app.orchestrator.user_command_bus.subscribe();
    let path = PathBuf::from("/tmp/copied.md");
    app.orchestrator
        .user_command_bus
        .publish(UserCommand::CopyPath(path.clone()));
    assert_eq!(
        reader.try_recv_exposing_lag().unwrap(),
        UserCommand::CopyPath(path)
    );
}

#[test]
fn test_tab_show_in_explorer_published_via_bus() {
    let app = crate::ui::app::FastMdApp::empty_state(crate::config::AppConfig::default());
    let reader = app.orchestrator.user_command_bus.subscribe();
    let path = PathBuf::from("/tmp/show.md");
    app.orchestrator
        .user_command_bus
        .publish(UserCommand::ShowInExplorer(path.clone()));
    assert_eq!(
        reader.try_recv_exposing_lag().unwrap(),
        UserCommand::ShowInExplorer(path)
    );
}

#[test]
fn test_tab_open_in_editor_published_via_bus() {
    let app = crate::ui::app::FastMdApp::empty_state(crate::config::AppConfig::default());
    let reader = app.orchestrator.user_command_bus.subscribe();
    let path = PathBuf::from("/tmp/open.md");
    app.orchestrator
        .user_command_bus
        .publish(UserCommand::OpenInEditor(path.clone()));
    assert_eq!(
        reader.try_recv_exposing_lag().unwrap(),
        UserCommand::OpenInEditor(path)
    );
}
