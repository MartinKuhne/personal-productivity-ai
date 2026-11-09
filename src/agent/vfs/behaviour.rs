//! VFS behaviour — content-library helpers and the virtual-path resolver.
//! Unit tests live in the sibling `behaviour_tests.rs` sidecar.

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
#[path = "behaviour_tests.rs"]
mod tests;
