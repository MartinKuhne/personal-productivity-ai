//! Unit tests for the [TASK-022] iced reducer (`src/app/ui_iced/update.rs`).
//!
//! The Elm loop's side-effectful tasks (report/close/font loads) need a live
//! iced runtime; here only the STATE reductions and the command
//! publish-and-execute half are exercised. The window-close call itself
//! stays a manual-verify item ([TASK-009] testing rule 3).

use super::*;
use crate::bus::core::Bus;
use crate::bus::events::config::ConfigArrived;
use crate::bus::events::user_command::UserCommand;
use crate::config::{AppConfig, InMemoryConfigStorage};
use crate::ui::CURRENT_SCHEMA_VERSION;
use crate::ui_iced::app::{AppState, build_state};
use crate::ui_iced::message::{LayoutMessage, WindowMessage, WindowSnapshot};
use crate::ui_iced::persisted::InMemoryUiStateStorage;
use iced::{Point, Size};
use std::collections::HashSet;
use std::sync::Arc;

const VIEWPORT: f32 = 1000.0;

/// A populated persisted snapshot for seeding the booted state.
fn populated_persisted() -> crate::ui::PersistedUiState {
    crate::ui::PersistedUiState {
        schema_version: CURRENT_SCHEMA_VERSION,
        left_panel_width: Some(280.0),
        right_panel_width: Some(200.0),
        window_width: Some(998.0),
        window_height: Some(688.0),
        font_size_scale: Some(1.25),
        ..Default::default()
    }
}

/// A fresh `AppState` over an in-memory UI-state store whose concrete handle
/// is returned alongside (for close/save assertions).
fn fresh_state() -> (AppState, Arc<InMemoryUiStateStorage>) {
    let storage = Arc::new(InMemoryUiStateStorage::new());
    let state = build_state(
        AppConfig::default(),
        &Bus::<ConfigArrived>::new(),
        Arc::new(InMemoryConfigStorage::new()),
        Arc::clone(&storage) as Arc<dyn crate::ui_iced::persisted::UiStateStorage>,
        populated_persisted(),
    );
    (state, storage)
}

#[test]
fn test_update_window_resized_tracks_viewport() {
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Window(WindowMessage::Resized {
            id: iced::window::Id::unique(),
            size: Size::new(VIEWPORT, 700.0),
        }),
    );
    assert_eq!(state.viewport, Size::new(VIEWPORT, 700.0));
}

#[test]
fn test_update_scale_baseline_stores_os_scale() {
    let (mut state, _storage) = fresh_state();
    assert!(state.os_scale.is_none());
    let _ = update(
        &mut state,
        Message::Window(WindowMessage::ScaleBaseline(1.5)),
    );
    assert_eq!(state.os_scale, Some(1.5));
    assert_eq!(
        state.composed_scale_factor(),
        1.5 * 1.25,
        "DEC-008: baseline × persisted multiplier"
    );
}

#[test]
fn test_update_pane_resized_applies_clamped_widths_and_syncs_persisted() {
    let (mut state, _storage) = fresh_state();
    let _ = update(
        &mut state,
        Message::Window(WindowMessage::Resized {
            id: iced::window::Id::unique(),
            size: Size::new(VIEWPORT, 700.0),
        }),
    );

    let event = iced::widget::pane_grid::ResizeEvent {
        split: state.band.left_split,
        ratio: 0.25,
    };
    let _ = update(
        &mut state,
        Message::Layout(LayoutMessage::PaneResized(event)),
    );

    assert_eq!(
        state.app.layout().left_panel_width,
        Some(200.0),
        "25% of 1000 clamps to the 20% cap ([UI-016])"
    );
    assert_eq!(
        state.app.persisted_ui_state.left_panel_width,
        Some(200.0),
        "the persisted snapshot stays in sync for the close-time save"
    );
}

#[test]
fn test_close_snapshot_persists_before_close_reduction() {
    let (mut state, storage) = fresh_state();
    state.app.layout.set_width(Some(184.0));

    // SECOND phase of the close flow: by the time this arrives, DEC-006 has
    // already persisted (observable here), while the returned close task is
    // runtime-only.
    let _ = update(
        &mut state,
        Message::Window(WindowMessage::CloseSnapshotCaptured(WindowSnapshot {
            id: iced::window::Id::unique(),
            size: Size::new(1234.0, 567.0),
            position: Some(Point::new(11.0, 22.0)),
        })),
    );

    let json = storage.saved_json().expect("state must be persisted");
    assert!(json.contains("1234"), "saved payload: {json}");
    assert!(json.contains("567"), "saved payload: {json}");
    assert!(
        json.contains("184"),
        "panel width rides the snapshot: {json}"
    );
}

#[test]
fn test_close_metrics_carry_over_expanded_dirs_and_about_stamp() {
    let (mut state, storage) = fresh_state();
    let mut dirs = HashSet::new();
    dirs.insert(std::path::PathBuf::from("/notes/work"));
    state.app.persisted_ui_state.expanded_dirs = dirs;
    state.app.persisted_ui_state.about_shown_for_version = Some("0.2.0".to_owned());

    let _ = update(
        &mut state,
        Message::Window(WindowMessage::CloseSnapshotCaptured(WindowSnapshot {
            id: iced::window::Id::unique(),
            size: Size::new(500.0, 400.0),
            position: None,
        })),
    );

    let json = storage.saved_json().expect("payload saved");
    assert!(json.contains("notes/work"));
    assert!(json.contains("0.2.0"));
    // Position not reported → previously persisted coordinates untouched.
    assert!(json.contains("\"window_x\":null"), "saved payload: {json}");
}

#[test]
fn test_command_channel_publishes_and_executes() {
    // `Message::Command` must publish on the user-command bus AND drain so
    // the intent is executed in the same reduction ([FMD-068]).
    let (mut state, _storage) = fresh_state();
    assert!(state.app.tags().selected_tag.is_none());
    let _ = update(
        &mut state,
        Message::Command(UserCommand::SelectTagFilter(Some("iced".to_string()))),
    );
    assert_eq!(
        state.app.tags().selected_tag.as_deref(),
        Some("iced"),
        "the command must be executed via the orchestrator drain"
    );
}

#[test]
fn test_boot_font_load_finished_is_a_no_panic_reduction() {
    let (mut state, _storage) = fresh_state();
    // Both the failure and success arms reduce to `Task::none()` (the
    // failure arm additionally logs an ERROR; not asserted here).
    let _ = update(
        &mut state,
        Message::Boot(crate::ui_iced::message::BootMessage::FontLoadFinished(
            Some("boom".to_string()),
        )),
    );
    let _ = update(
        &mut state,
        Message::Boot(crate::ui_iced::message::BootMessage::FontLoadFinished(None)),
    );
    let _ = update(
        &mut state,
        Message::Boot(crate::ui_iced::message::BootMessage::ConfigPublished),
    );
}

#[test]
fn test_unhandled_channel_is_a_noop() {
    // [TASK-023] wires the panel channels; until then they must reduce to a
    // no-op without panicking.
    let (mut state, _storage) = fresh_state();
    let _ = update(&mut state, Message::Layout(LayoutMessage::TreeAutoFit));
    let _ = update(
        &mut state,
        Message::Layout(LayoutMessage::CursorMoved {
            point: Point::new(1.0, 2.0),
        }),
    );
}
