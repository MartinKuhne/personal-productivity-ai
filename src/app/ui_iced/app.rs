//! Iced application root — the egui-free state construction, the `boot`
//! entry and the `launch()` builder chain ([TASK-022], DEC-006/008/009/010;
//! `doc/planning/iced.md`).
//!
//! # What lives here
//!
//! * [`AppState`] — the iced runtime state. It is a **thin wrapper** around
//!   the egui-free [`FastMdApp`] domain state plus the iced-specific runtime
//!   bits the domain struct never carried (the DEC-008 OS scale baseline, the
//!   [TASK-010] pane-grid, the live [`BusReceivers`] and the injected
//!   [`UiStateStorage`]). It is constructible with zero `egui`/`eframe`
//!   references — the whole `ui_iced` tree is ([DEC-011]).
//! * [`build_state`] — the **parallel egui-free constructor**. It mirrors
//!   the plain (non-egui) wiring of `ui/app/init.rs::new` — background
//!   `Task`, MCP config subscriber, agent session, directory tracker,
//!   persisted-UI-state projection — but resolves the persisted snapshot
//!   through the injected [`UiStateStorage`] instead of `eframe::Storage`,
//!   direct-injects the loaded [`AppConfig`] (DEC-009) and hands the real
//!   background-event channel to the [TASK-019] subscription bridge.
//! * [`boot`] / [`boot_with_persisted`] — the iced `BootFn` body: build the
//!   state, then return the batched boot task chain (font registration,
//!   the DEC-008 scale-baseline capture, and the DEC-009
//!   `Task::perform(ConfigArrived)` publish for the background workers).
//! * [`launch`] — the full verified builder chain.
//!
//! # Why a parallel constructor rather than a shared `init.rs` core
//!
//! `init.rs::new` and the iced boot differ in exactly one structural way
//! that cannot be parameterised without touching the live egui path: the
//! background-event channel `rx: std::sync::mpsc::Receiver<BackgroundEvent>`
//! is **single-owner**. The egui path moves it into `AppOrchestrator.rx` and
//! drains it per frame; the iced path must move it into the [TASK-019]
//! `BusReceivers` bridge so background notifications arrive as
//! `Message::Inbound`. Sharing one core would force a fate-flag or an inert
//! dummy channel into the shipping egui build. The parallel constructor
//! keeps `init.rs` byte-identical (zero regression risk on the egui app that
//! lives until [TASK-024]) at the cost of a temporary duplication that dies
//! with the egui layer in [TASK-029].
//!
//! # Verified iced 0.14 builder chain
//!
//! Read from `iced-0.14.0/src/application.rs` (local registry source):
//!
//! | Builder method | Line |
//! | --- | --- |
//! | `iced::application(boot, update, view)` | `application.rs` (helper) |
//! | `.font(bytes)` | `:245` |
//! | `.window(window::Settings)` | `:253` |
//! | `.exit_on_close_request(bool)` | `:269` |
//! | `.window_size(Size)` | `:280` |
//! | `.position(window::Position)` | `:324` |
//! | `.title(...)` | `:346` |
//! | `.subscription(...)` | `:363` |
//! | `.theme(...)` | `:378` |
//! | `.scale_factor(...)` | `:410` |
//! | `.presets(...)` | `:448` |
//! | `.run() -> Result` | `:186` |
//!
//! The `(State, Task)` boot contract is `BootFn`/`IntoBoot`
//! (`application.rs:535-566`); `UpdateFn` is `:609-617`; `ViewFn` is
//! `:628-638`.
//!
//! Unit tests live in the sibling `app_tests.rs` sidecar.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use iced::widget::pane_grid;
use iced::{Size, Task};

