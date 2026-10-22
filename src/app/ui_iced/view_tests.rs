//! Unit tests for the iced view (`src/app/ui_iced/view.rs`).
//!
//! iced elements are not inspectable without a renderer, so the view is
//! tested two ways:
//!
//! * **smoke** — the full tree builds for every state that changes its
//!   shape (no file / file selected / every overlay mounted);
//! * **selector** — [`AppState::mounted_layers`] is the pure "what is
//!   mounted, in what order" decision the host renders, so asserting on it
//!   asserts the z-order without needing a renderer.

use super::*;
use crate::bus::core::Bus;
use crate::bus::events::config::ConfigArrived;
use crate::config::{AppConfig, InMemoryConfigStorage};
use crate::ui_iced::app::{ContextMenu, OverlayLayer, build_state};
use crate::ui_iced::dialogs::DialogKind;
use crate::ui_iced::editor::EditorViewModel;
use crate::ui_iced::menu::MenuItem;
use crate::ui_iced::persisted::InMemoryUiStateStorage;
use std::path::PathBuf;
use std::sync::Arc;

/// A fresh `AppState` over in-memory storage.
fn fresh_state() -> AppState {
    build_state(
        AppConfig::default(),
        &Bus::<ConfigArrived>::new(),
        Arc::new(InMemoryConfigStorage::new()),
        Arc::new(InMemoryUiStateStorage::new()),
        crate::ui::PersistedUiState::default(),
    )
}

/// Opens the background-process log window the way its `UserCommand` does.
fn show_logs(state: &mut AppState) {
    if let Ok(mut manager) = state.app.orchestrator.background_manager.lock() {
        manager.show_background_logs = true;
    }
}

/// Opens the agent debug window the way its `UserCommand` does.
fn show_debug(state: &mut AppState) {
    state.app.orchestrator.agent_panel_state.show_debug_window = true;
}

#[test]
fn test_band_spacing_matches_task_contract() {
    assert_eq!(BAND_SPACING_PIXELS, 8.0);
}

#[test]
fn test_nothing_is_mounted_on_a_fresh_state() {
    let state = fresh_state();
    assert_eq!(
        state.mounted_layers(),
        Vec::<OverlayLayer>::new(),
        "a booted app shows only the three bands"
    );
}

#[test]
fn test_context_menu_mounts_only_when_it_has_rows() {
    let mut state = fresh_state();

    state.overlay.context_menu = Some(ContextMenu::new(iced::Point::new(10.0, 20.0), Vec::new()));
    assert_eq!(
        state.mounted_layers(),
        Vec::<OverlayLayer>::new(),
        "an empty menu must not mount a click-blocking layer"
    );

    state.overlay.context_menu = Some(ContextMenu::new(
        iced::Point::new(10.0, 20.0),
        vec![MenuItem::action(
            "Open",
            Message::Tree(TreeMessage::ContextMenuClosed),
        )],
    ));
    assert_eq!(state.mounted_layers(), vec![OverlayLayer::ContextMenu]);
}

#[test]
fn test_editor_mounts_while_a_buffer_is_open() {
    let mut state = fresh_state();
    state.overlay.editor = Some(EditorViewModel::open("/notes/a.md", "# Title\n"));
    assert_eq!(state.mounted_layers(), vec![OverlayLayer::Editor]);
}

#[test]
fn test_small_dialog_mounts_when_open() {
    let mut state = fresh_state();
    state.overlay.dialog.open(DialogKind::CreateDirectory {
        parent: PathBuf::from("/notes"),
    });
    assert_eq!(state.mounted_layers(), vec![OverlayLayer::Dialog]);
}

#[test]
fn test_log_and_debug_windows_mount_from_their_domain_flags() {
    let mut state = fresh_state();
    show_logs(&mut state);
    state.rebuild_view_models();
    assert_eq!(state.mounted_layers(), vec![OverlayLayer::Logs]);

    let mut state = fresh_state();
    show_debug(&mut state);
    state.rebuild_view_models();
    assert_eq!(state.mounted_layers(), vec![OverlayLayer::AgentDebug]);
}

#[test]
fn test_tools_about_and_attributions_mount_individually() {
    let mut state = fresh_state();
    state.overlay.tools.open();
    assert_eq!(state.mounted_layers(), vec![OverlayLayer::Tools]);

    let mut state = fresh_state();
    state.overlay.about.open_about();
    assert_eq!(state.mounted_layers(), vec![OverlayLayer::About]);

    let mut state = fresh_state();
    state.overlay.about.open_attributions();
    assert_eq!(
        state.mounted_layers(),
        vec![OverlayLayer::Attributions],
        "the credits window replaces About rather than stacking beside it"
    );
}

