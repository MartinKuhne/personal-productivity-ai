//! Unit tests for `persisted.rs` ([TASK-009]) — the persisted-UI-state
//! save/load backend, DEC-008 scale composition, and window-field mapping.
//!
//! Per [RUST-006] every persistence path runs against either
//! `InMemoryUiStateStorage` or a `tempfile::TempDir` file; no test touches a
//! real user path.

use super::*;
use crate::ui::PersistedUiState;
use std::collections::HashSet;
use std::path::PathBuf;

/// Handler whose save always fails and whose load always reports an
/// unreadable payload — exercises the failure modes without any I/O.
struct FailingUiStateStorage;

impl UiStateStorage for FailingUiStateStorage {
    fn save_ui_state(&self, _json: &str) -> Result<PathBuf, String> {
        Err("simulated write failure".to_owned())
    }

    fn load_ui_state(&self) -> Option<Result<String, String>> {
        Some(Err("simulated read failure".to_owned()))
    }
}

fn populated_state() -> PersistedUiState {
    let mut state = PersistedUiState {
        schema_version: CURRENT_SCHEMA_VERSION,
        left_panel_width: Some(280.0),
        right_panel_width: Some(200.0),
        window_width: Some(1200.0),
        window_height: Some(800.0),
        window_x: Some(96.0),
        window_y: Some(48.0),
        font_size_scale: Some(1.25),
        ..Default::default()
    };
    let mut expanded_dirs = HashSet::new();
    expanded_dirs.insert(PathBuf::from("/notes/work"));
    state.expanded_dirs = expanded_dirs;
    state
}

/// Field-by-field equality for [`PersistedUiState`] (the model has no
/// `PartialEq` derive; its semantics stay untouched per [TASK-009] rule 1).
/// `expand_sets_equal` controls whether `expanded_dirs` is compared too.
fn assert_same_state(a: &PersistedUiState, b: &PersistedUiState) {
    assert_eq!(a.schema_version, b.schema_version);
    assert_eq!(a.left_panel_width, b.left_panel_width);
    assert_eq!(a.right_panel_width, b.right_panel_width);
    assert_eq!(a.window_width, b.window_width);
    assert_eq!(a.window_height, b.window_height);
    assert_eq!(a.window_x, b.window_x);
    assert_eq!(a.window_y, b.window_y);
    assert_eq!(a.font_size_scale, b.font_size_scale);
    assert_eq!(a.expanded_dirs.len(), b.expanded_dirs.len());
}

fn assert_default_state(state: &PersistedUiState) {
    assert_same_state(state, &PersistedUiState::default());
}

#[test]
fn test_save_and_load_roundtrip_with_in_memory_storage() {
    let storage = InMemoryUiStateStorage::new();
    let state = populated_state();

    let written = save_ui_state(&storage, &state);
    assert!(written.is_ok(), "save must succeed on the happy path");

    let json = storage.saved_json().expect("payload saved in memory");
    assert_eq!(json, serde_json::to_string(&state).unwrap());

    let restored = load_ui_state(&storage);
    assert_same_state(&restored, &state);
    assert!(
        restored
            .expanded_dirs
            .contains(&PathBuf::from("/notes/work"))
    );
}

#[test]
fn test_in_memory_storage_overwrites_previous_payload() {
    let storage = InMemoryUiStateStorage::new();
    let mut state = populated_state();

    assert!(save_ui_state(&storage, &state).is_ok());
    state.window_width = Some(1400.0);
    assert!(save_ui_state(&storage, &state).is_ok());

    let restored = load_ui_state(&storage);
    assert_eq!(restored.window_width, Some(1400.0));
    assert!(
        storage.saved_json().unwrap().contains("1400"),
        "the latest payload replaces the previous one"
    );
}

#[test]
fn test_load_on_empty_in_memory_storage_returns_default() {
    let storage = InMemoryUiStateStorage::new();
    assert_default_state(&load_ui_state(&storage));
}