use crate::background::Task as BackgroundTask;
use crate::background::logs::BackgroundLogs;
use crate::bus::core::Bus;
use crate::bus::events::config::ConfigArrived;
use crate::config::{AppConfig, ConfigStorageHandler, ContentLibrary};
use crate::ui::agent::panel_state::AgentPanelState;
use crate::ui::agent::transcript::AgentTranscript;
use crate::ui::app::FastMdApp;
use crate::ui::{Dialogs, FileSelection, PanelLayout, PersistedUiState, Tabs, TextBuffer};
use crate::ui_iced::fonts;
use crate::ui_iced::layout;
use crate::ui_iced::message::{BootMessage, Message, WindowMessage};
use crate::ui_iced::persisted::{self, UiStateStorage};
use crate::ui_iced::subscription::{self, BusReceivers};
use crate::ui_iced::update::update;
use crate::ui_iced::view::view;
use crate::workspace::Tags;
use crate::workspace::watcher::{DirectoryTracker, FileEventProcessor};
use fastmd_agent::AgentSession;

/// Window width ported from the egui `ViewportBuilder::with_inner_size`
/// (`src/app/main.rs:49`), in logical points; the DEFAULT applied when the
/// persisted state carries no window size ([TASK-009] (b)).
pub const WINDOW_WIDTH_POINTS: f32 = 1000.0;

/// Window height ported from the egui `ViewportBuilder::with_inner_size`
/// (`src/app/main.rs:49`); see [`WINDOW_WIDTH_POINTS`].
pub const WINDOW_HEIGHT_POINTS: f32 = 700.0;

/// Name given to the CLI-injected / current-directory workspace content
/// library (ported from `src/app/main.rs:82`, `:110`).
const WORKSPACE_LIBRARY_NAME: &str = "Workspace";

/// Content-library kind for a plain-text workspace ([RUST-021]).
const WORKSPACE_LIBRARY_KIND: &str = "text";

/// Environment variable that redirects the UI-state snapshot to an explicit
/// file ([`ui_state_storage_from_env`]). Kept from the [TASK-009] pilot
/// wiring so cross-run restore experiments can point it inside a scratch dir.
pub const UI_STATE_PATH_ENV: &str = "FASTMD_ICED_UI_STATE_PATH";

/// The iced application state ([TASK-022]).
///
/// A thin wrapper: the egui-free [`FastMdApp`] domain state plus the
/// iced-specific runtime bits. [TASK-023] grows the view-local half
/// (cursor anchor, overlay slot, dialog / about / debug window state,
/// scroll reports) onto this struct; the boot-owned half below is final.
pub struct AppState {
    /// The egui-free FastMD domain state (orchestrator + panels + persisted
    /// snapshot). All domain accessors route through here.
    pub app: FastMdApp,
    /// Live bus receivers for the [TASK-019] inbound subscription bridge.
    /// Built eagerly in [`build_state`] so the DEC-009 publish (which runs
    /// in the boot task, i.e. after construction) is never missed.
    pub receivers: BusReceivers,
    /// Injected UI-state storage handler ([RUST-024], DEC-006). Used by the
    /// close/save reduction in [`crate::ui_iced::update`].
    pub storage: Arc<dyn UiStateStorage>,
    /// OS-reported baseline scale captured at boot (DEC-008); `None` until
    /// the chained `window::latest()` → `window::scale_factor` task settles,
    /// in which case `1.0` applies.
    pub os_scale: Option<f32>,
    /// The last known logical window size; the pane-resize ratio input
    /// ([TASK-010]). Seeded from the booted (persisted) window size.
    pub viewport: Size,
    /// The [TASK-010] three-band middle pane-grid state.
    pub panes: pane_grid::State<layout::PaneKind>,
    /// The middle-band pane/split ids discovered at construction.
    pub band: layout::MiddleBandPanes,
}

impl AppState {
    /// The DEC-008 composed scale: OS baseline × persisted user multiplier.
    /// Fed to `.scale_factor(...)` per frame on the UI thread ([FMD-069]).
    pub fn composed_scale_factor(&self) -> f32 {
        persisted::compose_scale_factor(
            self.os_scale.unwrap_or(1.0),
            self.app.persisted_ui_state.font_size_scale,
        )
    }
}

