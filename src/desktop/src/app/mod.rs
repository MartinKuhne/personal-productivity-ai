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
pub mod dialog_manager;
pub mod document;
pub mod orchestrator;
pub mod panel_layout;
pub mod persisted;
pub mod print;
#[cfg(feature = "pdf-export")]
pub mod print_pdf;
pub mod selection_manager;
pub mod session;
pub mod tab_manager;
pub mod tag_manager;
pub mod text_buffer;
pub mod vfs;
pub mod watcher;

pub use dialog_manager::DialogManager;
pub use panel_layout::PanelLayout;
pub use persisted::PersistedUiState;
pub use selection_manager::SelectionManager;
pub use tab_manager::TabManager;
pub use tag_manager::TagManager;
pub use text_buffer::{Cursor, Selection, TextBuffer, UndoStack};
pub use vfs::{VirtualPath, VirtualPathError};
