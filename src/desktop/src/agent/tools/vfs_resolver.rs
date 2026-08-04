//! VFS resolver — path resolution without side effects.
//!
//! Extracted from [`ToolContext`] so read-only tools do not need a
//! bus handle. Implementations can be replaced at test time.

use crate::app::vfs;
use crate::config::AppConfig;

/// Thin resolver over [`vfs::behaviour::resolve`].
#[derive(Debug, Clone, Copy)]
pub struct VfsResolver<'a> {
    pub config: &'a AppConfig,
}

impl<'a> VfsResolver<'a> {
    pub fn new(config: &'a AppConfig) -> Self {
        Self { config }
    }

    /// Resolve a virtual path to an absolute filesystem path.
    pub fn resolve_virtual_path(
        &self,
        vpath: &str,
        allow_write: bool,
    ) -> Result<Option<(std::path::PathBuf, bool)>, String> {
        vfs::behaviour::resolve(vpath, allow_write, &self.config.content_libraries)
    }

    /// Resolve a virtual path for a mutating tool.
    pub fn resolve_writable(&self, vpath: &str) -> Result<std::path::PathBuf, String> {
        vfs::behaviour::resolve_writable(vpath, &self.config.content_libraries)
    }
}
