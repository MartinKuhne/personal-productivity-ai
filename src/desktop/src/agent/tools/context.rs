//! Tool context — provides tools with access to `AppConfig` and the file event bus, plus safe virtual-path resolution.

use crate::app::session::BrowserSession;
use crate::app::vfs;
use crate::bus::core::Bus;
use crate::bus::events::file::{FileEvent, FileEventKind, FileEventProducer};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Read-only VFS path resolver wrapping `AppConfig`. Owns an
/// `Arc<AppConfig>` so the resolver is `'static` and can be
/// embedded in a long-lived `ToolContext` or cloned across
/// parallel dispatch without lifetime juggling.
#[derive(Clone)]
pub struct VfsResolver {
    pub config: Arc<crate::config::AppConfig>,
}

impl VfsResolver {
    /// Create a new `VfsResolver`.
    pub fn new(config: Arc<crate::config::AppConfig>) -> Self {
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
/// Owns a `Bus<FileEvent>` clone (cheap, the underlying
/// `tokio::sync::broadcast::Sender` is `Arc`-backed) so the
/// publisher is `'static` and embeddable in a `ToolContext`.
#[derive(Clone)]
pub struct EventPublisher {
    pub file_event_bus: Bus<FileEvent>,
}

impl EventPublisher {
    /// Create a new `EventPublisher`.
    pub fn new(file_event_bus: Bus<FileEvent>) -> Self {
        Self { file_event_bus }
    }

    /// Publish a file event to the bus.
    pub fn publish_file_event(&self, kind: FileEventKind, path: &Path) {
        let producer = FileEventProducer::new(self.file_event_bus.clone());
        match kind {
            FileEventKind::Discovered => producer.publish_discovered(path),
            FileEventKind::Updated => producer.publish_updated(path),
            FileEventKind::Removed => producer.publish_removed(path),
            FileEventKind::DirDiscovered => producer.publish_dir_discovered(path),
            FileEventKind::DirRemoved => producer.publish_dir_removed(path),
        }
    }

    /// Obtain a [`FileEventProducer`] handle.
    pub fn file_event_producer(&self) -> FileEventProducer {
        FileEventProducer::new(self.file_event_bus.clone())
    }
}

/// Tool context — composite providing tools with access to `AppConfig`
/// and the file event bus, plus safe virtual-path resolution via
/// [`VfsResolver`] and event publishing via [`EventPublisher`].
///
/// `ToolContext` is `'static` and cheap to clone: every reference-
/// shaped field is now an owned `Arc` or a `Clone`-cheap `Bus`.
/// The `Clone` derive is what makes Phase 5's
/// `cargo-fuzz` targets (which need a `'static` context to
/// spawn) and the parallel-dispatch path in
/// [`ToolExecutor`](crate::agent::tool_executor::ToolExecutor)
/// (which needs an owned handle per `spawn_blocking`) work
/// without `unsafe` casts.
#[derive(Clone)]
pub struct ToolContext {
    pub config: Arc<crate::config::AppConfig>,
    pub file_event_bus: Bus<FileEvent>,
    pub resolver: VfsResolver,
    pub publisher: EventPublisher,
    /// Long-lived headless Firefox session, shared across every
    /// mutating browser tool call. `None` only in early-startup
    /// tests that don't care about the browser. Tools that
    /// don't use the browser ignore this field. When the
    /// `browser` Cargo feature is off the session is a stub;
    /// the `browser_*` tools are not registered and the field
    /// stays unused.
    pub browser_session: Arc<BrowserSession>,
    pub pdf_backing: std::sync::Arc<crate::app::session::PdfBackingTracker>,
    pub cache: std::sync::Arc<crate::agent::tools::manager::cache::ToolCache>,
    pub tool_manager: std::sync::Arc<std::sync::RwLock<crate::agent::tools::manager::ToolManager>>,
    pub uuid_gen: std::sync::Arc<dyn crate::utils::uuid::UuidGenerator>,
}

impl ToolContext {
    /// Create a new `ToolContext`.
    pub fn new(
        config: Arc<crate::config::AppConfig>,
        file_event_bus: Bus<FileEvent>,
        browser_session: Arc<BrowserSession>,
        pdf_backing: std::sync::Arc<crate::app::session::PdfBackingTracker>,
        cache: std::sync::Arc<crate::agent::tools::manager::cache::ToolCache>,
        tool_manager: std::sync::Arc<std::sync::RwLock<crate::agent::tools::manager::ToolManager>>,
        uuid_gen: std::sync::Arc<dyn crate::utils::uuid::UuidGenerator>,
    ) -> Self {
        let resolver = VfsResolver::new(config.clone());
        let publisher = EventPublisher::new(file_event_bus.clone());
        Self {
            config,
            file_event_bus,
            resolver,
            publisher,
            browser_session,
            pdf_backing,
            cache,
            tool_manager,
            uuid_gen,
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
    pub fn file_event_producer(&self) -> FileEventProducer {
        self.publisher.file_event_producer()
    }

    /// Check whether the given path is a Markdown file with a PDF sibling.
    pub fn is_pdf_backed(&self, path: &Path) -> bool {
        self.pdf_backing.is_pdf_backed(path)
    }
}

/// Compile-time assertion: `ToolContext` is `'static + Send + Sync`.
///
/// This is the contract the rewrite buys. Phase 5 of the
/// 'static-clean rewrite plan calls for cargo-fuzz targets that
/// need a `'static` context to `std::thread::spawn`, and the
/// parallel-dispatch path in `ToolExecutor` needs `Send + Sync`
/// to hand the context to a `tokio::task::JoinSet::spawn_blocking`
/// worker. A regression that breaks either property (e.g.
/// reintroducing a lifetime parameter, or accidentally holding a
/// `Rc` instead of an `Arc`) is caught at compile time.
#[allow(dead_code)]
const _: fn() = || {
    fn assert_send_sync_static<T: Send + Sync + 'static>() {}
    assert_send_sync_static::<ToolContext>();
};
