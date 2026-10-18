//! Iced pilot binary (`TASK-006`..`TASK-010`, `doc/planning/iced.md`) —
//! Phase 2 window bootstrap plus the Phase-3 persistence + layout chunk.
//!
//! Ports the egui window configuration from `src/app/main.rs` (left
//! untouched per TASK-006: `main.rs` keeps launching egui until `TASK-024`
//! flips the entry point) onto the iced `application` builder chain:
//!
//! - window title `strings::WINDOW_TITLE` ("FastMD AI", DEC-010) via
//!   `.title(...)` — verified `iced-0.14.0/src/application.rs:346-360`;
//! - window size/icon/`exit_on_close_request` via `.window(window::Settings)`
//!   — verified `iced-0.14.0/src/application.rs:250-255`; fields `size`
//!   (`iced_core-0.14.0/src/window/settings.rs:35-37`) and `position`
//!   (:45) are seeded from the PERSISTED UI STATE loaded at startup
//!   ([TASK-009] (b)), falling back to the egui-derived 1000×700 default;
//! - the icon decodes the shared `assets/icon-32.png` asset through
//!   `fastmd::utils::app_icon::decode_app_icon_rgba` + `iced::window::icon::
//!   from_rgba` (verified `iced-0.14.0/src/window/icon.rs:1-2`,
//!   `iced_core-0.14.0/src/window/icon.rs:7-11`);
//! - `exit_on_close_request: false` + the `close_requests()` subscription
//!   keep the window closable; on a close request `update` FIRST snapshots
//!   the window metrics and persists the UI state (DEC-006 standby save
//!   through the injected `fastmd::ui_iced::persisted::UiStateStorage`
//!   handler, [RUST-024]) and THEN issues `iced::window::close(id)`
//!   ([TASK-009] (a); runtime calls verified
//!   `iced_runtime-0.14.0/src/window.rs:283-286`);
//! - the composed UI scale (DEC-008) rides the `.scale_factor(builder)`
//!   chain (verified `iced-0.14.0/src/application.rs:410-424`,
//!   `Fn(&State) -> f32`): `boot` captures the OS baseline through
//!   `iced::window::latest() -> Task<Option<Id>>` (:294) chained into
//!   `iced::window::scale_factor(id) -> Task<f32>` (:376);
//! - the palette mapping from [`fastmd::ui_iced::theme`] is installed via
//!   `.theme(...)` (verified `application.rs:377-392`, [TASK-008]);
//! - fonts register through `fastmd::ui_iced::fonts::font_boot_task()` as
//!   the `(State, Task)` boot task ([TASK-007]; `BootFn` may return
//!   `(State, Task<Message>)` — `iced-0.14.0/src/application.rs:551-566`);
//! - the view composes the [TASK-010] `fastmd::ui_iced::layout` three-band
//!   geometry (top band stub, middle pane grid, bottom band stub) with the
//!   iced-logo section living in the CENTER pane. The right (TOC) pane is
//!   gated by the reused `should_show_panel` gate ([TASK-010]).
//!
//! Pilot storage wiring (DOCUMENTED choice, [TASK-009] rule 4): the real,
//! config-driven handler injection lands with TASK-022's `launch()`. The
//! pilot resolves its handler from `FASTMD_ICED_UI_STATE_PATH`: when set
//! (non-empty), snapshots go to that explicit file (point it INSIDE a
//! scratch/temp dir for cross-run restore experiments); otherwise state
//! persists in-process only (`InMemoryUiStateStorage`). The pilot NEVER
//! writes the platform-default user location ([RUST-024]/[RUST-006]).
//!
//! Unit tests live in the sibling `fastmd_iced_tests.rs` sidecar.

#[cfg(test)]
#[path = "fastmd_iced_tests.rs"]
mod tests;

use fastmd::ui::PersistedUiState;
use fastmd::ui_iced::{layout, persisted};
use iced::{Point, Size};
use std::path::PathBuf;
use std::sync::Arc;

use persisted::{FileUiStateStorage, InMemoryUiStateStorage, UiStateStorage};

/// Label rendered in the center pane next to the logo ([TASK-006] content).
const PILOT_LABEL: &str = "fastmd iced pilot";

