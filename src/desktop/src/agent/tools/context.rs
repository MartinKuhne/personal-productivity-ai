//! Tool context — provides tools with access to `AppConfig` and the file event bus, plus safe virtual-path resolution.

use crate::app::session::BrowserSession;
use crate::app::vfs;
use crate::bus::core::Bus;
use crate::bus::events::file::{FileEvent, FileEventKind, FileEventProducer};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Read-only VFS path resolver wrapping `AppConfig`.
#[derive(Clone, Copy)]
pub struct VfsResolver<'a> {
    pub config: &'a crate::config::AppConfig,
}

impl<'a> VfsResolver<'a> {
    /// Create a new `VfsResolver`.
    pub fn new(config: &'a crate::config::AppConfig) -> Self {
        Self { config }
    }

    /// Resolve a virtual path to an absolute filesystem path. Thin shim
    /// over [`vfs::behaviour::resolve`] that pulls the library list from
    /// the active config. Spec: [`app/vfs/SPEC.md`](../../app/vfs/SPEC.md) (VFS-004, VFS-009).
    pub fn resolve_virtual_path(
        &self,
        vpath: &str,
        allow_write: bool,
    ) -> Result<Option<(PathBuf, bool)>, String> {
        vfs::behaviour::resolve(vpath, allow_write, &self.config.content_libraries)
    }

    /// Resolve a virtual path for a mutating tool. Thin shim over
    /// [`vfs::behaviour::resolve_writable`]. Returns the absolute
    /// filesystem path on success.
    pub fn resolve_writable(&self, vpath: &str) -> Result<PathBuf, String> {
        vfs::behaviour::resolve_writable(vpath, &self.config.content_libraries)
    }
}

/// Event publisher wrapping the file event bus for side-effecting tools.
#[derive(Clone, Copy)]
pub struct EventPublisher<'a> {
    pub file_event_bus: &'a Bus<FileEvent>,
}

impl<'a> EventPublisher<'a> {
    /// Create a new `EventPublisher`.
    pub fn new(file_event_bus: &'a Bus<FileEvent>) -> Self {
        Self { file_event_bus }
    }

    /// Publish a file event to the bus.
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

    /// Obtain a [`FileEventProducer`] handle.
    pub fn file_event_producer(&self) -> FileEventProducer<'a> {
        FileEventProducer::new(self.file_event_bus)
    }
}

/// Tool context — composite providing tools with access to `AppConfig` and the file event bus,
/// plus safe virtual-path resolution via [`VfsResolver`] and event publishing via [`EventPublisher`].
pub struct ToolContext<'a> {
    pub config: &'a crate::config::AppConfig,
    pub file_event_bus: &'a Bus<FileEvent>,
    pub resolver: VfsResolver<'a>,
    pub publisher: EventPublisher<'a>,
    /// Long-lived headless Firefox session, shared across every
    /// mutating browser tool call. `None` only in early-startup
    /// tests that don't care about the browser. Tools that
    /// don't use the browser ignore this field. When the
    /// `browser` Cargo feature is off the session is a stub;
    /// the `browser_*` tools are not registered and the field
    /// stays unused.
    pub browser_session: Arc<BrowserSession>,
    pub pdf_backing: std::sync::Arc<crate::app::session::PdfBackingTracker>,
    pub cache: &'a crate::agent::tools::manager::cache::ToolCache,
    pub tool_manager: std::sync::Arc<std::sync::RwLock<crate::agent::tools::manager::ToolManager>>,
}

impl<'a> ToolContext<'a> {
    /// Create a new `ToolContext`.
    pub fn new(
        config: &'a crate::config::AppConfig,
        file_event_bus: &'a Bus<FileEvent>,
        browser_session: Arc<BrowserSession>,
        pdf_backing: std::sync::Arc<crate::app::session::PdfBackingTracker>,
        cache: &'a crate::agent::tools::manager::cache::ToolCache,
        tool_manager: std::sync::Arc<std::sync::RwLock<crate::agent::tools::manager::ToolManager>>,
    ) -> Self {
        Self {
            config,
            file_event_bus,
            resolver: VfsResolver::new(config),
            publisher: EventPublisher::new(file_event_bus),
            browser_session,
            pdf_backing,
            cache,
            tool_manager,
        }
    }

    /// Resolve a virtual path to an absolute filesystem path.
    pub fn resolve_virtual_path(
        &self,
        vpath: &str,
        allow_write: bool,
    ) -> Result<Option<(PathBuf, bool)>, String> {
        self.resolver.resolve_virtual_path(vpath, allow_write)
    }

    /// Resolve a virtual path for a mutating tool.
    pub fn resolve_writable(&self, vpath: &str) -> Result<PathBuf, String> {
        self.resolver.resolve_writable(vpath)
    }

    /// Publish a file event to the file event bus.
    pub fn publish_file_event(&self, kind: FileEventKind, path: &Path) {
        self.publisher.publish_file_event(kind, path);
    }

    /// Obtain a [`FileEventProducer`] handle.
    pub fn file_event_producer(&self) -> FileEventProducer<'a> {
        self.publisher.file_event_producer()
    }

    /// Check whether the given path is a Markdown file with a PDF sibling.
    pub fn is_pdf_backed(&self, path: &Path) -> bool {
        self.pdf_backing.is_pdf_backed(path)
    }
}
