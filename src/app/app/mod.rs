//! Application domain types — managers and persisted state.
//!
//! This module owns the egui-independent application state. The managers
//! here can be unit-tested in isolation: they do not import `eframe::egui`
//! or any UI crate, and the public API is plain Rust data and behaviour.
//!
//! The UI layer (`crate::ui`) is a thin adapter over this module; it
//! translates between egui types and the stable identifiers used here
//! (e.g. `ToCEntry::id` is a `String` here, which the right panel maps
//! to `egui::Id::new(&id_str)` at render time).
//!
//! The file-watcher plumbing lives in [`watcher`]. All cross-thread
//! messaging primitives (event buses, message types, bus routing) live
//! in [`crate::bus`].

pub use crate::agent::batch;
pub use crate::background;
pub use crate::background::task as background_task;
pub mod document;
pub use crate::bus::events::agent as events;
pub mod orchestrator;
pub use crate::agent::prompts;
pub use crate::agent::session;
#[cfg(feature = "pdf-export")]
pub use crate::export::pdf as print_pdf;
pub use crate::export::print;

pub use crate::ui::{
    Cursor, Dialogs, FileSelection, OAuthFlowStatus, PanelLayout, PersistedUiState, Selection,
    TabItem, Tabs, TextBuffer, TreeSearch, UndoStack, dialogs, panel_layout, persisted, selection,
    tabs, text_buffer, tree_search,
};
pub use crate::workspace::{Tags, VirtualPath, VirtualPathError, tags, vfs, watcher};