#[test]
fn test_batch_dialog_is_the_topmost_layer() {
    let mut state = fresh_state();
    state.overlay.batch.open(state.models.batch.clone());
    show_logs(&mut state);
    show_debug(&mut state);
    state.overlay.tools.open();
    state.overlay.about.open_about();
    state.overlay.editor = Some(EditorViewModel::open("/notes/a.md", "body"));
    state.overlay.dialog.open(DialogKind::DeleteConfirm {
        path: PathBuf::from("/notes/a.md"),
    });
    state.overlay.context_menu = Some(ContextMenu::new(
        iced::Point::new(0.0, 0.0),
        vec![MenuItem::action(
            "Row",
            Message::Tree(TreeMessage::ContextMenuClosed),
        )],
    ));
    state.rebuild_view_models();

    assert_eq!(
        state.mounted_layers(),
        vec![
            OverlayLayer::ContextMenu,
            OverlayLayer::Editor,
            OverlayLayer::Dialog,
            OverlayLayer::Logs,
            OverlayLayer::AgentDebug,
            OverlayLayer::Tools,
            OverlayLayer::About,
            OverlayLayer::Batch,
        ],
        "z-order matches the intended draw order"
    );
}

#[test]
fn test_view_builds_without_selected_file() {
    let state = fresh_state();
    let _element: iced::Element<'_, Message> = view(&state);
}

#[test]
fn test_view_builds_with_selected_file() {
    let mut state = fresh_state();
    state.app.selection_mut().selected_file = Some(PathBuf::from("/notes/a.md"));
    let _element: iced::Element<'_, Message> = view(&state);
}

#[test]
fn test_view_builds_with_every_overlay_mounted() {
    let mut state = fresh_state();
    state.overlay.batch.open(state.models.batch.clone());
    show_logs(&mut state);
    show_debug(&mut state);
    state.overlay.tools.open();
    state.overlay.about.open_attributions();
    state.overlay.editor = Some(EditorViewModel::open("/notes/a.md", "# Title\n"));
    state.overlay.dialog.open(DialogKind::Rename {
        path: PathBuf::from("/notes/a.md"),
    });
    state.overlay.context_menu = Some(ContextMenu::new(
        iced::Point::new(4.0, 4.0),
        vec![MenuItem::action(
            "Row",
            Message::Tree(TreeMessage::ContextMenuClosed),
        )],
    ));
    state.rebuild_view_models();

    let _element: iced::Element<'_, Message> = view(&state);
}

#[test]
fn test_view_builds_while_the_agent_session_owns_the_center_pane() {
    let mut state = fresh_state();
    state.app.orchestrator.agent_panel_state.show_results = true;
    state.rebuild_view_models();
    let _element: iced::Element<'_, Message> = view(&state);
}

#[test]
fn test_document_width_never_collapses_below_the_floor() {
    let mut state = fresh_state();
    state.viewport = iced::Size::new(100.0, 100.0);
    assert!(
        document_width(&state) >= MIN_DOCUMENT_WIDTH_PIXELS,
        "a tiny viewport must not hand the table solver a non-positive width"
    );
}

#[test]
fn test_document_width_excludes_both_side_panels_when_the_toc_shows() {
    let mut state = fresh_state();
    state.viewport = iced::Size::new(1000.0, 700.0);
    state.app.layout.set_width(Some(300.0));
    state.app.layout.set_right_width(Some(200.0));
    state.app.orchestrator.tabs.toc = vec![crate::markdown::ToCEntry::new("Intro", 1, "Intro")];
    state.app.selection_mut().selected_file = Some(PathBuf::from("/notes/a.md"));

    assert_eq!(document_width(&state), 500.0);
}

#[test]
fn test_document_width_ignores_the_right_pane_when_the_toc_is_hidden() {
    let mut state = fresh_state();
    state.viewport = iced::Size::new(1000.0, 700.0);
    state.app.layout.set_width(Some(300.0));
    state.app.layout.set_right_width(Some(200.0));

    assert_eq!(
        document_width(&state),
        700.0,
        "no ToC → the right pane is not mounted, so it takes no width"
    );
}
