//! Virtual File System abstraction and resolver for agent tools.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

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

    /// Check if a file or directory exists in the virtual filesystem.
    fn exists(&self, path: &Path) -> bool {
        self.metadata(path).is_ok()
    }

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

/// In-memory implementation of [`VirtualFileSystem`] for testing without touching the physical disk.
#[derive(Clone, Debug, Default)]
pub struct MockVirtualFileSystem {
    /// In-memory file storage: path -> bytes.
    pub files: Arc<Mutex<std::collections::HashMap<PathBuf, Vec<u8>>>>,
    /// In-memory directory storage.
    pub dirs: Arc<Mutex<std::collections::HashSet<PathBuf>>>,
    /// Configured content libraries for virtual path resolution.
    pub libraries: Arc<Mutex<Vec<crate::config::ContentLibrary>>>,
    /// Optional error override for `rename`.
    pub rename_err: Arc<Mutex<Option<&'static str>>>,
    /// Optional error override for `copy`.
    pub copy_err: Arc<Mutex<Option<&'static str>>>,
    /// Optional error override for `remove_file`.
    pub remove_file_err: Arc<Mutex<Option<&'static str>>>,
    /// Optional error override for `create_dir_all`.
    pub create_dir_all_err: Arc<Mutex<Option<&'static str>>>,
}

impl MockVirtualFileSystem {
    /// Creates a new empty in-memory mock filesystem.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new in-memory mock filesystem with the specified content libraries.
    pub fn with_libraries(libraries: Vec<crate::config::ContentLibrary>) -> Self {
        Self {
            libraries: Arc::new(Mutex::new(libraries)),
            ..Self::default()
        }
    }

    /// Adds a virtual file with content into in-memory storage.
    pub fn add_file(&self, path: impl AsRef<Path>, content: impl AsRef<[u8]>) {
        let p = path.as_ref().to_path_buf();
        if let Some(parent) = p.parent() {
            self.add_dir(parent);
        }
        self.files
            .lock()
            .unwrap()
            .insert(p, content.as_ref().to_vec());
    }

    /// Adds a directory into in-memory storage.
    pub fn add_dir(&self, path: impl AsRef<Path>) {
        let mut cur = Some(path.as_ref());
        let mut dirs = self.dirs.lock().unwrap();
        while let Some(p) = cur {
            dirs.insert(p.to_path_buf());
            cur = p.parent();
        }
    }

    /// Checks if a file exists in in-memory storage.
    pub fn file_exists(&self, path: impl AsRef<Path>) -> bool {
        self.files.lock().unwrap().contains_key(path.as_ref())
    }

    /// Checks if a directory exists in in-memory storage.
    pub fn dir_exists(&self, path: impl AsRef<Path>) -> bool {
        self.dirs.lock().unwrap().contains(path.as_ref())
    }
}

impl VirtualFileSystem for MockVirtualFileSystem {
    fn resolve_virtual_path(
        &self,
        vpath: &str,
        allow_write: bool,
    ) -> Result<Option<crate::vfs::ResolvedVirtualPath>, String> {
        let libs = self.libraries.lock().unwrap();
        crate::vfs::behaviour::resolve(vpath, allow_write, &libs)
    }

    fn resolve_writable(&self, vpath: &str) -> Result<PathBuf, String> {
        let libs = self.libraries.lock().unwrap();
        crate::vfs::behaviour::resolve_writable(vpath, &libs)
    }

