//! VFS behaviour — content-library helpers and the virtual-path resolver.
//!
//! Three things live here, all behaviour over the VFS data types in
//! [`crate::app::vfs::virtual_path`]:
//!
//! - [`ContentLibraryExt`] — the trait that gives
//!   [`crate::config::ContentLibrary`] the methods callers need to
//!   participate in the VFS (containment, sub-path resolution,
//!   display label, etc.). Defined here so [`crate::config`] can
//!   stay data-only.
//! - [`library_display_label`] — the free function that picks the
//!   first library containing a path and returns its display label.
//! - [`resolve`] / [`resolve_writable`] — the tool-facing entry points
//!   that turn a library-prefixed virtual path into an absolute
//!   filesystem path, enforcing traversal protection and read-only
//!   rules.
//!
//! Spec: [`app/vfs/SPEC.md`](../vfs/SPEC.md) (VFS-001..VFS-009).
use crate::app::vfs::virtual_path::{VirtualPath, VirtualPathError};
use crate::config::ContentLibrary;
use std::path::{Path, PathBuf};

/// Behaviour that callers need on a [`ContentLibrary`] to participate in
/// the VFS domain: containment checks, sub-path resolution,
/// read-only enforcement, and the user-facing display label.
///
/// This is a trait (rather than an inherent `impl` on `ContentLibrary`)
/// so that the methods can live in the VFS subsystem
/// ([`crate::app::vfs`]) while the data type stays in
/// [`crate::config`]. Callers that use any of these methods must
/// import the trait: `use crate::app::vfs::behaviour::ContentLibraryExt;`.
pub trait ContentLibraryExt {
    /// Purpose: Compute the user-facing display label for an absolute path under this library.
    /// Inputs: `path` (the absolute path to localize).
    /// Outputs: `Some(label)` when `path` lives inside `self.root_folder`, otherwise `None`.
    /// Purity: Pure function.
    /// Preconditions: `self.root_folder` should be an absolute path.
    /// Postconditions: The returned label uses `/` separators and `self.name` as the root segment; trailing separators from `Path::join` are trimmed.
    fn display_label_for(&self, path: &Path) -> Option<String>;

    /// Check if the given absolute path is inside this library's root folder.
    fn contains_path(&self, path: &Path) -> bool;

    /// Resolve a sub-path relative to this library's root folder.
    fn resolve(&self, sub: &Path) -> PathBuf;

    /// Returns `true` if this library allows writes.
    fn is_writable(&self) -> bool;

    /// Returns the root folder as a `PathBuf`.
    fn root_path(&self) -> PathBuf;
}

impl ContentLibraryExt for ContentLibrary {
    fn display_label_for(&self, path: &Path) -> Option<String> {
        let root = Path::new(&self.root_folder);
        let rel = path.strip_prefix(root).ok()?;
        let joined = Path::new(&self.name).join(rel);
        let mut label = joined.to_string_lossy().into_owned();
        if label.ends_with('\\') || label.ends_with('/') {
            label.pop();
        }
        Some(label)
    }

    fn contains_path(&self, path: &Path) -> bool {
        path.starts_with(&self.root_folder)
    }

    fn resolve(&self, sub: &Path) -> PathBuf {
        PathBuf::from(&self.root_folder).join(sub)
    }

    fn is_writable(&self) -> bool {
        !self.readonly
    }

    fn root_path(&self) -> PathBuf {
        PathBuf::from(&self.root_folder)
    }
}

/// Purpose: Find the first library that contains the given absolute path and return its display label.
/// Inputs: `libraries` (slice of registered content libraries), `path` (the absolute path to localize).
/// Outputs: `Some(label)` if any library contains `path`, otherwise `None`.
/// Purity: Pure function.
/// Preconditions: Each library's `root_folder` should be an absolute path.
/// Postconditions: Returns `None` if no library matches.
pub fn library_display_label(libraries: &[ContentLibrary], path: &Path) -> Option<String> {
    libraries.iter().find_map(|lib| lib.display_label_for(path))
}

