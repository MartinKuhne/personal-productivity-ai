//! Iced application root — the framework-free state construction, the `boot`
//! entry and the `launch()` builder chain.
//!
//! # What lives here
//!
//! * [`AppState`] — the iced runtime state. It is a **thin wrapper** around
//!   the framework-free [`FastMdApp`] domain state plus the iced-specific runtime
//!   bits the domain struct does not carry: the OS scale baseline, the
//!   pane-grid, the live [`BusReceivers`] and the injected [`UiStateStorage`].
//!   It is constructible with zero framework references, as is the whole
//!   `ui_iced` tree.
//! * [`build_state`] — the framework-free constructor. It wires the background
//!   `Task`, MCP config subscriber, agent session, directory tracker and
//!   persisted-UI-state projection, resolves the persisted snapshot through
//!   the injected [`UiStateStorage`], direct-injects the loaded [`AppConfig`]
//!   and hands the real background-event channel to the subscription bridge.
//! * [`boot`] / [`boot_with_persisted`] — the iced `BootFn` body: build the
//!   state, then return the batched boot task chain (font registration and
//!   the scale-baseline capture). The `ConfigArrived` publish is synchronous
//!   inside [`build_state`].
//! * [`launch`] — the full verified builder chain.
//!
//! # Why the orchestrator's `rx` is inert
//!
//! The background-event channel `rx: std::sync::mpsc::Receiver<BackgroundEvent>`
//! is **single-owner**. [`build_state`] moves the real receiver into the
//! [`BusReceivers`] bridge so background notifications arrive as
//! `Message::Inbound`; the orchestrator's own drain is therefore never used
//! and is handed an inert, never-fed channel instead. `try_recv` on that
//! channel returns immediately, so the unused drain cannot stall the update
//! loop — background work reaches the UI through exactly one inbound path.
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
use crate::ui_iced::menu::{MenuItem, MenuState};
use crate::ui_iced::message::{BootMessage, Message, WindowMessage};
use crate::ui_iced::overlay::MenuItem as LegacyMenuItem;
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
use crate::workspace::watcher::Workspace;
use fastmd_agent::AgentSession;
use iced_pro::binding::pre_rank::DragCapture;
use iced_pro::binding::pre_rank::PreRankCoordinator;
use iced_pro::surface::zorder::DeclineReason;
use iced_pro::surface::zorder::PreRankedEscape;
use iced_pro::surface::zorder::Surface;
use iced_pro::surface::zorder::SurfaceId;
use iced_pro::surface::zorder::SurfaceKind;
use iced_pro::surface::zorder::SurfaceRegistry;
use iced_pro::surface::zorder::UnwindAnswer;
use iced_pro::surface::zorder::escape_with_pre_rank;

/// Default window width, in logical points, applied when the persisted state
/// carries no window size.
pub const WINDOW_WIDTH_POINTS: f32 = 1000.0;

/// Default window height, in logical points; see [`WINDOW_WIDTH_POINTS`].
pub const WINDOW_HEIGHT_POINTS: f32 = 700.0;

/// Name given to the CLI-injected / current-directory workspace content
/// library.
const WORKSPACE_LIBRARY_NAME: &str = "Workspace";

/// Content-library kind for a plain-text workspace ([RUST-021]).
const WORKSPACE_LIBRARY_KIND: &str = "text";

/// Environment variable that redirects the UI-state snapshot to an explicit
/// file ([`ui_state_storage_from_env`]), so cross-run restore experiments can
/// point it inside a scratch dir.
pub const UI_STATE_PATH_ENV: &str = "FASTMD_ICED_UI_STATE_PATH";

/// Name of the production boot preset produced by [`launch_presets`].
///
/// The tier-4 `.ice` harness selects a boot strategy by this string; the
/// live launch chain never reads it (see [`launch_presets`]).
pub const LAUNCH_PRESET_DEFAULT: &str = "default";

/// The iced application state.
///
/// A thin wrapper: the framework-free [`FastMdApp`] domain state plus the
/// iced-specific runtime bits — the view-model half ([`ViewModels`]) and the
/// floating-window half ([`OverlayState`]).
pub struct AppState {
    /// The framework-free FastMD domain state (orchestrator + panels + persisted
    /// snapshot). All domain accessors route through here.
    pub app: FastMdApp,
    /// Live bus receivers for the inbound subscription bridge. Built eagerly in
    /// [`build_state`], before that function publishes the startup
    /// `ConfigArrived`, so the publish is never missed.
    pub receivers: BusReceivers,
    /// Injected UI-state storage handler ([RUST-024]). Used by the
    /// close/save reduction in [`crate::ui_iced::update`].
    pub storage: Arc<dyn UiStateStorage>,
    /// OS-reported DPI baseline captured at boot; `None` until the chained
    /// `window::latest()` → `window::scale_factor` task settles.
    ///
    /// Diagnostic only — read through [`AppState::effective_scale_factor`].
    /// iced already multiplies the `.scale_factor(...)` closure's return value
    /// by this same OS factor, so it must NOT be folded back into the request
    /// (see [`AppState::requested_scale_factor`]).
    pub os_scale: Option<f32>,
    /// The last known logical window size; the pane-resize ratio input. Seeded
    /// from the booted (persisted) window size.
    pub viewport: Size,
    /// The three-band middle pane-grid state.
    pub panes: pane_grid::State<layout::PaneKind>,
    /// The middle-band pane/split ids discovered at construction.
    pub band: layout::MiddleBandPanes,
    /// Every panel and dialog view-model the view renders, rebuilt at the end
    /// of each `update` by [`AppState::rebuild_view_models`].
    pub models: ViewModels,
    /// Reducer-owned floating-window state.
    pub overlay: OverlayState,
}

impl AppState {
    /// The value to hand to iced's `.scale_factor(...)` builder.
    ///
    /// **This is the user multiplier alone.** iced 0.14 multiplies whatever this
    /// closure returns by the window's own OS DPI factor before it builds the
    /// viewport (`iced_winit-0.14.1/src/window/state.rs:68` and `:212`:
    /// `Viewport::with_physical_size(physical, window.scale_factor() * scale_factor)`),
    /// so folding [`AppState::os_scale`] in here would apply the OS scale
    /// **twice**. The persisted multiplier sits on top of the OS baseline, and
    /// iced supplies that baseline itself.
    #[must_use]
    pub fn requested_scale_factor(&self) -> f32 {
        self.app.persisted_ui_state.font_size_scale.unwrap_or(1.0)
    }

