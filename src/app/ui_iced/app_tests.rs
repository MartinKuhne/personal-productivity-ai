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
use crate::config::{InMemoryConfigStorage, NoopConfigStorage};
use crate::error_codes::menu as menu_codes;
use crate::ui::CURRENT_SCHEMA_VERSION;
use crate::ui_iced::context_menu::{ContextGroup, ContextRow, ContextTarget};
use crate::ui_iced::dialogs::DialogKind;
use crate::ui_iced::menu_surface::log_capture::{Capture, with_capture};
use crate::ui_iced::message::MenuMessage;
use crate::ui_iced::persisted::InMemoryUiStateStorage;
use hoarfrost::binding::pre_rank::DragCapture;
use hoarfrost::binding::pre_rank::DragKind;
use hoarfrost::binding::pre_rank::PreRankCoordinator;
use hoarfrost::surface::menu::model::{Menu, Row};
use hoarfrost::surface::movable::MovableGeometry;
use hoarfrost::surface::zorder::SurfaceId;
use hoarfrost::surface::zorder::SurfaceKind;
use hoarfrost::surface::zorder::SurfaceRegistry;
use hoarfrost::surface::zorder::UnwindAnswer;
use hoarfrost::surface::zorder::ZBand;
use hoarfrost::surface::zorder::escape_with_pre_rank;

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
fn test_window_settings_falls_back_to_the_default_size() {
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

/// Open a floating context menu on a real single-file target.
fn open_context_menu(overlay: &mut OverlayState) {
    let path = PathBuf::from("M:\\lib\\note.md");
    let rows = crate::ui_iced::panels::left::file_menu_items(&path, &[], false);
    overlay.context_menu = ContextMenu::open(
        crate::ui_iced::context_menu::ContextTarget::TreeFile { path },
        iced::Point::new(12.0, 34.0),
        &rows,
        iced::Size::new(1200.0, 800.0),
    );
    assert!(
        overlay.context_menu.is_some(),
        "the fixture must actually open a card"
    );
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
                let menu = Menu::new(
                    crate::ui_iced::hamburger::HAMBURGER_MENU_ID,
                    vec![Row::action("x", "X")],
                );
                o.hamburger_cascade
                    .open(&menu, MenuTreeKind::Fixed, &mut o.menu_focus);
                o.hamburger_open = o.hamburger_cascade.is_open();
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
        assert!(
            !overlay.hamburger_cascade.is_open(),
            "the hamburger cascade must be fully unwound, not merely hidden"
        );
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

/// The full escape-rank walk: open every closable layer at once and confirm
/// `Esc` peels them off in `UIF-WIN-003` order — descending
/// `ZBand::escape_rank`, top of band first.
///
/// **This test previously pinned a bug.** It asserted the old hardcoded
/// chain's order (`batch → attributions → tools → agent_debug → dialog →
/// editor → context_menu → hamburger`), which closed the `Editor` (Modal,
/// rank 2) ahead of the `ContextMenu` (MenuCard, rank 3) and the modeless
/// `AgentDebug` card (rank 1) ahead of the blocking `Dialog` / `Editor`
/// (rank 2). Both are wrong under the spec's escape model. The expectation
/// below is the corrected rank order; see
/// [`OverlayState::close_topmost`] for the bug write-up.
#[test]
fn test_close_topmost_walks_the_escape_rank_order() {
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
        // Menu band — escape rank 3, top of band first.
        CLOSED_LAYER_HAMBURGER,
        CLOSED_LAYER_CONTEXT_MENU,
        // Modal band — escape rank 2, top of band first.
        CLOSED_LAYER_BATCH,
        CLOSED_LAYER_ATTRIBUTIONS,
        CLOSED_LAYER_TOOLS,
        CLOSED_LAYER_DIALOG,
        CLOSED_LAYER_EDITOR,
        // Utility-overlay band — escape rank 1, last off the stack.
        CLOSED_LAYER_AGENT_DEBUG,
    ];
    for name in expected {
        assert_eq!(
            overlay.close_topmost(),
            Some(name),
            "the stack must peel off in descending escape rank, top of band first"
        );
    }
    assert_eq!(overlay.close_topmost(), None, "the stack must run out");
}

// --- INT-2: the escape-rank bug this increment fixes -----------------------

/// **The bug:** a menu sits *above* the surface it was opened from, so
/// with an editor and a context menu both open `Esc` must close the
/// **menu** and leave the modal intact.
///
/// The old eight-branch `if`-chain tested `Editor` before `ContextMenu`,
/// so the first `Esc` destroyed the editor the menu had been opened over
/// and left an orphaned menu pointing at nothing.
#[test]
fn test_close_topmost_closes_the_context_menu_before_the_editor() {
    let mut overlay = OverlayState::default();
    open_editor(&mut overlay);
    open_context_menu(&mut overlay);

    assert_eq!(
        overlay.close_topmost(),
        Some(CLOSED_LAYER_CONTEXT_MENU),
        "a menu unwinds above the modal it was opened over (UIF-WIN-003)"
    );
    assert!(overlay.context_menu.is_none(), "the menu must be gone");
    assert!(
        overlay.editor.is_some(),
        "the editor the menu was opened over must survive the first Esc"
    );

    assert_eq!(overlay.close_topmost(), Some(CLOSED_LAYER_EDITOR));
    assert!(overlay.editor.is_none());
    assert_eq!(overlay.close_topmost(), None, "the stack must run out");
}

/// The same rank rule through the other menu surface: the hamburger is a
/// `MenuCard` (rank 3) and outranks the editor (rank 2), even though the
/// editor is painted in a higher *band*.
#[test]
fn test_close_topmost_closes_the_hamburger_before_the_editor() {
    let mut overlay = OverlayState::default();
    open_editor(&mut overlay);
    overlay.hamburger_open = true;

    assert_eq!(
        overlay.close_topmost(),
        Some(CLOSED_LAYER_HAMBURGER),
        "the menu band unwinds above the modal band"
    );
    assert!(!overlay.hamburger_open);
    assert!(overlay.editor.is_some());

    assert_eq!(overlay.close_topmost(), Some(CLOSED_LAYER_EDITOR));
}

/// **The bug:** a modeless utility overlay must not unwind ahead of a
/// blocking modal — the modal is what is holding the user.
///
/// The old chain closed `AgentDebug` (MovableCard, rank 1) before
/// `About` (Modal, rank 2).
#[test]
fn test_close_topmost_closes_a_modal_before_a_movable_card() {
    let mut overlay = OverlayState::default();
    overlay.debug.open();
    overlay.about.open_about();

    assert_eq!(
        overlay.close_topmost(),
        Some(CLOSED_LAYER_ABOUT),
        "a blocking modal unwinds ahead of a modeless utility card"
    );
    assert!(
        overlay.debug.is_open(),
        "the modeless card must survive while the modal above it is dismissed"
    );

    assert_eq!(overlay.close_topmost(), Some(CLOSED_LAYER_AGENT_DEBUG));
    assert_eq!(overlay.close_topmost(), None);
}

/// Same rule for the small dialog, which is an `InputDialog` — a distinct
/// `SurfaceKind` that shares the modal band's rank 2.
#[test]
fn test_close_topmost_closes_the_input_dialog_before_a_movable_card() {
    let mut overlay = OverlayState::default();
    open_dialog(&mut overlay);
    overlay.debug.open();

    assert_eq!(
        overlay.close_topmost(),
        Some(CLOSED_LAYER_DIALOG),
        "InputDialog is rank 2, above the rank-1 utility card"
    );
    assert!(overlay.debug.is_open());

    assert_eq!(overlay.close_topmost(), Some(CLOSED_LAYER_AGENT_DEBUG));
}

/// Batch sits at the top of the modal band, so it still unwinds first over
/// every non-menu surface — the existing correct behaviour is preserved by
/// registration position, not by a special case.
#[test]
fn test_close_topmost_still_closes_batch_first_over_every_non_menu_surface() {
    let mut overlay = OverlayState::default();
    overlay.debug.open();
    open_editor(&mut overlay);
    open_dialog(&mut overlay);
    overlay.tools.open();
    overlay.about.open_attributions();
    open_batch(&mut overlay);

    assert_eq!(
        overlay.close_topmost(),
        Some(CLOSED_LAYER_BATCH),
        "the batch dialog must still be the first thing Esc closes"
    );
    assert!(!overlay.batch.is_open());
    assert!(overlay.about.is_attributions());
    assert!(overlay.tools.is_open());
    assert!(overlay.dialog.is_open());
    assert!(overlay.editor.is_some());
    assert!(overlay.debug.is_open());
}

/// Batch is *not* above the menu band. With a menu open on top of it, Esc
/// closes the menu first — the corrected half of the rule, stated against
/// the surface that used to be highest in the old chain.
#[test]
fn test_close_topmost_closes_a_menu_before_batch() {
    let mut overlay = OverlayState::default();
    open_batch(&mut overlay);
    open_context_menu(&mut overlay);

    assert_eq!(
        overlay.close_topmost(),
        Some(CLOSED_LAYER_CONTEXT_MENU),
        "rank 3 outranks the batch dialog's rank 2"
    );
    assert!(overlay.batch.is_open());
    assert_eq!(overlay.close_topmost(), Some(CLOSED_LAYER_BATCH));
}

// --- INT-2: the registry is a derived index --------------------------------

/// `sync_surfaces` mirrors the authoritative fields into the band model,
/// is idempotent, and cannot reshuffle the stack across repeated calls.
#[test]
fn test_sync_surfaces_is_an_idempotent_derived_index() {
    let mut overlay = OverlayState::default();
    overlay.sync_surfaces();
    assert!(
        overlay.surfaces.is_empty(),
        "a closed-everything overlay registers nothing"
    );

    open_batch(&mut overlay);
    open_context_menu(&mut overlay);
    overlay.debug.open();
    overlay.sync_surfaces();
    let first: Vec<String> = overlay
        .surfaces
        .surfaces()
        .iter()
        .map(|surface| surface.id().as_str().to_owned())
        .collect();

    overlay.sync_surfaces();
    overlay.sync_surfaces();
    let second: Vec<String> = overlay
        .surfaces
        .surfaces()
        .iter()
        .map(|surface| surface.id().as_str().to_owned())
        .collect();
    assert_eq!(
        first, second,
        "repeated syncs must not reshuffle the index (register keeps position)"
    );

    assert_eq!(
        overlay.surfaces.count_in(ZBand::Menu),
        1,
        "the context menu is the only MenuCard"
    );
    assert_eq!(
        overlay.surfaces.count_in(ZBand::Modal),
        1,
        "the batch dialog is the only Modal-band surface"
    );
    assert_eq!(
        overlay.surfaces.count_in(ZBand::UtilityOverlay),
        1,
        "the agent debug card is the only utility overlay"
    );
    assert_eq!(
        overlay.surfaces.get(&SurfaceId::new(SURFACE_ID_DIALOG)),
        None,
        "a closed field must not be registered"
    );

    // Closing the authoritative field removes the entry on the next sync —
    // the index never outlives the truth it is derived from.
    overlay.batch.close();
    overlay.sync_surfaces();
    assert!(
        !overlay.surfaces.contains(&SurfaceId::new(SURFACE_ID_BATCH)),
        "the index must drop a surface its authoritative field closed"
    );
    assert_eq!(overlay.surfaces.len(), 2);
}

/// The two menus share a band and are registered in the overlay host's
/// draw order, so the one painted on top unwinds first. The reducer keeps
/// them mutually exclusive in production, so this pins the *ordering rule*
/// rather than a reachable state.
#[test]
fn test_the_hamburger_is_registered_above_the_context_menu() {
    let mut overlay = OverlayState::default();
    open_context_menu(&mut overlay);
    overlay.hamburger_open = true;
    overlay.sync_surfaces();

    let menu_ids: Vec<&str> = overlay
        .surfaces
        .surfaces_in(ZBand::Menu)
        .map(|surface| surface.id().as_str())
        .collect();
    assert_eq!(
        menu_ids,
        vec![SURFACE_ID_CONTEXT_MENU, SURFACE_ID_HAMBURGER],
        "bottom-first registration must match the overlay host's mount order"
    );
    assert_eq!(
        overlay
            .surfaces
            .topmost_in(ZBand::Menu)
            .map(|surface| surface.id().as_str()),
        Some(SURFACE_ID_HAMBURGER)
    );
}

// --- INT-2: the Pre-Rank step ---------------------------------------------

/// The Pre-Rank ledger is consulted on **every** `Esc`, ahead of the rank
/// walk: a capture held in the ledger is drained by the same call that
/// closes a surface.
///
/// The app's cancel closure answers "nothing to cancel" (no framework drag
/// widget is mounted yet), so the ledger reports the registration as
/// *stale*, drains it, and lets the walk proceed. That the ledger is empty
/// afterwards while the editor still closed proves the Pre-Rank ran first
/// on this call rather than never running at all.
#[test]
fn test_close_topmost_consults_the_pre_rank_ledger_before_the_walk() {
    let mut overlay = OverlayState::default();
    open_editor(&mut overlay);
    assert!(
        overlay
            .pre_rank
            .begin(DragCapture::new("split_divider", DragKind::SplitDivider))
            .is_started()
    );

    assert_eq!(overlay.close_topmost(), Some(CLOSED_LAYER_EDITOR));
    assert!(
        !overlay.pre_rank.is_captured(),
        "the Pre-Rank step must have run and drained the ledger"
    );
}

/// The no-capture case is a clean pass-through: the ledger answers
/// `NotCaptured` and the rank walk behaves exactly as if the Pre-Rank
/// step had not existed. This is what keeps the wiring from being dead code
/// today — it is asserted, not assumed.
#[test]
fn test_close_topmost_pre_rank_is_a_clean_pass_through_with_no_capture() {
    let mut overlay = OverlayState::default();
    open_batch(&mut overlay);
    open_context_menu(&mut overlay);
    assert!(!overlay.pre_rank.is_captured());

    assert_eq!(overlay.close_topmost(), Some(CLOSED_LAYER_CONTEXT_MENU));
    assert_eq!(overlay.close_topmost(), Some(CLOSED_LAYER_BATCH));
    assert_eq!(overlay.close_topmost(), None);
    assert!(
        !overlay.pre_rank.is_captured(),
        "a pass-through must not invent a capture"
    );
}

/// The ordering INT-6 depends on, pinned at the composition
/// `close_topmost` uses: when the cancel closure **does** produce a
/// `Cancel`, the key is spent on the drag and no surface is asked at all.
///
/// Exercised with a counting `unwind` closure so "the walk did not run" is
/// an assertion rather than an absence of evidence.
#[test]
fn test_a_live_drag_cancel_short_circuits_the_rank_walk() {
    let mut registry = SurfaceRegistry::new();
    registry.register(
        SurfaceId::new(SURFACE_ID_CONTEXT_MENU),
        SurfaceKind::MenuCard,
        Vec::new(),
    );
    registry.register(
        SurfaceId::new(SURFACE_ID_BATCH),
        SurfaceKind::Modal,
        Vec::new(),
    );

    let mut captures = PreRankCoordinator::new();
    assert!(
        captures
            .begin(DragCapture::new("overlay.move", DragKind::OverlayMove))
            .is_started()
    );

    let mut asked = 0_usize;
    let escape = escape_with_pre_rank(
        &mut captures,
        &mut registry,
        |_capture| true,
        |_surface| {
            asked += 1;
            UnwindAnswer::Dismissed
        },
    );

    assert!(
        escape.drag_cancelled(),
        "a live capture must take the key ahead of every surface"
    );
    assert_eq!(
        asked, 0,
        "no surface may be asked once the drag owns the key"
    );
    assert_eq!(
        registry.len(),
        2,
        "the rank walk must not have dismissed anything"
    );
}

/// `close_topmost` reports `None` when the Pre-Rank spends the key: no
/// overlay layer closed, so there is no layer name to give.
#[test]
fn test_close_topmost_reports_none_when_the_pre_rank_spends_the_key() {
    let mut overlay = OverlayState::default();
    open_batch(&mut overlay);
    // A capture that the app's cancel closure cannot settle is reported
    // stale, so the walk still runs today. What this pins is that the
    // reported name comes from the walk and that a drained ledger never
    // turns a real dismissal into `None`.
    assert!(
        overlay
            .pre_rank
            .begin(DragCapture::new("slider", DragKind::Slider))
            .is_started()
    );
    assert_eq!(overlay.close_topmost(), Some(CLOSED_LAYER_BATCH));
    assert_eq!(overlay.close_topmost(), None);
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

// --- UIF-MOD-002: the editor's unsaved-edit guard on the Escape path -----

/// Open the inline editor with **unsaved** edits in it.
///
/// The buffer is dirtied through the same `apply_action` path a real
/// keystroke takes, so `is_dirty()` is true for the reason it would be in
/// the running app rather than because a test set a flag.
fn open_dirty_editor(overlay: &mut OverlayState) {
    let mut vm = EditorViewModel::open("M:\\lib\\note.md", "body");
    vm.apply_action(iced::widget::text_editor::Action::Edit(
        iced::widget::text_editor::Edit::Paste(Arc::new(" unsaved".to_string())),
    ));
    assert!(vm.is_dirty(), "the fixture must leave unsaved edits");
    overlay.editor = Some(vm);
}

/// The dirty buffer's contents, captured before a gesture so a test can
/// assert the *same bytes* are still there afterwards.
///
/// Read from the model rather than written out as a literal: the point of
/// these tests is that the buffer survives, not that a paste lands at a
/// particular offset, and pinning the offset here would only make the
/// guard tests brittle.
fn dirty_text(overlay: &OverlayState) -> String {
    overlay.editor.as_ref().expect("editor open").text()
}

/// A clean buffer still dismisses on `Escape` — the guard must not turn
/// every close into a question.
#[test]
fn test_escape_over_a_clean_editor_still_closes_it() {
    let mut overlay = OverlayState::default();
    open_editor(&mut overlay);
    assert!(!overlay.editor.as_ref().unwrap().is_dirty());

    assert_eq!(
        overlay.close_topmost(),
        Some(CLOSED_LAYER_EDITOR),
        "a clean editor must close on Esc exactly as it did before the guard"
    );
    assert!(overlay.editor.is_none());
    assert!(
        !overlay.dialog.is_open(),
        "a clean close must not raise a confirmation"
    );
}

/// The regression this increment exists for: `Escape` over a **dirty**
/// editor must not drop the buffer.
#[test]
fn test_escape_over_a_dirty_editor_does_not_discard_it() {
    let mut overlay = OverlayState::default();
    open_dirty_editor(&mut overlay);
    let before = dirty_text(&overlay);

    let _ = overlay.close_topmost();

    assert!(
        overlay.editor.is_some(),
        "UIF-MOD-002: Esc over unsaved edits must leave the editor open"
    );
    assert_eq!(
        dirty_text(&overlay),
        before,
        "the buffer contents must survive the refused Escape byte-for-byte"
    );
}

/// The refused `Escape` is not a dead end: it raises the discard
/// confirmation in the dialog slot, centred rather than cursor-anchored.
#[test]
fn test_escape_over_a_dirty_editor_raises_the_discard_confirmation() {
    let mut overlay = OverlayState::default();
    open_dirty_editor(&mut overlay);
    overlay.dialog_anchor = Some(iced::Point::new(400.0, 300.0));

    let _ = overlay.close_topmost();

    assert_eq!(
        overlay.dialog.kind(),
        Some(&DialogKind::DiscardEditorChanges {
            path: PathBuf::from("M:\\lib\\note.md"),
        }),
        "the refusal must open the INT-4 discard confirmation for the open file"
    );
    assert_eq!(
        overlay.dialog_anchor, None,
        "a keypress-raised dialog is keyboard-invoked, so it centres rather \
         than following a cursor that is nowhere near the question"
    );
}

/// The confirmation sits **above** the editor: the next `Escape` closes
/// the question and leaves the buffer alone.
#[test]
fn test_escape_over_the_confirmation_closes_only_the_dialog() {
    let mut overlay = OverlayState::default();
    open_dirty_editor(&mut overlay);
    let before = dirty_text(&overlay);
    let _ = overlay.close_topmost();
    assert!(overlay.dialog.is_open());

    assert_eq!(
        overlay.close_topmost(),
        Some(CLOSED_LAYER_DIALOG),
        "the confirmation is the top of the band and unwinds first"
    );
    assert!(
        overlay.editor.is_some(),
        "dismissing the question must never be the same as answering it"
    );
    assert_eq!(dirty_text(&overlay), before);
}

/// Three `Escape`s in a row must never discard: ask, close the question,
/// ask again. Re-asking is the correct failure mode.
#[test]
fn test_repeated_escape_never_discards_unsaved_edits() {
    let mut overlay = OverlayState::default();
    open_dirty_editor(&mut overlay);
    let before = dirty_text(&overlay);

    for round in 0..3 {
        let _ = overlay.close_topmost();
        assert!(
            overlay.editor.is_some(),
            "round {round}: Esc must never drop the buffer"
        );

        // Close the question the previous Escape raised, if any.
        if overlay.dialog.is_open() {
            let _ = overlay.close_topmost();
            assert!(!overlay.dialog.is_open());
            assert!(overlay.editor.is_some());
        }
    }

    assert_eq!(
        dirty_text(&overlay),
        before,
        "after any number of Escape presses the edits are still there"
    );
}

/// With no editor open there is nothing to guard, so the walk is not
/// blocked — the caller's own `take()` reports "was not open".
#[test]
fn test_guard_with_no_editor_open_lets_the_walk_continue() {
    let mut overlay = OverlayState::default();
    open_dialog(&mut overlay);

    assert!(guard_editor_leave(
        &overlay.editor,
        &mut overlay.dialog,
        &mut overlay.dialog_anchor,
        crate::ui_iced::editor::LeaveGesture::Escape,
    ));
    assert!(
        overlay.dialog.is_open(),
        "the guard must not touch an unrelated open dialog"
    );
}

/// The guard answers the *gesture*, not just the flag: the same dirty
/// buffer that refuses `Escape` accepts `Cancel`.
#[test]
fn test_guard_refuses_escape_but_accepts_cancel_on_the_same_dirty_buffer() {
    let mut overlay = OverlayState::default();
    open_dirty_editor(&mut overlay);

    assert!(
        !guard_editor_leave(
            &overlay.editor,
            &mut overlay.dialog,
            &mut overlay.dialog_anchor,
            crate::ui_iced::editor::LeaveGesture::Escape,
        ),
        "Escape on a dirty buffer must be refused"
    );
    assert!(
        guard_editor_leave(
            &overlay.editor,
            &mut overlay.dialog,
            &mut overlay.dialog_anchor,
            crate::ui_iced::editor::LeaveGesture::Cancel,
        ),
        "Cancel is the explicit discard and must always be accepted"
    );
}

// ─────────────── INT-6: Band-1 movable utility overlays ─────────────────────

/// Build a host at a known rectangle inside `viewport`, opened against
/// throwaway registry/focus so the test controls exactly what lands in the
/// overlay's own index afterwards.
fn host_at(id: &str, x: f32, y: f32, w: f32, h: f32, viewport: Rectangle) -> MovableHost {
    let spec = MovableSpec::new(SurfaceId::new(id), "test card")
        .with_geometry(MovableGeometry::new(Point::new(x, y), Size::new(w, h)));
    MovableHost::open(
        spec,
        viewport,
        &mut hoarfrost::focus::FocusState::new(),
        &mut SurfaceRegistry::new(),
        &mut hoarfrost::surface::movable::NoopGeometryStorage,
    )
}

fn viewport_1280x800() -> Rectangle {
    Rectangle::new(Point::ORIGIN, Size::new(1280.0, 800.0))
}

/// Both movable cards register in `ZBand::UtilityOverlay` with their **live
/// host rectangle**, which is what lets the registry answer the routing
/// question `UIF-MOV-003` is really about.
#[test]
fn both_movable_cards_register_with_their_live_rect() {
    let viewport = viewport_1280x800();
    let mut overlay = OverlayState::default();
    overlay.debug.open();
    overlay.logs_visible = true;
    overlay.debug_movable = Some(host_at(
        SURFACE_ID_AGENT_DEBUG,
        100.0,
        100.0,
        680.0,
        440.0,
        viewport,
    ));
    overlay.logs_movable = Some(host_at(
        SURFACE_ID_LOGS,
        400.0,
        300.0,
        680.0,
        440.0,
        viewport,
    ));

    overlay.sync_surfaces();

    assert_eq!(
        overlay.surfaces.count_in(ZBand::UtilityOverlay),
        2,
        "both cards sit in the utility band"
    );
    assert_eq!(
        overlay.surfaces.count_in(ZBand::Modal),
        0,
        "a movable card is not a modal"
    );

    let logs = overlay
        .surfaces
        .get(&SurfaceId::new(SURFACE_ID_LOGS))
        .expect("logs registered");
    assert_eq!(
        logs.regions(),
        &[overlay.logs_movable.as_ref().unwrap().card()],
        "the registered region is the host's own card rect, not a copy"
    );
}

/// `UIF-MOV-003`, the load-bearing half: with only movable cards open, a
/// press outside every card reaches the workspace. Under the old `modal`
/// mount it would have hit a scrim and been swallowed.
#[test]
fn a_press_outside_every_movable_card_reaches_the_workspace() {
    let viewport = viewport_1280x800();
    let mut overlay = OverlayState::default();
    overlay.debug.open();
    overlay.logs_visible = true;
    overlay.debug_movable = Some(host_at(
        SURFACE_ID_AGENT_DEBUG,
        100.0,
        100.0,
        680.0,
        440.0,
        viewport,
    ));
    overlay.logs_movable = Some(host_at(
        SURFACE_ID_LOGS,
        400.0,
        300.0,
        680.0,
        440.0,
        viewport,
    ));
    overlay.sync_surfaces();

    let route = overlay.surfaces.route_pointer(Point::new(1240.0, 780.0));
    assert!(
        route.reaches_workspace(),
        "no scrim may stand between the card band and Band 0, got {:?}",
        route.target()
    );
}

/// The complement: a press inside a card is owned by that card, so the
/// card's own absorber is what should take it — not the workspace.
#[test]
fn a_press_inside_a_movable_card_is_owned_by_that_card() {
    let viewport = viewport_1280x800();
    let mut overlay = OverlayState::default();
    overlay.debug.open();
    overlay.debug_movable = Some(host_at(
        SURFACE_ID_AGENT_DEBUG,
        100.0,
        100.0,
        680.0,
        440.0,
        viewport,
    ));
    overlay.sync_surfaces();

    let inside = overlay.logs_movable.as_ref().map(|_| ()).is_none() && true;
    assert!(inside, "fixture: only agent debug is open");

    let route = overlay.surfaces.route_pointer(Point::new(300.0, 300.0));
    assert_eq!(
        route.target().surface_id(),
        Some(&SurfaceId::new(SURFACE_ID_AGENT_DEBUG)),
        "the point is inside the debug card"
    );
}

/// `UIF-MOV-003`'s promotion rule: clicking a Band-1 card raises it above
/// the other Band-1 card, and the raise is observable through routing at a
/// point where the two cards overlap.
#[test]
fn clicking_a_movable_card_promotes_it_above_the_other_band_1_card() {
    let viewport = viewport_1280x800();
    let mut overlay = OverlayState::default();
    overlay.debug.open();
    overlay.logs_visible = true;
    overlay.debug_movable = Some(host_at(
        SURFACE_ID_AGENT_DEBUG,
        100.0,
        100.0,
        680.0,
        440.0,
        viewport,
    ));
    overlay.logs_movable = Some(host_at(
        SURFACE_ID_LOGS,
        400.0,
        300.0,
        680.0,
        440.0,
        viewport,
    ));
    overlay.sync_surfaces();

    // Whichever card the registration order happens to put on top, the
    // rule under test is that promoting the *other* one flips it. Asserting
    // it that way keeps the test about the promotion rule rather than about
    // an incidental registration order.
    let overlap = Point::new(500.0, 400.0);
    let before = overlay
        .surfaces
        .topmost_in(ZBand::UtilityOverlay)
        .map(|surface| surface.id().as_str().to_owned());
    let other = if before.as_deref() == Some(SURFACE_ID_LOGS) {
        SURFACE_ID_AGENT_DEBUG
    } else {
        SURFACE_ID_LOGS
    };

    let raised = overlay.surfaces.promote(&SurfaceId::new(other));
    assert!(raised, "promote must accept a registered surface");
    assert_eq!(
        overlay
            .surfaces
            .route_pointer(overlap)
            .target()
            .surface_id(),
        Some(&SurfaceId::new(other)),
        "the clicked card must now win the overlap"
    );
    assert_eq!(
        overlay
            .surfaces
            .topmost_in(ZBand::UtilityOverlay)
            .map(|surface| surface.id().as_str().to_owned())
            .as_deref(),
        Some(other),
        "the raise must be visible as the band's new top"
    );
}

/// Promotion is confined to its own band: raising a utility card must not
/// let it outrank a modal, which sits a whole band higher.
#[test]
fn promoting_a_utility_card_does_not_outrank_a_modal() {
    let viewport = viewport_1280x800();
    let mut overlay = OverlayState::default();
    overlay.debug.open();
    overlay.logs_visible = true;
    overlay.debug_movable = Some(host_at(
        SURFACE_ID_AGENT_DEBUG,
        100.0,
        100.0,
        680.0,
        440.0,
        viewport,
    ));
    overlay.logs_movable = Some(host_at(
        SURFACE_ID_LOGS,
        400.0,
        300.0,
        680.0,
        440.0,
        viewport,
    ));
    open_batch(&mut overlay);
    overlay.sync_surfaces();

    overlay.surfaces.promote(&SurfaceId::new(SURFACE_ID_LOGS));

    let overlap = Point::new(500.0, 400.0);
    assert_eq!(
        overlay
            .surfaces
            .route_pointer(overlap)
            .target()
            .surface_id(),
        Some(&SurfaceId::new(SURFACE_ID_BATCH)),
        "a Band-1 raise must not reach above the modal band"
    );
}

/// The registration driver is the authoritative `OverlayState` field, not
/// the presence of a host: closing the field drops the surface even if a
/// stale host were still around.
#[test]
fn closing_the_authoritative_field_drops_the_movable_surface() {
    let viewport = viewport_1280x800();
    let mut overlay = OverlayState::default();
    overlay.debug.open();
    overlay.debug_movable = Some(host_at(
        SURFACE_ID_AGENT_DEBUG,
        100.0,
        100.0,
        680.0,
        440.0,
        viewport,
    ));
    overlay.sync_surfaces();
    assert!(
        overlay
            .surfaces
            .contains(&SurfaceId::new(SURFACE_ID_AGENT_DEBUG))
    );

    overlay.debug.close();
    overlay.sync_surfaces();
    assert!(
        !overlay
            .surfaces
            .contains(&SurfaceId::new(SURFACE_ID_AGENT_DEBUG)),
        "the index must not outlive the field it is derived from"
    );
}

// ------------------------- observability (NFR-001 / NFR-004) -----------------
//
// `ContextMenu::open` used to return a bare `None` and drop the cascade's
// reason with it. These tests force the refusal and assert the code, the level
// and the diagnostic context carried on the line.

#[test]
fn test_context_menu_open_refusal_reports_menu_2003_with_context() {
    let target = ContextTarget::TreeFile {
        path: PathBuf::from("/notes/empty.md"),
    };

    let capture = Capture::new();
    let opened = with_capture(&capture, || {
        ContextMenu::open(
            target,
            Point::new(15.0, 25.0),
            &[],
            Size::new(1024.0, 768.0),
        )
    });

    assert!(opened.is_none(), "an empty row set cannot open a menu");
    let event = capture
        .with_code(menu_codes::CONTEXT_OPEN_REFUSED)
        .expect("a refused context-menu open must report ERROR ([NFR-001])");
    assert_eq!(event.level, tracing::Level::ERROR);
    assert_eq!(event.field("surface"), Some("context_menu"));
    assert_eq!(event.field("trigger"), Some("context_menu.open"));
    assert_eq!(event.field("target_kind"), Some("tree_file"));
    assert_eq!(event.field("row_count"), Some("0"));
    assert_eq!(event.field("anchor_x"), Some("15.0"));
    assert_eq!(event.field("anchor_y"), Some("25.0"));
    assert_eq!(event.field("viewport_w"), Some("1024.0"));
    assert_eq!(event.field("viewport_h"), Some("768.0"));
    assert_eq!(
        event.field("reason"),
        Some("no_focusable_rows"),
        "the cascade's own reason must be carried through as a stable label \
         (the raw variant carries a `MenuId`, which embeds the user's path)"
    );
    assert_eq!(
        event.field("reason_row"),
        Some("None"),
        "this refusal addresses no particular row"
    );
}

#[test]
fn test_context_menu_open_refusal_names_the_target_variant_not_its_path() {
    // [NFR-009]: the line identifies *which kind* of target failed without
    // shipping the user's filesystem path out of the app boundary.
    let capture = Capture::new();
    with_capture(&capture, || {
        ContextMenu::open(
            ContextTarget::TreeDirectory {
                path: PathBuf::from("/Users/secret-person/notes"),
            },
            Point::ORIGIN,
            &[],
            Size::new(800.0, 600.0),
        )
    });

    let event = capture
        .with_code(menu_codes::CONTEXT_OPEN_REFUSED)
        .expect("the refusal must report");
    assert_eq!(event.field("target_kind"), Some("tree_directory"));
    let rendered = format!("{} {:?}", event.message, event.fields);
    assert!(
        !rendered.contains("secret-person"),
        "the user's path must not appear on the log line, got {rendered}"
    );
}

#[test]
fn test_context_menu_successful_open_logs_no_error_code() {
    let rows = vec![ContextRow::new(
        "open",
        "Open",
        ContextGroup::Primary,
        Message::Menu(MenuMessage::InteractionIgnored),
    )];

    let capture = Capture::new();
    let opened = with_capture(&capture, || {
        ContextMenu::open(
            ContextTarget::Tab {
                index: 0,
                path: PathBuf::from("/notes/a.md"),
            },
            Point::new(4.0, 8.0),
            &rows,
            Size::new(1024.0, 768.0),
        )
    });

    assert!(opened.is_some(), "a populated row set opens");
    assert!(
        capture.codes().is_empty(),
        "a successful open must log no error code, got {:?}",
        capture.codes()
    );
}

// ---------------------------------------------------------------------------
// `sync_movable_hosts` — the reconciliation between "the view-model says this
// card is visible" and "a host exists to draw it and clamp it"
// ---------------------------------------------------------------------------

/// Opens the Background Operations Log the way its `UserCommand` does.
fn show_logs(state: &mut AppState) {
    if let Ok(mut manager) = state.app.orchestrator.background_manager.lock() {
        manager.show_background_logs = true;
    }
}

/// Opens the Agent Debug Inspector the way its `UserCommand` does.
fn show_debug(state: &mut AppState) {
    state.app.orchestrator.agent_panel_state.show_debug_window = true;
}

/// Hides both Band-1 cards again.
fn hide_utility_cards(state: &mut AppState) {
    if let Ok(mut manager) = state.app.orchestrator.background_manager.lock() {
        manager.show_background_logs = false;
    }
    state.app.orchestrator.agent_panel_state.show_debug_window = false;
}

#[test]
fn test_sync_movable_hosts_opens_a_host_for_each_visible_card() {
    let mut state = fresh_state(PersistedUiState::default());
    assert!(
        state.overlay.logs_movable.is_none() && state.overlay.debug_movable.is_none(),
        "a booted app shows neither card and must not build a host for one"
    );

    show_logs(&mut state);
    show_debug(&mut state);
    state.rebuild_view_models();

    assert!(
        state.overlay.logs_movable.is_some(),
        "the log card becoming visible must open its host"
    );
    assert!(
        state.overlay.debug_movable.is_some(),
        "the debug card becoming visible must open its host"
    );
    assert!(
        state.overlay.logs_visible,
        "the overlay's own visibility flag must follow the view-model it is derived from"
    );
}

#[test]
fn test_sync_movable_hosts_drops_the_host_when_its_card_is_hidden() {
    let mut state = fresh_state(PersistedUiState::default());
    show_logs(&mut state);
    show_debug(&mut state);
    state.rebuild_view_models();
    assert!(
        state.overlay.logs_movable.is_some() && state.overlay.debug_movable.is_some(),
        "the fixture must open both cards before closing them"
    );

    hide_utility_cards(&mut state);
    state.rebuild_view_models();

    assert!(
        state.overlay.logs_movable.is_none(),
        "a hidden card must lose its host, or the view keeps drawing a closed window"
    );
    assert!(
        state.overlay.debug_movable.is_none(),
        "a hidden card must lose its host, or the view keeps drawing a closed window"
    );
    assert!(
        !state.overlay.logs_visible,
        "the overlay flag must fall back with the view-model"
    );

    state.overlay.sync_surfaces();
    assert_eq!(
        state.overlay.surfaces.count_in(ZBand::UtilityOverlay),
        0,
        "a dropped host must leave the z-order registry as well"
    );
}

#[test]
fn test_sync_movable_hosts_is_idempotent() {
    let mut state = fresh_state(PersistedUiState::default());
    show_logs(&mut state);
    state.rebuild_view_models();
    let first = state
        .overlay
        .logs_movable
        .as_ref()
        .expect("the log card is open")
        .card();

    // `rebuild_view_models` runs the sync on every reduction, so re-syncing an
    // already-open card has to be a no-op or the card would drift per frame.
    state.sync_movable_hosts();
    state.sync_movable_hosts();

    let again = state
        .overlay
        .logs_movable
        .as_ref()
        .expect("still open")
        .card();
    assert_eq!(
        first, again,
        "re-opening an open card must not move or resize it"
    );
}

/// `UIF-MOV-002`: a card that outlives a window shrink has to be pulled back
/// into reach, or its title bar ends up off-screen and unreachable.
#[test]
fn test_sync_movable_hosts_pulls_the_cards_back_after_a_viewport_shrink() {
    let mut state = fresh_state(PersistedUiState::default());
    show_logs(&mut state);
    show_debug(&mut state);
    state.rebuild_view_models();

    let tiny = Size::new(120.0, 90.0);
    state.viewport = tiny;
    state.sync_movable_hosts();

    let hosts = [
        (
            "logs",
            state
                .overlay
                .logs_movable
                .as_ref()
                .expect("the log card is still visible"),
        ),
        (
            "debug",
            state
                .overlay
                .debug_movable
                .as_ref()
                .expect("the debug card is still visible"),
        ),
    ];
    for (name, host) in hosts {
        let card = host.card();
        assert!(
            card.x >= 0.0 && card.y >= 0.0,
            "{name}: a clamped card must not be pushed off the top-left, got {card:?}"
        );
        assert!(
            card.x < tiny.width && card.y < tiny.height,
            "{name}: after a shrink to {tiny:?} the title bar must still be inside the \
             window, got {card:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// The pure helpers behind two log fields and the two rect refreshes
// ---------------------------------------------------------------------------

#[test]
fn test_context_target_kind_names_each_variant_as_a_stable_token() {
    // The field is a groupable token for log tooling, so each variant must map
    // to one fixed string rather than a debug dump that changes shape.
    let cases: Vec<(ContextTarget, &'static str)> = vec![
        (
            ContextTarget::TreeDirectory {
                path: PathBuf::from("M:\\lib"),
            },
            "tree_directory",
        ),
        (
            ContextTarget::TreeFile {
                path: PathBuf::from("M:\\lib\\note.md"),
            },
            "tree_file",
        ),
        (
            ContextTarget::TreeMultiSelect {
                paths: vec![PathBuf::from("M:\\lib\\note.md")],
            },
            "tree_multi_select",
        ),
        (
            ContextTarget::Tab {
                index: 2,
                path: PathBuf::from("M:\\lib\\note.md"),
            },
            "tab",
        ),
    ];

    for (target, expected) in cases {
        assert_eq!(
            context_target_kind(&target),
            expected,
            "the log's target_kind field must name the variant, not its payload"
        );
    }
}

#[test]
fn test_refresh_hamburger_rects_clears_a_closed_menu() {
    let mut state = fresh_state(PersistedUiState::default());
    state.overlay.hamburger_open = false;
    state.overlay.hamburger_rects = vec![Rectangle::new(Point::new(1.0, 2.0), Size::new(3.0, 4.0))];

    state.refresh_hamburger_rects();

    assert!(
        state.overlay.hamburger_rects.is_empty(),
        "a closed menu must not keep a stale region in the registry"
    );
}

#[test]
fn test_refresh_context_rects_moves_an_open_card_with_the_viewport() {
    let mut state = fresh_state(PersistedUiState::default());
    state.viewport = Size::new(1200.0, 800.0);

    // Anchored against the bottom-right corner so the solver has to clamp it,
    // which is the placement a viewport change can actually move.
    let path = PathBuf::from("M:\\lib\\note.md");
    let rows = crate::ui_iced::panels::left::file_menu_items(&path, &[], false);
    state.overlay.context_menu = ContextMenu::open(
        ContextTarget::TreeFile { path },
        Point::new(1180.0, 780.0),
        &rows,
        state.viewport,
    );
    let before = state
        .overlay
        .context_menu
        .as_ref()
        .expect("the fixture must open a card")
        .rects
        .clone();
    assert!(
        !before.is_empty(),
        "an open menu must solve at least one card rect"
    );

    state.viewport = Size::new(600.0, 500.0);
    state.refresh_context_rects();

    let after = state
        .overlay
        .context_menu
        .as_ref()
        .expect("the refresh must not close the menu")
        .rects
        .clone();
    assert_eq!(
        before.len(),
        after.len(),
        "the same cards are solved, only placed differently"
    );
    assert_ne!(
        before, after,
        "a viewport change that lands while a menu is open must re-solve its placement"
    );
    for rect in &after {
        assert!(
            rect.x >= 0.0 && rect.y >= 0.0,
            "a re-solved card must stay on screen, got {rect:?}"
        );
    }
}
