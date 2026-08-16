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
}
