use std::path::{Path, PathBuf};

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

        let path = Path::new(vpath);
        if path
            .components()
            .any(|c| c == std::path::Component::ParentDir)
        {
            return Err(VirtualPathError::TraversalDetected);
        }

        let vpath_normalized = vpath.replace('\\', "/");
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
                if sub_path
                    .components()
                    .any(|c| c == std::path::Component::ParentDir)
                {
                    return Err(VirtualPathError::TraversalDetected);
                }

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

    pub fn resolve(
        &self,
        libraries: &[crate::config::ContentLibrary],
    ) -> Result<PathBuf, VirtualPathError> {
        let lib = libraries
            .iter()
            .find(|l| l.name == self.library)
            .ok_or_else(|| VirtualPathError::LibraryNotFound(self.library.clone()))?;
        Ok(PathBuf::from(&lib.root_folder).join(&self.sub_path))
    }

    pub fn is_writable(
        &self,
        libraries: &[crate::config::ContentLibrary],
    ) -> Result<bool, VirtualPathError> {
        let lib = libraries
            .iter()
            .find(|l| l.name == self.library)
            .ok_or_else(|| VirtualPathError::LibraryNotFound(self.library.clone()))?;
        Ok(!lib.readonly)
    }

    pub fn to_string(&self) -> String {
        format!("{}/{}", self.library, self.sub_path.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ContentLibrary;

    fn test_libraries() -> Vec<ContentLibrary> {
        vec![ContentLibrary {
            root_folder: "C:/lib/one".to_string(),
            name: "One".to_string(),
            kind: "text".to_string(),
            readonly: true,
            priority: 0,
        }]
    }

    #[test]
    fn test_parse_basic() {
        let vp = VirtualPath::parse("Lib/file.md").unwrap();
        assert_eq!(vp.library, "Lib");
        assert_eq!(vp.sub_path, PathBuf::from("file.md"));
    }

    #[test]
    fn test_parse_nested() {
        let vp = VirtualPath::parse("Lib/dir1/dir2/file.md").unwrap();
        assert_eq!(vp.library, "Lib");
        assert_eq!(vp.sub_path, PathBuf::from("dir1/dir2/file.md"));
    }

    #[test]
    fn test_parse_traversal_rejected() {
        assert_eq!(
            VirtualPath::parse("../outside"),
            Err(VirtualPathError::TraversalDetected)
        );
    }

    #[test]
    fn test_parse_traversal_after_library() {
        assert_eq!(
            VirtualPath::parse("Lib/../outside"),
            Err(VirtualPathError::TraversalDetected)
        );
    }

    #[test]
    fn test_parse_traversal_deep() {
        assert_eq!(
            VirtualPath::parse("Lib/a/b/../../outside"),
            Err(VirtualPathError::TraversalDetected)
        );
    }

    #[test]
    fn test_parse_empty() {
        assert_eq!(VirtualPath::parse(""), Err(VirtualPathError::EmptyPath));
    }

    #[test]
    fn test_parse_no_separator() {
        let result = VirtualPath::parse("JustALibraryName");
        assert!(matches!(result, Err(VirtualPathError::InvalidFormat(_))));
    }

    #[test]
    fn test_parse_backslash_normalized() {
        let vp = VirtualPath::parse("Lib\\sub\\file.md").unwrap();
        assert_eq!(vp.library, "Lib");
        assert_eq!(vp.sub_path, PathBuf::from("sub/file.md"));
    }

    #[test]
    fn test_parse_empty_library() {
        let result = VirtualPath::parse("/file.md");
        assert!(matches!(result, Err(VirtualPathError::InvalidFormat(_))));
    }

    #[test]
    fn test_roundtrip() {
        let vp = VirtualPath::parse("MyLib/some/path/file.md").unwrap();
        let s = vp.to_string();
        assert_eq!(s, "MyLib/some/path/file.md");
        let vp2 = VirtualPath::parse(&s).unwrap();
        assert_eq!(vp, vp2);
    }

    #[test]
    fn test_resolve_valid() {
        let vp = VirtualPath::parse("One/note.md").unwrap();
        let path = vp.resolve(&test_libraries()).unwrap();
        assert_eq!(path, PathBuf::from("C:/lib/one/note.md"));
    }

    #[test]
    fn test_resolve_unknown_library() {
        let vp = VirtualPath::parse("Unknown/file.md").unwrap();
        assert_eq!(
            vp.resolve(&test_libraries()),
            Err(VirtualPathError::LibraryNotFound("Unknown".to_string()))
        );
    }

    #[test]
    fn test_is_writable_readonly() {
        let vp = VirtualPath::parse("One/file.md").unwrap();
        assert_eq!(vp.is_writable(&test_libraries()), Ok(false));
    }

    #[test]
    fn test_is_writable_writable() {
        let libs = vec![ContentLibrary {
            root_folder: "C:/lib/writable".to_string(),
            name: "Writable".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        }];
        let vp = VirtualPath::parse("Writable/file.md").unwrap();
        assert_eq!(vp.is_writable(&libs), Ok(true));
    }

    #[test]
    fn test_is_writable_unknown_library() {
        let vp = VirtualPath::parse("Unknown/file.md").unwrap();
        assert_eq!(
            vp.is_writable(&test_libraries()),
            Err(VirtualPathError::LibraryNotFound("Unknown".to_string()))
        );
    }

    #[test]
    fn test_error_display() {
        let e = VirtualPathError::EmptyPath;
        assert!(!e.to_string().is_empty());

        let e = VirtualPathError::TraversalDetected;
        assert!(e.to_string().contains("path traversal"));

        let e = VirtualPathError::InvalidFormat("test".to_string());
        assert!(e.to_string().contains("test"));

        let e = VirtualPathError::LibraryNotFound("Foo".to_string());
        assert!(e.to_string().contains("Foo"));

        let e = VirtualPathError::LibraryNotWritable("Foo".to_string());
        assert!(e.to_string().contains("Foo"));
    }

    #[test]
    fn test_parse_traversal_in_subpath_only() {
        assert_eq!(
            VirtualPath::parse("Lib/ok/../../nope"),
            Err(VirtualPathError::TraversalDetected)
        );
    }

    #[test]
    fn test_resolve_nested_subpath() {
        let vp = VirtualPath::parse("One/a/b/c.md").unwrap();
        let path = vp.resolve(&test_libraries()).unwrap();
        assert_eq!(path, PathBuf::from("C:/lib/one/a/b/c.md"));
    }
}