/// `AppState` is not `Debug`-derivable (`FastMdApp` is not `Debug`), so the
/// iced runtime bits are reported and the domain state is named opaquely.
impl core::fmt::Debug for AppState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AppState")
            .field("app", &"<FastMdApp>")
            .field("receivers", &"<BusReceivers>")
            .field("storage", &"<dyn UiStateStorage>")
            .field("os_scale", &self.os_scale)
            .field("viewport", &self.viewport)
            .field("panes", &self.panes)
            .field("band", &self.band)
            .finish()
    }
}

/// Everything [`launch`] / [`boot`] needs, fully injected ([RUST-024]).
///
/// The composition root (a binary) resolves the concrete handlers; the
/// library never hardcodes a platform-default location. `Clone` so the
/// iced `BootFn` closure can re-call [`boot_with_persisted`] without
/// moving the captured handlers out.
#[derive(Clone)]
pub struct LaunchInputs {
    /// The loaded application configuration (pre-CLI-injection).
    pub config: AppConfig,
    /// The `Bus<ConfigArrived>` fan-out channel (DEC-009).
    pub config_bus: Bus<ConfigArrived>,
    /// Injected configuration storage handler ([RUST-024]).
    pub config_storage: Arc<dyn ConfigStorageHandler>,
    /// Injected persisted-UI-state storage handler ([RUST-024], DEC-006).
    pub ui_state_storage: Arc<dyn UiStateStorage>,
    /// Process arguments (`std::env::args()`), the input to the CLI
    /// workspace injection moved from `main.rs:59-115`.
    pub args: Vec<String>,
}

