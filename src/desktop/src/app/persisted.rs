//! UI state persisted across application restarts via `eframe::Storage`.
//!
//! Lives in the `app` module because it is egui-independent data; the
//! UI layer is responsible for serialising it into / restoring it from
//! the framework's storage.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

/// UI state persisted across application restarts via `eframe::Storage`.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct PersistedUiState {
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
        assert_eq!(restored.left_panel_width, default.left_panel_width);
        assert_eq!(restored.right_panel_width, default.right_panel_width);
        assert_eq!(restored.window_width, default.window_width);
        assert_eq!(restored.window_height, default.window_height);
        assert_eq!(restored.window_x, default.window_x);
        assert_eq!(restored.window_y, default.window_y);
        assert_eq!(restored.font_size_scale, default.font_size_scale);
        assert_eq!(restored.expanded_dirs.len(), default.expanded_dirs.len());
    }
}