    /// The effective on-screen scale: the OS DPI factor iced applies times the
    /// persisted user multiplier.
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
    /// reducer-held overlay state.
    ///
    /// This is how the view-model lifetime problem is resolved: every panel
    /// function returns an `Element<'_, Message>` that **borrows** its
    /// view-model, so a view-model built inside `view` cannot outlive the
    /// call (E0515). Building them at the end of `update` instead — where
    /// the domain state has just finished mutating — lets `view` stay a pure
    /// borrow over `&AppState`.
    pub fn rebuild_view_models(&mut self) {
        self.models = ViewModels::from_app(&self.app, &self.overlay);
    }

    /// Which floating windows are showing, bottom-most layer first.
    /// Delegates to [`mounted_layers`].
    #[must_use]
    pub fn mounted_layers(&self) -> Vec<OverlayLayer> {
        mounted_layers(&self.overlay, &self.models)
    }
}

/// A floating context menu: where it was opened and the rows it shows.
///
/// The reducer builds the rows from the panel view-model that owns the
/// target (`LeftPanelViewModel::menu_items_for` /
/// `CenterPanelViewModel::menu_items_for`) so the view stays a pure
/// projection and never reads the domain model to decide menu contents.
#[derive(Clone, Debug, PartialEq)]
pub struct ContextMenu {
    /// Root-space point in window coordinates, taken from the
    /// [`LayoutMessage::CursorMoved`](crate::ui_iced::message::LayoutMessage::CursorMoved)
    /// that preceded the open request.
    pub anchor: iced::Point,
    /// The rows to render, in display order.
    pub items: Vec<MenuItem<Message>>,
    /// Reducer-owned nested-menu path for this surface.
    pub state: MenuState,
}

impl ContextMenu {
    /// Creates a context menu with its submenu path closed.
    #[must_use]
    pub fn new(anchor: iced::Point, items: Vec<MenuItem<Message>>) -> Self {
        Self {
            anchor,
            items,
            state: MenuState::default(),
        }
    }

    /// Converts legacy flat rows into the reusable menu model.
    #[must_use]
    pub(crate) fn from_legacy_items(
        anchor: iced::Point,
        items: Vec<LegacyMenuItem<Message>>,
    ) -> Self {
        Self::new(
            anchor,
            items
                .into_iter()
                .map(|item| MenuItem::action(item.label, item.message))
                .collect(),
        )
    }
}

/// Reducer-owned floating-window state.
///
/// Several floating surfaces can genuinely be open at once — a context menu
/// over an open About window — so each one gets its own slot rather than a
/// single blocking overlay. [`mounted_layers`] turns the open slots back into
/// a z-ordered stack.
///
/// # Framework state (integration increment INT-2)
///
/// [`Self::surfaces`] and [`Self::pre_rank`] are the `iced-pro` band model
/// and Pre-Rank ledger. They are a **derived index** of the fields above,
/// never a second source of truth: the individual window-state fields stay
/// authoritative for *whether* a surface is open, and
/// [`sync_surfaces`](Self::sync_surfaces) mirrors them into the registry on
/// demand. Nothing reads the registry to decide what to draw or what is
/// open; only [`close_topmost`](Self::close_topmost) consults it, to learn
/// the *order* in which to unwind.
///
/// # Tab / focus projection is deliberately deferred
///
/// The app captures **only** `Escape` off the runtime keyboard stream
/// (`ui_iced/subscription.rs:442`, `escape_to_message`); `Tab` traversal
/// is owned exclusively by iced's built-in router today. Introducing
/// [`iced_pro::focus::FocusState`] plus
/// [`iced_pro::bridge::tab_task`] here would run a *second* focus model in
/// parallel with iced's, with nothing keeping its member set synchronised
/// against the live widget tree — the app never enumerates its focusables,
/// so the framework's member set would be fiction. That sync problem
/// disappears once the framework widgets of INT-3..7 self-register their
/// focusables as they are built, so the projection belongs there and not
/// now.
///
/// Note for the next reader: the original integration plan named
/// `bridge::advance` / `bridge::sync` / `bridge::apply_transition`. **Those
/// do not exist.** The real bridge API is
/// [`iced_pro::bridge::tab_task`]`(model, direction)` /
/// [`iced_pro::bridge::focus_task`] / [`iced_pro::bridge::focus_operation`]
/// / [`iced_pro::bridge::widget_id`].
#[derive(Debug)]
pub struct OverlayState {
    /// The small name-entry / move / delete-confirmation dialog.
    pub dialog: DialogState,
    /// Where the small dialog was anchored (`None` = centred).
    pub dialog_anchor: Option<iced::Point>,
    /// The floating tree / tab context menu.
    pub context_menu: Option<ContextMenu>,
    /// Whether the top-bar hamburger menu is open. A reducer-held bool because
    /// the shared menu surface is mounted by the overlay host from state every
    /// frame.
    pub hamburger_open: bool,
    /// Reducer-owned submenu path for the hamburger surface.
    pub hamburger_menu_state: MenuState,
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
    /// The agent transcript's "Thinking Process" disclosure state.
    pub thinking_expanded: bool,
    /// Last reported background-logs viewport.
    pub logs_scroll: Option<ScrollSnapshot>,
    /// Last reported agent-transcript viewport.
    pub agent_scroll: Option<ScrollSnapshot>,
    /// Last reported debug-entry viewport.
    pub debug_scroll: Option<ScrollSnapshot>,
    /// Keyboard modifier state mirrored off the runtime event stream by the
    /// subscription bridge's keyboard lane (UI-022).
    ///
    /// iced's `MouseArea::on_press` and `mouse::Event::ButtonPressed` both
    /// carry **no** modifier state, so the only way to know a tree row was
    /// Shift-clicked is to track the keyboard independently and read the
    /// bit back when the click is reduced. The Shift keydown is delivered
    /// and reduced before the click message, so the tracked value is
    /// current at that point.
    ///
    /// Only the `SHIFT` bit is maintained today; the field keeps the full
    /// [`iced::keyboard::Modifiers`] set so a later `CTRL`-based gesture
    /// needs no shape change.
    ///
    /// Grounded in `src/app/ui/SPEC.md` UI-022: "When the user holds the
    /// shift key, the system shall allow the user to select multiple
    /// documents."
    pub modifiers: iced::keyboard::Modifiers,
    /// The `iced-pro` z-band index of the surfaces currently open
    /// (`UIF-ZORD-*`), derived from the fields above by
    /// [`sync_surfaces`](Self::sync_surfaces).
    ///
    /// It carries **no** authority over what is open: it exists so
    /// [`close_topmost`](Self::close_topmost) can walk the `UIF-WIN-003`
    /// escape order instead of re-deriving it from a hardcoded chain.
    /// Every entry is registered with an empty region list, so this index
    /// is not usable for pointer routing — see
    /// [`sync_surfaces`](Self::sync_surfaces) for why that is deliberate
    /// and what has to happen before it changes.
    pub surfaces: SurfaceRegistry,
    /// The `iced-pro` Pre-Rank ledger (`UIF-WIN-003` step 1, §3.0.4.2):
    /// which control, if any, holds a pointer-captured drag that `Escape`
    /// must cancel **before** any surface is asked.
    ///
    /// Nothing writes it yet — the app has no framework drag widget until
    /// INT-3..7 — but [`close_topmost`](Self::close_topmost) runs the
    /// Pre-Rank step on every `Escape` so the ordering is established and
    /// tested now, rather than being retrofitted under a live drag in
    /// INT-6.
    pub pre_rank: PreRankCoordinator,
}

