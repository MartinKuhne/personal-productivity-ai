//! Iced application root — the framework-free state construction, the `boot`
//! entry and the `launch()` builder chain ([TASK-022], DEC-006/008/009/010;
//! `doc/planning/iced.md`).
//!
//! # What lives here
//!
//! * [`AppState`] — the iced runtime state. It is a **thin wrapper** around
//!   the framework-free [`FastMdApp`] domain state plus the iced-specific runtime
//!   bits the domain struct never carried (the DEC-008 OS scale baseline, the
//!   [TASK-010] pane-grid, the live [`BusReceivers`] and the injected
//!   [`UiStateStorage`]). It is constructible with zero framework
//!   references — the whole `ui_iced` tree is ([DEC-011]).
//! * [`build_state`] — the **parallel framework-free constructor**. It mirrors
//!   the plain (non-the legacy UI) wiring of `ui/app/init.rs::new` — background
//!   `Task`, MCP config subscriber, agent session, directory tracker,
//!   persisted-UI-state projection — but resolves the persisted snapshot
//!   through the injected [`UiStateStorage`] instead of the legacy `Storage` backend,
//!   direct-injects the loaded [`AppConfig`] (DEC-009) and hands the real
//!   background-event channel to the [TASK-019] subscription bridge.
//! * [`boot`] / [`boot_with_persisted`] — the iced `BootFn` body: build the
//!   state, then return the batched boot task chain (font registration,
//!   the DEC-008 scale-baseline capture). The DEC-009 `ConfigArrived`
//!   publish is synchronous inside [`build_state`].
//! * [`launch`] — the full verified builder chain.
//!
//! # Why a parallel constructor rather than a shared `init.rs` core
//!
//! `init.rs::new` and the iced boot differ in exactly one structural way
//! that cannot be parameterised without touching the live the legacy UI path: the
//! background-event channel `rx: std::sync::mpsc::Receiver<BackgroundEvent>`
//! is **single-owner**. The legacy UI path moves it into `AppOrchestrator.rx` and
//! drains it per frame; the iced path must move it into the [TASK-019]
//! `BusReceivers` bridge so background notifications arrive as
//! `Message::Inbound`. Sharing one core would force a fate-flag or an inert
//! dummy channel into the shipping the legacy UI build. The parallel constructor
//! keeps `init.rs` byte-identical (zero regression risk on the legacy UI app that
//! lives until [TASK-024]) at the cost of a temporary duplication that dies
//! with the legacy UI layer in [TASK-029].
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
use crate::ui_iced::about::{AboutViewModel, AboutWindowState};
use crate::ui_iced::agent::panel_state::AgentPanelViewModel;
use crate::ui_iced::agent_debug::{AgentDebugViewModel, AgentDebugWindowState};
use crate::ui_iced::attributions::AttributionsViewModel;
use crate::ui_iced::batch_dialog::{BatchDialogState, BatchDialogViewModel, BatchRunStatus};
use crate::ui_iced::dialogs::DialogState;
use crate::ui_iced::editor::EditorViewModel;
use crate::ui_iced::fonts;
use crate::ui_iced::layout;
use crate::ui_iced::logs::{BackgroundLogsViewModel, ScrollSnapshot};
use crate::ui_iced::message::{BootMessage, Message, WindowMessage};
use crate::ui_iced::overlay::MenuItem;
use crate::ui_iced::panels::bottom::BottomPanelViewModel;
use crate::ui_iced::panels::center::CenterPanelViewModel;
use crate::ui_iced::panels::left::LeftPanelViewModel;
use crate::ui_iced::panels::top::TopBarViewModel;
use crate::ui_iced::persisted::{self, UiStateStorage};
use crate::ui_iced::search::SearchViewModel;
use crate::ui_iced::subscription::{self, BusReceivers};
use crate::ui_iced::tools_dialog::{ToolsDialogState, ToolsDialogViewModel};
use crate::ui_iced::update::update;
use crate::ui_iced::view::view;
use crate::workspace::Tags;
use crate::workspace::watcher::{DirectoryTracker, FileEventProcessor};
use fastmd_agent::AgentSession;

/// Window width ported from the legacy UI `ViewportBuilder::with_inner_size`
/// (`src/app/main.rs:49`), in logical points; the DEFAULT applied when the
/// persisted state carries no window size ([TASK-009] (b)).
pub const WINDOW_WIDTH_POINTS: f32 = 1000.0;

