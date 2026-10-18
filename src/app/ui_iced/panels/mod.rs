//! Iced panel views — one submodule per FastMD pane ([TASK-011]..[TASK-016]).
//!
//! A panel here is a **pure view function**: it takes an immutable view-model
//! snapshot plus the data it needs to render, and returns an
//! [`iced::Element`] of [`Message`](crate::ui_iced::message::Message). Panels
//! never read `FastMdApp` inside the widget tree and never publish to the
//! `UserCommand` bus themselves — they *emit*
//! [`Message::Command`](crate::ui_iced::message::Message::Command) and the
//! `update` reducer ([TASK-022]/[TASK-023]) performs the publish, which is
//! what keeps user intent replayable ([FMD-068]) and the view free of
//! side effects.
//!
//! Submodules mirror the egui panes they replace
//! (`src/app/ui/panels/`):
//!
//! - [`top`] — the top toolbar: logo mark, app title, indexing status,
//!   tag filter and the UI-061..UI-066 hamburger menu ([TASK-011]).
//!
//! `bottom`, `left`, `right` and `center` follow in [TASK-012]..[TASK-016].

pub mod top;