impl Default for OverlayState {
    /// A closed-everything overlay: the thinking block starts expanded and no
    /// scroll viewport has been reported yet.
    fn default() -> Self {
        Self {
            dialog: DialogState::new(),
            dialog_anchor: None,
            context_menu: None,
            hamburger_open: false,
            hamburger_menu_state: MenuState::default(),
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
            modifiers: iced::keyboard::Modifiers::empty(),
            surfaces: SurfaceRegistry::new(),
            pre_rank: PreRankCoordinator::new(),
        }
    }
}

/// Refresh or remove one entry of the derived surface index so it agrees
/// with whether `open` says the surface is showing.
///
/// [`SurfaceRegistry::register`] refreshes an already-registered id in
/// place and **keeps its position** in its band, which is what makes the
/// whole sync pass order-stable: a card rebuilt every frame is neither
/// promoted nor demoted, and calling this twice changes nothing.
/// `unregister` on an absent id is a no-op.
///
/// [RUST-021]: the id is always a `&'static str` const, never an inline
/// literal at the call site.
fn sync_surface(registry: &mut SurfaceRegistry, id: &str, kind: SurfaceKind, open: bool) {
    if open {
        registry.register(SurfaceId::new(id), kind, Vec::new());
    } else {
        registry.unregister(&SurfaceId::new(id));
    }
}

/// Settle a pointer-captured drag for `UIF-WIN-003` step 1.
///
/// Always answers `false` — "the control had nothing to cancel" — because
/// no framework drag widget is mounted in the app yet, so no capture can
/// exist and this body is never reached in production.
///
/// **INT-6 must replace the body** with a dispatch to the capturing
/// control's own
/// [`GestureSlot::cancel`](iced_pro::binding::snapshot::GestureSlot::cancel),
/// keyed on [`DragCapture::id`]. Until it does, the Pre-Rank step is a
/// clean pass-through, which
/// `test_close_topmost_pre_rank_is_a_clean_pass_through_with_no_capture`
/// pins so the change cannot land unnoticed.
///
/// Named rather than left an inline closure so the INT-6 obligation is a
/// greppable symbol instead of a `false` buried in an argument list.
fn cancel_drag_capture(_capture: &DragCapture) -> bool {
    false
}

impl OverlayState {
    /// Reconciles the derived [`SurfaceRegistry`] against the
    /// reducer-held window-state fields (`UIF-ZORD-*`, `UIF-WIN-003`).
    ///
    /// # The registry is an INDEX, not a second source of truth
    ///
    /// [`Self::batch`], [`Self::about`], [`Self::tools`], [`Self::debug`],
    /// [`Self::dialog`], [`Self::editor`], [`Self::context_menu`] and
    /// [`Self::hamburger_open`] remain the single authority for *whether*
    /// a surface is open. This method only mirrors that into the band
    /// model — open field → `register`, closed field → `unregister`. No
    /// code reads the registry to decide what to draw, and no code sets a
    /// field from a registry answer. The registry contributes exactly one
    /// thing the fields cannot: the **order** in which surfaces unwind.
    ///
    /// # Idempotent and cheap
    ///
    /// Nine `O(n)` lookups over a list that never exceeds nine entries, and
    /// safe to call any number of times in any order: `register` keeps an
    /// existing id's band position, `unregister` tolerates an absent id.
    ///
    /// # Registration order is the overlay host's own draw order
    ///
    /// Surfaces are registered bottom-first within each band, matching the
    /// overlay host's mount order (`ui_iced/view.rs`: the context menu is
    /// mounted before the hamburger, and the batch dialog is mounted
    /// last). Because the escape walk asks the *top* of a band first, that
    /// makes the card actually painted on top the one that unwinds first,
    /// with no depth parameter anywhere.
    ///
    /// # Regions are empty on purpose — review area
    ///
    /// Every surface is registered with an empty region list. Per
    /// [`SurfaceRegistry`]'s own contract a regionless surface is never
    /// hit by the pointer but still stacks and still takes part in
    /// `Escape`, which is exactly the half this increment wires.
    /// **Consequence to review before this index is ever used for pointer
    /// routing:** [`SurfaceRegistry::route_pointer`] can never answer
    /// `PointerTarget::Surface` here — it falls through to the band rule,
    /// i.e. `DismissBand(Menu)` or `Scrim(..)`. Populating real viewport
    /// rectangles belongs to the widget increments (INT-3..7), where the
    /// overlay host actually knows each card's geometry.
    ///
    /// # Call site
    ///
    /// [`close_topmost`](Self::close_topmost) calls this on entry, which
    /// is the only moment the index must be exact. Hooking every update
    /// path is deliberately not attempted in this increment.
    ///
    /// [RUST-020]: mutates only `self`; no I/O, no bus publish.
    pub fn sync_surfaces(&mut self) {
        // ── Menu band (`ZBand::Menu`, escape rank 3) ────────────────────
        // Registered bottom-first, matching `overlay_host`'s mount order.
        sync_surface(
            &mut self.surfaces,
            SURFACE_ID_CONTEXT_MENU,
            SurfaceKind::MenuCard,
            self.context_menu.is_some(),
        );
        sync_surface(
            &mut self.surfaces,
            SURFACE_ID_HAMBURGER,
            SurfaceKind::MenuCard,
            self.hamburger_open,
        );

        // ── Modal band (`ZBand::Modal`, escape rank 2) ──────────────────
        sync_surface(
            &mut self.surfaces,
            SURFACE_ID_EDITOR,
            SurfaceKind::Modal,
            self.editor.is_some(),
        );
        // The app's small name-entry / move / delete-confirmation dialog is
        // a cursor-anchored input dialog, not a blocking card: `InputDialog`
        // is its own `SurfaceKind` and unwinds at the same rank 2 as a
        // `Modal`, so the old relative order against the other cards is
        // preserved by registration position rather than by a special case.
        sync_surface(
            &mut self.surfaces,
            SURFACE_ID_DIALOG,
            SurfaceKind::InputDialog,
            self.dialog.is_open(),
        );
        sync_surface(
            &mut self.surfaces,
            SURFACE_ID_TOOLS,
            SurfaceKind::Modal,
            self.tools.is_open(),
        );
        // `AboutWindowState` holds a single `Option`, so the two identity
        // windows are mutually exclusive and at most one of these two ids
        // is ever registered.
        sync_surface(
            &mut self.surfaces,
            SURFACE_ID_ABOUT,
            SurfaceKind::Modal,
            self.about.is_about(),
        );
        sync_surface(
            &mut self.surfaces,
            SURFACE_ID_ATTRIBUTIONS,
            SurfaceKind::Modal,
            self.about.is_attributions(),
        );
        sync_surface(
            &mut self.surfaces,
            SURFACE_ID_BATCH,
            SurfaceKind::Modal,
            self.batch.is_open(),
        );

        // ── Utility-overlay band (`ZBand::UtilityOverlay`, rank 1) ──────
        sync_surface(
            &mut self.surfaces,
            SURFACE_ID_AGENT_DEBUG,
            SurfaceKind::MovableCard,
            self.debug.is_open(),
        );
    }