/// Window height ported from the legacy UI `ViewportBuilder::with_inner_size`
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

/// The iced application state ([TASK-022], [TASK-023]).
///
/// A thin wrapper: the framework-free [`FastMdApp`] domain state plus the
/// iced-specific runtime bits. [TASK-023] adds the view-model half
/// ([`ViewModels`]) and the floating-window half ([`OverlayState`]); the
/// boot-owned half below is final.
pub struct AppState {
    /// The framework-free FastMD domain state (orchestrator + panels + persisted
    /// snapshot). All domain accessors route through here.
    pub app: FastMdApp,
    /// Live bus receivers for the [TASK-019] inbound subscription bridge.
    /// Built eagerly in [`build_state`] so the DEC-009 publish (which runs
    /// in the boot task, i.e. after construction) is never missed.
    pub receivers: BusReceivers,
    /// Injected UI-state storage handler ([RUST-024], DEC-006). Used by the
    /// close/save reduction in [`crate::ui_iced::update`].
    pub storage: Arc<dyn UiStateStorage>,
    /// OS-reported DPI baseline captured at boot (DEC-008); `None` until the
    /// chained `window::latest()` → `window::scale_factor` task settles.
    ///
    /// Diagnostic only — read through [`AppState::effective_scale_factor`].
    /// iced already multiplies the `.scale_factor(...)` closure's return value
    /// by this same OS factor, so it must NOT be folded back into the request
    /// (see [`AppState::requested_scale_factor`]).
    pub os_scale: Option<f32>,
    /// The last known logical window size; the pane-resize ratio input
    /// ([TASK-010]). Seeded from the booted (persisted) window size.
    pub viewport: Size,
    /// The [TASK-010] three-band middle pane-grid state.
    pub panes: pane_grid::State<layout::PaneKind>,
    /// The middle-band pane/split ids discovered at construction.
    pub band: layout::MiddleBandPanes,
    /// Every panel and dialog view-model the view renders, rebuilt at the end
    /// of each `update` by [`AppState::rebuild_view_models`] ([TASK-023]).
    pub models: ViewModels,
    /// Reducer-owned floating-window state ([TASK-023]).
    pub overlay: OverlayState,
}

impl AppState {
    /// The value to hand to iced's `.scale_factor(...)` builder ([DEC-008]).
    ///
    /// **This is the user multiplier alone.** iced 0.14 multiplies whatever this
    /// closure returns by the window's own OS DPI factor before it builds the
    /// viewport (`iced_winit-0.14.1/src/window/state.rs:68` and `:212`:
    /// `Viewport::with_physical_size(physical, window.scale_factor() * scale_factor)`),
    /// so folding [`AppState::os_scale`] in here applies the OS scale **twice**.
    /// That was the bug that made the whole UI jump larger a moment after boot:
    /// the first frame renders at `os × 1.0`, then the DEC-008 scale-baseline
    /// task lands and the composed value became `os × os`.
    ///
    /// The DEC-008 intent — "the persisted multiplier sits on top of the OS
    /// baseline" — is preserved, because iced supplies the baseline itself.
    #[must_use]
    pub fn requested_scale_factor(&self) -> f32 {
        self.app.persisted_ui_state.font_size_scale.unwrap_or(1.0)
    }

    /// The effective on-screen scale: the OS DPI factor iced applies times the
    /// persisted user multiplier ([DEC-008]).
    ///
    /// Diagnostic only — never feed this to `.scale_factor(...)`, it would
    /// double-count the OS factor (see [`Self::requested_scale_factor`]).
    #[must_use]
    pub fn effective_scale_factor(&self) -> f32 {
        persisted::compose_scale_factor(
            self.os_scale.unwrap_or(1.0),
            self.app.persisted_ui_state.font_size_scale,
        )
    }

    /// Re-snapshots every view-model from the domain state and the
    /// reducer-held overlay state ([TASK-023]).
    ///
    /// This is the resolution of the view-model lifetime problem: every
    /// panel function returns an `Element<'_, Message>` that **borrows** its
    /// view-model, so a view-model built inside `view` cannot outlive the
    /// call (E0515). Building them at the end of `update` instead — where
    /// the domain state has just finished mutating — lets `view` stay a pure
    /// borrow over `&AppState`.
    pub fn rebuild_view_models(&mut self) {
        self.models = ViewModels::from_app(&self.app, &self.overlay);
    }

