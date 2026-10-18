//! Unit tests for the iced platform-shell helpers in `os_shell.rs`
//! ([TASK-029], [RUST-003]).
//!
//! Each helper is a one-line `opener` wrapper whose result is discarded, so
//! there is exactly one code path per function and nothing to branch on.
//! Following the precedent the egui `os_shell_tests.rs` set ("no `opener`
//! invocations"), **no test here actually launches an OS process** — a real
//! open/reveal is an un-cancellable external side effect and is not
//! test-environment safe ([RUST-006]).
//!
//! What IS pinned is the property this task exists to guarantee: every
//! signature is **egui-free**. The `fn`-pointer assignments below fail to
//! compile the moment an `egui`/`eframe` type leaks into a parameter or
//! return position, which is a stronger check than a runtime assertion.

use super::*;

/// [`open_in_system_editor`] takes exactly a `&Path` and returns `()`.
#[test]
fn open_in_system_editor_signature_is_egui_free() {
    let _f: fn(&Path) = open_in_system_editor;
}

/// [`show_in_file_explorer`] takes exactly a `&Path` and returns `()`.
#[test]
fn show_in_file_explorer_signature_is_egui_free() {
    let _f: fn(&Path) = show_in_file_explorer;
}

/// [`open_url`] takes exactly a `&str` and returns `()`.
#[test]
fn open_url_signature_is_egui_free() {
    let _f: fn(&str) = open_url;
}

/// The three helpers are distinct entry points, not aliases: the editor and
/// the explorer wrappers go through different `opener` calls (`open` vs
/// `reveal`), so collapsing them would silently lose the "reveal in explorer"
/// behaviour the tree context menu depends on.
///
/// Pinned by resolving each through a common `fn(&Path)` slot and checking
/// the function addresses differ.
#[test]
fn editor_and_explorer_helpers_are_distinct_functions() {
    let editor: fn(&Path) = open_in_system_editor;
    let explorer: fn(&Path) = show_in_file_explorer;

    assert_ne!(
        editor as usize, explorer as usize,
        "`open` and `reveal` must stay separate wrappers"
    );
}
