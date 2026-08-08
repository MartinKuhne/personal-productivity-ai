//! Platform-independent helpers — open a file in the system default editor, reveal a file in the OS file explorer, and dispatch egui `OutputCommand::OpenUrl` commands to the system default browser.
//!
//! Unit tests live in the sibling `os_shell_tests.rs` sidecar.

use std::path::Path;

use eframe::egui;

/// Open a file in the system default editor.
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
/// `open_in_system_editor` / `show_in_file_explorer` helpers
/// above.
pub fn open_url(url: &str) {
    let _ = opener::open(url);
}

/// Drain a slice of [`egui::OutputCommand`]s and dispatch each
/// `OpenUrl` to the supplied opener.
///
/// # Why this exists
///
/// `egui::Ui::hyperlink_to` (the widget used by the markdown
/// viewer for every `InlineElem::Link`, the
/// `[text](url)` inline link, the `<https://…>` autolink form,
/// and every heading/table-cell link) emits an
/// `OutputCommand::OpenUrl` on the egui `PlatformOutput` when the
/// user clicks. eframe 0.36's native (winit) runtime does **not**
/// process `OutputCommand::OpenUrl` — only the `web` target
/// handles it (see `eframe/src/web/app_runner.rs:407` and
/// `eframe/src/web/mod.rs:351`). On native the commands list is
/// collected and dropped without any consumer touching the
/// `OpenUrl` variant.
///
/// Without an app-level dispatcher, clicking a hyperlink in the
/// markdown viewer does nothing — the user-visible "click on a
/// URL in a markdown document, nothing happens" bug. This
/// function is the bridge from egui platform output to the OS
/// shell.
///
/// The opener is passed as a `FnMut` closure so the function is
/// pure (no I/O in the function body) and the regression tests
/// in [`os_shell_tests`] can record the dispatched URLs without
/// actually invoking the system browser.
pub fn dispatch_platform_commands<F>(commands: &[egui::OutputCommand], mut open_url_fn: F)
where
    F: FnMut(&str),
{
    for cmd in commands {
        if let egui::OutputCommand::OpenUrl(open_url_cmd) = cmd {
            open_url_fn(&open_url_cmd.url);
        }
    }
}

#[cfg(test)]
#[path = "os_shell_tests.rs"]
mod os_shell_tests;