/// Builds the full [`AppState`] without touching `egui`/`eframe`
/// ([TASK-022], [DEC-011]).
///
/// Mirrors the plain wiring of `ui/app/init.rs::new` minus the egui theme /
/// font / repaint-callback setup, with three iced-specific differences:
///
/// 1. the persisted snapshot is supplied by the caller (loaded through the
///    injected [`UiStateStorage`]) rather than `eframe::Storage`;
/// 2. the loaded `config` is **direct-injected** into the orchestrator and
///    the agent (DEC-009) — the UI does not wait on the startup broadcast;
/// 3. the real background-event `rx` is handed to the [TASK-019]
///    [`BusReceivers`] bridge, so the orchestrator's own `rx` drain is
///    unused and gets an inert, never-fed channel ([FMD-065]: the bridge is
///    the single inbound path in the iced build).
///
/// Side effects at construction: spawns the background worker thread
/// (`BackgroundTask::new`), the MCP config-subscriber thread
/// (`spawn_config_subscription`) and the `std::sync::mpsc` → async
/// forwarding thread (inside [`BusReceivers`]) — the same threads the egui
/// `new` path spawns. None of them touch the filesystem until a config
/// arrives on the bus.
pub fn build_state(
    config: AppConfig,
    config_bus: &Bus<ConfigArrived>,
    config_storage: Arc<dyn ConfigStorageHandler>,
    ui_state_storage: Arc<dyn UiStateStorage>,
    persisted: PersistedUiState,
) -> AppState {
    // Background worker + event channel (mirrors init.rs::new).
    let background_task = BackgroundTask::new(config_bus.clone());
    // The tools manager subscribes to the same bus and performs the one-time
    // MCP initialisation off the UI thread.
    let tool_context = std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(
        crate::agent::AgentToolContext::new(crate::agent::tools::registry::ToolRegistry::new()),
    ));
    crate::agent::spawn_config_subscription(
        tool_context.clone(),
        config_bus.clone(),
        background_task.tx.clone(),
    );
    let finished_watcher_slot = background_task.finished_watcher.clone();
    let file_processor = FileEventProcessor::new(background_task.file_event_bus.subscribe());
    let background_manager = Arc::new(Mutex::new(BackgroundLogs::new()));
    crate::background::logs::register_ui_logs(background_manager.clone());

    let browser_session = std::sync::Arc::new(crate::agent::BrowserSession::new(
        &crate::config::AppConfig::default(),
    ));
    let pdf_backing_tracker = crate::agent::session::PdfBackingTracker::new();

    // The agent's streaming-event bus + observer fan-out (bus observer for
    // the UI, conversation logger for disk) — identical to the egui `new`
    // path, so the iced UI sees the same `AgentEvent` stream.
    let agent_event_bus = Bus::new();
    let agent_event_reader = agent_event_bus.subscribe();
    let agent_event_bus_clone = agent_event_bus.clone();
    let observer_factory: crate::agent::events::AgentObserverFactory =
        std::sync::Arc::new(move |session_id| {
            let bus_observer =
                std::sync::Arc::new(crate::bus::events::agent::BusAgentEventObserver::new(
                    session_id,
                    agent_event_bus_clone.clone(),
                ));
            let logger_observer = std::sync::Arc::new(
                crate::ui::agent::conversation_logger::ConversationLoggerObserver::new(
                    session_id,
                    crate::config::AppConfig::get_system_library_path().join("Conversations"),
                ),
            );
            std::sync::Arc::new(crate::bus::events::agent::CompositeAgentEventObserver::new(
                vec![bus_observer, logger_observer],
            ))
        });

    let agent_builder = AgentSession::builder()
        .with_observer_factory(observer_factory)
        .with_file_observer(std::sync::Arc::new(
            crate::agent::session::bus_observer::AppFileObserver::new(
                background_task.file_event_bus.clone(),
            ),
        ))
        .with_extension(browser_session.clone())
        .with_extension(Arc::new(crate::agent::tools::browser::BrowserExt(
            browser_session.clone(),
        )))
        .with_extension(Arc::new(pdf_backing_tracker.clone()))
        .with_tool_call_policy(Arc::new(pdf_backing_tracker.clone()))
        .with_tool_context(tool_context.clone());
    #[cfg(feature = "vector-search")]
    let agent_builder = agent_builder.with_extension(Arc::new(
        crate::agent::tools::vector_search::VectorSearchExt(
            background_task.vector_search_service.clone(),
        ),
    ));
    let agent = agent_builder.build();
    // DEC-009 direct injection: the agent gets the loaded config now, so it
    // never depends on the startup broadcast for its own initialisation.
    agent.set_agent_config(config.to_agent_config());

    let event_bus = background_task.file_event_bus;
    let dir_tracker = DirectoryTracker::new(event_bus.subscribe());

    // Project the persisted snapshot onto the live panel/selection/dialog
    // state (the egui `new` path's `init.rs:191-214`, egui-free).
    let mut persisted = persisted;
    let mut layout = PanelLayout::new();
    if let Some(w) = persisted.left_panel_width {
        layout.left_panel_width = Some(w);
    }
    if let Some(w) = persisted.right_panel_width {
        layout.right_panel_width = Some(w);
    }
    let mut selection = FileSelection::new();
    for dir in &persisted.expanded_dirs {
        selection.expanded_dirs.insert(dir.clone());
    }
    let mut dialogs = Dialogs::new();
    dialogs.batch_dialog_config = crate::agent::batch::types::BatchDialogConfig {
        available_dirs: config
            .content_libraries
            .iter()
            .map(|lib| PathBuf::from(&lib.root_folder))
            .collect(),
        ..Default::default()
    };
    // First-run auto-show (spec FR-016), carried over from `init.rs:210-214`.
    crate::ui::about_dialog::apply_first_run_auto_show(
        &mut persisted,
        &mut dialogs,
        crate::ui::about_dialog::APP_VERSION,
    );

    let user_command_bus = crate::bus::core::Bus::new();
    let user_command_reader = user_command_bus.subscribe();

    // The real background-event channel goes to the [TASK-019] bridge; the
    // orchestrator's own `rx` drain is unused in the iced build, so it gets
    // an inert, never-fed channel. `try_recv` on it returns immediately
    // (no blocking) — [FMD-065] single inbound path.
    let (_inert_tx, inert_rx) = std::sync::mpsc::channel();
    let receivers = BusReceivers::new(config_bus, &event_bus, &agent_event_bus, background_task.rx);

    let app = FastMdApp::from_parts(
        crate::orchestrator::AppOrchestrator {
            content_libraries: config.content_libraries.clone(),
            rx: inert_rx,
            tx: background_task.tx,
            file_event_reader: Some(event_bus.subscribe()),
            file_event_bus: event_bus,
            file_processor,
            pdf_backing_tracker,
            directory_tracker: dir_tracker,
            tags: Tags::new(),
            selection,
            tabs: Tabs::new(),
            _watcher: None,
            agent,
            dialogs,
            text_buffer: TextBuffer::new(),
            inline_editor_enabled: config.inline_editor_enabled,
            background_manager,
            config,
            config_storage,
            // The iced UI reads config arrivals through the subscription
            // bridge, not the orchestrator's sync reader.
            config_reader: None,
            pending_file_load: None,
            finished_watcher_slot,
            tool_context,
            agent_event_bus,
            agent_event_reader: Some(agent_event_reader),
            agent_event_lagged: false,
            agent_transcript: AgentTranscript::new(uuid::Uuid::nil()),
            agent_panel_state: AgentPanelState::new(),
            user_command_bus,
            user_command_reader: Some(user_command_reader),
        },
        layout,
        persisted,
    );

    let viewport = persisted::restore_window_size(&app.persisted_ui_state)
        .unwrap_or(Size::new(WINDOW_WIDTH_POINTS, WINDOW_HEIGHT_POINTS));
    let (panes, band) = layout::middle_band_state();

    AppState {
        app,
        receivers,
        storage: ui_state_storage,
        os_scale: None,
        viewport,
        panes,
        band,
    }
}

