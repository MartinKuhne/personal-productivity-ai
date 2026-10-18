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
//! - [`bottom`] — the command band: the prompt prefix and active-file
//!   context, the command input and the agent Stop button, with the
//!   queue-vs-submit intent solver extracted to [`bottom::submit_commands`]
//!   ([TASK-016], UI-001).
//! - [`right`] — the ToC pane's two **pure** rules (the visibility gate and
//!   the anchor → heading-id resolver), relocated out of the egui panel for
//!   the zero-egui migration ([TASK-029]). The pane *body* itself is still
//!   rendered inline by [`view::toc_body`](crate::ui_iced::view).
//!
//! The Background Processes viewer ([TASK-016], UI-047) is not a pane — the
//! egui build shows it as a floating window — so it lives in
//! [`crate::ui_iced::logs`] and is mounted by the overlay/window host in
//! [TASK-017]/[TASK-023].
//!
//! # Self-containment ([TASK-029])
//!
//! Every framework-free helper these four panes need is **owned** by the
//! iced module that uses it, not `pub use`d out of the egui
//! `src/app/ui/panels/` tree — those files are deleted by [TASK-029b].
//! See each submodule's header for what was relocated and why.

pub mod bottom;
pub mod center;
pub mod left;
pub mod right;
pub mod top;
