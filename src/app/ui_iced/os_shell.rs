//! Platform-shell helpers for the iced layer ([TASK-029]).
//!
//! The three OS-facing wrappers the iced reducer needs — open a file in the
//! system default editor, reveal a file in the OS file explorer, and open a
//! URL in the default browser. They are copied out of the legacy UI
//! `src/app/ui/os_shell.rs` so `ui_iced` no longer depends on a file
//! [TASK-029b] deletes.
//!
//! # framework-free by construction ([TASK-029] verification)
//!
//! Every item here routes through the `opener` crate (`Cargo.toml`) and takes
//! only `&Path` / `&str`. **No framework type appears in any
//! signature or body**, unlike the legacy UI original, whose fourth function
//! (`dispatch_platform_commands`) drains an `&[OutputCommand]`.
//! That drainer is deliberately **not** copied: iced has no platform-output
//! command list, and the iced link path calls [`open_url`] from the reducer
//! instead (see the TASK-020 audit note in
//! `doc/planning/iced-migration-map.md`).
//!
//! Unit tests live in the sibling `os_shell_tests.rs` sidecar.

#[cfg(test)]
#[path = "os_shell_tests.rs"]
mod tests;

use std::path::Path;

/// Open a file in the system default editor.
///
/// The result is discarded — a failed open is surfaced by the caller's own
/// logging and the user can retry ([NFR-001] is the caller's contract here,
/// same as the legacy UI helper's).
pub fn open_in_system_editor(path: &Path) {
    let _ = opener::open(path);
}

/// Show a file in the system file explorer with the file selected.
pub fn show_in_file_explorer(path: &Path) {
    let _ = opener::reveal(path);
}

/// Open a URL in the system default browser.
///
/// Thin wrapper over [`opener::open`] (already a dependency,
/// `Cargo.toml`). `opener` dispatches by URL scheme: `http` /
/// `https` to the default browser, `mailto:` to the default mail
/// client, `file://` to the OS file handler, etc. The result is
/// discarded — a failed open is logged elsewhere by the caller
/// and the user can retry. Symmetric with the
/// [`open_in_system_editor`] / [`show_in_file_explorer`] helpers
/// above.
pub fn open_url(url: &str) {
    let _ = opener::open(url);
}