/// CLI-argument workspace injection, moved VERBATIM from `main.rs:61-115`
/// ([TASK-022]) and made a pure function of `(config, args)` so it is
/// unit-testable without a process.
///
/// Behaviour (unchanged):
/// 1. if `args[1]` names an existing directory, append it as a `Workspace`
///    content library unless one with the same root is already present;
/// 2. if the config still holds only the system library, append the current
///    working directory as a `Workspace` content library.
///
/// [RUST-020]: mutates only `config`; reads the filesystem for existence /
/// canonicalisation exactly as the original did.
pub fn apply_cli_workspace(config: &mut AppConfig, args: &[String]) {
    if let Some(arg) = args.get(1) {
        let path = PathBuf::from(arg);
        if path.exists() && path.is_dir() {
            let mut path_str = path
                .canonicalize()
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();
            if path_str.starts_with(r"\\?\") {
                path_str = path_str[4..].to_string();
            }
            let found = config
                .content_libraries
                .iter()
                .any(|lib| lib.root_folder == path_str);
            if !found {
                config.content_libraries.push(ContentLibrary {
                    root_folder: path_str,
                    name: WORKSPACE_LIBRARY_NAME.to_string(),
                    kind: WORKSPACE_LIBRARY_KIND.to_string(),
                    readonly: false,
                    priority: 0,
                });
            }
        }
    }

    if config.content_libraries.len() <= 1
        && config
            .content_libraries
            .iter()
            .all(|lib| lib.name == config.system_library_display_name())
    {
        let mut current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        if let Ok(canon) = std::fs::canonicalize(&current_dir) {
            current_dir = canon;
        }
        let mut path_str = current_dir.to_string_lossy().to_string();
        if path_str.starts_with(r"\\?\") {
            path_str = path_str[4..].to_string();
        }
        config.content_libraries.push(ContentLibrary {
            root_folder: path_str,
            name: WORKSPACE_LIBRARY_NAME.to_string(),
            kind: WORKSPACE_LIBRARY_KIND.to_string(),
            readonly: false,
            priority: 0,
        });
    }
}

