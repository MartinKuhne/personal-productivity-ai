//! Tests for `tree/render.rs`.

use crate::bus::events::user_command::UserCommand;
use crate::ui::tree::context::TreeNodeContext;
use crate::ui::tree::flatten::FlatRow;
use std::path::PathBuf;

fn make_ctx() -> TreeNodeContext {
    let selection = crate::ui::FileSelection::default();
    let tabs = crate::ui::Tabs::default();
    let layout = crate::ui::panel_layout::PanelLayout::default();
    let content_libraries = vec![];
    let file_event_bus = crate::bus::core::Bus::new();
    let pdf_backing_tracker = crate::agent::session::PdfBackingTracker::default();
    let user_command_bus = crate::bus::core::Bus::new();
    TreeNodeContext::from_app_state(
        &selection,
        &tabs,
        &layout,
        &content_libraries,
        None,
        file_event_bus,
        true,
        eframe::egui::Modifiers::default(),
        pdf_backing_tracker,
        user_command_bus,
    )
}

#[test]
fn test_render_file_row_click_publishes_select_file() {
    let row = FlatRow {
        depth: 0,
        name: "note.md".to_string(),
        path: PathBuf::from("/tmp/note.md"),
        is_dir: false,
        is_expanded: false,
    };
    let mut ctx = make_ctx();
    let reader = ctx.user_command_bus.subscribe();
    // apply_file_row_click is the pure helper used by render_flat_row
    let cmd = crate::ui::tree::handlers::apply_file_row_click(&mut ctx, &row);
    ctx.user_command_bus.publish(cmd.clone());
    let received = reader.try_recv_exposing_lag().unwrap();
    assert_eq!(received, cmd);
    assert_eq!(
        received,
        UserCommand::SelectFile {
            path: PathBuf::from("/tmp/note.md"),
            multi: false
        }
    );
}

#[test]
fn test_render_dir_row_click_publishes_select_directory() {
    let row = FlatRow {
        depth: 0,
        name: "docs".to_string(),
        path: PathBuf::from("/tmp/docs"),
        is_dir: true,
        is_expanded: false,
    };
    let mut ctx = make_ctx();
    let reader = ctx.user_command_bus.subscribe();
    let cmd = crate::ui::tree::handlers::apply_directory_row_click(&mut ctx, &row);
    ctx.user_command_bus.publish(cmd.clone());
    let received = reader.try_recv_exposing_lag().unwrap();
    assert_eq!(
        received,
        UserCommand::SelectDirectory {
            path: PathBuf::from("/tmp/docs"),
            toggle_expand: true
        }
    );
}

#[test]
fn test_render_context_menus_publish_correct_commands() {
    let ctx = make_ctx();
    let reader = ctx.user_command_bus.subscribe();
    // Simulate dir context menu actions directly publishing via bus
    ctx.user_command_bus
        .publish(UserCommand::CopyPath(PathBuf::from("/tmp/docs")));
    ctx.user_command_bus
        .publish(UserCommand::ShowInExplorer(PathBuf::from("/tmp/docs")));
    ctx.user_command_bus
        .publish(UserCommand::Delete(PathBuf::from("/tmp/note.md")));
    assert_eq!(
        reader.try_recv_exposing_lag().unwrap(),
        UserCommand::CopyPath(PathBuf::from("/tmp/docs"))
    );
    assert_eq!(
        reader.try_recv_exposing_lag().unwrap(),
        UserCommand::ShowInExplorer(PathBuf::from("/tmp/docs"))
    );
    assert_eq!(
        reader.try_recv_exposing_lag().unwrap(),
        UserCommand::Delete(PathBuf::from("/tmp/note.md"))
    );
}

#[test]
fn test_multi_select_delete_publishes_per_file() {
    let files = vec![PathBuf::from("/tmp/a.md"), PathBuf::from("/tmp/b.md")];
    let mut ctx = make_ctx();
    // Setup multi-selection
    for f in &files {
        ctx.selected_files.insert(f.clone());
    }
    let reader = ctx.user_command_bus.subscribe();
    // Mimic show_multi_select_file_context_menu delete branch
    for file in files.clone() {
        ctx.user_command_bus.publish(UserCommand::Delete(file));
    }
    let mut received = Vec::new();
    while let Ok(cmd) = reader.try_recv_exposing_lag() {
        received.push(cmd);
    }
    assert_eq!(received.len(), 2);
    assert!(received.contains(&UserCommand::Delete(PathBuf::from("/tmp/a.md"))));
    assert!(received.contains(&UserCommand::Delete(PathBuf::from("/tmp/b.md"))));
}
