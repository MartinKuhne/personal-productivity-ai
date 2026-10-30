//! File-change observer bridging `OnFileChanged` to the `Bus<FileChanged>` broadcast.
//!
//! Unit tests live in the sibling `bus_observer_tests.rs` sidecar.

use crate::agent::tools::observer::OnFileChanged;
use crate::bus::core::Bus;
use crate::bus::events::file::{FileChanged, FileEventProducer};

/// Bridges file-change notifications to the `Bus<FileChanged>` broadcast.
pub struct AppFileObserver {
    producer: FileEventProducer,
}

impl AppFileObserver {
    /// Create a new observer that publishes `[FileChanged][Changed]` events onto `bus`.
    pub fn new(bus: Bus<FileChanged>) -> Self {
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
