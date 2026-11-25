//! Tests for [`initial_rename_value`] (`ui::tree::flatten`).

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
