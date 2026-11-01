//! UI state persisted across application restarts.
//!
//! Lives in the `app` module because it is framework-independent data; the
//! UI layer is responsible for serialising it into / restoring it from
//! the framework's storage.
//!
//! Unit tests live in the sibling `persisted_tests.rs` sidecar.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

/// Current on-disk schema version.
///
/// Bump this whenever the meaning or the write-path of any field
/// changes in a backwards-incompatible way. On load,
/// [`PersistedUiState::schema_version`] is compared against
/// [`CURRENT_SCHEMA_VERSION`]; older state is migrated by
/// [`crate::ui_iced::persisted::migrate_legacy_state`].
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

/// UI state persisted across application restarts.
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
    /// Remembered geometry of the app's overlays — the movable Band-1 utility
    /// cards (`UIF-MOV-001`) and, since defect #6, the size a user dragged a
    /// centred modal to.
    ///
    /// Keyed by the surface's kind and id: `movable:<surface-id>` for a movable
    /// card (see
    /// `hoarfrost::surface::movable::storage_key`) and
    /// `modal:<surface-id>` for a centred modal (see
    /// [`MODAL_GEOMETRY_KEY_PREFIX`](crate::ui_iced::tools_dialog::MODAL_GEOMETRY_KEY_PREFIX)).
    /// The prefix names the kind because the two kinds remember different things:
    /// a movable card remembers where it is, a centred modal only how big it is.
    /// Each holds that crate's canonical `x,y,width,height` text form
    /// (`hoarfrost::surface::movable::MovableGeometry::encode`).
    ///
    /// Stored as the framework's own encoded string rather than four loose
    /// numbers so this model does not re-derive a format the framework
    /// already owns and guarantees lossless across a round trip. A value
    /// that will not decode is treated as absent, which is the same state a
    /// first launch is in — the card simply opens at the framework default.
    ///
    /// `#[serde(default)]` keeps every pre-existing v1 payload readable:
    /// an absent map means "no remembered position", so this addition moves
    /// neither [`CURRENT_SCHEMA_VERSION`] nor the v0→v1 migration.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub overlay_geometry: BTreeMap<String, String>,
}

#[cfg(test)]
#[path = "persisted_tests.rs"]
mod tests;
