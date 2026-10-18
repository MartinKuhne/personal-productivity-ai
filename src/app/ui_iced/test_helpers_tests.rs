//! Unit tests for the egui-free iced test fixtures ([TASK-029], [RUST-003]).
//!
//! The point of these fixtures is that they build real domain state **without
//! egui** and **without touching a user path**. The tests pin both halves of
//! that promise.

use super::*;
use crate::config::ContentLibrary;
use iced::Size;

#[test]
fn test_app_builds_an_empty_workspace_egui_free() {
    let app = test_app();

    // A default config has no content libraries, so the workspace starts empty.
    assert!(
        app.content_libraries().is_empty(),
        "the default fixture must not invent libraries"
    );
    assert!(
        app.tabs().toc.is_empty(),
        "the default fixture must start with no ToC"
    );
    assert!(
        app.selection().selected_file.is_none(),
        "the default fixture must start with nothing selected"
    );
}

#[test]
fn test_app_with_config_forwards_the_config() {
    let mut config = AppConfig::default();
    config.content_libraries.push(ContentLibrary {
        root_folder: "M:\\fixture_lib".to_string(),
        name: "FixtureLib".to_string(),
        kind: "text".to_string(),
        readonly: false,
        priority: 0,
    });

    let app = test_app_with_config(config);

    assert_eq!(
        app.content_libraries()
            .iter()
            .map(|lib| lib.name.as_str())
            .collect::<Vec<_>>(),
        vec!["FixtureLib"],
        "the supplied config must reach the built app"
    );
}

#[test]
fn test_app_with_api_url_registers_a_chat_model() {
    let app = test_app_with_api_url("http://127.0.0.1:9/api");
    let model = app
        .config()
        .models
        .get("test")
        .expect("the fixture must register a `test` model");

    assert_eq!(model.api_url, "http://127.0.0.1:9/api");
    assert_eq!(
        model.use_case,
        vec!["chat".to_string()],
        "the fixture model must be chat-capable so the top-bar picker sees it"
    );
}

#[test]
fn test_app_state_builds_a_valid_iced_state() {
    let state = test_app_state();

    // The iced root is fully wired: domain state, pane grid and view-models
    // all exist, and the config direct-injection (DEC-009) landed.
    assert!(
        state.app.config().models.contains_key("default") || state.app.config().models.is_empty(),
        "the config must be reachable off the built state"
    );
    assert_eq!(
        state.panes.panes.len(),
        3,
        "the three-band middle pane grid must be built"
    );
    assert!(
        state.os_scale.is_none(),
        "no OS scale is reported until the boot task settles"
    );
    assert!(
        state.viewport.width > 0.0 && state.viewport.height > 0.0,
        "the viewport must be seeded from the (persisted) window size"
    );
}

#[test]
fn test_app_state_with_seeds_persisted_geometry() {
    let persisted = PersistedUiState {
        left_panel_width: Some(280.0),
        right_panel_width: Some(200.0),
        window_width: Some(998.0),
        window_height: Some(688.0),
        font_size_scale: Some(1.25),
        ..Default::default()
    };

    let state = test_app_state_with(AppConfig::default(), persisted);

    assert_eq!(state.app.layout.left_panel_width, Some(280.0));
    assert_eq!(state.app.layout.right_panel_width, Some(200.0));
    assert_eq!(state.viewport, Size::new(998.0, 688.0));
    // DEC-008: with no OS baseline yet, the composed scale is the persisted
    // multiplier alone.
    assert!((state.composed_scale_factor() - 1.25).abs() < 1e-6);
}

#[test]
fn test_app_state_view_models_rebuild_without_egui() {
    let mut state = test_app_state();

    // The view-model rebuild is what every iced panel renders from; running it
    // here proves the whole snapshot path is egui-free.
    state.rebuild_view_models();

    assert!(
        !state.models.top.indexing_finished,
        "a fresh fixture has not finished indexing"
    );
    assert!(
        state.models.left.rows.is_empty(),
        "an empty workspace flattens to no tree rows"
    );
}
