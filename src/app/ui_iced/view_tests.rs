//! Unit tests for the [TASK-022] iced view (`src/app/ui_iced/view.rs`).
//!
//! iced elements are not inspectable without a renderer, so these are
//! construction smoke tests: the three-band tree builds for both the
//! no-file and selected-file gate states, and the placeholder/spacing
//! constants hold.

use super::*;
use crate::bus::core::Bus;
use crate::bus::events::config::ConfigArrived;
use crate::config::{AppConfig, InMemoryConfigStorage};
use crate::ui_iced::app::build_state;
use crate::ui_iced::persisted::InMemoryUiStateStorage;
use std::sync::Arc;

/// A fresh `AppState` over in-memory storage.
fn fresh_state() -> AppState {
    build_state(
        AppConfig::default(),
        &Bus::<ConfigArrived>::new(),
        Arc::new(InMemoryConfigStorage::new()),
        Arc::new(InMemoryUiStateStorage::new()),
        crate::ui::PersistedUiState::default(),
    )
}

#[test]
fn test_band_spacing_matches_task_contract() {
    assert_eq!(BAND_SPACING_PIXELS, 8.0);
}

#[test]
fn test_center_placeholder_text_is_named() {
    assert_eq!(CENTER_PLACEHOLDER, "TASK-023: document viewer");
}

#[test]
fn test_view_builds_without_selected_file() {
    let state = fresh_state();
    let _element: iced::Element<'_, Message> = view(&state);
}

#[test]
fn test_view_builds_with_selected_file() {
    let mut state = fresh_state();
    state.app.selection_mut().selected_file = Some(std::path::PathBuf::from("/notes/a.md"));
    let _element: iced::Element<'_, Message> = view(&state);
}

#[test]
fn test_center_placeholder_builds() {
    let _element: iced::Element<'static, Message> = center_placeholder();
}
