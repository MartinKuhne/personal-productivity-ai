use crate::bus::events::file::{FileEvent, FileEventKind, FileEventProducer};
use crate::bus::core::Bus;
use crate::agent::tools::observer::OnFileChanged;

pub struct AppFileObserver {
    producer: FileEventProducer,
}

impl AppFileObserver {
    pub fn new(bus: Bus<FileEvent>) -> Self {
        Self {
            producer: FileEventProducer::new(bus),
        }
    }
}

impl OnFileChanged for AppFileObserver {
    fn on_file_changed(&self, path: &std::path::Path, kind: FileEventKind) {
        match kind {
            FileEventKind::Discovered => self.producer.publish_discovered(path),
            FileEventKind::Updated => self.producer.publish_updated(path),
            FileEventKind::Removed => self.producer.publish_removed(path),
            _ => {} // dir events not typically emitted by agent tools
        }
    }
}
