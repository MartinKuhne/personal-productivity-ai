//! Virtual path syntax — library-prefixed paths (e.g. `library/relative/path`) mapped to real filesystem locations with traversal protection.

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
mod tests {
    use super::*;

    fn make_lib(root: &str, name: &str, readonly: bool) -> ContentLibrary {
        ContentLibrary {
            root_folder: root.to_string(),
            name: name.to_string(),
            kind: "text".to_string(),
            readonly,
            priority: 0,
        }
    }

    #[test]
    fn test_parse_valid() {
        let vp = VirtualPath::parse("TestLib/sub/file.md").unwrap();
        assert_eq!(vp.library, "TestLib");
        assert_eq!(vp.sub_path, PathBuf::from("sub/file.md"));
    }

    #[test]
    fn test_parse_nested_subpath() {
        let vp = VirtualPath::parse("TestLib/a/b/c/file.md").unwrap();
        assert_eq!(vp.library, "TestLib");
        assert_eq!(vp.sub_path, PathBuf::from("a/b/c/file.md"));
    }

    #[test]
    fn test_parse_empty() {
        let err = VirtualPath::parse("").unwrap_err();
        assert_eq!(err, VirtualPathError::EmptyPath);
    }

    #[test]
    fn test_parse_no_separator() {
        let err = VirtualPath::parse("justaname").unwrap_err();
        assert_eq!(
            err,
            VirtualPathError::InvalidFormat(
                "missing '/' separator between library name and sub-path".to_string()
            )
        );
    }

    #[test]
    fn test_parse_empty_library() {
        let err = VirtualPath::parse("/sub/file.md").unwrap_err();
        assert_eq!(
            err,
            VirtualPathError::InvalidFormat("library name is empty".to_string())
        );
    }

    #[test]
    fn test_parse_empty_subpath() {
        let err = VirtualPath::parse("TestLib/").unwrap_err();
        assert_eq!(
            err,
            VirtualPathError::InvalidFormat("sub-path is empty".to_string())
        );
    }

    #[test]
    fn test_parse_traversal_dotdot() {
        let err = VirtualPath::parse("TestLib/../secret").unwrap_err();
        assert_eq!(err, VirtualPathError::TraversalDetected);
    }

    #[test]
    fn test_parse_traversal_dotdot_nested() {
        let err = VirtualPath::parse("TestLib/sub/../../secret").unwrap_err();
        assert_eq!(err, VirtualPathError::TraversalDetected);
    }

    #[test]
    fn test_parse_backslash_normalization() {
        let vp = VirtualPath::parse("TestLib\\sub\\file.md").unwrap();
        assert_eq!(vp.library, "TestLib");
        assert_eq!(vp.sub_path, PathBuf::from("sub/file.md"));
    }

    #[test]
    fn test_resolve_success() {
        let libs = vec![
            make_lib("C:/lib1", "Lib1", false),
            make_lib("C:/lib2", "Lib2", false),
        ];
        let vp = VirtualPath::parse("Lib2/notes/todo.md").unwrap();
        let path = vp.resolve(&libs).unwrap();
        assert_eq!(path, PathBuf::from("C:/lib2/notes/todo.md"));
    }

    #[test]
    fn test_resolve_not_found() {
        let libs = vec![make_lib("C:/lib1", "Lib1", false)];
        let vp = VirtualPath::parse("UnknownLib/file.md").unwrap();
        let err = vp.resolve(&libs).unwrap_err();
        assert_eq!(
            err,
            VirtualPathError::LibraryNotFound("UnknownLib".to_string())
        );
    }

    #[test]
    fn test_is_writable_true() {
        let libs = vec![make_lib("C:/lib1", "Lib1", false)];
        let vp = VirtualPath::parse("Lib1/file.md").unwrap();
        assert!(vp.is_writable(&libs).unwrap());
    }

    #[test]
    fn test_is_writable_false() {
        let libs = vec![make_lib("C:/lib1", "Lib1", true)];
        let vp = VirtualPath::parse("Lib1/file.md").unwrap();
        assert!(!vp.is_writable(&libs).unwrap());
    }

    #[test]
    fn test_display() {
        let vp = VirtualPath {
            library: "MyLib".to_string(),
            sub_path: PathBuf::from("path/to/note.md"),
        };
        assert_eq!(format!("{vp}"), "MyLib/path/to/note.md");
    }
}
