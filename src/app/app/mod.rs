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

pub mod background;
pub mod background_task;
pub mod batch;
#[cfg(feature = "browser")]
pub mod browser;
pub mod dialogs;
pub mod document;
pub mod events;
pub mod orchestrator;
pub mod panel_layout;
pub mod persisted;
pub mod print;
#[cfg(feature = "pdf-export")]
pub mod print_pdf;
pub mod prompts;
pub mod selection;
pub mod session;
pub mod tabs;
pub mod tags;
pub mod text_buffer;

pub use crate::workspace::{vfs, watcher};

pub use dialogs::Dialogs;
pub use panel_layout::PanelLayout;
pub use persisted::PersistedUiState;
pub use selection::FileSelection;
pub use tabs::Tabs;
pub use tags::Tags;
pub use text_buffer::{Cursor, Selection, TextBuffer, UndoStack};
pub use vfs::{VirtualPath, VirtualPathError};