/// Resolve a virtual path against the configured libraries.
///
/// Inputs: `vpath` (the library-prefixed virtual path),
/// `allow_write` (whether the caller is performing a mutating operation),
/// `libraries` (the configured content libraries, used to look up the
/// root folder and read-only flag).
///
/// Outputs:
/// - `Ok(None)` when `vpath` is the virtual root (`/`, `.`, or empty
///   after stripping). Used by `list_notes` to mean "enumerate the
///   libraries".
/// - `Ok(Some((path, readonly)))` when the path resolves; `path` is the
///   absolute filesystem path and `readonly` is the library's write flag.
/// - `Err(String)` on any failure (traversal, missing library, read-only
///   library when writes are not allowed). The string is human-readable
///   and intended to surface to the LLM.
///
/// Purity: pure function. No I/O.
pub fn resolve(
    vpath: &str,
    allow_write: bool,
    libraries: &[ContentLibrary],
) -> Result<Option<(PathBuf, bool)>, String> {
    let normalized = vpath.replace('\\', "/");
    let trimmed = normalized.trim_matches('/');
    if trimmed.is_empty() || trimmed == "." {
        return Ok(None);
    }

    // Parse the leading/trailing-slash-stripped form so that inputs like
    // `/Wiki/Career/SQA.md` resolve to `library="Wiki", sub="Career/SQA.md"`
    // instead of being misrejected as an empty library name. The raw `vpath`
    // is retained only for the error message below.
    let vp = match VirtualPath::parse(trimmed) {
        Ok(vp) => vp,
        Err(VirtualPathError::InvalidFormat(_)) => {
            // Library-only inputs (no sub-path, e.g. "Wiki" or "/Wiki")
            // land here; treat the entire trimmed string as a library name.
            let lib = libraries.iter().find(|l| l.name == trimmed);
            if let Some(lib) = lib {
                if allow_write && !lib.is_writable() {
                    return Err("Cannot perform this operation on a read-only library".to_string());
                }
                return Ok(Some((lib.root_path(), lib.readonly)));
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

    Ok(Some((lib.resolve(&vp.sub_path), lib.readonly)))
}

/// Resolve a virtual path for a mutating tool. Bundles the three lines
/// every mutating tool needed: path resolution, the virtual-root
/// rejection, and the read-only-library rejection. [`resolve`] already
/// returns an error when the target library is read-only, so this
/// helper is sufficient on its own (no separate `if readonly` check
/// needed at the call site). Returns the absolute filesystem path on
/// success.
pub fn resolve_writable(vpath: &str, libraries: &[ContentLibrary]) -> Result<PathBuf, String> {
    match resolve(vpath, true, libraries)? {
        Some((path, _readonly)) => Ok(path),
        None => Err("Cannot perform this operation on the virtual root".to_string()),
    }
}

#[allow(dead_code)]
fn _silence_unused_warnings(_: &Path) {}

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
        let (path, readonly) = result.unwrap().unwrap();
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
        let (path, readonly) = result.unwrap().unwrap();
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

    /// Regression: a leading slash on a library-prefixed path must not cause
    /// the entire trimmed path to be treated as a library name. Before the
    /// fix, `VirtualPath::parse` saw the leading `/` as the separator and
    /// returned `InvalidFormat("library name is empty")`, which routed the
    /// lookup through the fallback branch that compared the full
    /// `"Wiki/Career/SQA.md"` string against library names — failing with a
    /// misleading "Content library not found" error despite a valid library.
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

        // Leading-slash form must resolve to lib="Wiki", sub="Career/SQA.md".
        let result = resolve("/Wiki/Career/SQA.md", false, &libs);
        assert!(
            result.is_ok(),
            "leading slash must resolve; got {:?}",
            result.err()
        );
        let (path, readonly) = result.unwrap().unwrap();
        assert_eq!(path, PathBuf::from("/tmp/wiki/Career/SQA.md"));
        assert!(!readonly);

        // Backslash + leading-slash form must also work.
        let result = resolve("\\Wiki\\Career\\SQA.md", false, &libs);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap().unwrap().0,
            PathBuf::from("/tmp/wiki/Career/SQA.md")
        );

        // Trailing slash must not break resolution either.
        let result = resolve("/Wiki/Career/SQA.md/", false, &libs);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap().unwrap().0,
            PathBuf::from("/tmp/wiki/Career/SQA.md")
        );
    }

    /// Regression: a path referencing an unknown library with a leading slash
    /// still produces the expected "library not found" error (referencing the
    /// *library name*, not the full sub-path, in its error message).
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
