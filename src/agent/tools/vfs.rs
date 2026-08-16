//! Virtual File System abstraction and resolver for agent tools.

use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Metadata for a virtual filesystem entry.
#[derive(Debug)]
pub struct VfsMetadata {
    pub is_file: bool,
    pub is_dir: bool,
    pub len: u64,
}

/// Directory entry for virtual filesystem traversal.
#[derive(Debug)]
pub struct VfsDirEntry {
    pub path: PathBuf,
    pub is_file: bool,
    pub is_dir: bool,
}

/// Trait abstracting filesystem operations for agent tools.
pub trait VirtualFileSystem: Send + Sync {
    /// Resolve a virtual path against configured content libraries.
    fn resolve_virtual_path(
        &self,
        vpath: &str,
        allow_write: bool,
    ) -> Result<Option<crate::vfs::ResolvedVirtualPath>, String>;

    /// Resolve a virtual path for a mutating tool.
    fn resolve_writable(&self, vpath: &str) -> Result<PathBuf, String>;

    /// Read the entire content of a file at `path` as a UTF-8 string.
    fn read_to_string(&self, path: &Path) -> std::io::Result<String>;

    /// Write `content` bytes to the file at `path`.
    fn write(&self, path: &Path, content: &[u8]) -> std::io::Result<()>;

    /// Append `content` bytes to the file at `path`.
    fn append(&self, path: &Path, content: &[u8]) -> std::io::Result<()>;

    /// Create all directories along the given `path` if they do not already exist.
    fn create_dir_all(&self, path: &Path) -> std::io::Result<()>;

    /// Read directory entries at the given `path`.
    fn read_dir(&self, path: &Path) -> std::io::Result<Vec<VfsDirEntry>>;

    /// Query filesystem metadata for the item at `path`.
    fn metadata(&self, path: &Path) -> std::io::Result<VfsMetadata>;

    /// Rename or move a file from `from` to `to`.
    fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()>;

    /// Remove a file at `path`.
    fn remove_file(&self, path: &Path) -> std::io::Result<()>;

    /// Copy a file from `from` to `to`.
    fn copy(&self, from: &Path, to: &Path) -> std::io::Result<u64>;
}

/// Extension wrapper for storing [`VirtualFileSystem`] in a tool context.
#[derive(Clone)]
pub struct VirtualFileSystemExt(pub Arc<dyn VirtualFileSystem>);

impl std::fmt::Debug for VirtualFileSystemExt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("VirtualFileSystemExt")
            .field(&"<dyn VirtualFileSystem>")
            .finish()
    }
}

/// Default implementation of [`VirtualFileSystem`] backed by standard filesystem I/O and app config.
#[derive(Clone, Debug)]
pub struct VfsResolver {
    pub config: Arc<crate::config::AgentConfig>,
}

impl VfsResolver {
    /// Create a new `VfsResolver` backed by `config`.
    pub fn new(config: Arc<crate::config::AgentConfig>) -> Self {
        Self { config }
    }
}

impl VirtualFileSystem for VfsResolver {
    fn resolve_virtual_path(
        &self,
        vpath: &str,
        allow_write: bool,
    ) -> Result<Option<crate::vfs::ResolvedVirtualPath>, String> {
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

    fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()> {
        std::fs::rename(from, to)
    }

    fn remove_file(&self, path: &Path) -> std::io::Result<()> {
        std::fs::remove_file(path)
    }

    fn copy(&self, from: &Path, to: &Path) -> std::io::Result<u64> {
        std::fs::copy(from, to)
    }
}
