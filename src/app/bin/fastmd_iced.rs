//! Iced pilot binary (`TASK-006`, `doc/planning/iced.md`) — Phase 2 window
//! bootstrap on top of the `TASK-004` runtime spike.
//!
//! Ports the egui window configuration from `src/app/main.rs` (left
//! untouched per TASK-006: `main.rs` keeps launching egui until `TASK-024`
//! flips the entry point) onto the iced `application` builder chain:
//!
//! - window title `strings::WINDOW_TITLE` ("FastMD AI", DEC-010) via
//!   `.title(...)` — verified `iced-0.14.0/src/application.rs:346-360`;
//! - window size/icon/`exit_on_close_request` via `.window(window::Settings)`
//!   — verified `iced-0.14.0/src/application.rs:250-255`; fields
//!   `size` (`iced_core-0.14.0/src/window/settings.rs:35-37`), `icon` (:87)
//!   and `exit_on_close_request` (:92-100, default `true`). The icon decodes
//!   the shared `assets/icon-32.png` asset through
//!   `fastmd::utils::app_icon::decode_app_icon_rgba` + `iced::window::icon::
//!   from_rgba` (verified `iced-0.14.0/src/window/icon.rs:1-2`,
//!   `iced_core-0.14.0/src/window/icon.rs:7-11`);
//! - `exit_on_close_request: false` pre-enables the DEC-006 save-on-close
//!   flow. Because the runtime then keeps windows open on close requests, a
//!   MINIMAL close handler is wired NOW so the pilot stays closable:
//!   `iced::window::close_requests()` (verified
//!   `iced_runtime-0.14.0/src/window.rs:259-268`) → `Message::CloseRequested`
//!   → `iced::window::close(id)` (:283-286). `TASK-009` extends this with
//!   the persistence-save step.
//! - the palette mapping from [`fastmd::ui_iced::theme`] is installed via
//!   `.theme(...)` (verified `application.rs:377-392`, [TASK-008]).
//! - fonts register through `fastmd::ui_iced::fonts::font_boot_task()` as
//!   the `(State, Task)` boot task ([TASK-007]; `BootFn` may return
//!   `(State, Task<Message>)` — `iced-0.14.0/src/application.rs:551-566`).
//!
//! The pilot still renders the iced logo widget plus label (`TASK-004`
//! smoke content) and is contained in this file; real state and the full
//! view land in Phase 3/4 (`TASK-010`/`TASK-022`, `doc/planning/iced.md`).

/// Label rendered by the pilot view.
const PILOT_LABEL: &str = "fastmd iced pilot";

/// Logo size, in logical pixels.
const LOGO_TEXT_PIXELS: f32 = 48.0;

/// Pilot-label size, in logical pixels.
const LABEL_TEXT_PIXELS: f32 = 20.0;

/// Vertical gap between logo and label, in logical pixels.
const COLUMN_SPACING_PIXELS: f32 = 16.0;

/// Window width ported from the egui `ViewportBuilder::with_inner_size`
/// (`src/app/main.rs:49`), in logical points.
const WINDOW_WIDTH_POINTS: f32 = 1000.0;

/// Window height ported from the egui `ViewportBuilder::with_inner_size`
/// (`src/app/main.rs:49`), in logical points.
const WINDOW_HEIGHT_POINTS: f32 = 700.0;

/// NFR-001 error message prefix for failed font registrations (TASK-007).
const FONT_LOAD_ERROR_CODE: &str = "TASK-007 font registration failure";

fn main() -> iced::Result {
    iced::application(boot, update, view)
        .title(fastmd::ui::strings::WINDOW_TITLE)
        .window(window_settings())
        .theme(|_state: &PilotState| fastmd::ui_iced::theme::fastmd_dark_theme())
        .subscription(subscription)
        .run()
}