    fn read_to_string(&self, path: &Path) -> std::io::Result<String> {
        let files = self.files.lock().unwrap();
        let bytes = files.get(path).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("File not found in mock VFS: {}", path.display()),
            )
        })?;
        String::from_utf8(bytes.clone())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))
    }

    fn write(&self, path: &Path, content: &[u8]) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            self.create_dir_all(parent)?;
        }
        self.files
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), content.to_vec());
        Ok(())
    }

    fn append(&self, path: &Path, content: &[u8]) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            self.create_dir_all(parent)?;
        }
        let mut files = self.files.lock().unwrap();
        let entry = files.entry(path.to_path_buf()).or_default();
        entry.extend_from_slice(content);
        Ok(())
    }

    fn create_dir_all(&self, path: &Path) -> std::io::Result<()> {
        if let Some(msg) = *self.create_dir_all_err.lock().unwrap() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                msg,
            ));
        }
        let mut cur = Some(path);
        let mut dirs = self.dirs.lock().unwrap();
        while let Some(p) = cur {
            dirs.insert(p.to_path_buf());
            cur = p.parent();
        }
        Ok(())
    }

    fn read_dir(&self, path: &Path) -> std::io::Result<Vec<VfsDirEntry>> {
        let files = self.files.lock().unwrap();
        let dirs = self.dirs.lock().unwrap();
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for fpath in files.keys() {
            if let Some(parent) = fpath.parent()
                && parent == path
                && seen.insert(fpath.clone())
            {
                out.push(VfsDirEntry {
                    path: fpath.clone(),
                    is_file: true,
                    is_dir: false,
                });
            }
        }
        for dpath in dirs.iter() {
            if let Some(parent) = dpath.parent()
                && parent == path
                && seen.insert(dpath.clone())
            {
                out.push(VfsDirEntry {
                    path: dpath.clone(),
                    is_file: false,
                    is_dir: true,
                });
            }
        }
        Ok(out)
    }

    fn metadata(&self, path: &Path) -> std::io::Result<VfsMetadata> {
        let files = self.files.lock().unwrap();
        if let Some(bytes) = files.get(path) {
            return Ok(VfsMetadata {
                is_file: true,
                is_dir: false,
                len: bytes.len() as u64,
            });
        }
        let dirs = self.dirs.lock().unwrap();
        if dirs.contains(path) {
            return Ok(VfsMetadata {
                is_file: false,
                is_dir: true,
                len: 0,
            });
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Path not found in mock VFS: {}", path.display()),
        ))
    }

    fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()> {
        if let Some(msg) = *self.rename_err.lock().unwrap() {
            return Err(std::io::Error::new(std::io::ErrorKind::CrossesDevices, msg));
        }
        if let Some(parent) = to.parent() {
            self.create_dir_all(parent)?;
        }
        let mut files = self.files.lock().unwrap();
        if let Some(bytes) = files.remove(from) {
            files.insert(to.to_path_buf(), bytes);
            return Ok(());
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("File not found for rename in mock VFS: {}", from.display()),
        ))
    }

    fn remove_file(&self, path: &Path) -> std::io::Result<()> {
        if let Some(msg) = *self.remove_file_err.lock().unwrap() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                msg,
            ));
        }
        let mut files = self.files.lock().unwrap();
        if files.remove(path).is_some() {
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "File not found for deletion in mock VFS: {}",
                    path.display()
                ),
            ))
        }
    }

    fn copy(&self, from: &Path, to: &Path) -> std::io::Result<u64> {
        if let Some(msg) = *self.copy_err.lock().unwrap() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                msg,
            ));
        }
        let mut files = self.files.lock().unwrap();
        let bytes = files.get(from).cloned().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "Source file not found for copy in mock VFS: {}",
                    from.display()
                ),
            )
        })?;
        let len = bytes.len() as u64;
        files.insert(to.to_path_buf(), bytes);
        drop(files);
        Ok(len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_virtual_file_system_file_ops() {
        let vfs = MockVirtualFileSystem::new();
        let file_path = PathBuf::from("/notes/todo.md");

        assert!(!vfs.file_exists(&file_path));
        vfs.write(&file_path, b"Initial content").unwrap();
        assert!(vfs.file_exists(&file_path));
        assert!(vfs.dir_exists("/notes"));

        let content = vfs.read_to_string(&file_path).unwrap();
        assert_eq!(content, "Initial content");

        vfs.append(&file_path, b"\nAppended line").unwrap();
        let updated = vfs.read_to_string(&file_path).unwrap();
        assert_eq!(updated, "Initial content\nAppended line");

        let meta = vfs.metadata(&file_path).unwrap();
        assert!(meta.is_file);
        assert!(!meta.is_dir);
        assert_eq!(meta.len, updated.len() as u64);

        vfs.remove_file(&file_path).unwrap();
        assert!(!vfs.file_exists(&file_path));
    }

    #[test]
    fn test_mock_virtual_file_system_dir_ops() {
        let vfs = MockVirtualFileSystem::new();
        let dir_path = PathBuf::from("/system/Skills/Note");
        vfs.create_dir_all(&dir_path).unwrap();

        assert!(vfs.dir_exists("/system"));
        assert!(vfs.dir_exists("/system/Skills"));
        assert!(vfs.dir_exists(&dir_path));

        let file_path = dir_path.join("Skill.md");
        vfs.write(&file_path, b"# Skill").unwrap();

        let entries = vfs.read_dir(&dir_path).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, file_path);
        assert!(entries[0].is_file);
    }

    #[test]
    fn test_mock_virtual_file_system_rename_and_copy() {
        let vfs = MockVirtualFileSystem::new();
        let from = PathBuf::from("/a/doc.md");
        let to_copy = PathBuf::from("/b/doc_copy.md");
        let to_rename = PathBuf::from("/c/doc_renamed.md");

        vfs.write(&from, b"Hello VFS").unwrap();
        vfs.copy(&from, &to_copy).unwrap();
        assert!(vfs.file_exists(&from));
        assert!(vfs.file_exists(&to_copy));

        vfs.rename(&to_copy, &to_rename).unwrap();
        assert!(!vfs.file_exists(&to_copy));
        assert!(vfs.file_exists(&to_rename));
    }

    #[test]
    fn test_mock_rename_does_not_mutate_when_destination_parent_creation_fails() {
        let vfs = MockVirtualFileSystem::new();
        let from = PathBuf::from("/source/doc.md");
        let to = PathBuf::from("/destination/doc.md");
        vfs.write(&from, b"content").unwrap();
        *vfs.create_dir_all_err.lock().unwrap() = Some("permission denied");

        assert!(vfs.rename(&from, &to).is_err());
        assert!(vfs.file_exists(&from));
        assert!(!vfs.file_exists(&to));
    }
}
