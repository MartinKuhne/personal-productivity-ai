//! VFS behaviour — content-library helpers and the virtual-path resolver.

use super::virtual_path::{VirtualPath, VirtualPathError};
use crate::config::{ContentLibrary, ContentLibraryExt};
use std::path::{Path, PathBuf};

/// Purpose: Find the first library that contains the given absolute path and return its display label.
/// Inputs: `libraries` (slice of registered content libraries), `path` (the absolute path to localize).
/// Outputs: `Some(label)` if any library contains `path`, otherwise `None`.
/// Purity: Pure function.
/// Preconditions: Each library's `root_folder` should be an absolute path.
/// Postconditions: Returns `None` if no library matches.
pub fn library_display_label(libraries: &[ContentLibrary], path: &Path) -> Option<String> {
    libraries.iter().find_map(|lib| lib.display_label_for(path))
}

/// Result of resolving a virtual path against the configured
/// libraries: the absolute filesystem path plus whether the owning
/// content library is read-only.
///
/// Returned by [`resolve`] and
/// [`crate::tools::context::ToolContext::resolve_virtual_path`] in
/// place of a bare `(PathBuf, bool)` tuple so callers read the
/// semantic meaning (`path` vs `readonly`) at the call site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedVirtualPath {
    /// Absolute filesystem path the virtual path maps to.
    pub path: PathBuf,
    /// Whether the owning content library is read-only. Read-only
    /// libraries reject mutating operations.
    pub readonly: bool,
}

/// Resolve a virtual path against the configured libraries.
pub fn resolve(
    vpath: &str,
    allow_write: bool,
    libraries: &[ContentLibrary],
) -> Result<Option<ResolvedVirtualPath>, String> {
    let normalized = vpath.replace('\\', "/");
    let trimmed = normalized.trim_matches('/');
    if trimmed.is_empty() || trimmed == "." {
        return Ok(None);
    }

    let vp = match VirtualPath::parse(trimmed) {
        Ok(vp) => vp,
        Err(VirtualPathError::InvalidFormat(_)) => {
            let lib = libraries.iter().find(|l| l.name == trimmed);
            if let Some(lib) = lib {
                if allow_write && !lib.is_writable() {
                    return Err("Cannot perform this operation on a read-only library".to_string());
                }
                return Ok(Some(ResolvedVirtualPath {
                    path: lib.root_path(),
                    readonly: lib.readonly,
                }));
            }
            return Err(format!(
                "Content library '{}' not found in virtual path '{}'",
                trimmed, vpath
            ));
        }
        Err(e) => return Err(e.to_string()),
    };

    let lib = libraries
        .iter()
        .find(|l| l.name == vp.library)
        .ok_or_else(|| {
            format!(
                "Content library '{}' not found in virtual path '{}'",
                vp.library, vpath
            )
        })?;

    if allow_write && !lib.is_writable() {
        return Err("Cannot perform this operation on a read-only library".to_string());
    }

    Ok(Some(ResolvedVirtualPath {
        path: lib.resolve(&vp.sub_path),
        readonly: lib.readonly,
    }))
}

