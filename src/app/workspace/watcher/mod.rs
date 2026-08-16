//! File-watcher plumbing — its consumers (file processor and directory
//! tracker) and the `notify`-based filesystem watcher.
//!
//! All items in this module are egui-free; they are unit-tested without
//! driving the UI. The UI layer treats them as pure state-holding types.
//!
//! The bus transport and event payload types live in [`crate::bus`].

pub mod directory_tracker;
pub mod file_processor;
pub mod file_watcher;
pub mod pdf_backing_tracker;

pub use directory_tracker::DirectoryTracker;
pub use file_processor::FileEventProcessor;
pub use file_watcher::FileWatcher;
pub use pdf_backing_tracker::PdfBackingTracker;
