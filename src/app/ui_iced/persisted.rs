//! Persisted-UI-state persistence for the iced UI.
//!
//! The pure serde model is [`crate::ui::PersistedUiState`] (defined in
//! `src/app/ui/persisted.rs`), unchanged in shape and semantics. This
//! module is the backend around it:
//!
//! - [`save_ui_state`]/[`load_ui_state`] serialize the model behind an
//!   injected [`UiStateStorage`] handler — never against a hardcoded
//!   platform-default location ([RUST-024], [RUST-006]).
//! - [`migrate_legacy_state`] carries the v0→v1 `font_size_scale`
//!   migration: state below [`CURRENT_SCHEMA_VERSION`] gets the legacy
//!   absolute scale cleared and the version bumped.
//! - [`compose_scale_factor`] composes the OS baseline
//!   (`iced::window::scale_factor`, captured at boot) with the persisted
//!   user-chosen multiplier.
//! - [`restore_window_size`]/[`restore_window_position`] map the model's
//!   window fields onto the booted `iced::window::Settings` (`size`/
//!   `position` — verified `iced_core-0.14.0/src/window/settings.rs:35,45`),
//!   and [`apply_close_metrics`] writes the same fields back at close time
//!   (the standby save, then `iced::window::close`).
//!
//! The [`UiStateStorage`] trait deliberately mirrors the
//! [`crate::config::ConfigStorageHandler`] conventions (`Send + Sync`,
//! `Result<_, String>`, in-memory/noop/file implementations) instead of
//! wrapping it: that trait is `AppConfig`-specific (`save_config(&AppConfig)`)
//! and is shared with the orchestrator, so it cannot carry a keyed UI-state
//! payload.
//!
//! Unit tests live in the sibling `persisted_tests.rs` sidecar.

#[cfg(test)]
#[path = "persisted_tests.rs"]
mod tests;

use crate::ui::{CURRENT_SCHEMA_VERSION, PersistedUiState};
use iced::window::Position;
use iced::{Point, Size};
use std::path::{Path, PathBuf};

/// NFR-001/NFR-004 error-code name for failed UI-state saves.
pub const UI_STATE_SAVE_ERROR_CODE: &str = "ui_state.save.failed";

/// NFR-001/NFR-004 error-code name for failed UI-state loads (corrupt
/// payloads are restored to the default; see [`load_ui_state`]).
pub const UI_STATE_LOAD_ERROR_CODE: &str = "ui_state.load.failed";

/// Storage handler for the persisted UI-state snapshot ([RUST-024]).
///
/// The payload is the serde-JSON serialization of [`PersistedUiState`].
pub trait UiStateStorage: Send + Sync {
    /// Persist the JSON payload. Returns the path written, or a
    /// descriptive tag for memory-backed implementations.
    fn save_ui_state(&self, json: &str) -> Result<PathBuf, String>;

    /// Load the previously saved JSON payload. `None` when nothing has
    /// been persisted yet; `Some(Err(_))` when the payload could not be
    /// read (callers treat both as "restore the default").
    fn load_ui_state(&self) -> Option<Result<String, String>>;
}

/// A [`UiStateStorage`] implementation that discards writes.
///
/// Mirrors [`crate::config::NoopConfigStorage`]; used to guarantee no files
/// are touched.
#[derive(Debug, Clone, Default)]
pub struct NoopUiStateStorage;

impl UiStateStorage for NoopUiStateStorage {
    fn save_ui_state(&self, _json: &str) -> Result<PathBuf, String> {
        Ok(PathBuf::new())
    }

    fn load_ui_state(&self) -> Option<Result<String, String>> {
        None
    }
}

/// A [`UiStateStorage`] implementation that records the last saved payload
/// in memory. Useful for unit tests and in-process pilot runs; the payload
/// does not survive process restart.
#[derive(Debug, Default)]
pub struct InMemoryUiStateStorage {
    saved: std::sync::Mutex<Option<String>>,
}

impl InMemoryUiStateStorage {
    /// Create a new empty in-memory UI-state storage.
    pub fn new() -> Self {
        Self::default()
    }

