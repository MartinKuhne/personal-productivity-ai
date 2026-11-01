//! Unit tests for the iced platform-shell helpers and the [`OsShell`] seam in
//! `os_shell.rs` ([RUST-003]).
//!
//! Each free helper is a one-line `opener` wrapper whose result is discarded, so
//! there is exactly one code path per function and nothing to branch on.
//! **No test here actually launches an OS process** — a real open/reveal is
//! an un-cancellable external side effect and is not test-environment safe
//! ([RUST-006]). That is also why [`OpenerShell`] is only ever *named* below,
//! never called: the recording double is what gets driven.
//!
//! What IS pinned is the property these helpers must keep: every signature is
//! **framework-free**. The `fn`-pointer assignments below fail to compile the
//! moment a framework type leaks into a parameter or return position, which is a
//! stronger check than a runtime assertion. The trait-object tests then pin the
//! other half: the seam stays object-safe and `Send + Sync`, so it can live on
//! `AppState` behind an `Arc`.

use std::sync::Arc;

use super::*;

/// [`open_in_system_editor`] takes exactly a `&Path` and returns `()`.
#[test]
fn open_in_system_editor_signature_is_framework_free() {
    let _f: fn(&Path) = open_in_system_editor;
}

/// [`show_in_file_explorer`] takes exactly a `&Path` and returns `()`.
#[test]
fn show_in_file_explorer_signature_is_framework_free() {
    let _f: fn(&Path) = show_in_file_explorer;
}

/// [`open_url`] takes exactly a `&str` and returns `()`.
#[test]
fn open_url_signature_is_framework_free() {
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

// ───────────────────────────── the `OsShell` seam ─────────────────────────────

/// Both implementations must be usable as `Arc<dyn OsShell>`, because that is
/// the type `AppState` holds. This fails to compile the moment the trait stops
/// being object-safe or loses `Send + Sync`.
#[test]
fn both_implementations_are_object_safe_and_thread_safe() {
    let _production: Arc<dyn OsShell> = Arc::new(OpenerShell);
    let _recording: Arc<dyn OsShell> = Arc::new(RecordingOsShell::new());

    fn assert_send_sync<T: Send + Sync>(_: &T) {}
    assert_send_sync(&OpenerShell);
    assert_send_sync(&RecordingOsShell::new());
}

/// The recorder must capture all three effects, in call order, with their
/// arguments intact — this is the whole reason the seam exists: a reducer test
/// asserts the intent it produced ([RUST-006]).
#[test]
fn recording_shell_captures_every_effect_in_call_order() {
    let shell = RecordingOsShell::new();
    let editor = Path::new("/notes/a.md");
    let explorer = Path::new("/notes/b.md");

    shell.open_url("https://example.com");
    shell.open_in_system_editor(editor);
    shell.show_in_file_explorer(explorer);

    assert_eq!(
        shell.calls(),
        vec![
            ShellCall::OpenUrl("https://example.com".to_string()),
            ShellCall::OpenInSystemEditor(editor.to_path_buf()),
            ShellCall::ShowInFileExplorer(explorer.to_path_buf()),
        ],
        "the log must be the calls in order, arguments included"
    );
}

/// Dispatch through the trait object must reach the recorder, not a static
/// copy: `AppState` only ever holds `Arc<dyn OsShell>`, so dyn-dispatch is the
/// path every reducer test takes.
#[test]
fn recording_shell_captures_through_dyn_dispatch() {
    let recorder = Arc::new(RecordingOsShell::new());
    let shell: Arc<dyn OsShell> = recorder.clone();

    shell.open_url("mailto:someone@example.com");

    assert_eq!(
        recorder.calls(),
        vec![ShellCall::OpenUrl("mailto:someone@example.com".to_string())],
    );
}

/// The two path effects must stay distinguishable even for the **same** path:
/// collapsing them would silently turn a "reveal in explorer" into "open in
/// editor" (or the reverse) and no reducer test would notice.
#[test]
fn recording_shell_keeps_the_two_path_effects_distinct() {
    let shell = RecordingOsShell::new();
    let path = Path::new("/notes/a.md");

    shell.open_in_system_editor(path);
    shell.show_in_file_explorer(path);

    let calls = shell.calls();
    assert_eq!(calls.len(), 2);
    assert_ne!(
        calls[0], calls[1],
        "editor and explorer must not collapse into one recorded variant"
    );
    assert_eq!(calls[0], ShellCall::OpenInSystemEditor(path.to_path_buf()));
    assert_eq!(calls[1], ShellCall::ShowInFileExplorer(path.to_path_buf()));
}

/// [`RecordingOsShell::urls`] is the convenience assertion reducer tests use;
/// it must ignore the path effects rather than mislabel them as URLs.
#[test]
fn recording_shell_urls_lists_only_the_url_effects() {
    let shell = RecordingOsShell::new();
    shell.open_in_system_editor(Path::new("/notes/a.md"));
    shell.open_url("https://one.example");
    shell.show_in_file_explorer(Path::new("/notes/b.md"));
    shell.open_url("https://two.example");

    assert_eq!(
        shell.urls(),
        vec![
            "https://one.example".to_string(),
            "https://two.example".to_string()
        ],
    );
}

/// An untouched recorder must report an empty log — the negative assertion that
/// makes "the reducer reached the shell" tests meaningful.
#[test]
fn recording_shell_starts_empty_and_clears() {
    let shell = RecordingOsShell::new();
    assert_eq!(shell.calls(), Vec::<ShellCall>::new());
    assert_eq!(shell.urls(), Vec::<String>::new());

    shell.open_url("https://example.com");
    assert_eq!(shell.calls().len(), 1);

    shell.clear();
    assert_eq!(shell.calls(), Vec::<ShellCall>::new());
}
