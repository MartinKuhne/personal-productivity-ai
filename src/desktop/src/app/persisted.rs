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
    pub expanded_dirs: HashSet<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_empty() {
        let s = PersistedUiState::default();
        assert!(s.left_panel_width.is_none());
        assert!(s.expanded_dirs.is_empty());
    }

    #[test]
    fn test_round_trip_serialization_preserves_state() {
        let mut s = PersistedUiState {
            left_panel_width: Some(250.0),
            ..Default::default()
        };
        s.expanded_dirs.insert(PathBuf::from("/notes/work"));
        s.expanded_dirs.insert(PathBuf::from("/notes/personal"));

        let json = serde_json::to_string(&s).unwrap();
        let restored: PersistedUiState = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.left_panel_width, Some(250.0));
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
        assert_eq!(restored.expanded_dirs.len(), default.expanded_dirs.len());
    }
}