    /// Closes the **top-most** open floating layer, one layer per call
    /// (UI-056 — "the dialog shall provide a `Close` action (button and
    /// `Esc` keypress …) that hides the dialog without persisting further
    /// state").
    ///
    /// Returns the name of the layer it closed, or `None` when nothing was
    /// open. The name is a diagnostic identifier for logs and tests, not
    /// user-facing text — the UI strings live in `crate::ui::strings`.
    ///
    /// # The rule
    ///
    /// One `Esc` dismisses exactly one surface, walking the derived
    /// [`SurfaceRegistry`] through
    /// [`escape_with_pre_rank`] in `UIF-WIN-003` order: the Pre-Rank drag
    /// ledger first, then by descending [`ZBand::escape_rank`](iced_pro::surface::zorder::ZBand::escape_rank)
    /// — and within a rank, the top of the band first.
    ///
    /// `Menu` is rank **3**, `Modal` rank **2**, `UtilityOverlay` rank
    /// **1** — note that band index and escape rank are *not* the same
    /// ordering (`ZBand::Menu` is band index 2 but rank 3), which is
    /// precisely the distinction this function used to get wrong.
    ///
    /// # The escape-rank bug this replaced (INT-2)
    ///
    /// The previous implementation was a hardcoded eight-branch `if`-chain
    /// that unwound in this order:
    ///
    /// `Batch` → `Attributions` → `About` → `Tools` → **`AgentDebug`** →
    /// `Dialog` → **`Editor`** → `ContextMenu` → `hamburger`
    ///
    /// Two of those steps contradict `UIF-WIN-003`, which unwinds
    /// *menus before dialogs* and *never a modeless utility card ahead of
    /// a blocking modal*:
    ///
    /// * **`Editor` (Modal, rank 2) closed ahead of `ContextMenu`
    ///   (MenuCard, rank 3).** A menu sits above the surface it was opened
    ///   from; the menu's owner is what the user is looking at *through*
    ///   the menu. Dismissing the owner while its menu is still up throws
    ///   away the context the menu was opened in. `Esc` over an open
    ///   editor with a context menu on top destroyed the editor and left
    ///   the orphan menu.
    /// * **`AgentDebug` (MovableCard, rank 1) closed ahead of `Dialog` /
    ///   `Editor` (rank 2).** A modeless utility overlay must not unwind
    ///   ahead of a blocking modal; the modal is what is holding the user.
    ///
    /// The corrected order for the same fully-open stack is:
    ///
    /// `hamburger` → `ContextMenu` → `Batch` → `Attributions` → `About`
    /// → `Tools` → `Dialog` → `Editor` → `AgentDebug`
    ///
    /// That is what makes `Esc` behave like a native stack: pressing it
    /// over the batch dialog does not also tear down the About window
    /// underneath, a menu on top of a modal is dismissed *first*, and the
    /// modeless debug card is left alone until everything blocking has gone.
    ///
    /// # What `Esc` deliberately does NOT close
    ///
    /// The **docked background-log viewer** ([`OverlayLayer::Logs`]) is
    /// skipped. It is a docked panel, not a modal: it never blocks the
    /// panels beneath it for a *dismissal*, its visibility is application
    /// state owned by `UserCommand::ToggleBackgroundLogs` rather than
    /// reducer-held overlay state, and the user expects it to stay put
    /// while they keep working. It is therefore not in the `Esc` stack at
    /// all — and note it is not even reachable from here, since its
    /// visibility lives on `BackgroundLogsViewModel`, not on
    /// [`OverlayState`].
    ///
    /// The [`OverlayLayer::ContextMenu`] entry is closed whenever the slot
    /// holds a menu. [`mounted_layers`] additionally requires a non-empty
    /// item list before it *mounts* one; that is a render concern, and a
    /// menu slot that is `Some` with no rows is still reducer state worth
    /// clearing rather than leaving stranded.
    ///
    /// # The Pre-Rank step, and why it is wired with no drag to cancel
    ///
    /// `UIF-WIN-003` step 1 is the pointer-captured drag the user is
    /// still holding, and it must settle **before** the rank walk runs —
    /// otherwise the focused surface dismisses while a slider the user is
    /// still holding keeps tracking a pointer nobody is watching. The app
    /// has no framework drag widget yet, so the `cancel_capture` closure
    /// always answers `false` ("nothing to cancel"), which makes the step
    /// a provably clean pass-through today. It is wired now, and tested as
    /// a pass-through, so INT-6 inherits the ordering rather than
    /// re-deriving it — and so the pass-through is not dead code that a
    /// later reader has to trust.
    ///
    /// A `PreRankedEscape::DragCancelled` reports `None` here: the key was
    /// spent on a drag, so no overlay layer closed and there is no layer
    /// name to give. Whether a drag cancel deserves its own reported name
    /// is an INT-6 decision.
    ///
    /// [RUST-020]: mutates only `self`; performs no I/O and publishes no
    /// bus event, which is what keeps the `Esc` reduction thin, and
    /// [FMD-069] keeps the state change on the UI thread that received the
    /// key.
    pub fn close_topmost(&mut self) -> Option<&'static str> {
        self.sync_surfaces();