    /// Which floating windows are showing, bottom-most layer first
    /// ([TASK-023]). Delegates to [`mounted_layers`].
    #[must_use]
    pub fn mounted_layers(&self) -> Vec<OverlayLayer> {
        mounted_layers(&self.overlay, &self.models)
    }
}

/// A floating context menu: where it was opened and the rows it shows
/// ([TASK-023], DEC-005).
///
/// The reducer builds the rows from the panel view-model that owns the
/// target (`LeftPanelViewModel::menu_items_for` /
/// `CenterPanelViewModel::menu_items_for`) so the view stays a pure
/// projection and never reads the domain model to decide menu contents.
#[derive(Clone, Debug, PartialEq)]
pub struct ContextMenu {
    /// Top-left anchor in window coordinates, taken from the
    /// [`LayoutMessage::CursorMoved`](crate::ui_iced::message::LayoutMessage::CursorMoved)
    /// that preceded the open request.
    pub anchor: iced::Point,
    /// The rows to render, in display order.
    pub items: Vec<MenuItem<Message>>,
}

/// Reducer-owned floating-window state ([TASK-023]).
///
/// the legacy UI kept each of these as a `bool` on [`crate::ui::Dialogs`] plus
/// widget memory for the contents. iced has to hold the contents
/// explicitly, and because the legacy UI build could genuinely show several at
/// once (a context menu over an open About window), the single-blocking
/// overlay [`crate::ui_iced::overlay::OverlaySlot`] does not describe the
/// real model. One slot per window does; [`mounted_layers`] turns them back
/// into a z-ordered stack.
#[derive(Debug)]
pub struct OverlayState {
    /// The small name-entry / move / delete-confirmation dialog.
    pub dialog: DialogState,
    /// Where the small dialog was anchored (`None` = centred).
    pub dialog_anchor: Option<iced::Point>,
    /// The floating tree / tab context menu.
    pub context_menu: Option<ContextMenu>,
    /// Whether the top-bar hamburger menu is open ([TASK-023] fix). A
    /// reducer-held bool because the menu card is mounted by the shared
    /// overlay host, which rebuilds from state every frame.
    pub hamburger_open: bool,
    /// The inline editor buffer (owns the live `text_editor::Content`, so
    /// the overlay as a whole is not `Clone`).
    pub editor: Option<EditorViewModel>,
    /// The agent debug window's disclosure state.
    pub debug: AgentDebugWindowState,
    /// The tools dialog window.
    pub tools: ToolsDialogState,
    /// The About / attributions window.
    pub about: AboutWindowState,
    /// The batch prompt dialog and its composed selection.
    pub batch: BatchDialogState,
    /// The workspace content-search state (the left panel's search box).
    pub search: SearchViewModel,
    /// Last reported pointer position, the anchor source for context menus.
    pub cursor: Option<iced::Point>,
    /// The agent transcript's "Thinking Process" disclosure (replaces
    /// the legacy UI's `collapsing` widget memory).
    pub thinking_expanded: bool,
    /// Last reported background-logs viewport.
    pub logs_scroll: Option<ScrollSnapshot>,
    /// Last reported agent-transcript viewport.
    pub agent_scroll: Option<ScrollSnapshot>,
    /// Last reported debug-entry viewport.
    pub debug_scroll: Option<ScrollSnapshot>,
}

impl Default for OverlayState {
    /// A closed-everything overlay with the legacy UI-matching defaults: the
    /// thinking block starts expanded (the legacy UI's `collapsing` opened that way)
    /// and no scroll viewport has been reported yet.
    fn default() -> Self {
        Self {
            dialog: DialogState::new(),
            dialog_anchor: None,
            context_menu: None,
            hamburger_open: false,
            editor: None,
            debug: AgentDebugWindowState::new(),
            tools: ToolsDialogState::new(),
            about: AboutWindowState::new(),
            batch: BatchDialogState::new(),
            search: SearchViewModel::new(),
            cursor: None,
            thinking_expanded: true,
            logs_scroll: None,
            agent_scroll: None,
            debug_scroll: None,
        }
    }
}