/// Logo size, in logical pixels.
const LOGO_TEXT_PIXELS: f32 = 48.0;

/// Pilot-label size, in logical pixels.
const LABEL_TEXT_PIXELS: f32 = 20.0;

/// Vertical gap between logo and label, in logical pixels.
const COLUMN_SPACING_PIXELS: f32 = 16.0;

/// Vertical gap between the three bands, in logical pixels ([TASK-010]).
const BAND_SPACING_PIXELS: f32 = 8.0;

/// Window width ported from the egui `ViewportBuilder::with_inner_size`
/// (`src/app/main.rs:49`), in logical points; the DEFAULT applied when the
/// persisted state carries no window size ([TASK-009] (b)).
const WINDOW_WIDTH_POINTS: f32 = 1000.0;

/// Window height ported from the egui `ViewportBuilder::with_inner_size`
/// (`src/app/main.rs:49`), in logical points; see [`WINDOW_WIDTH_POINTS`].
const WINDOW_HEIGHT_POINTS: f32 = 700.0;

/// Pilot TOC gate input ([TASK-010]): the pilot has no document model yet,
/// so the gate reads false and the right pane stays collapsed.
const PILOT_HAS_TOC: bool = false;

/// Pilot TOC gate input; see [`PILOT_HAS_TOC`].
const PILOT_HAS_SELECTED_FILE: bool = false;

/// NFR-001 error message prefix for failed font registrations (TASK-007).
const FONT_LOAD_ERROR_CODE: &str = "TASK-007 font registration failure";

/// Environment variable that redirects the pilot's UI-state snapshot to an
/// explicit file ([TASK-009] pilot wiring, documented in the module header).
const PILOT_UI_STATE_PATH_ENV: &str = "FASTMD_ICED_UI_STATE_PATH";

fn main() -> iced::Result {
    // Single load ([TASK-009]): the persisted state is loaded once here and
    // feeds BOTH the booted window settings and the boot closure, so the
    // pilot reconstructs the same snapshot deterministically ([FMD-068]).
    let storage = pilot_storage_handler(std::env::var(PILOT_UI_STATE_PATH_ENV).ok());
    let persisted = persisted::load_ui_state(storage.as_ref());
    let settings = window_settings(&persisted);
    iced::application(
        // The closure satisfies the iced 0.14 boot contract `BootFn = Fn()
        // -> C where C: IntoBoot` (`iced-0.14.0/src/application.rs:
        // 540-548`): it never moves the captured handler out, it clones
        // the snapshot per call, so the Elm boot stays re-callable and
        // deterministic ([FMD-068]).
        move || boot(&storage, persisted.clone()),
        update,
        view,
    )
    .title(fastmd::ui::strings::WINDOW_TITLE)
    .window(settings)
    .theme(|_state: &PilotState| fastmd::ui_iced::theme::fastmd_dark_theme())
    // DEC-008: composed scale = OS baseline × persisted multiplier,
    // evaluated on the main/UI thread only ([FMD-069]).
    .scale_factor(|state: &PilotState| state.composed_scale_factor())
    .subscription(subscription)
    .run()
}

/// Resolves the pilot's injected UI-state store ([TASK-009] rule 4).
///
/// `None` (or an empty override) keeps every snapshot in-process
/// ([`InMemoryUiStateStorage`]); an override path stores the snapshot in
/// that explicit file ([`FileUiStateStorage`]). No platform-default
/// resolution happens anywhere in the pilot ([RUST-024], [RUST-006]).
fn pilot_storage_handler(env_override: Option<String>) -> Arc<dyn UiStateStorage> {
    match env_override {
        Some(path) if !path.is_empty() => Arc::new(FileUiStateStorage::new(PathBuf::from(path))),
        _ => Arc::new(InMemoryUiStateStorage::new()),
    }
}

