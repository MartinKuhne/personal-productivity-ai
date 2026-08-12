//! Tool context — provides tools with access to the global `AppConfig` and the file event bus, plus safe virtual-path resolution.

use crate::app::session::BrowserSession;
use crate::app::vfs;
use crate::bus::core::Bus;
use crate::bus::events::file::{FileEvent, FileEventKind, FileEventProducer};
use crate::config::AppConfig;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Read-only VFS path resolver wrapping `AppConfig`. Owns an
/// `Arc<AppConfig>` so the resolver is `'static` and can be embedded
/// in a long-lived `ToolContext` or cloned across parallel dispatch
/// without lifetime juggling.
#[derive(Clone)]
pub struct VfsResolver {
    pub config: Arc<AppConfig>,
}

impl VfsResolver {
    /// Create a new `VfsResolver`.
    pub fn new(config: Arc<AppConfig>) -> Self {
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
    pub config: Arc<AppConfig>,
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
    pub cache: std::sync::Arc<crate::agent::tools::registry::cache::ToolCache>,
    pub tool_manager:
        std::sync::Arc<arc_swap::ArcSwap<crate::agent::tools::registry::ToolRegistry>>,
    pub uuid_gen: std::sync::Arc<dyn crate::utils::uuid::UuidGenerator>,
}

impl ToolContext {
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

pub struct ToolContextBuilder {
    config: Arc<AppConfig>,
    file_event_bus: Bus<FileEvent>,
    tool_manager: std::sync::Arc<arc_swap::ArcSwap<crate::agent::tools::registry::ToolRegistry>>,
    cache: std::sync::Arc<crate::agent::tools::registry::cache::ToolCache>,
    uuid_gen: std::sync::Arc<dyn crate::utils::uuid::UuidGenerator>,
    browser_session: Option<Arc<BrowserSession>>,
    pdf_backing: Option<std::sync::Arc<crate::app::session::PdfBackingTracker>>,
}

impl ToolContextBuilder {
    pub fn new(
        config: Arc<AppConfig>,
        file_event_bus: Bus<FileEvent>,
        tool_manager: std::sync::Arc<
            arc_swap::ArcSwap<crate::agent::tools::registry::ToolRegistry>,
        >,
        cache: std::sync::Arc<crate::agent::tools::registry::cache::ToolCache>,
        uuid_gen: std::sync::Arc<dyn crate::utils::uuid::UuidGenerator>,
    ) -> Self {
        Self {
            config,
            file_event_bus,
            tool_manager,
            cache,
            uuid_gen,
            browser_session: None,
            pdf_backing: None,
        }
    }

    pub fn with_browser_session(mut self, browser_session: Arc<BrowserSession>) -> Self {
        self.browser_session = Some(browser_session);
        self
    }

    pub fn with_pdf_backing(
        mut self,
        pdf_backing: std::sync::Arc<crate::app::session::PdfBackingTracker>,
    ) -> Self {
        self.pdf_backing = Some(pdf_backing);
        self
    }

    pub fn build(self) -> ToolContext {
        let default_browser = Arc::new(BrowserSession::new(&self.config));
        let resolver = VfsResolver::new(self.config.clone());
        let publisher = EventPublisher::new(self.file_event_bus.clone());
        ToolContext {
            config: self.config,
            file_event_bus: self.file_event_bus,
            resolver,
            publisher,
            browser_session: self.browser_session.unwrap_or(default_browser),
            pdf_backing: self
                .pdf_backing
                .unwrap_or_else(|| Arc::new(crate::app::session::PdfBackingTracker::new())),
            cache: self.cache,
            tool_manager: self.tool_manager,
            uuid_gen: self.uuid_gen,
        }
    }
}