#[test]
fn test_load_on_noop_storage_returns_default() {
    assert_default_state(&load_ui_state(&NoopUiStateStorage));
}

#[test]
fn test_save_through_noop_storage_succeeds_and_discards() {
    let storage = NoopUiStateStorage;
    let written = save_ui_state(&storage, &populated_state());
    assert!(written.is_ok(), "noop save must not fail");
    assert!(
        storage.load_ui_state().is_none(),
        "noop storage never yields a payload"
    );
}

#[test]
fn test_file_storage_roundtrip_in_temp_dir() {
    let temp = tempfile::TempDir::new().unwrap();
    let path = temp.path().join("ui_state.json");
    let storage = FileUiStateStorage::new(path.clone());

    let written = save_ui_state(&storage, &populated_state()).unwrap();
    assert_eq!(written, path);
    assert!(storage.path().exists());

    let restored = load_ui_state(&storage);
    assert_same_state(&restored, &populated_state());
    assert!(
        restored
            .expanded_dirs
            .contains(&PathBuf::from("/notes/work"))
    );
}

#[test]
fn test_file_storage_creates_parent_directories() {
    let temp = tempfile::TempDir::new().unwrap();
    let nested = temp.path().join("state").join("ui_state.json");
    let storage = FileUiStateStorage::new(nested);

    assert!(save_ui_state(&storage, &populated_state()).is_ok());
}

#[test]
fn test_file_storage_load_on_missing_file_returns_default() {
    let temp = tempfile::TempDir::new().unwrap();
    let storage = FileUiStateStorage::new(temp.path().join("absent.json"));

    assert!(
        storage.load_ui_state().is_none(),
        "absent payload must read as None (no saved state)"
    );
    assert_default_state(&load_ui_state(&storage));
}

#[test]
fn test_corrupt_payload_restores_default() {
    let temp = tempfile::TempDir::new().unwrap();
    let path = temp.path().join("ui_state.json");

    // A garbage payload simulates a corrupted snapshot (disk failure /
    // truncated write): loading must not panic (FMD-064) and must restore
    // the default state, logging [NFR-001].
    std::fs::write(&path, "this is not json{{{{").expect("temp file write");
    let storage = FileUiStateStorage::new(path);

    assert_default_state(&load_ui_state(&storage));
}

#[test]
fn test_unreadable_payload_restores_default() {
    assert_default_state(&load_ui_state(&FailingUiStateStorage));
}

#[test]
fn test_save_failure_is_returned_not_raised() {
    let save_result = save_ui_state(&FailingUiStateStorage, &populated_state());
    assert_eq!(
        save_result,
        Err("simulated write failure".to_owned()),
        "failures propagate to the caller for the NFR-001 ERROR log"
    );
}

#[test]
fn test_legacy_v0_state_is_migrated() {
    let mut legacy = PersistedUiState::default();
    legacy.schema_version = 0;
    legacy.font_size_scale = Some(1.5);

    let migrated = migrate_legacy_state(legacy);
    assert_eq!(migrated.schema_version, CURRENT_SCHEMA_VERSION);
    assert_eq!(
        migrated.font_size_scale, None,
        "v0 held the absolute OS ppp; the multiplier must be cleared"
    );
}

#[test]
fn test_current_version_state_is_unmigrated() {
    let state = populated_state();
    let migrated = migrate_legacy_state(state.clone());
    assert_same_state(&migrated, &state);
}

#[test]
fn test_future_version_state_is_preserved_verbatim() {
    let mut state = populated_state();
    state.schema_version = CURRENT_SCHEMA_VERSION + 5;
    let migrated = migrate_legacy_state(state.clone());
    assert_same_state(&migrated, &state);
}

#[test]
fn test_compose_scale_factor_multiplies_baseline_and_multiplier() {
    // f32: 1.5 × 1.2 = 1.8000001 (1.2 is not exactly representable), so
    // compare with a tolerance instead of exact equality.
    let composed = compose_scale_factor(1.5, Some(1.2));
    assert!(
        (composed - 1.8).abs() < 1e-6,
        "expected ≈1.8, got {composed}"
    );
}

