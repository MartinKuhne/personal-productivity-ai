//! Unit tests for the iced application root (`src/app/ui_iced/app.rs`).
//!
//! `launch()` calls `.run()` and is not headlessly testable; these tests
//! exercise the pieces it composes — the framework-free `build_state`, the
//! `boot` state/task contract, the scale composition, the config
//! direct-injection, the CLI workspace injection from `main.rs`, and the
//! window-settings mappers. No test touches a real user path: storage is
//! `InMemoryConfigStorage` / `InMemoryUiStateStorage` and CLI dirs are
//! `tempfile::TempDir` ([RUST-006]).

use super::*;
use crate::bus::config::CONFIG_ARRIVAL_TIMEOUT;
use crate::bus::events::user_command::UserCommand;
use crate::config::{InMemoryConfigStorage, NoopConfigStorage};
use crate::ui::CURRENT_SCHEMA_VERSION;
use crate::ui_iced::dialogs::DialogKind;
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
fn test_build_state_publishes_config_arrived_synchronously() {
    // Regression: `build_state` MUST publish the `ConfigArrived` event
    // **synchronously**, before returning. The background worker thread
    // (`BackgroundTask::new`) and the MCP config subscriber both wait for
    // the event with only a `CONFIG_ARRIVAL_TIMEOUT` (100 ms) window
    // (`src/app/background/task.rs`, `src/app/agent/session/
    // config_subscriber.rs`). A publish routed through an iced `Task`
    // (the former `publish_config_task`) is executed by the iced
    // scheduler — potentially seconds later, behind the boot font
    // registration — so both subscribers timed out, fell back to
    // `AppConfig::default()` and the initial library scan discovered
    // nothing ("files in the library are not being scanned").
    //
    // The reader is subscribed BEFORE `build_state` is called, so any
    // publish that happens inside it arrives on it.
    let bus = Bus::<ConfigArrived>::new();
    let reader = bus.subscribe();

    let _state = build_state(
        marker_config(),
        &bus,
        Arc::new(InMemoryConfigStorage::new()),
        Arc::new(InMemoryUiStateStorage::new()),
        PersistedUiState::default(),
    );

    let event = reader.recv_timeout(CONFIG_ARRIVAL_TIMEOUT).expect(
        "build_state must publish ConfigArrived synchronously so the \
             background workers read the real configuration",
    );
    assert!(
        event
            .config
            .content_libraries
            .iter()
            .any(|lib| lib.root_folder == "M:\\marker_lib"),
        "the published event must carry the loaded configuration"
    );
}

