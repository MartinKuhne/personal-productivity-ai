//! Workspace domain subsystem — virtual file system, library resolution, and file watcher.
//!
//! Owns the egui-independent workspace and file tracker data structures.

pub(crate) mod tags;
pub mod vfs;
pub(crate) mod watcher;

pub use tags::Tags;
pub use vfs::{VirtualPath, VirtualPathError, library_display_label};
pub use watcher::{DirectoryTracker, FileEventProcessor, FileWatcher};
