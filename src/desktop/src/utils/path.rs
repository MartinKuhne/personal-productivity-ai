//! Safe-basename validation — ensures a user-supplied filename is a traversal-free single path segment.

use std::path::{Component, Path};

/// Purpose: Validate that a user-supplied name is a single safe path segment (no traversal, no separators).
/// Inputs: `name` (the trimmed candidate basename).
/// Outputs: `true` when `name` is non-empty, contains no `..` component, and contains no `/` or `\\` separator.
/// Purity: Pure function.
/// Preconditions: Caller should trim whitespace before calling.
/// Postconditions: Returns `false` for any string that would, if joined onto a parent directory, escape that directory.
pub fn is_safe_basename(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    if name.contains('/') || name.contains('\\') {
        return false;
    }
    if Path::new(name)
        .components()
        .any(|c| c == Component::ParentDir)
    {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_safe_basename_accepts_normal_names() {
        assert!(is_safe_basename("file.md"));
        assert!(is_safe_basename("a"));
        assert!(is_safe_basename("with spaces.txt"));
        assert!(is_safe_basename("file.name.with.dots"));
    }

    #[test]
    fn test_is_safe_basename_rejects_empty() {
        assert!(!is_safe_basename(""));
    }

    #[test]
    fn test_is_safe_basename_rejects_separators() {
        assert!(!is_safe_basename("a/b"));
        assert!(!is_safe_basename(r"a\b"));
        assert!(!is_safe_basename("/etc/passwd"));
        assert!(!is_safe_basename(r"\windows\system32"));
    }

    #[test]
    fn test_is_safe_basename_rejects_traversal() {
        assert!(!is_safe_basename(".."));
        assert!(!is_safe_basename("../etc"));
        assert!(!is_safe_basename("a/../b"));
    }
}
