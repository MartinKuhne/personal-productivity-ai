//! Unit tests for the `fastmd_iced` pilot binary ([TASK-009]/[TASK-010]).
//!
//! The Elm loop's side-effectful tasks (report/close/font loads) require a
//! live iced runtime; here only the STATE reductions and the pure helpers
//! are exercised ([TASK-009] testing rule 3). The window-close call itself
//! stays a manual-verify item for the orchestrator/user run.

use super::*;
use std::collections::HashSet;

const VIEWPORT: f32 = 1000.0;

/// A populated persisted snapshot for seeding the pilot state.
fn populated_persisted() -> PersistedUiState {
    PersistedUiState {
        schema_version: fastmd::ui::CURRENT_SCHEMA_VERSION,
        left_panel_width: Some(280.0),
        right_panel_width: Some(200.0),
        window_width: Some(998.0),
        window_height: Some(688.0),
        font_size_scale: Some(1.25),
        ..Default::default()
    }
}

/// A pilot state over an in-memory storage whose concrete handle is
/// returned alongside, plus a fresh probe state/band for resize events.
fn fresh_pilot_state() -> (
    PilotState,
    Arc<InMemoryUiStateStorage>,
    layout::MiddleBandPanes,
) {
    let storage = Arc::new(InMemoryUiStateStorage::new());
    let storage_dyn: Arc<dyn UiStateStorage> = Arc::clone(&storage) as _;
    let state = PilotState::new(storage_dyn, populated_persisted());
    let band = state.band;
    (state, storage, band)
}

#[test]
fn test_pilot_state_seeds_viewport_from_persisted_size() {
    let (state, _storage, _band) = fresh_pilot_state();
    assert_eq!(state.viewport, Size::new(998.0, 688.0));
}

#[test]
fn test_pilot_state_default_viewport_when_nothing_persisted() {
    let state = PilotState::new(
        Arc::new(InMemoryUiStateStorage::new()),
        PersistedUiState::default(),
    );
    assert_eq!(
        state.viewport,
        Size::new(WINDOW_WIDTH_POINTS, WINDOW_HEIGHT_POINTS)
    );
}

#[test]
fn test_window_settings_uses_persisted_size_and_default_position() {
    let settings = window_settings(&populated_persisted());
    assert_eq!(settings.size, Size::new(998.0, 688.0));
    assert!(matches!(settings.position, iced::window::Position::Default));
    assert!(!settings.exit_on_close_request);
}

#[test]
fn test_window_settings_falls_back_to_egui_defaults() {
    let settings = window_settings(&PersistedUiState::default());
    assert_eq!(
        settings.size,
        Size::new(WINDOW_WIDTH_POINTS, WINDOW_HEIGHT_POINTS)
    );
}

#[test]
fn test_update_window_resized_tracks_viewport() {
    let (mut state, _storage, _band) = fresh_pilot_state();
    let _ = update(
        &mut state,
        Message::WindowResized(Size::new(VIEWPORT, 700.0)),
    );
    assert_eq!(state.viewport, Size::new(VIEWPORT, 700.0));
}

#[test]
fn test_update_scale_baseline_stores_os_scale() {
    let (mut state, _storage, _band) = fresh_pilot_state();
    assert!(state.os_scale.is_none());
    let _ = update(&mut state, Message::ScaleBaseline(1.5));
    assert_eq!(state.os_scale, Some(1.5));
    assert_eq!(
        state.composed_scale_factor(),
        1.5 * 1.25,
        "DEC-008: baseline × persisted multiplier"
    );
}

#[test]
fn test_composed_scale_factor_defaults_when_no_baseline_yet() {
    let (state, _storage, _band) = fresh_pilot_state();
    assert_eq!(state.composed_scale_factor(), 1.0 * 1.25);
}

