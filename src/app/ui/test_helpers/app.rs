//! Test-app and bus helpers for UI integration tests.
//!
//! Consolidates `create_test_app`, `create_test_app_with_api_url`, and
//! `noop_producer` that were previously duplicated across 8+ test modules
//! (W3-01, W3-03). Helpers are pure and honest (RUST-020) and live in the
//! `test_helpers` subsystem by concern (RUST-051).

use crate::bus::core::Bus;
use crate::bus::events::file::FileEventProducer;
use crate::config::{AppConfig, LlmConfig};
use crate::ui::app::FastMdApp;
use std::collections::HashMap;

/// Create a test [`FastMdApp`] with a default [`AppConfig`].
///
/// Pure helper with no global state. Used by UI panel tests that only need
/// an empty workspace and no LLM wiring.
pub fn test_app() -> FastMdApp {
    FastMdApp::empty_state(AppConfig::default())
}

/// Create a test [`FastMdApp`] with the given [`AppConfig`].
///
/// Pure helper that forwards the config to [`FastMdApp::empty_state`].
pub fn test_app_with_config(config: AppConfig) -> FastMdApp {
    FastMdApp::empty_state(config)
}

/// Create a test [`FastMdApp`] whose chat model posts to `api_url`.
///
/// Tests that drive a `RunAgent` dispatch must route the LLM call to an
/// in-process wiremock endpoint instead of the dead `localhost:0` default.
/// The helper builds a single `test` model with the given URL.
pub fn test_app_with_api_url(api_url: &str) -> FastMdApp {
    let mut models = HashMap::new();
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

/// Create a no-op [`FileEventProducer`] backed by a fresh [`Bus`].
///
/// The producer is not connected to the app's event bus, so `save` calls
/// in tests do not publish file events. Pure helper with no global state.
pub fn noop_producer() -> FileEventProducer {
    FileEventProducer::new(Bus::new())
}
