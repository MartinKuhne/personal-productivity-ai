//! Unit tests for `persisted.rs` — the persisted-UI-state save/load
//! backend, scale composition, and window-field mapping.
//!
//! Per [RUST-006] every persistence path runs against either
//! `InMemoryUiStateStorage` or a `tempfile::TempDir` file; no test touches a
//! real user path.
//!
//! Cases that differ only in their input — the schema-version bands, the
//! window-field combinations, and the storage handlers that all fall back to
//! the default state — are grouped into one process each. Every check names its
//! case in the failure message, so a merged failure still says which input
//! broke.

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
/// `PartialEq` derive). `expanded_dirs` is compared by length only.
fn assert_same_state(label: &str, a: &PersistedUiState, b: &PersistedUiState) {
    assert_eq!(
        a.schema_version, b.schema_version,
        "{label}: schema_version must match"
    );
    assert_eq!(
        a.left_panel_width, b.left_panel_width,
        "{label}: left_panel_width must match"
    );
    assert_eq!(
        a.right_panel_width, b.right_panel_width,
        "{label}: right_panel_width must match"
    );
    assert_eq!(
        a.window_width, b.window_width,
        "{label}: window_width must match"
    );
    assert_eq!(
        a.window_height, b.window_height,
        "{label}: window_height must match"
    );
    assert_eq!(a.window_x, b.window_x, "{label}: window_x must match");
    assert_eq!(a.window_y, b.window_y, "{label}: window_y must match");
    assert_eq!(
        a.font_size_scale, b.font_size_scale,
        "{label}: font_size_scale must match"
    );
    assert_eq!(
        a.expanded_dirs.len(),
        b.expanded_dirs.len(),
        "{label}: the expanded-directory count must match"
    );
}

fn assert_default_state(label: &str, state: &PersistedUiState) {
    assert_same_state(label, state, &PersistedUiState::default());
}

// --- save / load through the injected handler -------------------------------