        // Borrow the closable fields out separately from `surfaces` /
        // `pre_rank` so the registry walk (which needs `&mut` of both)
        // and the field writes the walk performs are disjoint borrows
        // rather than one `&mut self` held across the call.
        let OverlayState {
            context_menu,
            hamburger_open,
            hamburger_menu_state,
            editor,
            dialog,
            tools,
            about,
            batch,
            debug,
            surfaces,
            pre_rank,
            ..
        } = self;

        let mut closable = ClosableSurfaces {
            context_menu,
            hamburger_open,
            hamburger_menu_state,
            editor,
            dialog,
            tools,
            about,
            batch,
            debug,
            closed: None,
        };

        let escape = escape_with_pre_rank(
            pre_rank,
            surfaces,
            // No framework drag widget is mounted yet, so there is never a
            // capture to settle. See [`cancel_drag_capture`].
            cancel_drag_capture,
            |surface| closable.unwind(surface),
        );

        match escape {
            // Step 1 spent the key on a drag; no overlay layer closed.
            PreRankedEscape::DragCancelled { .. } => None,
            PreRankedEscape::RankWalked { .. } => closable.closed,
        }
    }
}

/// The app-side fields one `Escape` may close, borrowed out of
/// [`OverlayState`] so the registry walk owns `&mut SurfaceRegistry` and
/// `&mut PreRankCoordinator` while these are written independently.
///
/// Dispatch is on the [`SurfaceId`], not the [`SurfaceKind`]: several
/// app surfaces share a kind (`MenuCard` covers both menus, `Modal`
/// covers four cards), so the kind alone cannot say which field to clear.
struct ClosableSurfaces<'a> {
    context_menu: &'a mut Option<ContextMenu>,
    hamburger_open: &'a mut bool,
    hamburger_menu_state: &'a mut MenuState,
    editor: &'a mut Option<EditorViewModel>,
    dialog: &'a mut DialogState,
    tools: &'a mut ToolsDialogState,
    about: &'a mut AboutWindowState,
    batch: &'a mut BatchDialogState,
    debug: &'a mut AgentDebugWindowState,
    /// The [`CLOSED_LAYER_*`](CLOSED_LAYER_BATCH) name of the surface that
    /// accepted the walk, once one has.
    closed: Option<&'static str>,
}

impl ClosableSurfaces<'_> {
    /// Answer one `Escape` for one registered surface: close the
    /// app-side field its [`SurfaceId`] names and report the layer
    /// constant back.
    ///
    /// A surface whose field turns out not to be open answers
    /// [`UnwindAnswer::Declined`] rather than pretending to have closed
    /// something, so the walk keeps travelling instead of consuming the
    /// key on a ghost.
    fn unwind(&mut self, surface: &Surface) -> UnwindAnswer {
        let (name, closed_something) = match surface.id().as_str() {
            SURFACE_ID_CONTEXT_MENU => (
                CLOSED_LAYER_CONTEXT_MENU,
                self.context_menu.take().is_some(),
            ),
            SURFACE_ID_HAMBURGER => {
                let was_open = std::mem::take(self.hamburger_open);
                if was_open {
                    self.hamburger_menu_state.close_all();
                }
                (CLOSED_LAYER_HAMBURGER, was_open)
            }
            SURFACE_ID_EDITOR => (CLOSED_LAYER_EDITOR, self.editor.take().is_some()),
            SURFACE_ID_DIALOG => (CLOSED_LAYER_DIALOG, self.dialog.close()),
            SURFACE_ID_TOOLS => (CLOSED_LAYER_TOOLS, self.tools.close()),
            SURFACE_ID_ABOUT => (CLOSED_LAYER_ABOUT, self.about.close()),
            SURFACE_ID_ATTRIBUTIONS => (CLOSED_LAYER_ATTRIBUTIONS, self.about.close()),
            SURFACE_ID_BATCH => (CLOSED_LAYER_BATCH, self.batch.close()),
            SURFACE_ID_AGENT_DEBUG => (CLOSED_LAYER_AGENT_DEBUG, self.debug.close()),
            // `sync_surfaces` is the only writer of this index, so an
            // unknown id means the two halves are wired wrong — not a
            // user-facing state worth inventing a layer name for.
            _ => return UnwindAnswer::Declined(DeclineReason::CannotUnwind),
        };

        if closed_something {
            self.closed = Some(name);
            UnwindAnswer::Dismissed
        } else {
            UnwindAnswer::Declined(DeclineReason::NotOpen)
        }
    }
}

/// One floating-window layer of the overlay host, bottom-most first.
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
    /// The batch prompt-processing dialog (topmost).
    Batch,
}

/// Layer name [`OverlayState::close_topmost`] reports for the batch
/// prompt-processing dialog ([UI-056]).
pub const CLOSED_LAYER_BATCH: &str = "batch";

/// Layer name [`OverlayState::close_topmost`] reports for the third-party
/// attributions window ([UI-056]).
pub const CLOSED_LAYER_ATTRIBUTIONS: &str = "attributions";

/// Layer name [`OverlayState::close_topmost`] reports for the About window
/// ([UI-056]).
pub const CLOSED_LAYER_ABOUT: &str = "about";

/// Layer name [`OverlayState::close_topmost`] reports for the tools dialog
/// ([UI-056], UI-051..UI-060).
pub const CLOSED_LAYER_TOOLS: &str = "tools";

/// Layer name [`OverlayState::close_topmost`] reports for the agent debug
/// window ([UI-056], UI-048).
pub const CLOSED_LAYER_AGENT_DEBUG: &str = "agent_debug";

