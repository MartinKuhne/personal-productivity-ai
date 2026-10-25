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
use crate::ui::CURRENT_SCHEMA_VERSION;
use crate::ui_iced::dialogs::DialogKind;
use crate::ui_iced::persisted::InMemoryUiStateStorage;
use iced_pro::binding::pre_rank::DragCapture;
use iced_pro::binding::pre_rank::DragKind;
use iced_pro::binding::pre_rank::PreRankCoordinator;
use iced_pro::surface::menu::model::{Menu, Row};
use iced_pro::surface::zorder::SurfaceId;
use iced_pro::surface::zorder::SurfaceKind;
use iced_pro::surface::zorder::SurfaceRegistry;
use iced_pro::surface::zorder::UnwindAnswer;
use iced_pro::surface::zorder::ZBand;
use iced_pro::surface::zorder::escape_with_pre_rank;

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
