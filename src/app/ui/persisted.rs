//! UI state persisted across application restarts via `eframe::Storage`.
//!
//! Lives in the `app` module because it is egui-independent data; the
//! UI layer is responsible for serialising it into / restoring it from
//! the framework's storage.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

/// Current on-disk schema version.
///
/// Bump this whenever the meaning or the write-path of any field
/// changes in a backwards-incompatible way. On load,
/// [`PersistedUiState::schema_version`] is compared against
/// [`CURRENT_SCHEMA_VERSION`]; older state is migrated (see
/// `FastMdApp::new`).
///
/// History:
/// - `0` — pre-fix state. The `font_size_scale` field actually
///   held the absolute OS-reported ppp (e.g. `Some(1.5)` on a
///   150% DPI display) and was re-applied on every launch as a
///   multiplier on top of the same ppp, compounding the font
///   size. Migrated to v1 by clearing the field; the user
///   starts at the OS default and may re-apply any preferred
///   zoom through the in-app control (if/when one is added).
/// - `1` — the corrected scale-as-multiplier semantics. The
///   `font_size_scale` field holds the user-chosen multiplier
///   relative to the OS baseline. No field-shape change from v0;
///   only the meaning and write/read paths changed.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// UI state persisted across application restarts via `eframe::Storage`.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct PersistedUiState {
    /// On-disk schema version (see [`CURRENT_SCHEMA_VERSION`]).
    /// Defaults to `0` so state written by builds that pre-date
    /// the schema-version field round-trips as v0 and gets
    /// migrated on the next launch.
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub left_panel_width: Option<f32>,
    #[serde(default)]
    pub right_panel_width: Option<f32>,
    #[serde(default)]
    pub window_width: Option<f32>,
    #[serde(default)]
    pub window_height: Option<f32>,
    #[serde(default)]
    pub window_x: Option<f32>,
    #[serde(default)]
    pub window_y: Option<f32>,
    #[serde(default)]
    pub font_size_scale: Option<f32>,
    #[serde(default)]
    pub expanded_dirs: HashSet<PathBuf>,
    /// App version the About dialog was last shown for (`None` = never
    /// shown, including state written before this field existed). Drives
    /// the first-run auto-show (spec FR-016): fresh or upgraded versions
    /// open the dialog once; the same version stays quiet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about_shown_for_version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_empty() {
        let s = PersistedUiState::default();
        assert_eq!(s.schema_version, 0);
        assert!(s.left_panel_width.is_none());
        assert!(s.right_panel_width.is_none());
        assert!(s.window_width.is_none());
        assert!(s.window_height.is_none());
        assert!(s.window_x.is_none());
        assert!(s.window_y.is_none());
        assert!(s.font_size_scale.is_none());
        assert!(s.expanded_dirs.is_empty());
    }

    #[test]
    fn test_round_trip_serialization_preserves_state() {
        let mut s = PersistedUiState {
            schema_version: CURRENT_SCHEMA_VERSION,
            left_panel_width: Some(250.0),
            right_panel_width: Some(300.0),
            window_width: Some(1200.0),
            window_height: Some(800.0),
            window_x: Some(100.0),
            window_y: Some(50.0),
            font_size_scale: Some(1.2),
            ..Default::default()
        };
        s.expanded_dirs.insert(PathBuf::from("/notes/work"));
        s.expanded_dirs.insert(PathBuf::from("/notes/personal"));

        let json = serde_json::to_string(&s).unwrap();
        let restored: PersistedUiState = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(restored.left_panel_width, Some(250.0));
        assert_eq!(restored.right_panel_width, Some(300.0));
        assert_eq!(restored.window_width, Some(1200.0));
        assert_eq!(restored.window_height, Some(800.0));
        assert_eq!(restored.window_x, Some(100.0));
        assert_eq!(restored.window_y, Some(50.0));
        assert_eq!(restored.font_size_scale, Some(1.2));
        assert_eq!(restored.expanded_dirs.len(), 2);
        assert!(
            restored
                .expanded_dirs
                .contains(&PathBuf::from("/notes/work"))
        );
    }

    #[test]
    fn test_empty_json_object_deserialises_to_default() {
        // Older builds may have written an empty `{}`; make sure the
        // current struct (with `#[serde(default)]`) still parses it.
        let restored: PersistedUiState = serde_json::from_str("{}").unwrap();
        let default = PersistedUiState::default();
        assert_eq!(restored.schema_version, default.schema_version);
        assert_eq!(restored.left_panel_width, default.left_panel_width);
        assert_eq!(restored.right_panel_width, default.right_panel_width);
        assert_eq!(restored.window_width, default.window_width);
        assert_eq!(restored.window_height, default.window_height);
        assert_eq!(restored.window_x, default.window_x);
        assert_eq!(restored.window_y, default.window_y);
        assert_eq!(restored.font_size_scale, default.font_size_scale);
        assert_eq!(restored.expanded_dirs.len(), default.expanded_dirs.len());
    }

    /// State written by the pre-fix build has no `schema_version`
    /// field; it deserialises to `0`. The migration in
    /// `FastMdApp::new` must clear `font_size_scale` for any
    /// version below `CURRENT_SCHEMA_VERSION` so the user is
    /// not silently carrying forward the absolute-ppp value that
    /// the old bug used to compound.
    #[test]
    fn test_pre_fix_state_round_trips_as_v0() {
        // Hand-written JSON mimicking the pre-fix on-disk shape
        // (no `schema_version` field, `font_size_scale` holds the
        // absolute ppp from a 150% DPI display).
        let legacy_json = r#"{
            "left_panel_width": null,
            "right_panel_width": null,
            "window_width": null,
            "window_height": null,
            "window_x": null,
            "window_y": null,
            "font_size_scale": 1.5,
            "expanded_dirs": []
        }"#;
        let restored: PersistedUiState = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(restored.schema_version, 0);
        assert_eq!(restored.font_size_scale, Some(1.5));
        // The migration (in app.rs) is responsible for clearing
        // font_size_scale when schema_version < CURRENT_SCHEMA_VERSION.
        assert!(restored.schema_version < CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn test_corrupted_non_json_input_fails_to_deserialize() {
        // Garbage that is not valid JSON must fail loudly rather than
        // silently yielding a default, so a corrupt state file is
        // surfaced instead of being masked.
        let result: Result<PersistedUiState, _> = serde_json::from_str("this is not json{{{{");
        assert!(result.is_err());
    }

    #[test]
    fn test_future_schema_version_round_trips_unchanged() {
        // State written by a newer build carries a higher schema
        // version; loading it must preserve the version verbatim so the
        // caller can decide how to handle it (rather than clobbering it).
        let s = PersistedUiState {
            schema_version: CURRENT_SCHEMA_VERSION + 5,
            left_panel_width: Some(100.0),
            ..Default::default()
        };
        let json = serde_json::to_string(&s).unwrap();
        let restored: PersistedUiState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.schema_version, CURRENT_SCHEMA_VERSION + 5);
        assert_eq!(restored.left_panel_width, Some(100.0));
    }

    #[test]
    fn test_nan_and_infinite_widths_round_trip_as_null() {
        // serde_json serialises non-finite floats as JSON `null`, so a
        // NaN/Inf width round-trips to `None` rather than corrupting
        // the state file or panicking.
        let nan_state = PersistedUiState {
            left_panel_width: Some(f32::NAN),
            ..Default::default()
        };
        let json = serde_json::to_string(&nan_state).unwrap();
        let restored: PersistedUiState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.left_panel_width, None);

        let inf_state = PersistedUiState {
            window_width: Some(f32::INFINITY),
            ..Default::default()
        };
        let json = serde_json::to_string(&inf_state).unwrap();
        let restored: PersistedUiState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.window_width, None);
    }

    #[test]
    fn test_non_finite_json_width_is_rejected() {
        // A JSON payload containing an explicitly non-finite float must
        // also fail to deserialize.
        let json = r#"{"left_panel_width": NaN}"#;
        let result: Result<PersistedUiState, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_about_shown_for_version_round_trips() {
        // Spec FR-016: the first-run version stamp defaults to unseen,
        // survives a serde round-trip, and is absent (not an error) in
        // state written before the field existed.
        let default = PersistedUiState::default();
        assert_eq!(default.about_shown_for_version, None);

        let mut s = PersistedUiState {
            about_shown_for_version: Some("0.2.0".to_owned()),
            ..Default::default()
        };
        let json = serde_json::to_string(&s).unwrap();
        let restored: PersistedUiState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.about_shown_for_version, Some("0.2.0".to_owned()));

        s.about_shown_for_version = None;
        let json = serde_json::to_string(&s).unwrap();
        assert!(
            !json.contains("about_shown_for_version"),
            "unseen state should omit the field (skip_serializing_if); got {json}"
        );

        let legacy_json = r#"{"left_panel_width": null, "expanded_dirs": []}"#;
        let legacy: PersistedUiState = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(
            legacy.about_shown_for_version, None,
            "pre-field state must deserialize to unseen, triggering one post-upgrade auto-show"
        );
    }
}
