//! Unit tests for the [TASK-022] iced application root
//! (`src/app/ui_iced/app.rs`).
//!
//! `launch()` calls `.run()` and is not headlessly testable; these tests
//! exercise the pieces it composes — the egui-free `build_state`, the
//! `boot` state/task contract, the DEC-008 scale composition, the
//! DEC-009 config direct-injection, the CLI workspace injection moved
//! from `main.rs`, and the window-settings mappers. No test touches a real
//! user path: storage is `InMemoryConfigStorage` / `InMemoryUiStateStorage`
//! and CLI dirs are `tempfile::TempDir` ([RUST-006]).

use super::*;
use crate::config::{InMemoryConfigStorage, NoopConfigStorage};
use crate::ui::CURRENT_SCHEMA_VERSION;
use crate::ui_iced::persisted::InMemoryUiStateStorage;

/// A config carrying a recognisable marker library so direct-injection is
/// observable (the marker is NOT the system-library display name, so the
/// CLI cwd fallback never fires for it).
fn marker_config() -> AppConfig {
    let mut config = AppConfig::default();
    config.content_libraries.push(ContentLibrary {
        root_folder: "M:\\marker_lib".to_string(),
        name: "MarkerLib".to_string(),
        kind: "text".to_string(),
        readonly: false,
        priority: 0,
    });
    config
}

/// A populated persisted snapshot for seeding the booted state.
fn populated_persisted() -> PersistedUiState {
    PersistedUiState {
        schema_version: CURRENT_SCHEMA_VERSION,
        left_panel_width: Some(280.0),
        right_panel_width: Some(200.0),
        window_width: Some(998.0),
        window_height: Some(688.0),
        font_size_scale: Some(1.25),
        ..Default::default()
    }
}

/// Build a fresh `AppState` over in-memory storage with a marker config.
fn fresh_state(persisted: PersistedUiState) -> AppState {
    let bus = Bus::<ConfigArrived>::new();
    build_state(
        marker_config(),
        &bus,
        Arc::new(InMemoryConfigStorage::new()),
        Arc::new(InMemoryUiStateStorage::new()),
        persisted,
    )
}

#[test]
fn test_boot_injects_config_into_state() {
    // DEC-009: the loaded config is direct-injected during boot, so the UI
    // never waits on the startup broadcast for its own state.
    let bus = Bus::<ConfigArrived>::new();
    let (state, _task) = boot(LaunchInputs {
        config: marker_config(),
        config_bus: bus,
        config_storage: Arc::new(InMemoryConfigStorage::new()),
        ui_state_storage: Arc::new(InMemoryUiStateStorage::new()),
        args: vec!["fastmd_iced".to_string()],
    });
    let libs = state.app.content_libraries();
    assert!(
        libs.iter().any(|lib| lib.root_folder == "M:\\marker_lib"),
        "the loaded config must be direct-injected: {libs:?}"
    );
}

#[test]
fn test_boot_returns_a_constructed_task() {
    // The boot task chain (fonts + scale + publish) is constructed; running
    // it needs a live iced runtime, so only its presence is asserted here.
    let (_state, task) = boot(LaunchInputs {
        config: marker_config(),
        config_bus: Bus::<ConfigArrived>::new(),
        config_storage: Arc::new(InMemoryConfigStorage::new()),
        ui_state_storage: Arc::new(InMemoryUiStateStorage::new()),
        args: vec!["fastmd_iced".to_string()],
    });
    // `Task` has no `is_empty`; constructing it without panic is the
    // contract check (the sidecar cannot drive the runtime).
    let _ = task;
}

#[test]
fn test_build_state_seeds_viewport_from_persisted_size() {
    let state = fresh_state(populated_persisted());
    assert_eq!(state.viewport, Size::new(998.0, 688.0));
}

#[test]
fn test_build_state_default_viewport_when_nothing_persisted() {
    let state = fresh_state(PersistedUiState::default());
    assert_eq!(
        state.viewport,
        Size::new(WINDOW_WIDTH_POINTS, WINDOW_HEIGHT_POINTS)
    );
}

#[test]
fn test_build_state_projects_persisted_panel_widths() {
    let state = fresh_state(populated_persisted());
    assert_eq!(state.app.layout().left_panel_width, Some(280.0));
    assert_eq!(state.app.layout().right_panel_width, Some(200.0));
}

#[test]
fn test_composed_scale_factor_uses_persisted_multiplier_without_baseline() {
    let state = fresh_state(populated_persisted());
    // No OS baseline yet → 1.0 × persisted 1.25.
    assert_eq!(state.composed_scale_factor(), 1.0 * 1.25);
}

#[test]
fn test_composed_scale_factor_multiplies_baseline_by_multiplier() {
    // DEC-008 composition: baseline × persisted multiplier.
    let mut state = fresh_state(populated_persisted());
    state.os_scale = Some(1.5);
    assert_eq!(state.composed_scale_factor(), 1.5 * 1.25);
}

#[test]
fn test_composed_scale_factor_defaults_to_one_when_no_multiplier() {
    let state = fresh_state(PersistedUiState::default());
    assert_eq!(state.composed_scale_factor(), 1.0);
}

#[test]
fn test_window_settings_uses_persisted_size_and_position() {
    let mut persisted = populated_persisted();
    persisted.window_x = Some(40.0);
    persisted.window_y = Some(50.0);
    let settings = window_settings(&persisted);
    assert_eq!(settings.size, Size::new(998.0, 688.0));
    assert!(matches!(
        settings.position,
        iced::window::Position::Specific(_)
    ));
    assert!(!settings.exit_on_close_request);
}

