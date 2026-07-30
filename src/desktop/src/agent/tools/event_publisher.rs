//! Event publisher — filesystem event bus handle without config access.
//!
//! Extracted from [`ToolContext`] so mutating tools can pick up
//! exactly the bus capability they need.

use crate::app::watcher::events::{Bus, FileEvent, FileEventKind, FileEventProducer};
use std::path::Path;

/// Thin bus publisher that owns only the event sender.
#[derive(Debug, Clone, Copy)]
pub struct EventPublisher<'a> {
    pub bus: &'a Bus<FileEvent>,
}

impl<'a> EventPublisher<'a> {
    pub fn new(bus: &'a Bus<FileEvent>) -> Self {
        Self { bus }
    }

    pub fn publish_file_event(&self, kind: FileEventKind, path: &Path) {
        let producer = FileEventProducer::new(self.bus);
        match kind {
            FileEventKind::Discovered => producer.publish_discovered(path),
            FileEventKind::Updated => producer.publish_updated(path),
            FileEventKind::Removed => producer.publish_removed(path),
            FileEventKind::DirDiscovered => producer.publish_dir_discovered(path),
            FileEventKind::DirRemoved => producer.publish_dir_removed(path),
        }
    }

    pub fn file_event_producer(&self) -> FileEventProducer<'a> {
        FileEventProducer::new(self.bus)
    }
}
