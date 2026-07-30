//! Tool context — provides tools with access to `AppConfig` and the file event bus, plus safe virtual-path resolution.

use crate::app::vfs;
use crate::app::watcher::events::{Bus, FileEvent, FileEventKind, FileEventProducer};
use std::path::{Path, PathBuf};

pub struct ToolContext<'a> {
    pub config: &'a crate::config::AppConfig,
    pub file_event_bus: &'a Bus<FileEvent>,
}

impl<'a> ToolContext<'a> {
    pub fn new(config: &'a crate::config::AppConfig, file_event_bus: &'a Bus<FileEvent>) -> Self {
        Self {
            config,
            file_event_bus,
        }
    }

    /// Resolve a virtual path to an absolute filesystem path. Thin shim
    /// over [`vfs::resolve::resolve`] that pulls the library list from
    /// the active config. Spec: [`app/vfs/SPEC.md`](../../app/vfs/SPEC.md) (VFS-004, VFS-009).
    pub fn resolve_virtual_path(
        &self,
        vpath: &str,
        allow_write: bool,
    ) -> Result<Option<(PathBuf, bool)>, String> {
        vfs::resolve::resolve(vpath, allow_write, &self.config.content_libraries)
    }

    /// Resolve a virtual path for a mutating tool. Thin shim over
    /// [`vfs::resolve::resolve_writable`]. Returns the absolute
    /// filesystem path on success.
    pub fn resolve_writable(&self, vpath: &str) -> Result<PathBuf, String> {
        vfs::resolve::resolve_writable(vpath, &self.config.content_libraries)
    }

    pub fn publish_file_event(&self, kind: FileEventKind, path: &Path) {
        let producer = FileEventProducer::new(self.file_event_bus);
        match kind {
            FileEventKind::Discovered => producer.publish_discovered(path),
            FileEventKind::Updated => producer.publish_updated(path),
            FileEventKind::Removed => producer.publish_removed(path),
            FileEventKind::DirDiscovered => producer.publish_dir_discovered(path),
            FileEventKind::DirRemoved => producer.publish_dir_removed(path),
        }
    }

    pub fn file_event_producer(&self) -> FileEventProducer<'a> {
        FileEventProducer::new(self.file_event_bus)
    }
}
