//! Pure data-transform helpers: flatten a [`TreeNode`] hierarchy into a [`Vec<FlatRow>`].

use crate::ui::TreeNode;
use std::collections::HashSet;
use std::path::PathBuf;

/// Purpose: Build the initial value for the rename text field, offering
/// only the file stem (no extension) so the user types a new base name
/// and the rename modal reattaches the original extension on submit.
/// Inputs: `path` - The file the user wants to rename; `fallback_name` -
/// The display name to fall back to if the path has no usable stem.
/// Outputs: A `String` containing just the file name without extension.
/// Purity: Pure.
/// Preconditions: `path` is the canonical path of the file to rename.
/// Postconditions: Returns a `String` with the file stem; the original
/// extension is intentionally excluded.
pub fn initial_rename_value(path: &std::path::Path, fallback_name: &str) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(fallback_name)
        .to_string()
}

/// Maximum tree depth to prevent stack overflow in flatten_tree and visual overflow in rendering.
pub const MAX_TREE_DEPTH: usize = 1000;

/// A single visible row in the flattened virtual tree list.
///
/// `Debug` and `PartialEq` are required because the view model holds
/// `FlatRow`s directly and tests compare flattened row lists against
/// expected ones.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlatRow {
    /// Indentation depth (0 = top-level child of root).
    pub depth: usize,
    /// Display name of the node.
    pub name: String,
    /// Full path of the node.
    pub path: PathBuf,
    /// Whether this node is a directory.
    pub is_dir: bool,
    /// For directories: whether currently expanded. Always false for files.
    pub is_expanded: bool,
}

/// Slot height, in logical pixels, passed to a virtualised tree list.
///
/// The value is the rendered row height (18px) minus the 3px of
/// between-row spacing a virtualised list adds on top of the slot, so
/// the slot matches the rendered content exactly and empty space does
/// not accumulate at the bottom of each row. The row content is a
/// horizontal block whose height is the maximum of its children: a
/// selectable label whose interactive height is 18px — the 1px top and
/// 1px bottom button padding sits inside the frame, and the text is
/// short enough that the interactive height dominates.
pub const TREE_ROW_HEIGHT: f32 = 15.0;

/// Flatten a `TreeNode` hierarchy into a `Vec<FlatRow>` in DFS pre-order,
/// respecting the set of expanded directories.
pub fn flatten_tree(
    node: &TreeNode,
    depth: usize,
    expanded: &HashSet<PathBuf>,
    rows: &mut Vec<FlatRow>,
) {
    // Prevent stack overflow on maliciously deep directory structures.
    if depth > MAX_TREE_DEPTH {
        return;
    }
    if depth > 0 {
        rows.push(FlatRow {
            depth: depth - 1,
            name: node.name.clone(),
            path: node.path.clone(),
            is_dir: node.is_dir,
            is_expanded: node.is_dir && expanded.contains(&node.path),
        });
    }
    if node.is_dir && (depth == 0 || expanded.contains(&node.path)) {
        let mut children: Vec<_> = node.children.values().collect();
        children.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));
        for child in children {
            flatten_tree(child, depth + 1, expanded, rows);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Regression: the rename dialog must pre-fill with just the file stem
    /// (no extension). The modal reattaches the original extension on
    /// submit, so the user should only ever type the base name. Both
    /// rename call sites go through `initial_rename_value`, so testing
    /// the helper covers them together.
    #[test]
    fn test_initial_rename_value_strips_extension() {
        assert_eq!(
            initial_rename_value(&PathBuf::from("/notes/today.md"), "today.md"),
            "today"
        );
        assert_eq!(
            initial_rename_value(&PathBuf::from("/notes/today.markdown"), "today.markdown"),
            "today"
        );
        assert_eq!(
            initial_rename_value(&PathBuf::from("/notes/2023-01-15.md"), "2023-01-15.md"),
            "2023-01-15"
        );
        assert_eq!(
            initial_rename_value(&PathBuf::from("/notes/notes.txt"), "notes.txt"),
            "notes",
            ".txt extension must also be stripped so the modal re-adds it"
        );
    }

    /// Edge case: a file with no extension should still pre-fill with the
    /// full name, since there is nothing to strip.
    #[test]
    fn test_initial_rename_value_no_extension() {
        assert_eq!(
            initial_rename_value(&PathBuf::from("/notes/Makefile"), "Makefile"),
            "Makefile"
        );
    }

    /// Edge case: an empty file stem falls back to the display name so we
    /// never hand the user a blank text field.
    #[test]
    fn test_initial_rename_value_falls_back_to_display_name() {
        assert_eq!(
            initial_rename_value(&PathBuf::from("/notes/.hidden"), ".hidden"),
            ".hidden",
            "a dotfile's stem is the empty string — display name is the right fallback"
        );
    }
}