/// Builds the `window::Settings` for the pilot window ([TASK-006]/
/// [TASK-009] (b)).
///
/// `Settings` fields verified in `iced_core-0.14.0/src/window/settings.rs`
/// (`size`:35, `position`:45, `icon`:87, `exit_on_close_request`:92-100).
/// Size/position seed from the loaded [`PersistedUiState`], falling back to
/// the egui-derived 1000×700 defaults (`main.rs:49`).
fn window_settings(persisted_state: &PersistedUiState) -> iced::window::Settings {
    let size = persisted::restore_window_size(persisted_state)
        .unwrap_or(Size::new(WINDOW_WIDTH_POINTS, WINDOW_HEIGHT_POINTS));
    iced::window::Settings {
        size,
        position: persisted::restore_window_position(persisted_state),
        icon: window_icon(),
        // DEC-006 groundwork: the runtime must NOT auto-close, so the app
        // can intercept `CloseRequested` and persist before closing
        // ([TASK-009] (a)).
        exit_on_close_request: false,
        ..Default::default()
    }
}

/// Loads the application icon as an iced window icon ([TASK-006]).
///
/// Decodes the SAME shared asset (`assets/icon-32.png`, 32px winit-default,
/// in `fastmd::utils::app_icon`) that `main.rs` feeds
/// `egui::ViewportBuilder::with_icon`, then wraps the straight RGBA pixels
/// with `iced::window::icon::from_rgba` (`iced_core-0.14.0/src/window/
/// icon.rs:7-11`). Falls back to `None` (iced default) if decoding fails.
fn window_icon() -> Option<iced::window::Icon> {
    let (rgba, width, height) = fastmd::utils::app_icon::decode_app_icon_rgba()?;
    iced::window::icon::from_rgba(rgba, width, height).ok()
}

/// The pilot application model ([TASK-009]/[TASK-010]).
struct PilotState {
    /// Injected storage handler for the persisted UI state ([RUST-024]).
    storage: Arc<dyn UiStateStorage>,
    /// The persisted UI-state snapshot; window metrics are refreshed at
    /// close time via `persisted::apply_close_metrics` (DEC-006).
    persisted: PersistedUiState,
    /// The last known logical window size; the resize-reduction input
    /// ([TASK-010]). Seeded with the booted window size.
    viewport: Size,
    /// The [TASK-010] panel widths (`fastmd::ui::PanelLayout`, [UI-016]).
    layout: fastmd::ui::PanelLayout,
    /// The middle-band pane grid state.
    panes: iced::widget::pane_grid::State<layout::PaneKind>,
    /// The middle-band pane/split ids discovered at construction.
    band: layout::MiddleBandPanes,
    /// OS-reported baseline scale captured at boot (DEC-008); `None` until
    /// the chained boot task settles, in which case `1.0` applies.
    os_scale: Option<f32>,
}

impl PilotState {
    /// Assembles the pilot state from the injected store and the loaded
    /// snapshot ([TASK-009] (b)).
    fn new(storage: Arc<dyn UiStateStorage>, persisted_state: PersistedUiState) -> Self {
        let (panes, band) = layout::middle_band_state();
        let viewport = persisted::restore_window_size(&persisted_state)
            .unwrap_or(Size::new(WINDOW_WIDTH_POINTS, WINDOW_HEIGHT_POINTS));
        Self {
            storage,
            persisted: persisted_state,
            viewport,
            layout: fastmd::ui::PanelLayout::new(),
            panes,
            band,
            os_scale: None,
        }
    }

    /// The DEC-008 composed scale: OS baseline × persisted multiplier.
    fn composed_scale_factor(&self) -> f32 {
        persisted::compose_scale_factor(
            self.os_scale.unwrap_or(1.0),
            self.persisted.font_size_scale,
        )
    }
}

impl core::fmt::Debug for PilotState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PilotState")
            .field("storage", &"<dyn UiStateStorage>")
            .field("persisted", &self.persisted)
            .field("viewport", &self.viewport)
            .field("layout", &self.layout)
            .field("panes", &self.panes)
            .field("band", &self.band)
            .field("os_scale", &self.os_scale)
            .finish()
    }
}