/// Resolve a virtual path for a mutating tool.
pub fn resolve_writable(vpath: &str, libraries: &[ContentLibrary]) -> Result<PathBuf, String> {
    match resolve(vpath, true, libraries)? {
        Some(resolved) => Ok(resolved.path),
        None => Err("Cannot perform this operation on the virtual root".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ContentLibrary;

    fn make_lib(root_folder: &str, name: &str, readonly: bool) -> ContentLibrary {
        ContentLibrary {
            root_folder: root_folder.to_string(),
            name: name.to_string(),
            kind: "text".to_string(),
            readonly,
            priority: 0,
        }
    }

    #[test]
    fn test_content_library_contains_path_inside() {
        let lib = make_lib("C:/lib/one", "One", true);
        assert!(lib.contains_path(Path::new("C:/lib/one/sub/file.md")));
        assert!(lib.contains_path(Path::new("C:/lib/one")));
    }

    #[test]
    fn test_content_library_contains_path_outside() {
        let lib = make_lib("C:/lib/one", "One", true);
        assert!(!lib.contains_path(Path::new("C:/lib/two/file.md")));
        assert!(!lib.contains_path(Path::new("D:/other/path.md")));
    }

    #[test]
    fn test_content_library_resolve() {
        let lib = make_lib("C:/base", "Base", false);
        assert_eq!(
            lib.resolve(Path::new("sub/file.md")),
            PathBuf::from("C:/base/sub/file.md")
        );
        assert_eq!(lib.resolve(Path::new("")), PathBuf::from("C:/base"));
    }

    #[test]
    fn test_content_library_is_writable() {
        let writable = make_lib("C:/w", "W", false);
        assert!(writable.is_writable());

        let readonly = make_lib("C:/r", "R", true);
        assert!(!readonly.is_writable());
    }

    #[test]
    fn test_content_library_root_path() {
        let lib = make_lib("C:/my/library", "Test", false);
        assert_eq!(lib.root_path(), PathBuf::from("C:/my/library"));
    }

    #[test]
    fn test_content_library_display_label_for_member() {
        let lib = make_lib("C:/my/test/dir", "TestLib", true);
        let expected = PathBuf::from("TestLib").join("sub/file.md");
        let actual = lib
            .display_label_for(Path::new("C:/my/test/dir/sub/file.md"))
            .expect("path is inside library");
        assert_eq!(actual, expected.to_string_lossy());
        assert_eq!(
            lib.display_label_for(Path::new("C:/my/test/dir")),
            Some("TestLib".to_string())
        );
        assert!(
            lib.display_label_for(Path::new("C:/other/path.md"))
                .is_none()
        );
    }

    #[test]
    fn test_library_display_label_finds_first_match() {
        let libs = vec![
            make_lib("C:/lib/one", "One", true),
            make_lib("C:/lib/two", "Two", true),
        ];
        let expected = PathBuf::from("Two").join("note.md");
        let actual = library_display_label(&libs, Path::new("C:/lib/two/note.md"))
            .expect("path is inside a library");
        assert_eq!(actual, expected.to_string_lossy());
        assert!(library_display_label(&libs, Path::new("C:/other/note.md")).is_none());
    }

    fn test_config() -> Vec<ContentLibrary> {
        vec![
            ContentLibrary {
                name: "TestLib".to_string(),
                root_folder: "/tmp/testlib".to_string(),
                kind: "text".to_string(),
                readonly: false,
                priority: 0,
            },
            ContentLibrary {
                name: "ReadOnlyLib".to_string(),
                root_folder: "/tmp/readonly".to_string(),
                kind: "text".to_string(),
                readonly: true,
                priority: 0,
            },
        ]
    }

    #[test]
    fn test_resolve_valid_path() {
        let libs = test_config();
        let result = resolve("TestLib/sub/file.md", false, &libs);
        assert!(result.is_ok());
        let (path, readonly) = match result.unwrap().unwrap() {
            ResolvedVirtualPath { path, readonly } => (path, readonly),
        };
        assert_eq!(path, PathBuf::from("/tmp/testlib/sub/file.md"));
        assert!(!readonly);
    }

    #[test]
    fn test_resolve_traversal_rejected() {
        let libs = test_config();
        let result = resolve("TestLib/../outside", false, &libs);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("path traversal"));
    }

    #[test]
    fn test_resolve_unknown_library() {
        let libs = test_config();
        let result = resolve("NonExistent/file.md", false, &libs);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Content library 'NonExistent' not found")
        );
    }

    #[test]
    fn test_resolve_readonly_write() {
        let libs = test_config();
        let result = resolve("ReadOnlyLib/file.md", true, &libs);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("read-only library"));
    }

    #[test]
    fn test_resolve_readonly_read() {
        let libs = test_config();
        let result = resolve("ReadOnlyLib/file.md", false, &libs);
        assert!(result.is_ok());
        let (path, readonly) = match result.unwrap().unwrap() {
            ResolvedVirtualPath { path, readonly } => (path, readonly),
        };
        assert_eq!(path, PathBuf::from("/tmp/readonly/file.md"));
        assert!(readonly);
    }

    #[test]
    fn test_resolve_root_path() {
        let libs = test_config();
        let result = resolve("/", false, &libs);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        let result2 = resolve(".", false, &libs);
        assert!(result2.is_ok());
        assert!(result2.unwrap().is_none());
    }

    #[test]
    fn test_resolve_leading_slash_path() {
        let mut libs = test_config();
        libs.push(ContentLibrary {
            name: "Wiki".to_string(),
            root_folder: "/tmp/wiki".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        });

        let result = resolve("/Wiki/Career/SQA.md", false, &libs);
        assert!(
            result.is_ok(),
            "leading slash must resolve; got {:?}",
            result.err()
        );
        let (path, readonly) = match result.unwrap().unwrap() {
            ResolvedVirtualPath { path, readonly } => (path, readonly),
        };
        assert_eq!(path, PathBuf::from("/tmp/wiki/Career/SQA.md"));
        assert!(!readonly);

        let result = resolve("\\Wiki\\Career\\SQA.md", false, &libs);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap().unwrap().path,
            PathBuf::from("/tmp/wiki/Career/SQA.md")
        );

        let result = resolve("/Wiki/Career/SQA.md/", false, &libs);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap().unwrap().path,
            PathBuf::from("/tmp/wiki/Career/SQA.md")
        );
    }

    #[test]
    fn test_resolve_leading_slash_unknown_library() {
        let mut libs = test_config();
        libs.push(ContentLibrary {
            name: "Wiki".to_string(),
            root_folder: "/tmp/wiki".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        });

        let result = resolve("/UnknownLib/sub/file.md", false, &libs);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err == "Content library 'UnknownLib' not found in virtual path '/UnknownLib/sub/file.md'"
                || err.contains("Content library 'UnknownLib' not found"),
            "unexpected error: {}",
            err
        );
    }
}