#[test]
fn test_save_and_load_roundtrip_with_in_memory_storage() {
    let storage = InMemoryUiStateStorage::new();
    let state = populated_state();

    let written = save_ui_state(&storage, &state);
    assert!(written.is_ok(), "save must succeed on the happy path");

    let json = storage.saved_json().expect("payload saved in memory");
    assert_eq!(json, serde_json::to_string(&state).unwrap());

    let restored = load_ui_state(&storage);
    assert_same_state("in-memory round trip", &restored, &state);
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

/// Every handler that cannot produce a usable payload — nothing saved, writes
/// discarded, or an unreadable read — restores the default state rather than
/// failing the boot.
#[test]
fn test_a_handler_without_a_usable_payload_restores_the_default() {
    let handlers: Vec<(&str, Box<dyn UiStateStorage>)> = vec![
        (
            "empty in-memory storage",
            Box::new(InMemoryUiStateStorage::new()),
        ),
        ("noop storage", Box::new(NoopUiStateStorage)),
        ("unreadable payload", Box::new(FailingUiStateStorage)),
    ];

    for (label, handler) in &handlers {
        assert_default_state(label, &load_ui_state(&**handler));
    }
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
    assert_same_state("file round trip", &restored, &populated_state());
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

/// A file that cannot yield a state — absent, or present but corrupt — reads
/// as the default state instead of failing or panicking.
#[test]
fn test_a_file_that_cannot_yield_a_state_restores_the_default() {
    // Absent payload: nothing has ever been saved.
    let absent_temp = tempfile::TempDir::new().unwrap();
    let absent = FileUiStateStorage::new(absent_temp.path().join("absent.json"));
    assert!(
        absent.load_ui_state().is_none(),
        "absent file: absent payload must read as None (no saved state)"
    );
    assert_default_state("absent file", &load_ui_state(&absent));

    // A garbage payload simulates a corrupted snapshot (disk failure /
    // truncated write): loading must not panic and must restore
    // the default state, logging [NFR-001].
    let corrupt_temp = tempfile::TempDir::new().unwrap();
    let corrupt_path = corrupt_temp.path().join("ui_state.json");
    std::fs::write(&corrupt_path, "this is not json{{{{").expect("temp file write");
    let corrupt = FileUiStateStorage::new(corrupt_path);
    assert!(
        corrupt.load_ui_state().is_some(),
        "corrupt file: the bytes are readable — the defect is the parse, not the read"
    );
    assert_default_state("corrupt file", &load_ui_state(&corrupt));
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

/// Garbage that is not JSON is classified by serde as a parse failure (not
/// silently taken as an empty state), which is the failure mode exercised in
/// `test_a_file_that_cannot_yield_a_state_restores_the_default`.
#[test]
fn test_raw_corrupt_json_fails_to_deserialize() {
    assert!(serde_json::from_str::<PersistedUiState>("this is not json{{{{").is_err());
}

// --- schema migration -------------------------------------------------------

/// The three version bands: below current gets the legacy scale cleared and
/// the version bumped, current is untouched, and above current is preserved
/// verbatim for a future migration to read.
#[test]
fn test_migrate_legacy_state_handles_every_schema_version_band() {
    let current = populated_state();
    let mut future = populated_state();
    future.schema_version = CURRENT_SCHEMA_VERSION + 5;

    let legacy_input = PersistedUiState {
        schema_version: 0,
        font_size_scale: Some(1.5),
        ..PersistedUiState::default()
    };
    let legacy_expected = PersistedUiState {
        schema_version: CURRENT_SCHEMA_VERSION,
        font_size_scale: None,
        ..PersistedUiState::default()
    };

    let cases = [
        ("v0 legacy state", legacy_input, legacy_expected),
        ("current-version state", current.clone(), current.clone()),
        ("future-version state", future.clone(), future.clone()),
    ];
    for (label, input, expected) in cases {
        let migrated = migrate_legacy_state(input);
        assert_same_state(label, &migrated, &expected);
    }

    let migrated_legacy = migrate_legacy_state(PersistedUiState {
        schema_version: 0,
        font_size_scale: Some(1.5),
        ..PersistedUiState::default()
    });
    assert_eq!(
        migrated_legacy.font_size_scale, None,
        "v0 legacy state: v0 held the absolute OS ppp; the multiplier must be cleared"
    );
}

// --- scale composition ------------------------------------------------------

/// The persisted multiplier composes on top of the OS baseline; `None` keeps
/// the baseline verbatim.
#[test]
fn test_compose_scale_factor_multiplies_the_baseline_by_the_multiplier() {
    let cases: &[(&str, f32, Option<f32>, f32)] = &[
        ("no multiplier keeps the baseline", 1.5, None, 1.5),
        ("a 1.0 multiplier is the identity", 1.0, Some(1.0), 1.0),
        ("no multiplier on a 1.0 baseline", 1.0, None, 1.0),
    ];
    for (label, baseline, multiplier, expected) in cases {
        assert_eq!(
            compose_scale_factor(*baseline, *multiplier),
            *expected,
            "{label}: the composition must be exact"
        );
    }

    // f32: 1.5 × 1.2 = 1.8000001 (1.2 is not exactly representable), so
    // compare with a tolerance instead of exact equality.
    let composed = compose_scale_factor(1.5, Some(1.2));
    assert!(
        (composed - 1.8).abs() < 1e-6,
        "a 1.2 multiplier on a 1.5 baseline: expected ≈1.8, got {composed}"
    );
}

#[test]
fn test_compose_scale_factor_does_not_compound_after_migration() {
    // Regression shape: v0's absolute-ppp bug re-applied 1.5 on top of
    // 1.5 (compound 2.25). After the v1 migration the persisted
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

// --- window metrics mapping -------------------------------------------------

/// The closing metrics are recorded into the model; a runtime that reports no
/// position leaves the previously persisted coordinates untouched.
#[test]
fn test_apply_close_metrics_records_size_and_position() {
    let mut recorded = PersistedUiState::default();
    apply_close_metrics(
        &mut recorded,
        Size::new(1234.0, 567.0),
        Some(Point::new(12.0, 34.0)),
    );
    assert_eq!(
        recorded.window_width,
        Some(1234.0),
        "reported position: width"
    );
    assert_eq!(
        recorded.window_height,
        Some(567.0),
        "reported position: height"
    );
    assert_eq!(recorded.window_x, Some(12.0), "reported position: x");
    assert_eq!(recorded.window_y, Some(34.0), "reported position: y");

    let mut kept = PersistedUiState {
        window_x: Some(5.0),
        window_y: Some(6.0),
        ..Default::default()
    };
    apply_close_metrics(&mut kept, Size::new(800.0, 600.0), None);
    assert_eq!(kept.window_width, Some(800.0), "absent position: width");
    assert_eq!(kept.window_height, Some(600.0), "absent position: height");
    assert_eq!(kept.window_x, Some(5.0), "absent position: x is kept");
    assert_eq!(kept.window_y, Some(6.0), "absent position: y is kept");
}

/// A half-persisted size is never invented from a single axis.
#[test]
fn test_restore_window_size_maps_only_a_complete_pair() {
    assert_eq!(
        restore_window_size(&populated_state()),
        Some(Size::new(1200.0, 800.0)),
        "a fully persisted size restores exactly"
    );

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

/// Only a complete coordinate pair becomes `Position::Specific`; anything
/// less falls back to `Position::Default`.
#[test]
fn test_restore_window_position_maps_only_a_complete_pair() {
    assert!(
        matches!(
            restore_window_position(&populated_state()),
            Position::Specific(_)
        ),
        "full pair: a fully persisted position rounds to Position::Specific"
    );

    let absent = PersistedUiState::default();
    assert!(
        matches!(restore_window_position(&absent), Position::Default),
        "absent pair: nothing persisted falls back to Position::Default"
    );

    let partial = PersistedUiState {
        window_x: Some(17.0),
        ..Default::default()
    };
    assert!(
        matches!(restore_window_position(&partial), Position::Default),
        "half pair: a partial position must not invent the missing axis"
    );
}

/// [RUST-006] runtime shield: the platform-default constructor must panic
/// inside test contexts so no test can ever write a production state file.
#[test]
#[should_panic(expected = "FATAL")]
fn test_platform_default_constructor_is_shielded_in_tests() {
    let _ = FileUiStateStorage::platform_default();
}

// ───────────────── INT-6: movable overlay geometry sink ─────────────────────

/// Saves round trip through the injected handler: the stored text is the
/// framework's own encoding, the round trip is lossless, and two cards sharing
/// one model do not clobber each other.
#[test]
fn geometry_saves_round_trip_through_the_injected_handler() {
    // One card: the framework's canonical encoded form is what lands in the
    // model, so a later reader never has to guess a format this crate invented.
    {
        let storage = InMemoryUiStateStorage::new();
        let mut model = PersistedUiState::default();
        let wanted = MovableGeometry::new(Point::new(120.0, 80.0), Size::new(680.0, 440.0));

        UiStateGeometryStorage::new(&mut model, &storage).save("movable:overlay.logs", wanted);

        assert_eq!(
            model
                .overlay_geometry
                .get("movable:overlay.logs")
                .map(String::as_str),
            Some(wanted.encode().as_str()),
            "single card: the stored text must be the framework's own `x,y,width,height` form"
        );

        let restored =
            UiStateGeometryStorage::new(&mut model, &storage).load("movable:overlay.logs");
        assert_eq!(
            restored,
            Some(wanted),
            "single card: round trip must be lossless"
        );
    }

    // Two cards share one model and one handler: a second save must not
    // clobber the first card's entry.
    {
        let storage = InMemoryUiStateStorage::new();
        let mut model = PersistedUiState::default();

        let logs = MovableGeometry::new(Point::new(10.0, 20.0), Size::new(680.0, 440.0));
        let debug = MovableGeometry::new(Point::new(300.0, 400.0), Size::new(700.0, 500.0));

        {
            let mut sink = UiStateGeometryStorage::new(&mut model, &storage);
            sink.save("movable:overlay.logs", logs);
            sink.save("movable:overlay.agent_debug", debug);
        }

        let mut sink = UiStateGeometryStorage::new(&mut model, &storage);
        assert_eq!(
            sink.load("movable:overlay.logs"),
            Some(logs),
            "two cards: the first card's entry survives the second save"
        );
        assert_eq!(
            sink.load("movable:overlay.agent_debug"),
            Some(debug),
            "two cards: the second card's entry lands"
        );
    }
}

/// A save writes through to the handler, not only into the in-memory model —
/// otherwise a crash between settles would lose the position the user just
/// dragged the card to.
#[test]
fn save_writes_through_to_the_handler() {
    let storage = InMemoryUiStateStorage::new();
    let mut model = PersistedUiState::default();

    UiStateGeometryStorage::new(&mut model, &storage)
        .save("movable:overlay.agent_debug", MovableGeometry::DEFAULT);

    let json = storage.saved_json().expect("write must reach the handler");
    assert!(
        json.contains("movable:overlay.agent_debug"),
        "the persisted payload must carry the key, got: {json}"
    );
}

/// Clearing a saved key forgets it; clearing a key that was never saved must
/// not rewrite the payload — a no-op that still hit storage would turn every
/// card close into a write.
#[test]
fn clearing_a_geometry_forgets_it_without_writing_an_absent_key() {
    {
        let storage = InMemoryUiStateStorage::new();
        let mut model = PersistedUiState::default();
        let key = "movable:overlay.logs";

        UiStateGeometryStorage::new(&mut model, &storage).save(key, MovableGeometry::DEFAULT);
        UiStateGeometryStorage::new(&mut model, &storage).clear(key);

        assert_eq!(
            UiStateGeometryStorage::new(&mut model, &storage).load(key),
            None,
            "saved key: the geometry is forgotten"
        );
        assert!(
            model.overlay_geometry.is_empty(),
            "saved key: the model's map is emptied too"
        );
    }

    {
        let storage = InMemoryUiStateStorage::new();
        let mut model = PersistedUiState::default();

        UiStateGeometryStorage::new(&mut model, &storage).clear("movable:overlay.logs");

        assert!(
            storage.saved_json().is_none(),
            "absent key: clearing nothing must not produce a save"
        );
    }
}

/// A value that will not decode reads as absent, so a corrupt entry falls
/// back to the framework default rather than to a garbage rectangle.
#[test]
fn a_corrupt_saved_value_reads_as_absent() {
    let storage = InMemoryUiStateStorage::new();
    let mut model = PersistedUiState::default();
    model.overlay_geometry.insert(
        "movable:overlay.logs".to_owned(),
        "not,a,geometry".to_owned(),
    );

    assert_eq!(
        UiStateGeometryStorage::new(&mut model, &storage).load("movable:overlay.logs"),
        None
    );
}

/// [RUST-024] / [RUST-006]: the sink reaches the filesystem only through
/// the handler it was handed. With the noop handler the write path is a
/// black hole, and no platform-default user location is ever named.
#[test]
fn the_sink_never_names_a_user_profile_path() {
    let mut model = PersistedUiState::default();
    let noop = NoopUiStateStorage;

    {
        let mut sink = UiStateGeometryStorage::new(&mut model, &noop);
        sink.save("movable:overlay.logs", MovableGeometry::DEFAULT);
        sink.save("movable:overlay.agent_debug", MovableGeometry::DEFAULT);
        sink.clear("movable:overlay.logs");
    }

    // The model is pure data; the only thing that could have touched a disk
    // is the handler, and `NoopUiStateStorage` discards every write.
    let serialised = serde_json::to_string(&model).expect("model serialises");
    for forbidden in ["APPDATA", "USERPROFILE", "\\\\.\\"] {
        assert!(
            !serialised.contains(forbidden),
            "persisted geometry must not name {forbidden}"
        );
    }
}

/// A payload written before `overlay_geometry` existed still loads, with the
/// map simply empty — the reason the schema version did not move.
#[test]
fn a_pre_int6_payload_still_loads() {
    let legacy = r#"{"schema_version":1,"left_panel_width":240.0}"#;
    let storage = InMemoryUiStateStorage::new();
    storage.save_ui_state(legacy).expect("seed");

    let loaded = load_ui_state(&storage);
    assert!(
        loaded.overlay_geometry.is_empty(),
        "an absent map reads as empty, not as an error"
    );
}
// -------------------------------------------------------------------
//  Observability: the load/save failure codes ([NFR-001], [NFR-004])
// -------------------------------------------------------------------

use crate::ui_iced::menu_surface::log_capture::{Capture, Captured, with_capture};

/// Find a captured event by its `name` field — the code convention this
/// module uses ([NFR-004]) instead of a dedicated `error_code` field.
fn named(capture: &Capture, code: &str) -> Option<Captured> {
    capture
        .events()
        .into_iter()
        .find(|event| event.field("name") == Some(code))
}

/// An unreadable payload costs the saved layout, nothing more — but it must
/// still surface as an ERROR carrying the load code ([NFR-001], [NFR-004]).
#[test]
fn an_unreadable_payload_reports_the_load_error_code() {
    let capture = Capture::new();
    let state = with_capture(&capture, || load_ui_state(&FailingUiStateStorage));

    assert_default_state("unreadable payload", &state);
    let event = named(&capture, UI_STATE_LOAD_ERROR_CODE)
        .expect("an unreadable payload must report ERROR ([NFR-001])");
    assert_eq!(
        event.level,
        tracing::Level::ERROR,
        "a failed UI-state read is an application failure, got {:?}",
        event.level
    );
    assert_eq!(
        event.field("error"),
        Some("simulated read failure"),
        "the handler's own reason must be carried through, got {:?}",
        event.fields
    );
}

/// Corrupt JSON is a *parse* failure rather than a read failure: it takes the
/// second error arm and must report the same code with the parser's message.
#[test]
fn a_corrupt_payload_reports_the_load_error_code_with_the_parser_message() {
    let payload = "{ this is not json }";
    let storage = InMemoryUiStateStorage::new();
    let _ = storage.save_ui_state(payload);

    let capture = Capture::new();
    let state = with_capture(&capture, || load_ui_state(&storage));

    assert_default_state("corrupt payload", &state);
    let event = named(&capture, UI_STATE_LOAD_ERROR_CODE)
        .expect("corrupt JSON must report ERROR ([NFR-001])");
    assert_eq!(event.level, tracing::Level::ERROR);
    let reason = event.field("error").unwrap_or_else(|| {
        panic!(
            "the parse failure must carry the parser's reason, got fields: {:?}",
            event.fields
        )
    });
    // serde rejects this payload while scanning for the object's first key, so
    // its message is "key must be a string at line 1 column 3" — it names the
    // rejected position and never the word "json". What [NFR-001]/[NFR-004]
    // require is that the parser's own reason reaches the log verbatim, so the
    // expectation is derived from the same payload instead of hard-coding
    // serde's wording (which is not a stability guarantee of the crate).
    let parser_message = serde_json::from_str::<PersistedUiState>(payload)
        .expect_err("the fixture payload must be rejected by serde")
        .to_string();
    assert_eq!(
        reason, parser_message,
        "the ERROR must carry serde's own rejection reason verbatim, got: {reason}"
    );
    assert!(
        reason.contains("line 1 column"),
        "the parser message must point at the position that was rejected, got: {reason}"
    );
}

/// A first launch has no snapshot at all. That is the normal case, not a
/// failure, and must not emit the load error code.
#[test]
fn an_absent_snapshot_is_not_reported_as_a_failure() {
    let capture = Capture::new();
    let state = with_capture(&capture, || load_ui_state(&NoopUiStateStorage));

    assert_default_state("absent snapshot", &state);
    assert!(
        named(&capture, UI_STATE_LOAD_ERROR_CODE).is_none(),
        "a missing snapshot is the fresh-install case, got names: {:?}",
        capture
            .events()
            .iter()
            .filter_map(|event| event.field("name"))
            .collect::<Vec<_>>()
    );
}

/// A geometry write-through that cannot reach storage is a persisted-state
/// loss and must report the save code rather than fail silently ([NFR-001]).
#[test]
fn a_failed_geometry_write_reports_the_save_error_code() {
    let mut model = PersistedUiState::default();
    let capture = Capture::new();

    with_capture(&capture, || {
        UiStateGeometryStorage::new(&mut model, &FailingUiStateStorage)
            .save("movable:overlay.logs", MovableGeometry::DEFAULT)
    });

    let event = named(&capture, UI_STATE_SAVE_ERROR_CODE)
        .expect("a failed geometry write must report ERROR ([NFR-001])");
    assert_eq!(event.level, tracing::Level::ERROR);
    assert_eq!(
        event.field("error"),
        Some("simulated write failure"),
        "the handler's own reason must be carried through, got {:?}",
        event.fields
    );
    assert!(
        model.overlay_geometry.contains_key("movable:overlay.logs"),
        "a failed write-through must not lose the geometry the model already knows"
    );
}

/// The v0 to v1 migration is a one-time, user-visible layout change: it logs
/// a WARN naming both schema versions so an operator can tell why a saved
/// font multiplier disappeared.
#[test]
fn a_legacy_schema_migration_logs_both_versions() {
    let capture = Capture::new();
    let migrated = with_capture(&capture, || {
        migrate_legacy_state(PersistedUiState {
            schema_version: 0,
            font_size_scale: Some(1.5),
            ..PersistedUiState::default()
        })
    });

    assert_eq!(migrated.schema_version, CURRENT_SCHEMA_VERSION);
    assert_eq!(
        migrated.font_size_scale, None,
        "the legacy absolute ppp must be dropped, not reinterpreted as a multiplier"
    );
    // This event logs through the message rather than a `name` code, so the
    // lookup goes by the rendered text.
    let event = capture
        .events()
        .into_iter()
        .find(|event| event.message.contains("migrating persisted UI state"))
        .expect("a schema migration must be logged");
    assert_eq!(
        event.level,
        tracing::Level::WARN,
        "a silent layout change deserves a warning, got {:?}",
        event.level
    );
    assert_eq!(
        event.field("from"),
        Some("0"),
        "the warning must name the schema version it migrated from"
    );
    let to_version = CURRENT_SCHEMA_VERSION.to_string();
    assert_eq!(
        event.field("to"),
        Some(to_version.as_str()),
        "the warning must name the schema version it migrated to"
    );
}

/// A current-version payload is not migrated, so nothing is logged — a
/// migration warning on every single launch would drown the real ones.
#[test]
fn a_current_schema_version_logs_no_migration_warning() {
    let capture = Capture::new();
    let input = PersistedUiState {
        schema_version: CURRENT_SCHEMA_VERSION,
        font_size_scale: Some(1.25),
        ..PersistedUiState::default()
    };

    let migrated = with_capture(&capture, || migrate_legacy_state(input.clone()));

    assert_same_state("current version", &migrated, &input);
    assert!(
        capture.events().is_empty(),
        "no migration means no log, got: {:?}",
        capture.events()
    );
}

// -------------------------------------------------------------------
//  FileUiStateStorage I/O failure arms
// -------------------------------------------------------------------

/// A parent directory that cannot be created is reported as a descriptive
/// `Err`, never a panic, so the caller keeps its [NFR-001] ERROR path.
#[test]
fn a_blocked_parent_directory_is_reported_by_the_file_storage() {
    let temp = tempfile::TempDir::new().unwrap();
    let blocker = temp.path().join("blocker");
    std::fs::write(&blocker, b"x").expect("fixture write");
    let storage = FileUiStateStorage::new(blocker.join("ui_state.json"));

    let result = storage.save_ui_state("{}");
    assert!(
        result.is_err(),
        "a parent blocked by a file cannot be created"
    );
    let message = result.unwrap_err();
    assert!(
        message.contains("failed to create UI-state parent dir"),
        "the error must name the failed step, got: {message}"
    );
}

/// The parent exists but the file itself cannot be written (the target is a
/// directory) — the second error arm of [`FileUiStateStorage::save_ui_state`].
#[test]
fn an_unwritable_target_is_reported_by_the_file_storage() {
    let temp = tempfile::TempDir::new().unwrap();
    let directory = temp.path().join("ui_state.json");
    std::fs::create_dir(&directory).expect("fixture dir");
    let storage = FileUiStateStorage::new(directory);

    let result = storage.save_ui_state("{}");
    assert!(
        result.is_err(),
        "writing over a directory must not silently succeed"
    );
    let message = result.unwrap_err();
    assert!(
        message.contains("failed to write UI state to"),
        "the error must name the failed write, got: {message}"
    );
}

/// A path that exists but cannot be read back yields `Some(Err(..))` — the
/// distinction the loader uses to tell "nothing saved" apart from "unreadable".
#[test]
fn an_unreadable_file_reports_some_err_rather_than_none() {
    let temp = tempfile::TempDir::new().unwrap();
    let directory = temp.path().join("ui_state.json");
    std::fs::create_dir(&directory).expect("fixture dir");
    let storage = FileUiStateStorage::new(directory);

    let loaded = storage.load_ui_state();
    assert!(
        loaded.is_some(),
        "the path exists, so the loader must report an attempt rather than None"
    );
    let error = loaded
        .expect("checked above")
        .expect_err("reading a directory is not a readable payload");
    assert!(
        error.contains("failed to read UI state from"),
        "the error must name the failed read, got: {error}"
    );

    // The free loader turns that into the default state plus an ERROR.
    let capture = Capture::new();
    let state = with_capture(&capture, || {
        let temp = tempfile::TempDir::new().unwrap();
        let directory = temp.path().join("ui_state.json");
        std::fs::create_dir(&directory).expect("fixture dir");
        load_ui_state(&FileUiStateStorage::new(directory))
    });
    assert_default_state("unreadable file", &state);
    assert!(
        named(&capture, UI_STATE_LOAD_ERROR_CODE).is_some(),
        "an unreadable file must report the load error code ([NFR-001])"
    );
}

// -------------------------------------------------------------------
//  Migration reached through the loader
// -------------------------------------------------------------------

/// A v0 payload read back from storage is migrated on the way in: the legacy
/// absolute scale is dropped and the version is bumped, without a second save.
#[test]
fn a_v0_payload_from_storage_is_migrated_on_load() {
    let storage = InMemoryUiStateStorage::new();
    storage
        .save_ui_state(r#"{"schema_version":0,"font_size_scale":1.75,"left_panel_width":240.0}"#)
        .expect("seed payload");

    let loaded = load_ui_state(&storage);
    assert_eq!(
        loaded.schema_version, CURRENT_SCHEMA_VERSION,
        "a v0 payload must be bumped to the current schema on load"
    );
    assert_eq!(
        loaded.font_size_scale, None,
        "the legacy absolute ppp must not survive as a multiplier"
    );
    assert_eq!(
        loaded.left_panel_width,
        Some(240.0),
        "migration must not discard unrelated persisted fields"
    );
}

/// A payload newer than the current schema is preserved verbatim so a future
/// migration can still read it.
#[test]
fn a_future_schema_payload_is_preserved_on_load() {
    let storage = InMemoryUiStateStorage::new();
    storage
        .save_ui_state(r#"{"schema_version":7,"font_size_scale":1.5,"left_panel_width":300.0}"#)
        .expect("seed payload");

    let loaded = load_ui_state(&storage);
    assert_eq!(
        loaded.schema_version, 7,
        "a newer payload must not be down-migrated"
    );
    assert_eq!(
        loaded.font_size_scale,
        Some(1.5),
        "a newer payload must not have its fields cleared"
    );
}

// -------------------------------------------------------------------
//  Remaining partial-input arms
// -------------------------------------------------------------------

/// The height-only half of a persisted size is rejected just like the
/// width-only half.
#[test]
fn restore_window_size_rejects_a_height_only_pair() {
    let partial = PersistedUiState {
        window_height: Some(600.0),
        ..Default::default()
    };
    assert_eq!(
        restore_window_size(&partial),
        None,
        "a height without a width must not be invented into a size"
    );
    assert_eq!(
        restore_window_size(&PersistedUiState::default()),
        None,
        "a completely absent size restores nothing"
    );
}

/// The y-only half of a persisted position falls back to `Position::Default`.
#[test]
fn restore_window_position_rejects_a_y_only_pair() {
    let partial = PersistedUiState {
        window_y: Some(42.0),
        ..Default::default()
    };
    assert!(
        matches!(restore_window_position(&partial), Position::Default),
        "a y without an x must not invent the missing axis"
    );
}

/// `apply_close_metrics` overwrites previously persisted dimensions with the
/// metrics of the window that is actually closing.
#[test]
fn apply_close_metrics_overwrites_previously_persisted_dimensions() {
    let mut state = PersistedUiState {
        window_width: Some(1000.0),
        window_height: Some(700.0),
        window_x: Some(1.0),
        window_y: Some(2.0),
        ..Default::default()
    };
    apply_close_metrics(
        &mut state,
        Size::new(1920.0, 1080.0),
        Some(Point::new(-8.0, -16.0)),
    );
    assert_eq!(state.window_width, Some(1920.0), "width is replaced");
    assert_eq!(state.window_height, Some(1080.0), "height is replaced");
    assert_eq!(
        state.window_x,
        Some(-8.0),
        "x is replaced, negatives included"
    );
    assert_eq!(
        state.window_y,
        Some(-16.0),
        "y is replaced, negatives included"
    );
}

/// Loading a key the model never saw reads as absent rather than as a decode
/// failure — the framework then falls back to its own default geometry.
#[test]
fn loading_an_unknown_geometry_key_reads_as_absent() {
    let storage = InMemoryUiStateStorage::new();
    let mut model = PersistedUiState::default();
    model.overlay_geometry.insert(
        "movable:overlay.logs".to_owned(),
        MovableGeometry::DEFAULT.encode(),
    );

    let mut sink = UiStateGeometryStorage::new(&mut model, &storage);
    assert_eq!(
        sink.load("movable:overlay.never_saved"),
        None,
        "an unknown key must read as absent, not as a corrupt entry"
    );
    assert_eq!(
        sink.load("movable:overlay.logs"),
        Some(MovableGeometry::DEFAULT),
        "the known key still resolves beside the unknown one"
    );
}

/// Clearing a key writes the whole live model through, so a sibling card's
/// geometry survives the removal in the persisted payload.
#[test]
fn clearing_one_key_preserves_sibling_geometry_in_the_payload() {
    let storage = InMemoryUiStateStorage::new();
    let mut model = PersistedUiState::default();
    let logs = MovableGeometry::new(Point::new(1.0, 2.0), Size::new(300.0, 200.0));
    let debug = MovableGeometry::new(Point::new(3.0, 4.0), Size::new(400.0, 300.0));

    {
        let mut sink = UiStateGeometryStorage::new(&mut model, &storage);
        sink.save("movable:overlay.logs", logs);
        sink.save("movable:overlay.agent_debug", debug);
        sink.clear("movable:overlay.logs");
    }

    let json = storage
        .saved_json()
        .expect("the clear must write through to the handler");
    assert!(
        !json.contains("movable:overlay.logs"),
        "the cleared key must be gone from the payload, got: {json}"
    );
    assert!(
        json.contains("movable:overlay.agent_debug"),
        "the sibling key must survive the cleared write-through, got: {json}"
    );
}