/// Resolves the pilot/launch UI-state storage from the
/// `FASTMD_ICED_UI_STATE_PATH` override ([TASK-009] pilot wiring, kept for
/// the shared launch path).
///
/// A non-empty override stores snapshots in that explicit file
/// ([`persisted::FileUiStateStorage`]); otherwise snapshots stay in-process
/// ([`persisted::InMemoryUiStateStorage`]). The platform-default user
/// location is deliberately NOT resolved here so the pilot binary never
/// writes `%APPDATA%` by accident ([RUST-024], [RUST-006]); the real
/// production location wiring lands with the [TASK-024] entry flip.
pub fn ui_state_storage_from_env(env_override: Option<String>) -> Arc<dyn UiStateStorage> {
    match env_override {
        Some(path) if !path.is_empty() => {
            Arc::new(persisted::FileUiStateStorage::new(PathBuf::from(path)))
        }
        _ => Arc::new(persisted::InMemoryUiStateStorage::new()),
    }
}

/// The iced `BootFn` body ([TASK-022], DEC-009).
///
/// `impl BootFn<State, Message> for T where T: Fn() -> C, C: IntoBoot`
/// (`iced-0.14.0/src/application.rs:540-566`): this returns the
/// `(AppState, Task<Message>)` pair. The task chain is
/// [`Task::batch`] of:
///
/// * the [TASK-007] font-registration task, mapped onto
///   [`BootMessage::FontLoadFinished`];
/// * the DEC-008 scale-baseline capture — `iced::window::latest()`
///   (`Task<Option<Id>>`) chained into `iced::window::scale_factor(id)`
///   (`Task<f32>`) via `Task::then`, mapped onto
///   [`WindowMessage::ScaleBaseline`];
/// * the DEC-009 `Task::perform` that publishes `ConfigArrived` on the
///   existing bus for the background workers, mapped onto
///   [`BootMessage::ConfigPublished`].
///
/// The publish runs in a task (not inline) so every subscriber — the
/// background `Task` thread, the MCP subscriber and the eager
/// [`BusReceivers`] — is constructed *before* the event is sent, which is
/// the ordering guarantee the retired `main.rs:147-160` workaround enforced.
pub fn boot(inputs: LaunchInputs) -> (AppState, Task<Message>) {
    let persisted = persisted::load_ui_state(inputs.ui_state_storage.as_ref());
    boot_with_persisted(inputs, persisted)
}

/// [`boot`] with the persisted snapshot supplied up-front, so [`launch`] can
/// load it once and reuse it for both the window settings and the boot
/// closure without a second read.
pub fn boot_with_persisted(
    inputs: LaunchInputs,
    persisted: PersistedUiState,
) -> (AppState, Task<Message>) {
    let mut config = inputs.config;
    // CLI workspace injection moved from main.rs (UNCHANGED logic).
    apply_cli_workspace(&mut config, &inputs.args);

    let state = build_state(
        config.clone(),
        &inputs.config_bus,
        inputs.config_storage,
        inputs.ui_state_storage,
        persisted,
    );

    let font_task = fonts::font_boot_task()
        .map(|outcome| Message::Boot(BootMessage::FontLoadFinished(outcome)));
    let scale_task = scale_baseline_task();
    let publish_task = publish_config_task(inputs.config_bus, config);

    (state, Task::batch([font_task, scale_task, publish_task]))
}

/// DEC-008 baseline capture: `window::latest()` → `window::scale_factor(id)`
/// chained with `Task::then` (`iced_runtime-0.14.0/src/task.rs:128-156`).
fn scale_baseline_task() -> Task<Message> {
    iced::window::latest().then(|latest| match latest {
        Some(id) => iced::window::scale_factor(id)
            .map(|scale| Message::Window(WindowMessage::ScaleBaseline(scale))),
        None => Task::none(),
    })
}

/// DEC-009 config-arrival publish: the loaded config is sent on the existing
/// `Bus<ConfigArrived>` from a `Task::perform`
/// (`iced_runtime-0.14.0/src/task.rs:48-57`) so background workers observe
/// the arrival event after construction. The UI itself was direct-injected
/// in [`build_state`] and does not depend on this delivery.
fn publish_config_task(bus: Bus<ConfigArrived>, config: AppConfig) -> Task<Message> {
    Task::perform(
        async move { bus.publish(ConfigArrived::new(config)) },
        |subscribers| {
            tracing::info!(
                name = "ui_iced.boot.config_published",
                subscribers,
                "DEC-009: startup ConfigArrived published for background workers"
            );
            Message::Boot(BootMessage::ConfigPublished)
        },
    )
}

