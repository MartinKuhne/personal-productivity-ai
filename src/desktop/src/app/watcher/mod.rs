//! File-watcher plumbing — the event bus, its consumers (file processor and
//! directory tracker), and the `notify`-based filesystem watcher.
//!
//! All items in this module are egui-free; they are unit-tested without
//! driving the UI. The UI layer treats them as pure state-holding types.

pub mod directory_tracker;
pub mod events;
pub mod file_processor;
pub mod file_watcher;

pub use directory_tracker::DirectoryTracker;
pub use events::{Bus, BusReader, FileEvent, FileEventKind, FileEventProducer};
pub use file_processor::FileEventProcessor;
pub use file_watcher::FileWatcher;
