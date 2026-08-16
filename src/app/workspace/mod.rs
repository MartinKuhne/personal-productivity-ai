//! Workspace domain subsystem — virtual file system, library resolution, and file watcher.
//!
//! Owns the egui-independent workspace and file tracker data structures.

pub mod vfs;
pub mod watcher;

pub use vfs::{VirtualPath, VirtualPathError, library_display_label};
pub use watcher::{DirectoryTracker, FileEventProcessor, FileWatcher};
