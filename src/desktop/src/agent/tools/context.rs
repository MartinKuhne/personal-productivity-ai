//! Tool context — provides tools with access to the global `AppConfig` and the file event bus, plus safe virtual-path resolution.

use crate::config::AgentConfig;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Tool context — composite providing tools with access to `AgentConfig`
/// and the file event bus, plus safe virtual-path resolution via
/// [`crate::tools::vfs::VirtualFileSystem`] and event publishing.
///
/// `ToolContext` is `'static` and cheap to clone: every reference-
/// shaped field is now an owned `Arc` or a `Clone`-cheap `Bus`.
/// The `Clone` derive is what makes Phase 5's
/// `cargo-fuzz` targets (which need a `'static` context to
/// spawn) and the parallel-dispatch path in
/// [`ToolExecutor`](crate::tool_executor::ToolExecutor)
/// (which needs an owned handle per `spawn_blocking`) work
/// without `unsafe` casts.
///
/// Note: the context deliberately does **not** carry a back-reference
/// to the tool registry. The executor, which dispatches to tools,
/// owns the registry and the dispatcher; the per-call context only
/// exposes the services a tool actually needs. This breaks the
/// implicit context ↔ registry cycle the previous design had.
pub struct ToolCacheExt(pub std::sync::Arc<crate::tools::registry::cache::ToolCache>);
pub struct UuidGeneratorExt(pub std::sync::Arc<dyn crate::utils::uuid::UuidGenerator>);

#[derive(Clone)]
pub struct ToolContext {
    pub config: Arc<AgentConfig>,
    pub extensions: crate::tools::extensions::Extensions,
}

impl ToolContext {
    pub fn vfs(&self) -> std::sync::Arc<dyn crate::tools::vfs::VirtualFileSystem> {
        self.extensions
            .get::<crate::tools::vfs::VirtualFileSystemExt>()
            .expect("VFS not injected")
            .0
            .clone()
    }

    /// Resolve a virtual path to an absolute filesystem path.
    pub fn resolve_virtual_path(
        &self,
        vpath: &str,
        allow_write: bool,
    ) -> Result<Option<(PathBuf, bool)>, String> {
        self.vfs().resolve_virtual_path(vpath, allow_write)
    }

    /// Resolve a virtual path for a mutating tool.
    pub fn resolve_writable(&self, vpath: &str) -> Result<PathBuf, String> {
        self.vfs().resolve_writable(vpath)
    }

    pub fn cache(&self) -> std::sync::Arc<crate::tools::registry::cache::ToolCache> {
        self.extensions
            .get::<ToolCacheExt>()
            .expect("ToolCache not injected")
            .0
            .clone()
    }

    pub fn uuid_gen(&self) -> std::sync::Arc<dyn crate::utils::uuid::UuidGenerator> {
        self.extensions
            .get::<UuidGeneratorExt>()
            .expect("UuidGenerator not injected")
            .0
            .clone()
    }

    /// Publish a file event to the file event bus.
    pub fn file_observer(&self) -> std::sync::Arc<dyn crate::tools::observer::OnFileChanged> {
        if let Some(ext) = self
            .extensions
            .get::<crate::tools::observer::OnFileChangedExt>()
        {
            ext.0.clone()
        } else {
            std::sync::Arc::new(crate::tools::observer::DefaultFileObserver)
        }
    }

    pub fn publish_file_event(&self, path: &Path) {
        self.file_observer().on_file_changed(path);
    }

    /// Check whether the given path is allowed to be written to. Returns an error string if blocked.
    pub fn check_write_allowed(&self, path: &Path) -> Result<(), String> {
        if let Some(ext) = self
            .extensions
            .get::<crate::tools::policy::ToolCallPolicyExt>()
        {
            ext.0.check_write_allowed(path)
        } else {
            Ok(())
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
    file_observer: std::sync::Arc<dyn crate::tools::observer::OnFileChanged>,
    extensions: crate::tools::extensions::Extensions,
}

impl ToolContextBuilder {
    pub fn new(
        config: Arc<AgentConfig>,
        file_observer: std::sync::Arc<dyn crate::tools::observer::OnFileChanged>,
    ) -> Self {
        Self {
            config,
            file_observer,
            extensions: crate::tools::extensions::Extensions::default(),
        }
    }

    pub fn with_tool_call_policy(
        mut self,
        policy: std::sync::Arc<dyn crate::tools::policy::ToolCallPolicy>,
    ) -> Self {
        self.extensions
            .insert(Arc::new(crate::tools::policy::ToolCallPolicyExt(policy)));
        self
    }

    pub fn with_extension<T: Send + Sync + 'static>(mut self, extension: Arc<T>) -> Self {
        self.extensions.insert(extension);
        self
    }

    pub fn build(self) -> ToolContext {
        let mut extensions = self.extensions;
        extensions.insert(Arc::new(crate::tools::observer::OnFileChangedExt(
            self.file_observer.clone(),
        )));

        if extensions
            .get::<crate::tools::vfs::VirtualFileSystemExt>()
            .is_none()
        {
            extensions.insert(std::sync::Arc::new(
                crate::tools::vfs::VirtualFileSystemExt(std::sync::Arc::new(
                    crate::tools::vfs::VfsResolver::new(self.config.clone()),
                )),
            ));
        }
        if extensions
            .get::<crate::tools::policy::ToolCallPolicyExt>()
            .is_none()
        {
            extensions.insert(Arc::new(crate::tools::policy::ToolCallPolicyExt(Arc::new(
                crate::tools::policy::DefaultToolCallPolicy,
            ))));
        }

        ToolContext {
            config: self.config,
            extensions,
        }
    }
}