/// One floating-window layer of the overlay host, bottom-most first
/// ([TASK-023]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayLayer {
    /// The floating tree / tab context menu.
    ContextMenu,
    /// The inline editor modal.
    Editor,
    /// A small name-entry / move / delete-confirmation dialog.
    Dialog,
    /// The background-process log viewer.
    Logs,
    /// The agent debug window.
    AgentDebug,
    /// The tools dialog.
    Tools,
    /// The About window.
    About,
    /// The third-party attributions window.
    Attributions,
    /// The batch prompt-processing dialog (topmost — the legacy UI drew it last).
    Batch,
}

/// The z-ordered set of showing layers ([TASK-023]).
///
/// Mirrors the legacy UI draw order of `ui/app/render.rs`: the context menu sat
/// above the panels, the editor above that, then the small dialogs, the log
/// viewer, the debug window, the tools dialog, the identity windows and the
/// batch dialog last.
///
/// [RUST-020]: pure.
#[must_use]
pub fn mounted_layers(overlay: &OverlayState, models: &ViewModels) -> Vec<OverlayLayer> {
    let mut layers = Vec::new();
    if overlay
        .context_menu
        .as_ref()
        .is_some_and(|menu| !menu.items.is_empty())
    {
        layers.push(OverlayLayer::ContextMenu);
    }
    if overlay.editor.is_some() {
        layers.push(OverlayLayer::Editor);
    }
    if overlay.dialog.is_open() {
        layers.push(OverlayLayer::Dialog);
    }
    if models.logs.visible {
        layers.push(OverlayLayer::Logs);
    }
    if models.debug.visible {
        layers.push(OverlayLayer::AgentDebug);
    }
    if overlay.tools.is_open() {
        layers.push(OverlayLayer::Tools);
    }
    if overlay.about.is_about() {
        layers.push(OverlayLayer::About);
    }
    if overlay.about.is_attributions() {
        layers.push(OverlayLayer::Attributions);
    }
    if overlay.batch.is_open() {
        layers.push(OverlayLayer::Batch);
    }
    layers
}

/// Every panel and dialog view-model the iced view renders ([TASK-023]).
///
/// The view holds no state of its own: each field is a snapshot the reducer
/// rebuilt after the last mutation, which is what lets
/// [`crate::ui_iced::view::view`] borrow them (see
/// [`AppState::rebuild_view_models`] for why they cannot be built there).
#[derive(Debug)]
pub struct ViewModels {
    /// Top toolbar.
    pub top: TopBarViewModel,
    /// Left file-tree / search panel.
    pub left: LeftPanelViewModel,
    /// Center tab strip and document viewer.
    pub center: CenterPanelViewModel,
    /// Bottom command-input band.
    pub bottom: BottomPanelViewModel,
    /// Agent session panel.
    pub agent: AgentPanelViewModel,
    /// Background-process log viewer.
    pub logs: BackgroundLogsViewModel,
    /// Agent debug window.
    pub debug: AgentDebugViewModel,
    /// Tools dialog card.
    pub tools: ToolsDialogViewModel,
    /// Batch prompt dialog card.
    pub batch: BatchDialogViewModel,
    /// About card.
    pub about: AboutViewModel,
    /// Third-party attributions card.
    pub attributions: AttributionsViewModel,
}

