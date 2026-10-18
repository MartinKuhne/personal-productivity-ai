//! Unit tests for the [TASK-024] production entry point (`src/app/main.rs`).
//!
//! `main()` routes to `app::launch()`, which calls `.run()` — not headlessly
//! testable, and no GUI-launch test is invented here. What *is* testable is the
//! piece the entry flip introduced: the persisted-UI-state location policy
//! ([RUST-024], DEC-006) and a compile-time pin that the entry's target really
//! is the iced launch path.
//!
//! No test touches a real user path: the override branch is pointed at a
//! `tempfile::TempDir`, and the platform-default branch is only ever asserted
//! as a *decision*, never resolved ([RUST-006]).

use super::*;

/// A non-empty `FASTMD_ICED_UI_STATE_PATH` selects that explicit file.
#[test]
fn non_empty_override_selects_the_explicit_file() {
    let source = resolve_ui_state_source(Some("M:\\scratch\\ui_state.json".to_string()));

    assert_eq!(
        source,
        UiStateSource::Override(PathBuf::from("M:\\scratch\\ui_state.json"))
    );
}

/// An empty override is not an override — it falls back, matching
/// `app::ui_state_storage_from_env`'s rule so the pilot and the shipping
/// binary agree on what counts as "set".
#[test]
fn empty_override_is_not_an_override() {
    assert_eq!(
        resolve_ui_state_source(Some(String::new())),
        UiStateSource::PlatformDefault
    );
}

/// A missing override falls back to the platform-default location.
#[test]
fn missing_override_selects_platform_default() {
    assert_eq!(
        resolve_ui_state_source(None),
        UiStateSource::PlatformDefault
    );
}

/// The override branch really builds a *file* storage at the injected path:
/// save lands on that exact file and loads back byte-for-byte ([RUST-024]).
#[test]
fn override_source_builds_a_file_storage_at_that_path() {
    let dir = tempfile::tempdir().expect("tempdir for injected UI state");
    let path = dir.path().join("ui_state.json");
    let payload = r#"{"window_width":1234.0}"#;

    let storage = ui_state_storage_for(UiStateSource::Override(path.clone()));
    let saved = storage
        .save_ui_state(payload)
        .expect("save via injected handler");

    assert_eq!(saved, path, "storage writes to the injected path");
    assert_eq!(
        std::fs::read_to_string(&path).expect("file exists"),
        payload,
        "payload is persisted verbatim"
    );
    assert_eq!(
        storage
            .load_ui_state()
            .map(|loaded| loaded.expect("readable")),
        Some(payload.to_string()),
        "round-trip through the injected handler"
    );
}

/// Compile-time pin for the entry wiring: `main()` calls `app::launch` with a
/// `LaunchInputs` and propagates its `iced::Result`. If the entry target's
/// signature drifts, this stops compiling.
#[test]
fn entry_target_is_the_iced_launch_path() {
    let _launch: fn(LaunchInputs) -> iced::Result = app::launch;
}
