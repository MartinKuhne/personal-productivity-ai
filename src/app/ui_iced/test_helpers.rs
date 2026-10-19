//! Framework-free test fixtures for the iced layer ([TASK-029]).
//!
//! The iced sidecar tests previously borrowed `test_app` /
//! `test_app_with_config` from the retired `ui/test_helpers/app` module, which
//! [TASK-029b] deleted along with the rest of the legacy UI layer. These are
//! the iced-owned replacements.
//!
//! # Framework-free by construction ([TASK-029] verification)
//!
//! Both fixtures build a [`FastMdApp`] through
//! [`FastMdApp::empty_state`], the bus-driven constructor that needs **no**
//! legacy creation context and applies **no** legacy visuals — the same path
//! the retired fixtures already used. This module imports no UI-framework type,
//! and [`test_app_state`] builds a full iced
//! [`AppState`] through [`build_state`], which [TASK-022] verified as the
//! framework-free state constructor.
//!
//! # Test-environment safety ([RUST-006])
//!
//! Nothing here touches a real user path. [`test_app_state`] wires
//! `InMemoryConfigStorage` and `InMemoryUiStateStorage`, so no
//! `%APPDATA%` / `~/.fastmd*` read or write happens.
//!
//! Gated `#[cfg(test)]` at its declaration in [`crate::ui_iced`] — it is not
//! part of the production crate.
//!
//! Unit tests live in the sibling `test_helpers_tests.rs` sidecar.

#[cfg(test)]
#[path = "test_helpers_tests.rs"]
mod tests;

use std::sync::Arc;

use crate::bus::core::Bus;
use crate::bus::events::config::ConfigArrived;
use crate::config::{AppConfig, InMemoryConfigStorage, LlmConfig};
use crate::ui::PersistedUiState;
use crate::ui::app::FastMdApp;
use crate::ui_iced::app::{AppState, build_state};
use crate::ui_iced::persisted::InMemoryUiStateStorage;

/// Create a test [`FastMdApp`] with a default [`AppConfig`].
///
/// Pure helper with no global state. Used by iced panel tests that only need
/// an empty workspace and no LLM wiring.
#[must_use]
pub fn test_app() -> FastMdApp {
    FastMdApp::empty_state(AppConfig::default())
}

/// Create a test [`FastMdApp`] with the given [`AppConfig`].
///
/// Pure helper that forwards the config to [`FastMdApp::empty_state`].
#[must_use]
pub fn test_app_with_config(config: AppConfig) -> FastMdApp {
    FastMdApp::empty_state(config)
}

/// Create a test [`FastMdApp`] whose chat model posts to `api_url`.
///
/// Mirrors the legacy UI fixture of the same name: tests that drive a `RunAgent`
/// dispatch must route the LLM call to an in-process wiremock endpoint
/// instead of the dead `localhost:0` default.
#[must_use]
pub fn test_app_with_api_url(api_url: &str) -> FastMdApp {
    let mut models = std::collections::HashMap::new();
    models.insert(
        "test".to_string(),
        LlmConfig {
            model: "test".to_string(),
            api_url: api_url.to_string(),
            api_key: "test-key".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        },
    );
    let config = AppConfig {
        models,
        ..AppConfig::default()
    };
    FastMdApp::empty_state(config)
}

/// Build a fresh iced [`AppState`] over in-memory storage ([TASK-029]).
///
/// The framework-free counterpart of the panel fixtures above for tests that need
/// the whole iced application root — view-model rebuilds, overlay state, the
/// pane grid — not just the domain `FastMdApp`. Both storage handlers are
/// in-memory, so no user path is touched ([RUST-006]).
#[must_use]
pub fn test_app_state() -> AppState {
    test_app_state_with(AppConfig::default(), PersistedUiState::default())
}

/// [`test_app_state`] with an explicit config and persisted snapshot.
///
/// Lets a test seed a specific library set or restored geometry without
/// reaching for the production boot chain.
#[must_use]
pub fn test_app_state_with(config: AppConfig, persisted: PersistedUiState) -> AppState {
    let bus = Bus::<ConfigArrived>::new();
    build_state(
        config,
        &bus,
        Arc::new(InMemoryConfigStorage::new()),
        Arc::new(InMemoryUiStateStorage::new()),
        persisted,
    )
}

/// Create a no-op [`FileEventProducer`] backed by a fresh [`Bus`].
///
/// The producer is not connected to the app's event bus, so `save` calls
/// in tests do not publish file events. Pure helper with no global state;
/// relocated here from the deleted `ui/test_helpers/app.rs` ([TASK-029b]).
#[must_use]
pub fn noop_producer() -> crate::bus::events::file::FileEventProducer {
    crate::bus::events::file::FileEventProducer::new(Bus::new())
}
