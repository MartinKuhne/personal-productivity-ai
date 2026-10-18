//! Iced pilot binary (`TASK-004`, `doc/planning/iced.md`) — Phase 1 runtime spike.
//!
//! Smoke-tests the `iced` 0.14 runtime bootstrap with a minimal Elm-architecture
//! application: an empty-state `boot`, a no-op `update` passthrough, and a
//! static `view` rendering the built-in iced logo widget plus a short label.
//!
//! The pilot state is deliberately tiny and contained in this file; real state
//! bootstrapping lands in Phase 2/4 (`TASK-006`/`TASK-022`,
//! `doc/planning/iced.md`). Success criteria for this binary are recorded in
//! the `doc/planning/iced-migration-map.md` footer (`TASK-005`).
//!
//! Verified APIs (local sources):
//! - `iced::application(boot, update, view).run()` — `iced-0.14.0/src/application.rs:79-89,185-186`
//! - `iced::widget::iced(text_size)` logo widget — `iced_widget-0.14.2/src/helpers.rs:1852-1894`

/// Label rendered by the pilot view.
const PILOT_LABEL: &str = "fastmd iced pilot";

/// Logo size, in logical pixels.
const LOGO_TEXT_PIXELS: f32 = 48.0;

/// Pilot-label size, in logical pixels.
const LABEL_TEXT_PIXELS: f32 = 20.0;

/// Vertical gap between logo and label, in logical pixels.
const COLUMN_SPACING_PIXELS: f32 = 16.0;

fn main() -> iced::Result {
    iced::application(boot, update, view).run()
}

/// The tiny pilot application model: carries no data yet (`PilotState`, TASK-004).
#[derive(Debug, Default)]
struct PilotState;

/// Pilot message type. The pilot has no interactions, so it is uninhabited.
#[derive(Debug, Clone)]
enum Message {}

/// Establishes the initial pilot state (`BootFn`, TASK-004).
///
/// `impl BootFn<State, Message> for T where T: Fn() -> C, C: IntoBoot`
/// (`iced-0.14.0/src/application.rs:540-548`); `State: IntoBoot` wraps the
/// value with `Task::none()` (`iced-0.14.0/src/application.rs:556-560`).
fn boot() -> PilotState {
    PilotState
}

/// No-op passthrough message handler (`UpdateFn`, TASK-004).
///
/// `impl UpdateFn ... for T where T: Fn(&mut State, Message) -> C, C: Into<Task<Message>>`
/// (`iced-0.14.0/src/application.rs:609-617`); the unit type converts into
/// `Task::none()`, so a plain `()` return keeps the handler a true passthrough.
fn update(_state: &mut PilotState, _message: Message) -> iced::Task<Message> {
    match _message {}
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
