//! Tool context — provides tools with access to the global `AppConfig` and the file event bus, plus safe virtual-path resolution.

use crate::app::session::BrowserSession;
use crate::app::vfs;
use crate::bus::core::Bus;
use crate::bus::events::file::{FileEvent, FileEventKind, FileEventProducer};
use crate::agent::config::AgentConfig;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Read-only VFS path resolver wrapping `AgentConfig`. Owns an
/// `Arc<AgentConfig>` so the resolver is `'static` and can be embedded
/// in a long-lived `ToolContext` or cloned across parallel dispatch
/// without lifetime juggling.
#[derive(Clone)]
pub struct VfsResolver {
    pub config: Arc<AgentConfig>,
}

impl VfsResolver {
    /// Create a new `VfsResolver`.
    pub fn new(config: Arc<AgentConfig>) -> Self {
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
        vfs::behaviour::resolve(vpath, allow_write, self.config.content_libraries())
    }

    /// Resolve a virtual path for a mutating tool. Thin shim over
    /// [`vfs::behaviour::resolve_writable`]. Returns the absolute
    /// filesystem path on success.
    pub fn resolve_writable(&self, vpath: &str) -> Result<PathBuf, String> {
        vfs::behaviour::resolve_writable(vpath, self.config.content_libraries())
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

/// Tool context — composite providing tools with access to `AgentConfig`
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
///
/// Note: the context deliberately does **not** carry a back-reference
/// to the tool registry. The executor, which dispatches to tools,
/// owns the registry and the dispatcher; the per-call context only
/// exposes the services a tool actually needs. This breaks the
/// implicit context ↔ registry cycle the previous design had.
#[derive(Clone)]
pub struct ToolContext {
    pub config: Arc<AgentConfig>,
    pub file_event_bus: Bus<FileEvent>,
    pub resolver: VfsResolver,
    pub publisher: EventPublisher,
    pub cache: std::sync::Arc<crate::agent::tools::registry::cache::ToolCache>,
    pub uuid_gen: std::sync::Arc<dyn crate::utils::uuid::UuidGenerator>,
    pub extensions: crate::agent::tools::extensions::Extensions,
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
        if let Some(pdf_backing) = self
            .extensions
            .get::<crate::app::session::PdfBackingTracker>()
        {
            pdf_backing.is_pdf_backed(path)
        } else {
            false
        }
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
    config: Arc<AgentConfig>,
    file_event_bus: Bus<FileEvent>,
    cache: std::sync::Arc<crate::agent::tools::registry::cache::ToolCache>,
    uuid_gen: std::sync::Arc<dyn crate::utils::uuid::UuidGenerator>,
    extensions: crate::agent::tools::extensions::Extensions,
}

impl ToolContextBuilder {
    pub fn new(
        config: Arc<AgentConfig>,
        file_event_bus: Bus<FileEvent>,
        cache: std::sync::Arc<crate::agent::tools::registry::cache::ToolCache>,
        uuid_gen: std::sync::Arc<dyn crate::utils::uuid::UuidGenerator>,
    ) -> Self {
        Self {
            config,
            file_event_bus,
            cache,
            uuid_gen,
            extensions: crate::agent::tools::extensions::Extensions::default(),
        }
    }

    pub fn with_browser_session(mut self, browser_session: Arc<BrowserSession>) -> Self {
        self.extensions.insert(browser_session);
        self
    }

    pub fn with_pdf_backing(
        mut self,
        pdf_backing: std::sync::Arc<crate::app::session::PdfBackingTracker>,
    ) -> Self {
        self.extensions.insert(pdf_backing);
        self
    }

    pub fn with_extension<T: Send + Sync + 'static>(mut self, extension: Arc<T>) -> Self {
        self.extensions.insert(extension);
        self
    }

    pub fn build(self) -> ToolContext {
        let resolver = VfsResolver::new(self.config.clone());
        let publisher = EventPublisher::new(self.file_event_bus.clone());

        let mut extensions = self.extensions;
        if extensions.get::<BrowserSession>().is_none() {
            extensions.insert(Arc::new(BrowserSession::with_resolved(self.config.browser().clone())));
        }
        if extensions
            .get::<crate::app::session::PdfBackingTracker>()
            .is_none()
        {
            extensions.insert(Arc::new(crate::app::session::PdfBackingTracker::new()));
        }

        ToolContext {
            config: self.config,
            file_event_bus: self.file_event_bus,
            resolver,
            publisher,
            cache: self.cache,
            uuid_gen: self.uuid_gen,
            extensions,
        }
    }
}