impl ViewModels {
    /// Snapshots every view-model off `app`, merging in the reducer-held
    /// state that `from_app` cannot see ([TASK-023]).
    ///
    /// Three merges are required because the legacy UI originals read state the
    /// iced reducer owns instead:
    ///
    /// * the left panel's search fields come from [`SearchViewModel`], not
    ///   the legacy UI `TreeSearch` that `LeftPanelViewModel::from_app` reads
    ///   (and which stays empty in the iced build);
    /// * the agent panel's disclosure comes from
    ///   [`OverlayState::thinking_expanded`], because
    ///   `AgentPanelViewModel::from_app` hardcodes `true`;
    /// * the debug window's expanded rows come from [`AgentDebugWindowState`],
    ///   because `AgentDebugViewModel::from_app` returns an empty set.
    ///
    /// Side effects: the remaining filesystem reads are the tree flatten and
    /// the skill listings. Everything a closed window would need is skipped so
    /// a workspace scan cannot starve the UI thread ([FMD-060]):
    ///
    /// * the Typst PATH probe is memoised process-wide
    ///   (`fastmd_pdf::find_typst_binary`);
    /// * the note-skill folder is read **once** here and handed to both the
    ///   left panel and the tab strip ([`note_skill_files`]);
    /// * the Tools registry deep clone is built only while that dialog is open;
    /// * the debug window's per-entry deep clone + JSON pretty-print is skipped
    ///   while it is closed ([`AgentDebugViewModel::closed`]);
    /// * the background-log ring buffer is not cloned while the viewer is
    ///   closed ([`BackgroundLogsViewModel::closed`]) — which also stops the
    ///   UI contending that mutex with the worker thread that fills it;
    /// * the agent transcript is not re-parsed while the session is off screen.
    ///
    /// [`note_skill_files`]: crate::ui_iced::panels::left::note_skill_files
    /// [`AgentDebugViewModel::closed`]: crate::ui_iced::agent_debug::AgentDebugViewModel::closed
    /// [`BackgroundLogsViewModel::closed`]: crate::ui_iced::logs::BackgroundLogsViewModel::closed
    #[must_use]
    pub fn from_app(app: &FastMdApp, overlay: &OverlayState) -> Self {
        // The note-skill folder backs both the left panel's file menu and the
        // tab strip's context menu. Read it once per rebuild and hand the same
        // listing to both, instead of each panel hitting the directory itself
        // ([FMD-060]).
        let note_skills = crate::ui_iced::panels::left::note_skill_files(app);

        let mut left = LeftPanelViewModel::from_app(app);
        left.query = overlay.search.query().to_string();
        left.is_searching = overlay.search.is_filtering();
        left.results = overlay.search.results_arc();

        let agent =
            AgentPanelViewModel::from_app(app).with_thinking_expanded(overlay.thinking_expanded);

        let mut debug = AgentDebugViewModel::from_app(app);
        debug.expanded = overlay.debug.expanded_ids().clone();

        let batch =
            BatchDialogViewModel::from_config(&app.orchestrator.dialogs.batch_dialog_config)
                .with_status(batch_run_status(&app.orchestrator.dialogs));

        // The tools snapshot deep-clones the whole tool-group registry plus
        // per-tool auth facts and char counts. It is only rendered while the
        // dialog is open, so building it every message is pure waste during
        // a workspace scan ([FMD-060]).
        let tools = if overlay.tools.is_open() {
            ToolsDialogViewModel::from_app(app)
        } else {
            ToolsDialogViewModel::empty()
        };

        Self {
            top: TopBarViewModel::from_app(app),
            left,
            center: CenterPanelViewModel::from_app_with_note_skills(app, note_skills),
            bottom: BottomPanelViewModel::from_app(app),
            agent,
            logs: BackgroundLogsViewModel::from_app(app),
            debug,
            tools,
            batch,
            about: AboutViewModel::from_build(),
            attributions: AttributionsViewModel::from_catalog(),
        }
    }
}

/// Projects the legacy UI [`crate::ui::Dialogs`] batch-run fields onto the iced
/// [`BatchRunStatus`] the card renders ([TASK-023]).
///
/// [RUST-020]: pure — reads the handle's liveness, mutates nothing.
#[must_use]
fn batch_run_status(dialogs: &crate::ui::Dialogs) -> BatchRunStatus {
    use std::sync::atomic::Ordering;

    BatchRunStatus {
        has_handle: dialogs.batch_handle.is_some(),
        finished: dialogs
            .batch_handle
            .as_ref()
            .is_some_and(|handle| handle.thread.is_finished()),
        cancel_requested: dialogs
            .batch_cancel_flag
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed)),
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
            .field("models", &self.models)
            .field("overlay", &self.overlay)
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