/// Layer name [`OverlayState::close_topmost`] reports for the small
/// name-entry / move / delete-confirmation dialog ([UI-056]).
pub const CLOSED_LAYER_DIALOG: &str = "dialog";

/// Layer name [`OverlayState::close_topmost`] reports for the inline editor
/// modal ([UI-056], UI-045).
pub const CLOSED_LAYER_EDITOR: &str = "editor";

/// Layer name [`OverlayState::close_topmost`] reports for the floating tree
/// / tab context menu ([UI-056]).
pub const CLOSED_LAYER_CONTEXT_MENU: &str = "context_menu";

/// Layer name [`OverlayState::close_topmost`] reports for the top-bar
/// hamburger menu ([UI-056]).
pub const CLOSED_LAYER_HAMBURGER: &str = "hamburger";

// ---------------------------------------------------------------------------
// Framework surface identities (INT-2)
// ---------------------------------------------------------------------------
//
// Registry keys for the derived [`SurfaceRegistry`] index, one per
// closable layer. They are **diagnostic identities**, not user-facing text
// and not `iced::widget::Id`s — the widget ids the reducer addresses live
// in the module that owns the widget (§5 of `src/app/AGENTS.md`). Kept as
// `&'static str` consts so no literal is repeated at a call site
// ([RUST-021]) and so the id a surface is registered under and the id the
// escape walk dispatches on cannot drift apart.

/// Registry id for the floating tree / tab context menu (`MenuCard`).
pub const SURFACE_ID_CONTEXT_MENU: &str = "overlay.context_menu";

/// Registry id for the top-bar hamburger menu (`MenuCard`).
pub const SURFACE_ID_HAMBURGER: &str = "overlay.hamburger";

/// Registry id for the inline editor modal (`Modal`).
pub const SURFACE_ID_EDITOR: &str = "overlay.editor";

/// Registry id for the small name-entry / move / delete-confirmation
/// dialog (`InputDialog`).
pub const SURFACE_ID_DIALOG: &str = "overlay.dialog";

/// Registry id for the tools dialog (`Modal`).
pub const SURFACE_ID_TOOLS: &str = "overlay.tools";

/// Registry id for the About window (`Modal`).
pub const SURFACE_ID_ABOUT: &str = "overlay.about";

/// Registry id for the third-party attributions window (`Modal`).
pub const SURFACE_ID_ATTRIBUTIONS: &str = "overlay.attributions";

/// Registry id for the batch prompt-processing dialog (`Modal`).
pub const SURFACE_ID_BATCH: &str = "overlay.batch";

/// Registry id for the agent debug window (`MovableCard`).
pub const SURFACE_ID_AGENT_DEBUG: &str = "overlay.agent_debug";

/// The z-ordered set of showing layers, bottom-most first.
///
/// The order is the draw order: the context menu sits above the panels, the
/// editor above that, then the small dialogs, the log viewer, the debug
/// window, the tools dialog, the identity windows and the batch dialog last.
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

/// Every panel and dialog view-model the iced view renders.
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
    /// Snapshots every view-model off `app`, merging in the reducer-held state
    /// that the individual `from_app` constructors cannot see.
    ///
    /// Four merges are required because each underlying `from_app` reports a
    /// default where the iced reducer owns the real value:
    ///
    /// * the left panel's search fields come from [`SearchViewModel`], because
    ///   `LeftPanelViewModel::from_app` reads the domain `TreeSearch`, which
    ///   stays empty in this build;
    /// * the left panel's `multi_select` comes from
    ///   [`OverlayState::modifiers`], because `LeftPanelViewModel::from_app`
    ///   has no modifier input and reports `false` (UI-022);
    /// * the agent panel's disclosure comes from
    ///   [`OverlayState::thinking_expanded`], because
    ///   `AgentPanelViewModel::from_app` hardcodes `true`;
    /// * the debug window's expanded rows come from [`AgentDebugWindowState`],
    ///   because `AgentDebugViewModel::from_app` returns an empty set.
    ///
    /// Side effects: the remaining filesystem reads are the tree flatten and
    /// the skill listings. Everything a closed window would need is skipped so
    /// a workspace scan cannot starve the UI thread:
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
        // listing to both, instead of each panel hitting the directory itself.
        let note_skills = crate::ui_iced::panels::left::note_skill_files(app);

        let mut left = LeftPanelViewModel::from_app(app);
        left.query = overlay.search.query().to_string();
        left.is_searching = overlay.search.is_filtering();
        left.results = overlay.search.results_arc();
        // UI-022: `from_app` has no modifier input and defaults the field
        // to `false`; the tracked keyboard state lives on the overlay, so
        // the override happens here alongside the other search merges.
        left.multi_select = overlay.modifiers.shift();

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
        // a workspace scan.
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

/// Projects the [`crate::ui::Dialogs`] batch-run fields onto the
/// [`BatchRunStatus`] the card renders.
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
    /// The `Bus<ConfigArrived>` fan-out channel.
    pub config_bus: Bus<ConfigArrived>,
    /// Injected configuration storage handler ([RUST-024]).
    pub config_storage: Arc<dyn ConfigStorageHandler>,
    /// Injected persisted-UI-state storage handler ([RUST-024]).
    pub ui_state_storage: Arc<dyn UiStateStorage>,
    /// Process arguments (`std::env::args()`), the input to the CLI
    /// workspace injection performed by [`apply_cli_workspace`].
    pub args: Vec<String>,
}

