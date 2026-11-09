//! Virtual path syntax — library-prefixed paths (e.g. `library/relative/path`) mapped to real filesystem locations with traversal protection.
//! Unit tests live in the sibling `virtual_path_tests.rs` sidecar.

use std::path::{Path, PathBuf};

use crate::config::{ContentLibrary, ContentLibraryExt};

#[derive(Debug, Clone, PartialEq)]
pub enum VirtualPathError {
    EmptyPath,
    TraversalDetected,
    InvalidFormat(String),
    LibraryNotFound(String),
    LibraryNotWritable(String),
}

impl std::fmt::Display for VirtualPathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VirtualPathError::EmptyPath => write!(f, "virtual path is empty"),
            VirtualPathError::TraversalDetected => {
                write!(
                    f,
                    "path traversal detected: '..' is not allowed in virtual paths"
                )
            }
            VirtualPathError::InvalidFormat(msg) => {
                write!(f, "invalid virtual path format: {}", msg)
            }
            VirtualPathError::LibraryNotFound(name) => {
                write!(f, "content library '{}' not found", name)
            }
            VirtualPathError::LibraryNotWritable(name) => {
                write!(f, "content library '{}' is read-only", name)
            }
        }
    }
}

impl std::error::Error for VirtualPathError {}

pub struct VirtualPath {
    pub library: String,
    pub sub_path: PathBuf,
}

impl std::fmt::Debug for VirtualPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VirtualPath")
            .field("library", &self.library)
            .field("sub_path", &self.sub_path)
            .finish()
    }
}

impl PartialEq for VirtualPath {
    fn eq(&self, other: &Self) -> bool {
        self.library == other.library && self.sub_path == other.sub_path
    }
}

impl VirtualPath {
    pub fn parse(vpath: &str) -> Result<Self, VirtualPathError> {
        if vpath.is_empty() {
            return Err(VirtualPathError::EmptyPath);
        }

        let vpath_normalized = vpath.replace('\\', "/");

        let path = Path::new(&vpath_normalized);
        if path
            .components()
            .any(|c| c == std::path::Component::ParentDir)
        {
            return Err(VirtualPathError::TraversalDetected);
        }

        let slash_pos = vpath_normalized.find('/');

        match slash_pos {
            Some(pos) => {
                let lib = &vpath_normalized[..pos];
                let sub = &vpath_normalized[pos + 1..];

                if lib.is_empty() {
                    return Err(VirtualPathError::InvalidFormat(
                        "library name is empty".to_string(),
                    ));
                }
                if sub.is_empty() {
                    return Err(VirtualPathError::InvalidFormat(
                        "sub-path is empty".to_string(),
                    ));
                }

                let sub_path = PathBuf::from(sub);

                Ok(VirtualPath {
                    library: lib.to_string(),
                    sub_path,
                })
            }
            None => Err(VirtualPathError::InvalidFormat(
                "missing '/' separator between library name and sub-path".to_string(),
            )),
        }
    }

    pub fn resolve(&self, libraries: &[ContentLibrary]) -> Result<PathBuf, VirtualPathError> {
        let lib = libraries
            .iter()
            .find(|l| l.name == self.library)
            .ok_or_else(|| VirtualPathError::LibraryNotFound(self.library.clone()))?;
        Ok(lib.resolve(&self.sub_path))
    }

    pub fn is_writable(&self, libraries: &[ContentLibrary]) -> Result<bool, VirtualPathError> {
        let lib = libraries
            .iter()
            .find(|l| l.name == self.library)
            .ok_or_else(|| VirtualPathError::LibraryNotFound(self.library.clone()))?;
        Ok(lib.is_writable())
    }
}

impl std::fmt::Display for VirtualPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.library, self.sub_path.display())
    }
}

#[cfg(test)]
#[path = "virtual_path_tests.rs"]
mod tests;