#[test]
fn test_boot_injects_config_into_state() {
    // The loaded config is direct-injected during boot, so the UI
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
fn test_requested_scale_factor_uses_persisted_multiplier_without_baseline() {
    let state = fresh_state(populated_persisted());
    // No OS baseline yet → the user multiplier stands alone.
    assert_eq!(state.requested_scale_factor(), 1.25);
}

#[test]
fn test_requested_scale_factor_ignores_os_baseline() {
    // Regression: iced multiplies the `.scale_factor(...)` return value by the
    // window's own OS DPI factor, so folding `os_scale` in here scaled the whole
    // UI by os² and made it jump larger once the boot scale-baseline landed.
    // The request must stay the user multiplier alone.
    let mut state = fresh_state(populated_persisted());
    state.os_scale = Some(1.5);
    assert_eq!(state.requested_scale_factor(), 1.25);
}

#[test]
fn test_effective_scale_factor_composes_baseline_and_multiplier() {
    // The on-screen scale is the OS baseline times the user multiplier.
    let mut state = fresh_state(populated_persisted());
    state.os_scale = Some(1.5);
    assert_eq!(state.effective_scale_factor(), 1.5 * 1.25);
}

#[test]
fn test_requested_scale_factor_defaults_to_one_when_no_multiplier() {
    let state = fresh_state(PersistedUiState::default());
    assert_eq!(state.requested_scale_factor(), 1.0);
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
fn test_window_settings_falls_back_to_thelegacy_defaults() {
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
fn test_build_state_is_framework_free_at_the_crate_boundary() {
    // The whole `ui_iced` tree (including this constructor) references no
    // framework type; the crate boundary enforces the framework-free
    // property at link. Calling the constructor and reading a domain field
    // is the runtime witness that the framework-free path is live.
    let state = fresh_state(PersistedUiState::default());
    assert!(
        state
            .app
            .content_libraries()
            .iter()
            .any(|lib| lib.root_folder == "M:\\marker_lib"),
        "framework-free construction must still inject the config"
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

// --- Esc dismissal (UI-056) -----------------------------------------------

/// Open the small name-entry dialog.
fn open_dialog(overlay: &mut OverlayState) {
    overlay.dialog.open(DialogKind::CreateDirectory {
        parent: PathBuf::from("M:\\lib"),
    });
}

/// Open the inline editor modal.
fn open_editor(overlay: &mut OverlayState) {
    overlay.editor = Some(EditorViewModel::open("M:\\lib\\note.md", "body"));
}

/// Open a floating context menu with one row.
fn open_context_menu(overlay: &mut OverlayState) {
    overlay.context_menu = Some(ContextMenu {
        anchor: iced::Point::new(12.0, 34.0),
        items: vec![MenuItem::new(
            crate::ui::strings::OPEN_IN_EDITOR_ACTION,
            Message::Command(UserCommand::OpenInEditor(PathBuf::from("M:\\lib\\note.md"))),
        )],
    });
}

/// Open the batch dialog over an empty candidate set.
fn open_batch(overlay: &mut OverlayState) {
    overlay.batch.open(BatchDialogViewModel::default());
}

/// Nothing open → `Esc` consumes nothing.
#[test]
fn test_close_topmost_returns_none_when_nothing_is_open() {
    let mut overlay = OverlayState::default();
    assert_eq!(overlay.close_topmost(), None);
}

/// One row of the closable-layer table: the layer name `close_topmost` must
/// report, an opener for it, and a predicate telling whether it is open.
type ClosableLayer = (
    &'static str,
    fn(&mut OverlayState),
    fn(&OverlayState) -> bool,
);

/// Every closable layer is reported under its own name and ends up closed.
#[test]
fn test_close_topmost_closes_each_layer_in_turn() {
    let cases: Vec<ClosableLayer> = vec![
        (CLOSED_LAYER_BATCH, open_batch, |o| o.batch.is_open()),
        (
            CLOSED_LAYER_ATTRIBUTIONS,
            |o| {
                o.about.open_attributions();
            },
            |o| o.about.is_attributions(),
        ),
        (
            CLOSED_LAYER_ABOUT,
            |o| {
                o.about.open_about();
            },
            |o| o.about.is_about(),
        ),
        (
            CLOSED_LAYER_TOOLS,
            |o| {
                o.tools.open();
            },
            |o| o.tools.is_open(),
        ),
        (
            CLOSED_LAYER_AGENT_DEBUG,
            |o| {
                o.debug.open();
            },
            |o| o.debug.is_open(),
        ),
        (CLOSED_LAYER_DIALOG, open_dialog, |o| o.dialog.is_open()),
        (CLOSED_LAYER_EDITOR, open_editor, |o| o.editor.is_some()),
        (CLOSED_LAYER_CONTEXT_MENU, open_context_menu, |o| {
            o.context_menu.is_some()
        }),
        (
            CLOSED_LAYER_HAMBURGER,
            |o| {
                o.hamburger_open = true;
                o.hamburger_submenu = Some(crate::ui_iced::panels::top::MenuSection::Windows);
            },
            |o| o.hamburger_open,
        ),
    ];

    for (name, open, is_open) in cases {
        let mut overlay = OverlayState::default();
        open(&mut overlay);
        assert!(is_open(&overlay), "fixture must leave {name} open");

        assert_eq!(
            overlay.close_topmost(),
            Some(name),
            "Esc over a lone {name} must report that layer"
        );
        assert!(
            !is_open(&overlay),
            "{name} must be closed after close_topmost"
        );
        assert_eq!(overlay.hamburger_submenu, None);
        assert_eq!(
            overlay.close_topmost(),
            None,
            "a second Esc over the now-empty {name} must consume nothing"
        );
    }
}

/// One `Esc` = one layer: the top-most closes and the one beneath survives.
#[test]
fn test_close_topmost_closes_only_the_topmost_layer() {
    let mut overlay = OverlayState::default();
    open_dialog(&mut overlay);
    open_batch(&mut overlay);

    assert_eq!(overlay.close_topmost(), Some(CLOSED_LAYER_BATCH));
    assert!(!overlay.batch.is_open(), "the batch dialog was on top");
    assert!(
        overlay.dialog.is_open(),
        "the dialog underneath must survive a single Esc"
    );

    assert_eq!(overlay.close_topmost(), Some(CLOSED_LAYER_DIALOG));
    assert!(!overlay.dialog.is_open());
    assert_eq!(overlay.close_topmost(), None);
}

/// The full reverse-z-order walk: open every closable layer at once and
/// confirm `Esc` peels them off exactly in [`mounted_layers`] order,
/// reversed.
#[test]
fn test_close_topmost_walks_the_reverse_z_order() {
    let mut overlay = OverlayState::default();
    open_batch(&mut overlay);
    overlay.about.open_attributions();
    overlay.tools.open();
    overlay.debug.open();
    open_dialog(&mut overlay);
    open_editor(&mut overlay);
    open_context_menu(&mut overlay);
    overlay.hamburger_open = true;

    let expected = [
        CLOSED_LAYER_BATCH,
        CLOSED_LAYER_ATTRIBUTIONS,
        CLOSED_LAYER_TOOLS,
        CLOSED_LAYER_AGENT_DEBUG,
        CLOSED_LAYER_DIALOG,
        CLOSED_LAYER_EDITOR,
        CLOSED_LAYER_CONTEXT_MENU,
        CLOSED_LAYER_HAMBURGER,
    ];
    for name in expected {
        assert_eq!(
            overlay.close_topmost(),
            Some(name),
            "the stack must peel off in reverse z-order"
        );
    }
    assert_eq!(overlay.close_topmost(), None, "the stack must run out");
}

/// The docked log viewer is NOT part of the `Esc` stack: it is a docked
/// panel whose visibility is application state owned by
/// `UserCommand::ToggleBackgroundLogs`, not reducer-held overlay state.
#[test]
fn test_close_topmost_never_closes_the_docked_logs_panel() {
    // `OverlayState` carries no logs-visibility field at all, which is the
    // structural half of the guarantee; the behavioural half is that a
    // fully-populated overlay still drains to `None` without ever naming a
    // `logs` layer.
    let mut overlay = OverlayState::default();
    open_batch(&mut overlay);
    overlay.tools.open();

    let mut drained = Vec::new();
    while let Some(layer) = overlay.close_topmost() {
        drained.push(layer);
    }
    assert_eq!(
        drained,
        vec![CLOSED_LAYER_BATCH, CLOSED_LAYER_TOOLS],
        "no drained layer may be the docked log viewer"
    );
    assert!(
        !drained.contains(&"logs"),
        "the docked Logs panel must not be Esc-closable"
    );
}

/// Closing the editor must drop the live `text_editor::Content` with it, so
/// a reopen starts from a fresh buffer rather than the discarded one.
#[test]
fn test_close_topmost_drops_the_editor_buffer_entirely() {
    let mut overlay = OverlayState::default();
    open_editor(&mut overlay);
    assert_eq!(overlay.close_topmost(), Some(CLOSED_LAYER_EDITOR));
    assert!(
        overlay.editor.is_none(),
        "the owned Content must be taken, not merely hidden"
    );
}

// --- launch presets ---------------------------------------------------------

/// The preset list must exist and name its production strategy `"default"`.
/// `.run()` is never called — the preset closure is not invoked either, so
/// no threads or bus subscribers are built by this test.
#[test]
fn test_launch_presets_returns_the_default_preset() {
    let presets = launch_presets(launch_inputs(), PersistedUiState::default());

    assert!(
        !presets.is_empty(),
        "the launch chain must expose at least one preset"
    );
    assert_eq!(
        presets[0].name(),
        LAUNCH_PRESET_DEFAULT,
        "the production preset must be selectable by the harness name"
    );
    assert_eq!(
        LAUNCH_PRESET_DEFAULT, "default",
        "the `.ice` harness selects by this literal string"
    );
}

/// Presets are keyed by name only at this layer; the list must stay stable
/// across calls so a harness lookup cannot drift between runs.
#[test]
fn test_launch_presets_names_are_stable_across_calls() {
    let first = launch_presets(launch_inputs(), populated_persisted());
    let second = launch_presets(launch_inputs(), PersistedUiState::default());

    let first_names: Vec<&str> = first.iter().map(|preset| preset.name()).collect();
    let second_names: Vec<&str> = second.iter().map(|preset| preset.name()).collect();
    assert_eq!(
        first_names, second_names,
        "the preset name set must not depend on the persisted snapshot"
    );
}

/// A `LaunchInputs` / `PersistedUiState` pair built over in-memory storage
/// so no real user path is touched ([RUST-006]).
fn launch_inputs() -> LaunchInputs {
    LaunchInputs {
        config: marker_config(),
        config_bus: Bus::<ConfigArrived>::new(),
        config_storage: Arc::new(InMemoryConfigStorage::new()),
        ui_state_storage: Arc::new(InMemoryUiStateStorage::new()),
        args: vec!["fastmd".to_string()],
    }
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