#[test]
fn test_window_settings_falls_back_to_egui_defaults() {
    let settings = window_settings(&PersistedUiState::default());
    assert_eq!(
        settings.size,
        Size::new(WINDOW_WIDTH_POINTS, WINDOW_HEIGHT_POINTS)
    );
    assert!(matches!(settings.position, iced::window::Position::Default));
}

#[test]
fn test_apply_cli_workspace_adds_existing_dir_argument() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut config = AppConfig::default();
    let args = vec![
        "fastmd".to_string(),
        dir.path().to_string_lossy().to_string(),
    ];
    apply_cli_workspace(&mut config, &args);
    let expected = strip_verbatim_prefix(&canonical(dir.path()));
    assert!(
        config
            .content_libraries
            .iter()
            .any(|lib| lib.root_folder == expected && lib.name == "Workspace"),
        "CLI dir must be appended as a Workspace library: {:?}",
        config.content_libraries
    );
}

#[test]
fn test_apply_cli_workspace_does_not_duplicate_existing_root() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = strip_verbatim_prefix(&canonical(dir.path()));
    let mut config = AppConfig::default();
    config.content_libraries.push(ContentLibrary {
        root_folder: root.clone(),
        name: "Existing".to_string(),
        kind: "text".to_string(),
        readonly: false,
        priority: 0,
    });
    let args = vec![
        "fastmd".to_string(),
        dir.path().to_string_lossy().to_string(),
    ];
    apply_cli_workspace(&mut config, &args);
    let matching = config
        .content_libraries
        .iter()
        .filter(|lib| lib.root_folder == root)
        .count();
    assert_eq!(matching, 1, "the existing root must not be duplicated");
}

#[test]
fn test_apply_cli_workspace_ignores_nonexistent_dir() {
    let mut config = marker_config();
    let args = vec![
        "fastmd".to_string(),
        "Z:\\definitely_not_a_real_dir_98765".to_string(),
    ];
    apply_cli_workspace(&mut config, &args);
    // The marker library is not the system library, so neither the CLI
    // block nor the cwd fallback fires: the config is unchanged.
    assert_eq!(config.content_libraries.len(), 1);
    assert_eq!(config.content_libraries[0].name, "MarkerLib");
}

#[test]
fn test_apply_cli_workspace_falls_back_to_cwd_for_system_only_config() {
    let mut config = AppConfig::default();
    config.content_libraries.push(ContentLibrary {
        root_folder: "M:\\system_lib".to_string(),
        name: config.system_library_display_name().to_string(),
        kind: "text".to_string(),
        readonly: false,
        priority: 0,
    });
    apply_cli_workspace(&mut config, &["fastmd".to_string()]);
    let expected = strip_verbatim_prefix(&canonical(&std::env::current_dir().unwrap()));
    assert!(
        config
            .content_libraries
            .iter()
            .any(|lib| lib.root_folder == expected && lib.name == "Workspace"),
        "system-only config must gain the cwd Workspace library: {:?}",
        config.content_libraries
    );
}

#[test]
fn test_ui_state_storage_from_env_override_is_file_backed() {
    let temp = tempfile::TempDir::new().unwrap();
    let path = temp.path().join("ui_state.json");
    let handler = ui_state_storage_from_env(Some(path.to_string_lossy().into_owned()));
    handler
        .save_ui_state("{}")
        .expect("override handler must be file-backed");
    assert!(path.exists());
}

#[test]
fn test_ui_state_storage_from_env_default_is_in_memory() {
    let handler = ui_state_storage_from_env(None);
    handler
        .save_ui_state(r#"{"window_width": 800}"#)
        .expect("default handler accepts saves");
    let restored = persisted::load_ui_state(handler.as_ref());
    assert_eq!(restored.window_width, Some(800.0));
}

#[test]
fn test_ui_state_storage_from_env_empty_override_is_in_memory() {
    let handler = ui_state_storage_from_env(Some(String::new()));
    handler
        .save_ui_state(r#"{"left_panel_width": 240}"#)
        .expect("empty override falls back to memory");
    let restored = persisted::load_ui_state(handler.as_ref());
    assert_eq!(restored.left_panel_width, Some(240.0));
}

#[test]
fn test_build_state_is_egui_free_at_the_crate_boundary() {
    // The whole `ui_iced` tree (including this constructor) references no
    // `egui`/`eframe` type; the crate boundary enforces the no-egui
    // property at link. Calling the constructor and reading a domain field
    // is the runtime witness that the egui-free path is live.
    let state = fresh_state(PersistedUiState::default());
    assert!(
        state
            .app
            .content_libraries()
            .iter()
            .any(|lib| lib.root_folder == "M:\\marker_lib"),
        "egui-free construction must still inject the config"
    );
}

#[test]
fn test_build_state_with_noop_storage_does_not_touch_disk() {
    // [RUST-006]: a NoopConfigStorage-backed state performs no config
    // writes; construction must succeed with no filesystem side effect.
    let bus = Bus::<ConfigArrived>::new();
    let state = build_state(
        marker_config(),
        &bus,
        Arc::new(NoopConfigStorage),
        Arc::new(InMemoryUiStateStorage::new()),
        PersistedUiState::default(),
    );
    assert_eq!(state.app.config().content_libraries.len(), 1);
}

// --- test-local helpers (mirror the production prefix-stripping) ---------

fn canonical(path: &std::path::Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string()
}

fn strip_verbatim_prefix(s: &str) -> String {
    s.strip_prefix(r"\\?\")
        .map(|v| v.to_string())
        .unwrap_or_else(|| s.to_string())
}