/// Pilot message set ([TASK-006]/[TASK-009]/[TASK-010]).
#[derive(Debug, Clone)]
enum Message {
    /// One font collection settled registering ([TASK-007]); `Some` holds
    /// the description of a failed load, `None` means the collection loaded
    /// successfully.
    FontLoadFinished(Option<String>),
    /// The user asked to close a window (title bar / Alt+F4). With
    /// `exit_on_close_request: false` the runtime does nothing on its own;
    /// `update` then fetches the window metrics (DEC-006) to persist before
    /// actually closing ([TASK-009] (a)).
    CloseRequested(iced::window::Id),
    /// The close-request metrics arrived via the chained
    /// `iced::window::size` → `iced::window::position` report tasks
    /// ([TASK-009] (a); `iced_runtime-0.14.0/src/window.rs:338,369`).
    CloseSnapshotCaptured {
        id: iced::window::Id,
        size: Size,
        position: Option<Point>,
    },
    /// The OS-reported baseline scale arrived (DEC-008, chained
    /// `window::latest()` → `window::scale_factor` boot task).
    ScaleBaseline(f32),
    /// A window was resized; refreshes the viewport for the pane-resize
    /// ratio math ([TASK-010]; subscription verified
    /// `iced_runtime-0.14.0/src/window.rs:249-257`).
    WindowResized(Size),
    /// A pane-grid split was dragged ([TASK-010]; event fields verified
    /// `iced_widget-0.14.2/src/pane_grid.rs:1226-1237`).
    PaneResized(iced::widget::pane_grid::ResizeEvent),
}

/// Establishes the initial pilot state plus the font-loading ([TASK-007])
/// and DEC-008 scale-baseline boot tasks.
///
/// `impl BootFn<State, Message> for T where T: Fn() -> C, C: IntoBoot`
/// (`iced-0.14.0/src/application.rs:540-566`); the scale capture chains
/// `iced::window::latest()` (`Task<Option<Id>>`, `window.rs:294`) into
/// `iced::window::scale_factor(id)` (`Task<f32>`, `window.rs:376`) with
/// `Task::then` (`iced_runtime-0.14.0/src/task.rs:128-156`).
fn boot(
    storage: &Arc<dyn UiStateStorage>,
    persisted: PersistedUiState,
) -> (PilotState, iced::Task<Message>) {
    let state = PilotState::new(Arc::clone(storage), persisted);
    let scale = iced::window::latest().then(|latest| match latest {
        Some(id) => iced::window::scale_factor(id).map(Message::ScaleBaseline),
        None => iced::Task::none(),
    });
    (
        state,
        fastmd::ui_iced::fonts::font_boot_task()
            .map(Message::FontLoadFinished)
            .chain(scale),
    )
}

/// Pilot message handler ([TASK-006]/[TASK-007]/[TASK-009]/[TASK-010]).
///
/// `impl UpdateFn ... for T where T: Fn(&mut State, Message) -> C, C: Into<Task<Message>>`
/// (`iced-0.14.0/src/application.rs:609-617`).
fn update(state: &mut PilotState, message: Message) -> iced::Task<Message> {
    match message {
        Message::FontLoadFinished(Some(error)) => {
            // NFR-001: every external-protocol (font registration) failure
            // must produce an ERROR log. Non-fatal: iced keeps its default
            // font set and the app stays usable (FMD-064).
            tracing::error!(name = FONT_LOAD_ERROR_CODE, %error, "Font registration failed");
            iced::Task::none()
        }
        Message::FontLoadFinished(None) => {
            // The font collection registered cleanly; nothing to do.
            iced::Task::none()
        }
        Message::CloseRequested(id) => {
            // DEC-006 phase 1 ([TASK-009] (a)): BEFORE closing, the current
            // window metrics are reported through two chained runtime
            // tasks — `iced::window::size(id) -> Task<Size>` (:338) and
            // `iced::window::position(id) -> Task<Option<Point>>` (:369) —
            // so the next message can persist + close in one reduction.
            iced::window::size(id).then(move |size| {
                iced::window::position(id).map(move |position| Message::CloseSnapshotCaptured {
                    id,
                    size,
                    position,
                })
            })
        }
        Message::CloseSnapshotCaptured { id, size, position } => {
            // DEC-006 phase 2 ([TASK-009] (a)): FIRST persist, THEN close.
            // The panel widths are copied from the live `PanelLayout` into
            // the snapshot right before serialising — the faithful port of
            // the egui `save()` sync (`ui/app/mod.rs:251-252`), so widths
            // set by ANY path (not just pane drags) are persisted. The
            // egui build's remaining fields (expanded dirs, about stamp)
            // belong to the egui path and are carried verbatim through
            // this snapshot's load.
            state.persisted.left_panel_width = state.layout.left_panel_width;
            state.persisted.right_panel_width = state.layout.right_panel_width;
            persisted::apply_close_metrics(&mut state.persisted, size, position);
            match persisted::save_ui_state(state.storage.as_ref(), &state.persisted) {
                Ok(path) => tracing::info!(?path, "Persisted UI state saved on close"),
                Err(error) => {
                    tracing::error!(
                        name = persisted::UI_STATE_SAVE_ERROR_CODE,
                        %error,
                        "Failed to persist UI state on close"
                    );
                }
            }
            // THEN close the requesting window ([TASK-006]; the runtime
            // keeps running until the last window closes).
            iced::window::close(id)
        }
        Message::ScaleBaseline(baseline) => {
            // DEC-008: store the OS baseline; the `.scale_factor(builder)`
            // composes it with the persisted multiplier per frame
            // ([FMD-069]: main/UI-thread state updates only).
            state.os_scale = Some(baseline);
            iced::Task::none()
        }
        Message::WindowResized(size) => {
            // Keep the logical viewport current ([TASK-010] input).
            state.viewport = size;
            iced::Task::none()
        }
        Message::PaneResized(event) => {
            // [TASK-010]: reduce the drag onto the UI-016-clamped widths,
            // replay the CLAMPED ratio into the pane grid (match geometry to
            // the persisted width), and keep the persisted snapshot
            // consistent for the next close-time save.
            if let Some((split, ratio)) = layout::apply_pane_resize(
                &state.band,
                event,
                state.viewport.width,
                &mut state.layout,
            ) {
                state.panes.resize(split, ratio);
                state.persisted.left_panel_width = state.layout.left_panel_width;
                state.persisted.right_panel_width = state.layout.right_panel_width;
            }
            iced::Task::none()
        }
    }
}

