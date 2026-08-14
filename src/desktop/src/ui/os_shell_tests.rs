//! Tests for `ui/os_shell.rs`.
//!
//! Regression coverage for the platform-helper module — see the
//! sibling `os_shell.rs`.

use super::*;
use eframe::egui;
use std::cell::RefCell;

/// Regression: clicking an `egui::Link` widget (every
/// `InlineElem::Link` in the markdown viewer, plus the
/// `<https://…>` autolink form) emits an
/// `OutputCommand::OpenUrl` on the egui `PlatformOutput` for the
/// current frame.
///
/// eframe 0.36's native (winit) runtime does **not** process
/// `OutputCommand::OpenUrl` — only the `web` target handles it
/// (see `eframe/src/web/app_runner.rs:407` and
/// `eframe/src/web/mod.rs:351`). On native, the
/// `pending_full_output.commands` list is collected and dropped
/// without a single consumer touching the `OpenUrl` variant.
///
/// Without an app-level dispatcher the command is silently lost
/// and clicking a hyperlink in the markdown viewer does nothing
/// — the user-visible "click on a URL in a markdown document,
/// nothing happens" bug.
///
/// `dispatch_platform_commands` is the app's bridge from egui
/// platform output to the OS shell. This test pins its
/// contract: every `OpenUrl` command must reach the supplied
/// opener exactly once and in input order; non-URL commands
/// (e.g. `CopyText`) must be ignored; the empty input must be a
/// no-op. The test is a pure function test — no I/O, no egui
/// `Context`, no `opener` invocations.
#[test]
fn dispatch_platform_commands_invokes_opener_for_each_open_url() {
    let opened: RefCell<Vec<String>> = RefCell::new(Vec::new());
    let record = |url: &str| {
        opened.borrow_mut().push(url.to_string());
    };

    let cmds = vec![
        egui::OutputCommand::OpenUrl(egui::OpenUrl::same_tab("https://example.com")),
        egui::OutputCommand::OpenUrl(egui::OpenUrl::same_tab("https://rust-lang.org")),
        // Non-URL command — must be ignored.
        egui::OutputCommand::CopyText("should be ignored".to_string()),
    ];

    dispatch_platform_commands(&cmds, record);

    let opened = opened.borrow();
    assert_eq!(
        opened.len(),
        2,
        "expected exactly 2 URLs to be opened, got {opened:?}"
    );
    assert_eq!(opened[0], "https://example.com");
    assert_eq!(opened[1], "https://rust-lang.org");
}

/// Empty input must be a no-op — relevant for the per-frame call
/// site in `FastMdApp::update_ui`, where most frames have no
/// `OutputCommand::OpenUrl` at all.
#[test]
fn dispatch_platform_commands_is_noop_on_empty_slice() {
    let opened: RefCell<Vec<String>> = RefCell::new(Vec::new());
    let record = |url: &str| {
        opened.borrow_mut().push(url.to_string());
    };

    dispatch_platform_commands(&[], record);

    assert!(
        opened.borrow().is_empty(),
        "no commands must not invoke the opener"
    );
}

/// A slice containing only non-URL commands must not invoke the
/// opener. The dispatcher should pass `CopyText`, `CopyImage`,
/// etc. through unchanged so the egui runtime (which does
/// process clipboard commands) can still see them on the same
/// frame.
#[test]
fn dispatch_platform_commands_skips_non_url_commands() {
    let opened: RefCell<Vec<String>> = RefCell::new(Vec::new());
    let record = |url: &str| {
        opened.borrow_mut().push(url.to_string());
    };

    let cmds = vec![
        egui::OutputCommand::CopyText("hello".to_string()),
        egui::OutputCommand::CopyText("world".to_string()),
    ];
    dispatch_platform_commands(&cmds, record);

    assert!(
        opened.borrow().is_empty(),
        "non-URL commands must not reach the opener, got {:?}",
        opened.borrow()
    );
}

/// The `new_tab` flag on `OpenUrl` is a web-only concept
/// (`true` ⇒ open in a new tab; `false` ⇒ reuse the same tab);
/// on native the same `url` string is what the opener receives.
/// Pin that the dispatcher does not silently drop URLs whose
/// `new_tab` field is set, and does not alter the URL.
#[test]
fn dispatch_platform_commands_forwards_url_string_unchanged() {
    let opened: RefCell<Vec<String>> = RefCell::new(Vec::new());
    let record = |url: &str| {
        opened.borrow_mut().push(url.to_string());
    };

    // `OpenUrl::new_tab` is a web-only flag; on native we still
    // open the same URL.
    let cmds = vec![egui::OutputCommand::OpenUrl(egui::OpenUrl::new_tab(
        "https://example.com/path?query=1",
    ))];
    dispatch_platform_commands(&cmds, record);

    assert_eq!(
        opened.borrow().as_slice(),
        &["https://example.com/path?query=1".to_string()]
    );
}
