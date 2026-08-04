//! Content-library behaviour and the real-path → virtual-label reverse mapping.
//!
//! [`ContentLibrary`] itself is a data type owned by [`crate::config`]
//! (it is part of the YAML schema). The behaviour on it lives here as
//! the [`ContentLibraryExt`] trait so that [`crate::config`] stays
//! data-only and the VFS domain is self-contained.
//!
//! Spec: [`app/vfs/SPEC.md`](../vfs/SPEC.md) (VFS-001, VFS-002, VFS-007, VFS-008).

use std::path::{Path, PathBuf};

use crate::config::ContentLibrary;

/// Behaviour that callers need on a [`ContentLibrary`] to participate in
/// the VFS domain: containment checks, sub-path resolution,
/// read-only enforcement, and the user-facing display label.
///
/// This is a trait (rather than an inherent `impl` on `ContentLibrary`)
/// so that the methods can live in the VFS subsystem
/// ([`crate::app::vfs`]) while the data type stays in
/// [`crate::config`]. Callers that use any of these methods must
/// import the trait: `use crate::app::vfs::library::ContentLibraryExt;`.
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
