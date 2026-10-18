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
//! - [`left`] — the file-tree pane: reused `flatten_tree` rows in
//!   UI-017/UI-018 order, the UI-050 content-search box, the UI-016
//!   auto-fit solver and the DEC-005 row context menus ([TASK-012]).
//! - [`center`] — the document pane: the UI-049 tab strip, the
//!   UI-026..UI-034 tab context menu, the UI-048 thinking gate and the
//!   viewer area whose markdown render is [TASK-015] ([TASK-013]).
//!
//! `bottom` and `right` follow in [TASK-014]..[TASK-016].

pub mod center;
pub mod left;
pub mod top;
