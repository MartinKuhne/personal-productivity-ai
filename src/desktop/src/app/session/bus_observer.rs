use crate::agent::tools::observer::OnFileChanged;
use crate::bus::core::Bus;
use crate::bus::events::file::{FileEvent, FileEventProducer};

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
    fn on_file_changed(&self, path: &std::path::Path) {
        self.producer.publish_updated(path);
    }
}
