use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct VfsMetadata {
    pub is_file: bool,
    pub is_dir: bool,
    pub len: u64,
}

pub struct VfsDirEntry {
    pub path: PathBuf,
    pub is_file: bool,
    pub is_dir: bool,
}

pub trait VirtualFileSystem: Send + Sync {
    fn resolve_virtual_path(
        &self,
        vpath: &str,
        allow_write: bool,
    ) -> Result<Option<(PathBuf, bool)>, String>;
    fn resolve_writable(&self, vpath: &str) -> Result<PathBuf, String>;

    fn read_to_string(&self, path: &Path) -> std::io::Result<String>;
    fn write(&self, path: &Path, content: &[u8]) -> std::io::Result<()>;
    fn append(&self, path: &Path, content: &[u8]) -> std::io::Result<()>;
    fn create_dir_all(&self, path: &Path) -> std::io::Result<()>;
    fn read_dir(&self, path: &Path) -> std::io::Result<Vec<VfsDirEntry>>;
    fn metadata(&self, path: &Path) -> std::io::Result<VfsMetadata>;
}

#[derive(Clone)]
pub struct VirtualFileSystemExt(pub Arc<dyn VirtualFileSystem>);

#[derive(Clone)]
pub struct VfsResolver {
    pub config: Arc<crate::config::AgentConfig>,
}

impl VfsResolver {
    pub fn new(config: Arc<crate::config::AgentConfig>) -> Self {
        Self { config }
    }
}

impl VirtualFileSystem for VfsResolver {
    fn resolve_virtual_path(
        &self,
        vpath: &str,
        allow_write: bool,
    ) -> Result<Option<(PathBuf, bool)>, String> {
        crate::vfs::behaviour::resolve(vpath, allow_write, self.config.content_libraries())
    }

    fn resolve_writable(&self, vpath: &str) -> Result<PathBuf, String> {
        crate::vfs::behaviour::resolve_writable(vpath, self.config.content_libraries())
    }

    fn read_to_string(&self, path: &Path) -> std::io::Result<String> {
        std::fs::read_to_string(path)
    }

    fn write(&self, path: &Path, content: &[u8]) -> std::io::Result<()> {
        std::fs::write(path, content)
    }

    fn append(&self, path: &Path, content: &[u8]) -> std::io::Result<()> {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)?;
        file.write_all(content)
    }

    fn create_dir_all(&self, path: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(path)
    }

    fn read_dir(&self, path: &Path) -> std::io::Result<Vec<VfsDirEntry>> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let meta = entry.metadata()?;
            entries.push(VfsDirEntry {
                path: entry.path(),
                is_file: meta.is_file(),
                is_dir: meta.is_dir(),
            });
        }
        Ok(entries)
    }

    fn metadata(&self, path: &Path) -> std::io::Result<VfsMetadata> {
        let meta = std::fs::metadata(path)?;
        Ok(VfsMetadata {
            is_file: meta.is_file(),
            is_dir: meta.is_dir(),
            len: meta.len(),
        })
    }
}
