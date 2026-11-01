//! Platform-shell helpers for the iced layer.
//!
//! The three OS-facing operations the application performs — open a file in the
//! system default editor, reveal a file in the OS file explorer, and open a URL
//! in the default browser — plus the [`OsShell`] seam that lets a test **record**
//! them instead of performing them.
//!
//! # Framework-free by construction
//!
//! Every item here routes through the `opener` crate (`Cargo.toml`) and takes
//! only `&Path` / `&str`. **No framework type appears in any signature or
//! body.** There is no platform-output command list to drain: iced has no such
//! concept, so the reducer calls [`OsShell::open_url`] on the shell it holds on
//! `AppState::shell` when it needs to open a link.
//!
//! # Why the seam exists
//!
//! A reducer that calls `opener` inline cannot be tested honestly: driving it
//! through a link press would really launch a browser — an un-cancellable
//! external side effect that is not test-environment safe ([RUST-006]). Holding
//! the shell as `Arc<dyn OsShell>` on `AppState` makes the effect observable
//! through [`RecordingOsShell`] without touching iced's fixed
//! `update(&mut state, message)` signature, and keeps the reducer's effect
//! injectable in the same way [`crate::ui_iced::persisted::UiStateStorage`]
//! makes persistence injectable ([RUST-020], [RUST-024]).
//!
//! # The two entry points are deliberate
//!
//! The free wrappers ([`open_in_system_editor`], [`show_in_file_explorer`],
//! [`open_url`]) stay public because the background file-operation worker
//! (`crate::background::user_file_operation`) runs off the UI thread with no
//! `AppState` to read a shell from. [`OpenerShell`] forwards to them, so each
//! effect still has exactly **one** implementation; the reducer must go through
//! the seam, the worker through the wrappers.
//!
//! Unit tests live in the sibling `os_shell_tests.rs` sidecar.

#[cfg(test)]
#[path = "os_shell_tests.rs"]
mod tests;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Open a file in the system default editor.
///
/// The result is discarded — a failed open is surfaced by the caller's own
/// logging and the user can retry ([NFR-001] is the caller's contract here).
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

/// The OS-shell side effects, behind one injectable seam.
///
/// The reducer holds an `Arc<dyn OsShell>` on `AppState` and calls this trait
/// instead of the free wrappers, so a unit test can swap in
/// [`RecordingOsShell`] and assert **which** effect was requested and with
/// **what** argument, without a process being spawned ([RUST-006]).
///
/// Like every other injected handler in this codebase the trait is
/// framework-free: `&Path` / `&str` in, `()` out.
pub trait OsShell: Send + Sync {
    /// Hand `path` to the system default editor.
    fn open_in_system_editor(&self, path: &Path);

    /// Reveal `path` in the OS file explorer.
    fn show_in_file_explorer(&self, path: &Path);

    /// Hand `url` to the OS URL handler (browser, mail client, …).
    fn open_url(&self, url: &str);
}

/// The production [`OsShell`]: forwards every call to the free `opener`
/// wrappers above, so the shipped behaviour is exactly what it was before the
/// seam existed.
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenerShell;

impl OsShell for OpenerShell {
    fn open_in_system_editor(&self, path: &Path) {
        open_in_system_editor(path);
    }

    fn show_in_file_explorer(&self, path: &Path) {
        show_in_file_explorer(path);
    }

    fn open_url(&self, url: &str) {
        open_url(url);
    }
}

/// One effect captured by [`RecordingOsShell`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellCall {
    /// [`OsShell::open_url`] with this URL.
    OpenUrl(String),
    /// [`OsShell::open_in_system_editor`] with this path.
    OpenInSystemEditor(PathBuf),
    /// [`OsShell::show_in_file_explorer`] with this path.
    ShowInFileExplorer(PathBuf),
}

/// An [`OsShell`] that records each call instead of performing it.
///
/// Mirrors [`crate::ui_iced::persisted::InMemoryUiStateStorage`]: it lives in
/// production code so any in-process harness can neutralise the OS side effects
/// the same way it neutralises file writes, and it is what reducer tests inject
/// to assert an open happened **without** launching anything ([RUST-006]).
///
/// Interior-mutable and `Send + Sync`, so one handle can be shared between the
/// state under test and the assertions that read it back.
#[derive(Debug, Default)]
pub struct RecordingOsShell {
    calls: Mutex<Vec<ShellCall>>,
}

impl RecordingOsShell {
    /// Create an empty recorder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Every call recorded so far, in order.
    #[must_use]
    pub fn calls(&self) -> Vec<ShellCall> {
        self.calls.lock().unwrap().clone()
    }

    /// Just the URLs passed to [`OsShell::open_url`], in order.
    #[must_use]
    pub fn urls(&self) -> Vec<String> {
        self.calls()
            .into_iter()
            .filter_map(|call| match call {
                ShellCall::OpenUrl(url) => Some(url),
                _ => None,
            })
            .collect()
    }

    /// Forget everything recorded so far.
    pub fn clear(&self) {
        self.calls.lock().unwrap().clear();
    }
}

impl OsShell for RecordingOsShell {
    fn open_in_system_editor(&self, path: &Path) {
        self.calls
            .lock()
            .unwrap()
            .push(ShellCall::OpenInSystemEditor(path.to_path_buf()));
    }

    fn show_in_file_explorer(&self, path: &Path) {
        self.calls
            .lock()
            .unwrap()
            .push(ShellCall::ShowInFileExplorer(path.to_path_buf()));
    }

    fn open_url(&self, url: &str) {
        self.calls
            .lock()
            .unwrap()
            .push(ShellCall::OpenUrl(url.to_string()));
    }
}