/// Declares the pilot's runtime subscriptions ([TASK-006]/[TASK-010]).
///
/// - `close_requests()` delivers exactly one message per user close request
///   (`Event::CloseRequested` filter verified in
///   `iced_runtime-0.14.0/src/window.rs:260-267`), keeping the
///   `exit_on_close_request: false` window closable;
/// - `resize_events()` (`window.rs:249-257`) keeps the viewport size
///   current for the pane-resize ratio math; the pilot windows the events
///   to the size only (single-window pilot, documented; TASK-022 will
///   carry the window `Id`).
fn subscription(_state: &PilotState) -> iced::Subscription<Message> {
    iced::Subscription::batch([
        iced::window::close_requests().map(Message::CloseRequested),
        iced::window::resize_events().map(|(_id, size)| Message::WindowResized(size)),
    ])
}

/// Builds the center-pane content (the [TASK-004] iced logo plus label).
/// A free function — `iced::Element` is not `Clone`, so the pane-grid view
/// callback reconstructs it per call.
fn center_pane_element() -> iced::Element<'static, Message> {
    use iced::Length;
    use iced::widget::{column, container, iced as iced_logo, text};

    container(
        column![
            iced_logo(LOGO_TEXT_PIXELS),
            text(PILOT_LABEL).size(LABEL_TEXT_PIXELS)
        ]
        .spacing(COLUMN_SPACING_PIXELS)
        .align_x(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}

/// Renders the [TASK-010] three-band layout with the iced logo living in
/// the center pane (`ViewFn`, TASK-004).
///
/// `impl ViewFn ... for T where T: Fn(&'a State) -> Widget, Widget: Into<Element<...>>`
/// (`iced-0.14.0/src/application.rs:628-638`).
// The `|| center_pane_element()` closure is NOT redundant: the fn item
// `fn() -> Element<'static, _>` forces the builder's `'a` to `'static`
// (E0521 on `&_state.panes`), while a closure infers its signature from
// the expected bound and coerces the `'static` return covariantly.
#[allow(clippy::redundant_closure)]
fn view(_state: &PilotState) -> iced::Element<'_, Message> {
    iced::widget::column![
        layout::top_band(),
        layout::middle_band(
            &_state.panes,
            || center_pane_element(),
            PILOT_HAS_TOC,
            PILOT_HAS_SELECTED_FILE,
            Message::PaneResized,
        ),
        layout::bottom_band(),
    ]
    .spacing(BAND_SPACING_PIXELS)
    .into()
}