/// Builds the full [`AppState`] without touching framework
/// ([TASK-022], [DEC-011]).
///
/// Mirrors the plain wiring of `ui/app/init.rs::new` minus the legacy UI theme /
/// font / repaint-callback setup, with three iced-specific differences:
///
/// 1. the persisted snapshot is supplied by the caller (loaded through the
///    injected [`UiStateStorage`]) rather than the legacy `Storage` backend;
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
/// forwarding thread (inside [`BusReceivers`]) — the same threads the legacy UI
/// `new` path spawns. The DEC-009 startup `ConfigArrived` is published
/// synchronously right before returning, after every subscriber above is
/// registered, so the `CONFIG_ARRIVAL_TIMEOUT` waits never miss it. Until
/// then, none of them touch the filesystem.
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
    // the UI, conversation logger for disk) — identical to the legacy UI `new`
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
    // state (the legacy UI `new` path's `init.rs:191-214`, framework-free).
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
    crate::ui_iced::about::apply_first_run_auto_show(
        &mut persisted,
        &mut dialogs,
        crate::ui_iced::about::APP_VERSION,
    );

    let user_command_bus = crate::bus::core::Bus::new();
    let user_command_reader = user_command_bus.subscribe();

    // The real background-event channel goes to the [TASK-019] bridge; the
    // orchestrator's own `rx` drain is unused in the iced build, so it gets
    // an inert, never-fed channel. `try_recv` on it returns immediately
    // (no blocking) — [FMD-065] single inbound path.
    let (_inert_tx, inert_rx) = std::sync::mpsc::channel();
    let receivers = BusReceivers::new(config_bus, &event_bus, &agent_event_bus, background_task.rx);

    // DEC-009 startup `ConfigArrived` publish, performed **synchronously:
    // the subscribers it must reach — the background worker thread
    // (`BackgroundTask::new`), the MCP config subscriber
    // (`spawn_config_subscription`) and the [TASK-019] bridge's config
    // lane (this publish is the third and last subscriber) — are all
    // registered above, so ordering is guaranteed without a task
    // round-trip. Publishing through an iced `Task` (the former
    // `publish_config_task`) deferred the send to the iced scheduler,
    // which can land seconds after `boot` (behind the font-registration
    // boot task); both waiting subscribers then hit their
    // `CONFIG_ARRIVAL_TIMEOUT`, fell back to `AppConfig::default()` and
    // the initial library scan discovered nothing.
    let subscribers = config_bus.publish(ConfigArrived::new(config.clone()));
    tracing::info!(
        name = "ui_iced.boot.config_published",
        subscribers,
        "DEC-009: startup ConfigArrived published for background workers"
    );

    let app = FastMdApp::from_parts(
        crate::orchestrator::AppOrchestrator {
            content_libraries: config.content_libraries.clone(),
            rx: inert_rx,
            tx: background_task.tx,
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
            // [FMD-065] single inbound path: the [TASK-019] bridge owns
            // `Bus<FileEvent>` delivery and the reducer applies each
            // `Inbound(FileChanged)` payload through
            // `AppOrchestrator::apply_file_event`. A live reader here would
            // apply every event a second time, so it is inert — the same
            // treatment `rx` above gets.
            file_event_reader: None,
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

    // The view-model half is seeded here so `view` is renderable from the
    // very first frame; every later frame re-snapshots it at the end of
    // `update` ([TASK-023]).
    let overlay = OverlayState::default();
    let models = ViewModels::from_app(&app, &overlay);

    AppState {
        app,
        receivers,
        storage: ui_state_storage,
        os_scale: None,
        viewport,
        panes,
        band,
        models,
        overlay,
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
///   [`WindowMessage::ScaleBaseline`].
///
/// The DEC-009 `ConfigArrived` publish for the background workers happens
/// synchronously inside [`build_state`] and is therefore not part of the
/// task chain (see its doc comment for why a `Task::perform` publish was
/// retired).
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

    // The DEC-009 `ConfigArrived` publish is performed synchronously inside
    // [`build_state`] (right after its last subscriber registers), not as a
    // `Task::perform` here: a boot task is executed by the iced scheduler
    // and can land seconds after `boot` returns, which made every
    // `CONFIG_ARRIVAL_TIMEOUT` subscriber miss the arrival and scan
    // nothing.
    (state, Task::batch([font_task, scale_task]))
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

/// Builds the `window::Settings` for the launched window ([TASK-009] (b),
/// DEC-006/DEC-010).
///
/// Size/position seed from the loaded [`PersistedUiState`], falling back to
/// the legacy UI-derived 1000×700 default (`main.rs:49`). `exit_on_close_request`
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
/// `ViewportBuilder::with_icon`, via
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
/// * `.scale_factor(...)` — the DEC-008 user multiplier; iced composes it
///   with the OS DPI factor itself (see
///   [`AppState::requested_scale_factor`]);
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
    .scale_factor(|state: &AppState| state.requested_scale_factor())
    .subscription(|state: &AppState| subscription::subscription(&state.receivers))
    .run();

    // the legacy UI `on_exit` port (`ui/app/mod.rs:245-248`): temp-file cleanup and
    // UI-log unregistration run when the iced runtime returns.
    crate::export::print::cleanup_temp_files();
    crate::background::logs::unregister_ui_logs();

    result
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