    /// Retrieve the last saved JSON payload, if any.
    pub fn saved_json(&self) -> Option<String> {
        self.saved.lock().unwrap().clone()
    }
}

impl UiStateStorage for InMemoryUiStateStorage {
    fn save_ui_state(&self, json: &str) -> Result<PathBuf, String> {
        *self.saved.lock().unwrap() = Some(json.to_owned());
        Ok(PathBuf::from("in-memory"))
    }

    fn load_ui_state(&self) -> Option<Result<String, String>> {
        self.saved.lock().unwrap().clone().map(Ok::<String, String>)
    }
}

/// A [`UiStateStorage`] implementation that writes the payload to an
/// explicitly injected file path. Mirrors
/// [`crate::config::FileConfigStorage`]: the path is always provided by the
/// caller (a `tempfile::TempDir` fixture in tests — [RUST-006]).
#[derive(Debug, Clone)]
pub struct FileUiStateStorage {
    path: PathBuf,
}

impl FileUiStateStorage {
    /// Create a new [`FileUiStateStorage`] pointing at `path`.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Returns the path the payload is written to.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Resolves the platform-default UI-state file location, mirroring
    /// [`crate::config::get_config_path`] (`%APPDATA%\fastmd\` /
    /// `~/.fastmd/`).
    ///
    /// # Safety Guard ([RUST-006], [RUST-024])
    /// Panics inside test environments: production wiring must resolve this
    /// at launch; tests must use an injected `tempfile::TempDir` path via
    /// [`FileUiStateStorage::new`] instead.
    pub fn platform_default() -> Self {
        #[cfg(test)]
        panic!(
            "FATAL: FileUiStateStorage::platform_default() was called in a test context. \
             Persisted-UI-state tests must use FileUiStateStorage::new with a temporary \
             path (RUST-006)."
        );
        #[allow(unreachable_code)]
        {
            if std::env::var("NEXTEST").is_ok() || std::env::var("FASTMD_IS_TESTING").is_ok() {
                panic!(
                    "FATAL: FileUiStateStorage::platform_default() was called under a test \
                     runner. Tests must use FileUiStateStorage::new with a temporary path \
                     (RUST-006)."
                );
            }
            let base = if let Ok(app_data) = std::env::var("APPDATA") {
                PathBuf::from(app_data).join("fastmd")
            } else if let Ok(user_profile) = std::env::var("USERPROFILE") {
                PathBuf::from(user_profile).join(".fastmd")
            } else {
                PathBuf::from(".")
            };
            Self::new(base.join(UI_STATE_FILENAME))
        }
    }
}

/// File name of the platform-default persisted UI-state snapshot,
/// written by [`FileUiStateStorage::platform_default`].
pub const UI_STATE_FILENAME: &str = "ui_state.json";

impl UiStateStorage for FileUiStateStorage {
    fn save_ui_state(&self, json: &str) -> Result<PathBuf, String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create UI-state parent dir {}: {error}",
                    parent.display()
                )
            })?;
        }
        std::fs::write(&self.path, json).map_err(|error| {
            format!(
                "failed to write UI state to {}: {error}",
                self.path.display()
            )
        })?;
        Ok(self.path.clone())
    }

    fn load_ui_state(&self) -> Option<Result<String, String>> {
        if !self.path.exists() {
            return None;
        }
        Some(std::fs::read_to_string(&self.path).map_err(|error| {
            format!(
                "failed to read UI state from {}: {error}",
                self.path.display()
            )
        }))
    }
}

/// Serializes `state` to JSON and persists it through the injected
/// `handler` ([RUST-024]).
///
/// Pure apart from the storage side effect; failures are returned, never
/// raised, so callers can produce the required [NFR-001] ERROR log.
pub fn save_ui_state(
    handler: &dyn UiStateStorage,
    state: &PersistedUiState,
) -> Result<PathBuf, String> {
    let json = serde_json::to_string(state)
        .map_err(|error| format!("failed to serialise PersistedUiState: {error}"))?;
    handler.save_ui_state(&json)
}

