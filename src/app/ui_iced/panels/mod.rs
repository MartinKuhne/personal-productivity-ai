//! Iced panel views — one submodule per FastMD pane.
//!
//! A panel here is a **pure view function**: it takes an immutable view-model
//! snapshot plus the data it needs to render, and returns an
//! [`iced::Element`] of [`Message`](crate::ui_iced::message::Message). Panels
//! never read `FastMdApp` inside the widget tree and never publish to the
//! `UserCommand` bus themselves — they *emit*
//! [`Message::Command`](crate::ui_iced::message::Message::Command) and the
//! `update` reducer performs the publish. Each emitted command is
//! self-contained and replayable on its own, which keeps the view free of
//! side effects.
//!
//! Submodules:
//!
//! - [`top`] — the top toolbar: logo mark, app title, indexing status,
//!   tag filter and the UI-061..UI-066 hamburger menu.
//! - [`left`] — the file-tree pane: `flatten_tree` rows in
//!   UI-017/UI-018 order, the UI-050 content-search box, the UI-016
//!   auto-fit solver and the row context menus.
//! - [`center`] — the document pane: the UI-049 tab strip, the
//!   UI-026..UI-034 tab context menu, the UI-048 thinking gate and the
//!   viewer area, which renders markdown.
//! - [`bottom`] — the command band: the prompt prefix and active-file
//!   context, the command input and the agent Stop button, with the
//!   queue-vs-submit intent solver extracted to [`bottom::submit_commands`]
//!   (UI-001).
//! - [`right`] — the ToC pane's two **pure** rules: the visibility gate and
//!   the anchor → heading-id resolver. The pane *body* itself is rendered
//!   inline by [`view::toc_body`](crate::ui_iced::view).
//!
//! The Background Processes viewer (UI-047) is not a pane — it is a floating
//! window — so it lives in [`crate::ui_iced::logs`] and is mounted by the
//! overlay/window host.
//!
//! # Self-containment
//!
//! Every framework-free helper these panes need is **owned** by the iced module
//! that uses it rather than re-exported from another tree, so each submodule
//! reads on its own.

pub mod bottom;
pub mod center;
pub mod left;
pub mod right;
pub mod top;
