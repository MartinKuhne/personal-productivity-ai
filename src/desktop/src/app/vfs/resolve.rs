//! Pure VFS resolution: turn a virtual path into an absolute filesystem path.
//!
//! This is the testable heart of the VFS: a free function that takes a
//! virtual path and a list of libraries, applies traversal protection
//! and read-only enforcement, and returns the absolute path on disk.
//!
//! [`crate::tools::context::ToolContext::resolve_virtual_path`] is a
//! one-line shim over [`resolve`] that pulls the libraries from the
//! active config.
//!
//! Spec: [`app/vfs/SPEC.md`](../vfs/SPEC.md) (VFS-002, VFS-009, plus the
//! grep-priority ordering rule from VFS-008).

use std::path::{Path, PathBuf};

use crate::app::vfs::library::ContentLibraryExt;
use crate::app::vfs::virtual_path::{VirtualPath, VirtualPathError};
use crate::config::ContentLibrary;

/// Resolve a virtual path against the configured libraries.
///
/// Inputs: `vpath` (the library-prefixed virtual path),
/// `allow_write` (whether the caller is performing a mutating operation),
/// `libraries` (the configured content libraries, used to look up the
/// root folder and read-only flag).
///
/// Outputs:
/// - `Ok(None)` when `vpath` is the virtual root (`/`, `.`, or empty
///   after stripping). Used by `list_files` to mean "enumerate the
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
