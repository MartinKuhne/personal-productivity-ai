//! File-change observer bridging `OnFileChanged` to the `Bus<FileEvent>` broadcast.
//!
//! Unit tests live in the sibling `bus_observer_tests.rs` sidecar.

use crate::agent::tools::observer::OnFileChanged;
use crate::bus::core::Bus;
use crate::bus::events::file::{FileEvent, FileEventProducer};

/// Bridges file-change notifications to the `Bus<FileEvent>` broadcast.
pub struct AppFileObserver {
    producer: FileEventProducer,
}

impl AppFileObserver {
    /// Create a new observer that publishes `Updated` events onto `bus`.
    pub fn new(bus: Bus<FileEvent>) -> Self {
        Self {
            producer: FileEventProducer::new(bus),
        }
    }
}

impl OnFileChanged for AppFileObserver {
    fn on_file_changed(&self, path: &std::path::Path) {
        self.producer.publish_updated(path);
    }
}

#[cfg(test)]
#[path = "bus_observer_tests.rs"]
mod tests;