/// Loads the persisted UI state through the injected `handler`, restoring
/// the default on absence (`None`), unreadable payloads, corrupt JSON, or a
/// legacy schema version. Never panics on corruption: a bad snapshot costs
/// the saved layout, nothing more.
///
/// Logs load failures as [NFR-001] ERROR (with
/// [`UI_STATE_LOAD_ERROR_CODE`]).
pub fn load_ui_state(handler: &dyn UiStateStorage) -> PersistedUiState {
    match handler.load_ui_state() {
        None => PersistedUiState::default(),
        Some(Err(error)) => {
            tracing::error!(
                name = UI_STATE_LOAD_ERROR_CODE,
                %error,
                "Failed to read persisted UI state; using defaults"
            );
            PersistedUiState::default()
        }
        Some(Ok(json)) => match serde_json::from_str::<PersistedUiState>(&json) {
            Ok(legacy) => migrate_legacy_state(legacy),
            Err(error) => {
                tracing::error!(
                    name = UI_STATE_LOAD_ERROR_CODE,
                    %error,
                    "Failed to parse persisted UI state; using defaults"
                );
                PersistedUiState::default()
            }
        },
    }
}

/// Applies the schema-version migration: any state below
/// [`CURRENT_SCHEMA_VERSION`] gets the legacy `font_size_scale` cleared
/// (schema v0 stored the absolute OS ppp, not a multiplier — see the
/// history in `src/app/ui/persisted.rs`) and the version bumped so the
/// payload round-trips as current on the next save. Newer payloads are
/// preserved verbatim so a future migration can read them.
pub fn migrate_legacy_state(mut state: PersistedUiState) -> PersistedUiState {
    if state.schema_version < CURRENT_SCHEMA_VERSION {
        tracing::warn!(
            from = state.schema_version,
            to = CURRENT_SCHEMA_VERSION,
            "migrating persisted UI state: clearing legacy font_size_scale"
        );
        state.font_size_scale = None;
        state.schema_version = CURRENT_SCHEMA_VERSION;
    }
    state
}

/// Composes the UI scale factor out of the OS-reported baseline and
/// the persisted user-chosen multiplier: the multiplier is applied
/// *on top of* the baseline, `None` keeping the baseline verbatim (so legacy
/// absolute-ppp values migrated in [`migrate_legacy_state`] cannot
/// compound).
///
/// [RUST-020]: pure.
pub fn compose_scale_factor(os_baseline: f32, persisted_multiplier: Option<f32>) -> f32 {
    os_baseline * persisted_multiplier.unwrap_or(1.0)
}

/// Snapshots the closing window's metrics into the model's window fields
/// (the standby save at close). The window width/height are
/// logical points (`iced::window::size(id) -> Task<Size>`); the position is
/// applied only when the runtime reported one (`Some(point)`), leaving
/// previously-mapped coordinates untouched otherwise.
///
/// Mutates `state` only; no I/O ([RUST-020], the handler call happens in the
/// caller's update reduction).
pub fn apply_close_metrics(state: &mut PersistedUiState, size: Size, position: Option<Point>) {
    state.window_width = Some(size.width);
    state.window_height = Some(size.height);
    if let Some(point) = position {
        state.window_x = Some(point.x);
        state.window_y = Some(point.y);
    }
}

/// Maps the persisted window size onto the booted `iced::window::Settings`
/// size field. `None` when either dimension was never persisted — the
/// caller's own default (1000×700, `WINDOW_WIDTH_POINTS` /
/// `WINDOW_HEIGHT_POINTS` in [`crate::ui_iced::app`]) applies.
///
/// [RUST-020]: pure.
pub fn restore_window_size(state: &PersistedUiState) -> Option<Size> {
    Some(Size::new(state.window_width?, state.window_height?))
}

/// Maps the persisted window position onto the booted
/// `iced::window::Position`. Partial coordinates (only one
/// axis persisted) fall back to [`Position::Default`] — mapping semantics of
/// never silently inventing the missing axis.
///
/// [RUST-020]: pure.
pub fn restore_window_position(state: &PersistedUiState) -> Position {
    match (state.window_x, state.window_y) {
        (Some(x), Some(y)) => Position::Specific(Point::new(x, y)),
        _ => Position::Default,
    }
}
