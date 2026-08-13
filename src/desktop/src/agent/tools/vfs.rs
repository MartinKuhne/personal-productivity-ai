use std::path::PathBuf;
use std::sync::Arc;

pub trait VirtualFileSystem: Send + Sync {
    fn resolve_virtual_path(&self, vpath: &str, allow_write: bool) -> Result<Option<(PathBuf, bool)>, String>;
    fn resolve_writable(&self, vpath: &str) -> Result<PathBuf, String>;
}

#[derive(Clone)]
pub struct VirtualFileSystemExt(pub Arc<dyn VirtualFileSystem>);

#[derive(Clone)]
pub struct VfsResolver {
    pub config: Arc<crate::agent::config::AgentConfig>,
}

impl VfsResolver {
    pub fn new(config: Arc<crate::agent::config::AgentConfig>) -> Self {
        Self { config }
    }
}

impl VirtualFileSystem for VfsResolver {
    fn resolve_virtual_path(
        &self,
        vpath: &str,
        allow_write: bool,
    ) -> Result<Option<(PathBuf, bool)>, String> {
        crate::app::vfs::behaviour::resolve(vpath, allow_write, self.config.content_libraries())
    }

    fn resolve_writable(&self, vpath: &str) -> Result<PathBuf, String> {
        crate::app::vfs::behaviour::resolve_writable(vpath, self.config.content_libraries())
    }
}