#[test]
fn test_compose_scale_factor_without_multiplier_keeps_baseline() {
    assert_eq!(compose_scale_factor(1.5, None), 1.5);
    assert_eq!(compose_scale_factor(1.0, Some(1.0)), 1.0);
    assert_eq!(compose_scale_factor(1.0, None), 1.0);
}

#[test]
fn test_compose_scale_factor_does_not_compound_after_migration() {
    // DEC-008 regression shape: v0's absolute-ppp bug re-applied 1.5 on
    // top of 1.5 (compound 2.25). After the v1 migration the persisted
    // multiplier is the ONLY source of additional scale.
    let baseline = 1.5;
    let migrated = migrate_legacy_state(PersistedUiState {
        schema_version: 0,
        font_size_scale: Some(1.5),
        ..Default::default()
    });
    assert_eq!(
        compose_scale_factor(baseline, migrated.font_size_scale),
        1.5,
        "migrated state must not compound into the baseline"
    );
}

#[test]
fn test_apply_close_metrics_records_size_and_position() {
    let mut state = PersistedUiState::default();
    apply_close_metrics(
        &mut state,
        Size::new(1234.0, 567.0),
        Some(Point::new(12.0, 34.0)),
    );
    assert_eq!(state.window_width, Some(1234.0));
    assert_eq!(state.window_height, Some(567.0));
    assert_eq!(state.window_x, Some(12.0));
    assert_eq!(state.window_y, Some(34.0));
}

#[test]
fn test_apply_close_metrics_keeps_position_when_runtime_reports_none() {
    let mut state = PersistedUiState {
        window_x: Some(5.0),
        window_y: Some(6.0),
        ..Default::default()
    };
    apply_close_metrics(&mut state, Size::new(800.0, 600.0), None);
    assert_eq!(state.window_width, Some(800.0));
    assert_eq!(state.window_height, Some(600.0));
    assert_eq!(state.window_x, Some(5.0));
    assert_eq!(state.window_y, Some(6.0));
}

#[test]
fn test_restore_window_size_from_persisted_fields() {
    assert_eq!(
        restore_window_size(&populated_state()),
        Some(Size::new(1200.0, 800.0))
    );
}

#[test]
fn test_restore_window_size_partial_fields_yields_none() {
    let partial = PersistedUiState {
        window_width: Some(1000.0),
        ..Default::default()
    };
    assert_eq!(
        restore_window_size(&partial),
        None,
        "a half-persisted size must not be invented from one axis"
    );
}

#[test]
fn test_restore_window_position_full_fields_map_to_specific() {
    let position = restore_window_position(&populated_state());
    assert!(
        matches!(position, Position::Specific(_)),
        "a fully persisted position rounds to Position::Specific"
    );
}

#[test]
fn test_restore_window_position_partial_or_absent_fields_map_to_default() {
    let absent = PersistedUiState::default();
    assert!(matches!(
        restore_window_position(&absent),
        Position::Default
    ));

    let partial = PersistedUiState {
        window_x: Some(17.0),
        ..Default::default()
    };
    assert!(matches!(
        restore_window_position(&partial),
        Position::Default
    ));
}

/// [RUST-006] runtime shield: the platform-default constructor must panic
/// inside test contexts so no test can ever write a production state file.
#[test]
#[should_panic(expected = "FATAL")]
fn test_platform_default_constructor_is_shielded_in_tests() {
    let _ = FileUiStateStorage::platform_default();
}

/// Garbage that is not JSON is classified by serde as a parse failure (not
/// silently taken as an empty state), which is the failure mode exercised in
/// `test_corrupt_payload_restores_default`.
#[test]
fn test_raw_corrupt_json_fails_to_deserialize() {
    assert!(serde_json::from_str::<PersistedUiState>("this is not json{{{{").is_err());
}