/// Builds the `window::Settings` for the pilot window ([TASK-006]).
///
/// `Settings` fields verified in `iced_core-0.14.0/src/window/settings.rs`
/// (`size`:35, `icon`:87, `exit_on_close_request`:92-100).
fn window_settings() -> iced::window::Settings {
    iced::window::Settings {
        size: iced::Size::new(WINDOW_WIDTH_POINTS, WINDOW_HEIGHT_POINTS),
        icon: window_icon(),
        // DEC-006 groundwork: the runtime must NOT auto-close, so the app
        // can intercept `CloseRequested` (`TASK-009` adds the save step).
        // The `CloseRequested` subscription below keeps the window closable.
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

/// The tiny pilot application model: carries no data yet (`PilotState`,
/// TASK-004).
#[derive(Debug, Default)]
struct PilotState;

/// Pilot message set ([TASK-006]): the runtime events the Elm loop reacts to.
#[derive(Debug, Clone)]
enum Message {
    /// One font collection settled registering ([TASK-007]); `Some` holds
    /// the description of a failed load, `None` means the collection loaded
    /// successfully.
    FontLoadFinished(Option<String>),
    /// The user asked to close a window (title bar / Alt+F4). With
    /// `exit_on_close_request: false` the runtime does nothing on its own,
    /// so `update` closes the window via `iced::window::close` to keep the
    /// pilot closable ([TASK-006]; `TASK-009` adds the save step first).
    CloseRequested(iced::window::Id),
}

/// Establishes the initial pilot state and the font-loading boot task
/// ([TASK-007]).
///
/// `impl BootFn<State, Message> for T where T: Fn() -> C, C: IntoBoot`
/// (`iced-0.14.0/src/application.rs:540-548`); a `(State, Task<Message>)`
/// return wraps into a state-with-task boot
/// (`iced-0.14.0/src/application.rs:551-566`), so the font collections are
/// registered with the runtime font system before the first frame settles.
fn boot() -> (PilotState, iced::Task<Message>) {
    let fonts = fastmd::ui_iced::fonts::font_boot_task();
    (PilotState, fonts.map(Message::FontLoadFinished))
}

/// Pilot message handler ([TASK-006]/[TASK-007]).
///
/// `impl UpdateFn ... for T where T: Fn(&mut State, Message) -> C, C: Into<Task<Message>>`
/// (`iced-0.14.0/src/application.rs:609-617`).
fn update(_state: &mut PilotState, message: Message) -> iced::Task<Message> {
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
            // Minimal close handler ([TASK-006]): closing the requesting
            // window restores the default exit-on-last-window behavior.
            iced::window::close(id)
        }
    }
}

/// Declares the pilot's runtime subscriptions ([TASK-006]).
///
/// `close_requests()` delivers exactly one message per user close request
/// (`Event::CloseRequested` filter verified in
/// `iced_runtime-0.14.0/src/window.rs:259-268`), unioned here so the
/// `exit_on_close_request: false` window remains closable. `TASK-009`
/// extends `update` to persist UI state before closing.
fn subscription(_state: &PilotState) -> iced::Subscription<Message> {
    iced::window::close_requests().map(Message::CloseRequested)
}

/// Renders the centered iced logo plus the pilot label (`ViewFn`, TASK-004).
///
/// `impl ViewFn ... for T where T: Fn(&'a State) -> Widget, Widget: Into<Element<...>>`
/// (`iced-0.14.0/src/application.rs:628-638`). The logo widget is
/// `iced::widget::iced(text_size)` (`iced_widget-0.14.2/src/helpers.rs:1852-1894`).
fn view(_state: &PilotState) -> iced::Element<'_, Message> {
    use iced::widget::{column, container, iced as iced_logo, text};
    use iced::{Alignment, Length};

    container(
        column![
            iced_logo(LOGO_TEXT_PIXELS),
            text(PILOT_LABEL).size(LABEL_TEXT_PIXELS)
        ]
        .spacing(COLUMN_SPACING_PIXELS)
        .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_y(Length::Fill)
    .into()
}