/// Builds the full [`AppState`] without touching the framework.
///
/// Three things the iced path needs are handled here:
///
/// 1. the persisted snapshot is supplied by the caller, loaded through the
///    injected [`UiStateStorage`];
/// 2. the loaded `config` is **direct-injected** into the orchestrator and
///    the agent — the UI does not wait on the startup broadcast;
/// 3. the real background-event `rx` is handed to the [`BusReceivers`]
///    bridge, so the orchestrator's own `rx` drain is unused and gets an
///    inert, never-fed channel. The bridge is the single inbound path.
///
/// Side effects at construction: spawns the background worker thread
/// (`BackgroundTask::new`), the MCP config-subscriber thread
/// (`spawn_config_subscription`) and the `std::sync::mpsc` → async
/// forwarding thread (inside [`BusReceivers`]). The startup `ConfigArrived`
/// is published synchronously right before returning, after every subscriber
/// above is registered, so the `CONFIG_ARRIVAL_TIMEOUT` waits never miss it.
/// Until then, none of them touch the filesystem.
pub fn build_state(
    config: AppConfig,
    config_bus: &Bus<ConfigArrived>,
    config_storage: Arc<dyn ConfigStorageHandler>,
    ui_state_storage: Arc<dyn UiStateStorage>,
    persisted: PersistedUiState,
) -> AppState {
    // Background worker + event channel.
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
    let background_manager = Arc::new(Mutex::new(BackgroundLogs::new()));
    crate::background::logs::register_ui_logs(background_manager.clone());

    let browser_session = std::sync::Arc::new(crate::agent::BrowserSession::new(
        &crate::config::AppConfig::default(),
    ));
    let pdf_backing_tracker = crate::agent::session::PdfBackingTracker::new();

    // The agent's streaming-event bus + observer fan-out (bus observer for
    // the UI, conversation logger for disk), so the UI sees the same
    // `AgentEvent` stream the session produces.
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
    // Direct injection: the agent gets the loaded config now, so it never
    // depends on the startup broadcast for its own initialisation.
    agent.set_agent_config(config.to_agent_config());

    let event_bus = background_task.file_event_bus;
    let dir_event_bus = background_task.dir_event_bus;
    // The single source of truth for file/directory state. Under the iced
    // bridge the reducer applies every event the messages carry, so the
    // projection's internal readers stay unwired (no double application).
    let workspace = Workspace::new(None, Some(dir_event_bus.subscribe()));

    // Project the persisted snapshot onto the live panel/selection/dialog
    // state.
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
    // First-run auto-show of the About window.
    crate::ui_iced::about::apply_first_run_auto_show(
        &mut persisted,
        &mut dialogs,
        crate::ui_iced::about::APP_VERSION,
    );

    let user_command_bus = crate::bus::core::Bus::new();
    let user_command_reader = user_command_bus.subscribe();

    // The real background-event channel goes to the [`BusReceivers`] bridge;
    // the orchestrator's own `rx` drain is unused here, so it gets an inert,
    // never-fed channel. `try_recv` on it returns immediately (no blocking) —
    // the bridge is the single inbound path.
    let (_inert_tx, inert_rx) = std::sync::mpsc::channel();
    let receivers = BusReceivers::new(
        config_bus,
        &event_bus,
        &dir_event_bus,
        &agent_event_bus,
        background_task.rx_futures,
    );

    // Startup `ConfigArrived` publish, performed **synchronously**: the
    // subscribers it must reach — the background worker thread
    // (`BackgroundTask::new`), the MCP config subscriber
    // (`spawn_config_subscription`) and the [`BusReceivers`] bridge's config
    // lane (this publish is the third and last subscriber) — are all
    // registered above, so ordering is guaranteed without a task
    // round-trip. Deferring the send through an iced `Task` would hand it to
    // the iced scheduler, which can land seconds after `boot` (behind the
    // font-registration boot task); the waiting subscribers would then hit
    // their `CONFIG_ARRIVAL_TIMEOUT`, fall back to `AppConfig::default()`
    // and the initial library scan would discover nothing.
    let subscribers = config_bus.publish(ConfigArrived::new(config.clone()));
    tracing::info!(
        name = "ui_iced.boot.config_published",
        subscribers,
        "startup ConfigArrived published for background workers"
    );

    let user_file_operation_worker =
        crate::background::UserFileOperationWorker::spawn(background_task.tx.clone());
    let app = FastMdApp::from_parts(
        crate::orchestrator::AppOrchestrator {
            content_libraries: config.content_libraries.clone(),
            rx: inert_rx,
            tx: background_task.tx,
            user_file_operation_worker,
            file_event_bus: event_bus,
            dir_event_bus,
            // Single inbound path: the [`BusReceivers`] bridge owns
            // `Bus<FileChanged>` delivery and the reducer applies each
            // `Inbound(FileChanged)` payload through
            // `AppOrchestrator::apply_file_changed`. A live reader here would
            // apply every event a second time, so it is inert — the same
            // treatment `rx` above gets.
            file_event_reader: None,
            workspace,
            pdf_backing_tracker,
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
            // Config arrivals reach the UI through the subscription bridge, not
            // the orchestrator's sync reader.
            config_reader: None,
            pending_file_load: None,
            pending_editor_open: None,
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
    // `update`.
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

/// CLI-argument workspace injection: a pure function of `(config, args)` so it
/// is unit-testable without a process.
///
/// Behaviour:
/// 1. if `args[1]` names an existing directory, append it as a `Workspace`
///    content library unless one with the same root is already present;
/// 2. if the config still holds only the system library, append the current
///    working directory as a `Workspace` content library.
///
/// [RUST-020]: mutates only `config`; reads the filesystem for existence /
/// canonicalisation.
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

/// Resolves the UI-state storage from the `FASTMD_ICED_UI_STATE_PATH` override.
///
/// A non-empty override stores snapshots in that explicit file
/// ([`persisted::FileUiStateStorage`]); otherwise snapshots stay in-process
/// ([`persisted::InMemoryUiStateStorage`]). The platform-default user
/// location is deliberately NOT resolved here so nothing writes `%APPDATA%`
/// by accident ([RUST-024], [RUST-006]).
pub fn ui_state_storage_from_env(env_override: Option<String>) -> Arc<dyn UiStateStorage> {
    match env_override {
        Some(path) if !path.is_empty() => {
            Arc::new(persisted::FileUiStateStorage::new(PathBuf::from(path)))
        }
        _ => Arc::new(persisted::InMemoryUiStateStorage::new()),
    }
}

/// The iced `BootFn` body.
///
/// `impl BootFn<State, Message> for T where T: Fn() -> C, C: IntoBoot`
/// (`iced-0.14.0/src/application.rs:540-566`): this returns the
/// `(AppState, Task<Message>)` pair. The task chain is
/// [`Task::batch`] of:
///
/// * the font-registration task, mapped onto
///   [`BootMessage::FontLoadFinished`];
/// * the scale-baseline capture — `iced::window::latest()`
///   (`Task<Option<Id>>`) chained into `iced::window::scale_factor(id)`
///   (`Task<f32>`) via `Task::then`, mapped onto
///   [`WindowMessage::ScaleBaseline`].
///
/// The `ConfigArrived` publish for the background workers happens
/// synchronously inside [`build_state`] and is therefore not part of the
/// task chain (see its doc comment for why the publish is not deferred
/// through `Task::perform`).
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
    // CLI workspace injection.
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

    // The `ConfigArrived` publish is performed synchronously inside
    // [`build_state`] (right after its last subscriber registers), not as a
    // `Task::perform` here: a boot task is executed by the iced scheduler
    // and can land seconds after `boot` returns, which would make every
    // `CONFIG_ARRIVAL_TIMEOUT` subscriber miss the arrival and scan nothing.
    (state, Task::batch([font_task, scale_task]))
}

/// Baseline capture: `window::latest()` → `window::scale_factor(id)`
/// chained with `Task::then` (`iced_runtime-0.14.0/src/task.rs:128-156`).
fn scale_baseline_task() -> Task<Message> {
    iced::window::latest().then(|latest| match latest {
        Some(id) => iced::window::scale_factor(id)
            .map(|scale| Message::Window(WindowMessage::ScaleBaseline(scale))),
        None => Task::none(),
    })
}

/// Builds the `window::Settings` for the launched window.
///
/// Size and position seed from the loaded [`PersistedUiState`], falling back to
/// the 1000×700 default ([`WINDOW_WIDTH_POINTS`] / [`WINDOW_HEIGHT_POINTS`]).
/// `exit_on_close_request` is `false` so the runtime waits for the app to
/// persist before closing (save-on-close).
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

/// Loads the application icon as an iced window icon.
///
/// Decodes the shared asset (`assets/icon-32.png`) via
/// `crate::utils::app_icon::decode_app_icon_rgba`, then wraps the RGBA
/// pixels with `iced::window::icon::from_rgba`. Falls back to `None` (iced
/// default) if decoding fails.
fn window_icon() -> Option<iced::window::Icon> {
    let (rgba, width, height) = crate::utils::app_icon::decode_app_icon_rgba()?;
    iced::window::icon::from_rgba(rgba, width, height).ok()
}

/// The named boot strategies the tier-4 `.ice` test harness selects by name.
///
/// A [`iced::Preset`] is a `(name, boot)` pair: the same
/// [`boot_with_persisted`] body reached by a stable string instead of by
/// whatever the launch chain happens to close over. `iced_test::run` picks
/// one by name to build a reproducible environment for a `.ice` scenario
/// instead of inheriting whatever the developer's persisted snapshot
/// happens to hold.
///
/// (The path is `iced::Preset`, not `iced::program::Preset`: iced 0.14
/// imports `program` privately at `iced-0.14.0/src/lib.rs:481` and
/// re-exports only the struct itself at `:540`.)
///
/// # Production behaviour is unchanged
///
/// iced's `Application::boot()` calls the `boot` closure handed to
/// `iced::application(...)` and **never consults the `presets` list**
/// (`iced-0.14.0/src/application.rs:124-126`, which delegates to
/// `self.boot.boot()`; the `presets` field is only threaded through the
/// builder at `:165`, `:181` and read back by `Program::presets` at
/// `:522-524`). Adding `.presets(...)` is therefore metadata for external
/// tooling and cannot alter what the shipped app does.
///
/// # The set
///
/// * `"default"` — the production boot: the caller's `inputs` over the
///   caller's `persisted` snapshot, exactly what the live launch chain
///   runs.
///
/// Additional named fixtures (an empty-workspace preset, a
/// pre-populated-dialog preset) belong here as more `.ice` scenarios are
/// written; the signature already carries everything a preset needs.
///
/// [`boot_with_persisted`]: crate::ui_iced::app::boot_with_persisted
#[must_use]
pub fn launch_presets(
    inputs: LaunchInputs,
    persisted: PersistedUiState,
) -> Vec<iced::Preset<AppState, Message>> {
    vec![iced::Preset::new(
        LAUNCH_PRESET_DEFAULT,
        // Same re-callable, clone-per-call boot contract as the live chain: a
        // user intent carries everything needed to replay it, and the
        // captured handlers are never moved out, so the preset can be booted
        // repeatedly by the harness.
        move || boot_with_persisted(inputs.clone(), persisted.clone()),
    )]
}

/// The full iced launch builder chain.
///
/// Wires [`boot`] + [`crate::ui_iced::update::update`] +
/// [`crate::ui_iced::view::view`] through the verified
/// `iced::application(...)` chain:
///
/// * `.title(strings::WINDOW_TITLE)` — "FastMD AI";
/// * `.window(window_settings)` — persisted size/position + icon +
///   `exit_on_close_request: false` (save-on-close);
/// * `.theme(...)` — the custom FastMD dark theme ([`crate::ui_iced::theme`]);
/// * `.scale_factor(...)` — the user multiplier; iced composes it with the
///   OS DPI factor itself (see [`AppState::requested_scale_factor`]);
/// * `.subscription(...)` — the bus bridge over the live [`BusReceivers`],
///   plus the window and keyboard lanes;
/// * `.presets(...)` — the named boot strategies for the tier-4 `.ice`
///   harness ([`launch_presets`]). Metadata only: iced's
///   `Application::boot()` never reads the list, so production behaviour
///   is identical with or without it.
///
/// Fonts register through the boot task returned by [`boot`] rather than
/// `.font(bytes)`.
///
/// Not unit-testable headlessly (it calls `.run()`); the pieces it composes
/// are tested in the sidecar.
pub fn launch(inputs: LaunchInputs) -> iced::Result {
    let persisted = persisted::load_ui_state(inputs.ui_state_storage.as_ref());
    let settings = window_settings(&persisted);
    // Built before the boot closure captures `inputs`, so the preset list
    // and the live boot strategy are the same pair of injected handlers.
    let presets = launch_presets(inputs.clone(), persisted.clone());

    let result = iced::application(
        // The closure clones the captured inputs per call so the Elm boot
        // stays re-callable and deterministic; it never moves the injected
        // handlers out.
        move || boot_with_persisted(inputs.clone(), persisted.clone()),
        update,
        view,
    )
    .title(crate::ui::strings::WINDOW_TITLE)
    .window(settings)
    .theme(|_state: &AppState| crate::ui_iced::theme::fastmd_dark_theme())
    .scale_factor(|state: &AppState| state.requested_scale_factor())
    .subscription(|state: &AppState| subscription::subscription(&state.receivers))
    .presets(presets)
    .run();

    // Shutdown: temp-file cleanup and UI-log unregistration run when the
    // iced runtime returns.
    crate::export::print::cleanup_temp_files();
    crate::background::logs::unregister_ui_logs();

    result
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
