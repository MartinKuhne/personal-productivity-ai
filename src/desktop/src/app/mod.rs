//! Application domain types — managers, persisted state, and the background-message channel.
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
//! The file-watcher plumbing lives in [`watcher`].

pub mod dialog_manager;
pub mod messages;
pub mod panel_layout;
pub mod persisted;
pub mod selection_manager;
pub mod tab_manager;
pub mod tag_manager;
pub mod text_buffer;
pub mod vfs;
pub mod watcher;

pub use dialog_manager::DialogManager;
pub use messages::{BackgroundMessage, TokenUsageInfo};
pub use panel_layout::PanelLayout;
pub use persisted::PersistedUiState;
pub use selection_manager::SelectionManager;
pub use tab_manager::{TabManager, ToCEntry};
pub use tag_manager::TagManager;
pub use text_buffer::{Cursor, Selection, TextBuffer, UndoStack};
pub use vfs::{VirtualPath, VirtualPathError};