/// Builds the `window::Settings` for the launched window ([TASK-009] (b),
/// DEC-006/DEC-010).
///
/// Size/position seed from the loaded [`PersistedUiState`], falling back to
/// the egui-derived 1000×700 default (`main.rs:49`). `exit_on_close_request`
/// is `false` so the runtime waits for the app to persist before closing
/// (DEC-006 save-on-close).
pub fn window_settings(persisted_state: &PersistedUiState) -> iced::window::Settings {
    let size = persisted::restore_window_size(persisted_state)
        .unwrap_or(Size::new(WINDOW_WIDTH_POINTS, WINDOW_HEIGHT_POINTS));
    iced::window::Settings {
        size,
        position: persisted::restore_window_position(persisted_state),
        icon: window_icon(),
        exit_on_close_request: false,
        ..Default::default()
    }
}

/// Loads the application icon as an iced window icon ([TASK-006]).
///
/// Decodes the SAME shared asset (`assets/icon-32.png`) that `main.rs` feeds
/// `egui::ViewportBuilder::with_icon`, via
/// `crate::utils::app_icon::decode_app_icon_rgba`, then wraps the RGBA
/// pixels with `iced::window::icon::from_rgba`. Falls back to `None` (iced
/// default) if decoding fails.
fn window_icon() -> Option<iced::window::Icon> {
    let (rgba, width, height) = crate::utils::app_icon::decode_app_icon_rgba()?;
    iced::window::icon::from_rgba(rgba, width, height).ok()
}

/// The full iced launch builder chain ([TASK-022], DEC-006/008/009/010).
///
/// Wires [`boot`] + [`crate::ui_iced::update::update`] +
/// [`crate::ui_iced::view::view`] through the verified
/// `iced::application(...)` chain:
///
/// * `.title(strings::WINDOW_TITLE)` — "FastMD AI" (DEC-010);
/// * `.window(window_settings)` — persisted size/position + icon +
///   `exit_on_close_request: false` (DEC-006);
/// * `.theme(...)` — the custom FastMD dark theme ([`crate::ui_iced::theme`],
///   [TASK-008]);
/// * `.scale_factor(...)` — the DEC-008 composed baseline × multiplier;
/// * `.subscription(...)` — the [TASK-019] bus bridge over the live
///   [`BusReceivers`].
///
/// Fonts register through the [TASK-007] boot task (returned by [`boot`])
/// rather than `.font(bytes)`, matching the pilot's working pattern.
///
/// Not unit-testable headlessly (it calls `.run()`); the pieces it composes
/// are tested in the sidecar.
pub fn launch(inputs: LaunchInputs) -> iced::Result {
    let persisted = persisted::load_ui_state(inputs.ui_state_storage.as_ref());
    let settings = window_settings(&persisted);

    let result = iced::application(
        // The closure clones the captured inputs per call so the Elm boot
        // stays re-callable and deterministic ([FMD-068]); it never moves
        // the injected handlers out.
        move || boot_with_persisted(inputs.clone(), persisted.clone()),
        update,
        view,
    )
    .title(crate::ui::strings::WINDOW_TITLE)
    .window(settings)
    .theme(|_state: &AppState| crate::ui_iced::theme::fastmd_dark_theme())
    .scale_factor(|state: &AppState| state.composed_scale_factor())
    .subscription(|state: &AppState| subscription::subscription(&state.receivers))
    .run();

    // egui `on_exit` port (`ui/app/mod.rs:245-248`): temp-file cleanup and
    // UI-log unregistration run when the iced runtime returns.
    crate::export::print::cleanup_temp_files();
    crate::background::logs::unregister_ui_logs();

    result
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