#[test]
fn test_update_pane_resized_applies_clamped_widths_and_syncs_persisted() {
    let (mut state, _storage, band) = fresh_pilot_state();
    let _ = update(
        &mut state,
        Message::WindowResized(Size::new(VIEWPORT, 700.0)),
    );

    let event = iced::widget::pane_grid::ResizeEvent {
        split: band.left_split,
        ratio: 0.25,
    };
    let _ = update(&mut state, Message::PaneResized(event));

    assert_eq!(
        state.layout.left_panel_width,
        Some(200.0),
        "25% of 1000 clamps to the 20% cap ([UI-016])"
    );
    assert_eq!(
        state.persisted.left_panel_width,
        Some(200.0),
        "the persisted snapshot stays in sync for the close-time save"
    );
}

#[test]
fn test_close_snapshot_persists_before_close_reduction() {
    let (mut state, storage, _band) = fresh_pilot_state();
    state.layout.set_width(Some(184.0));

    // This message is the SECOND phase of the close flow: by the time it
    // arrives, DEC-006 has already persisted (observable immediately here,
    // while the subsequently returned close task is runtime-only).
    let _ = update(
        &mut state,
        Message::CloseSnapshotCaptured {
            id: iced::window::Id::unique(),
            size: Size::new(1234.0, 567.0),
            position: Some(Point::new(11.0, 22.0)),
        },
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
    let (mut state, storage, _band) = fresh_pilot_state();
    let mut dirs = HashSet::new();
    dirs.insert(PathBuf::from("/notes/work"));
    state.persisted.expanded_dirs = dirs;
    state.persisted.about_shown_for_version = Some("0.2.0".to_owned());

    let _ = update(
        &mut state,
        Message::CloseSnapshotCaptured {
            id: iced::window::Id::unique(),
            size: Size::new(500.0, 400.0),
            position: None,
        },
    );

    let json = storage.saved_json().expect("payload saved");
    assert!(json.contains("notes/work"));
    assert!(json.contains("0.2.0"));
    // Position not reported → previously persisted coordinates untouched
    // (the pilot snapshot starts with no x/y at all).
    assert!(json.contains("\"window_x\":null"), "saved payload: {json}");
}

#[test]
fn test_pilot_handler_path_override_persists_to_disk() {
    let temp = tempfile::TempDir::new().unwrap();
    let path = temp.path().join("pilot_ui_state.json");
    let handler = pilot_storage_handler(Some(path.to_string_lossy().into_owned()));

    handler
        .save_ui_state("{}")
        .expect("override handler must be file-backed");
    assert!(path.exists());

    let restored = persisted::load_ui_state(handler.as_ref());
    assert_eq!(restored.window_width, None, "no size persisted yet");
}

#[test]
fn test_pilot_handler_default_is_in_memory() {
    let handler = pilot_storage_handler(None);
    assert!(
        handler.save_ui_state(r#"{"window_width": 800}"#).is_ok(),
        "default handler accepts saves"
    );
    // In-process persistence is visible through load (same process only).
    let restored = persisted::load_ui_state(handler.as_ref());
    assert_eq!(restored.window_width, Some(800.0));
}

#[test]
fn test_pilot_handler_empty_override_falls_back_to_memory() {
    let handler = pilot_storage_handler(Some(String::new()));
    assert!(
        handler
            .save_ui_state(r#"{"left_panel_width": 240}"#)
            .is_ok()
    );
    let restored = persisted::load_ui_state(handler.as_ref());
    assert_eq!(restored.left_panel_width, Some(240.0));
}

#[test]
fn test_boot_assembles_state_and_scale_task_contract() {
    // boot() constructs the state from the injected store + loaded
    // snapshot and chains the DEC-008 scale task; the returned task can
    // only be exercised by a live runtime, so only the state side is
    // asserted here ([UNVERIFIED]: baseline delivery, see verification
    // log).
    let state_and_task = boot(
        &(Arc::new(InMemoryUiStateStorage::new()) as Arc<dyn UiStateStorage>),
        populated_persisted(),
    );
    let (state, _task) = state_and_task;
    assert!(state.os_scale.is_none(), "baseline arrives async (DEC-008)");
    assert_eq!(state.viewport, Size::new(998.0, 688.0));
}
