//! Tests for `panels/top.rs`.

use super::*;
use crate::ui::test_helpers::run_ui_test;
use crate::ui::test_helpers::text::{assert_text_contains, extract_text};

/// Tier 4 click test: clicking Batch... inside the hamburger menu must open
/// the batch dialog (sets `app.orchestrator.dialogs.batch_dialog_open = true`)
/// and fire the `on_click("batch_button")` callback that the test harness
/// captures into its persistent state.
///
/// Uses the `stateful_harness` helper from `test_helpers::interact`
/// (R-3). The closure calls the production `show_top_panel_capture`
/// with a callback that pushes the event into the harness's
/// `T = Vec<&'static str>` state. After the click settles, we
/// read the state and verify the event was captured.
#[test]
fn test_batch_button_click_opens_dialog() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;

    let mut harness = stateful_harness(Vec::<&'static str>::new(), |ui, captured| {
        // `app` is moved into the closure. The closure is
        // called once per pass; the harness owns it for
        // its lifetime. After the harness drops, the
        // captured `&'static str` events are the only
        // post-click observable state (per the state-
        // capture pattern documented in
        // `test_helpers::interact`).
        let mut app = create_test_app();
        assert!(!app.orchestrator.dialogs.batch_dialog_open);
        show_top_panel_capture(&mut app, ui, |event| {
            captured.push(event);
        });
    });
    harness.set_size(egui::vec2(800.0, 600.0));
    harness.run_steps(2);

    // Open the hamburger menu.
    harness
        .get_by_label(crate::ui::strings::HAMBURGER_MENU_BUTTON)
        .click();
    harness.run_steps(2);
    harness.run_steps(2);

    // Click Batch... inside the dropdown.
    harness.get_by_label(crate::ui::strings::MENU_BATCH).click();
    harness.run_steps(2);
    harness.run_steps(2);

    let captured = harness.state();
    assert!(
        captured.contains(&"batch_button"),
        "clicking Batch... in the hamburger menu must fire the `batch_button` \
             on_click event; got: {:?}",
        captured
    );
}

/// Tier 4 click test: clicking Tools... inside the hamburger menu must open
/// the tools dialog (sets `app.orchestrator.dialogs.tools_dialog_open = true`)
/// and fire the `on_click("tools_button")` callback. Mirrors the batch test above.
#[test]
fn test_tools_button_click_opens_dialog() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;

    let mut harness = stateful_harness(Vec::<&'static str>::new(), |ui, captured| {
        let mut app = create_test_app();
        assert!(!app.orchestrator.dialogs.tools_dialog_open);
        show_top_panel_capture(&mut app, ui, |event| {
            captured.push(event);
        });
    });
    harness.set_size(egui::vec2(800.0, 600.0));
    harness.run_steps(2);

    // Open the hamburger menu.
    harness
        .get_by_label(crate::ui::strings::HAMBURGER_MENU_BUTTON)
        .click();
    harness.run_steps(2);
    harness.run_steps(2);

    // Click Tools... inside the dropdown.
    harness.get_by_label(crate::ui::strings::MENU_TOOLS).click();
    harness.run_steps(2);
    harness.run_steps(2);

    let captured = harness.state();
    assert!(
        captured.contains(&"tools_button"),
        "clicking Tools... in the hamburger menu must fire the `tools_button` \
             on_click event; got: {:?}",
        captured
    );
}

/// Tier 4 click test: clicking the hamburger menu button in the top
/// toolbar opens the menu dropdown, navigates into the Table wrap algorithm
/// submenu, and clicking a strategy option fires the
/// `on_click("table_width_strategy")` callback.
#[test]
fn test_hamburger_menu_click_opens_menu_and_selects_strategy() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;

    let mut harness = stateful_harness(Vec::<&'static str>::new(), |ui, captured| {
        let mut app = create_test_app();
        show_top_panel_capture(&mut app, ui, |event| {
            captured.push(event);
        });
    });
    harness.set_size(egui::vec2(800.0, 600.0));
    harness.run_steps(2);
    // Locate the hamburger menu button by its label ("☰") and click it.
    harness
        .get_by_label(crate::ui::strings::HAMBURGER_MENU_BUTTON)
        .click();
    harness.run_steps(2);
    harness.run_steps(2);

    // Open the "Table wrap algorithm" submenu (egui appends " ⏵" to submenu labels)
    harness
        .get_by_label_contains(crate::ui::strings::MENU_TABLE_WRAP_ALGORITHM)
        .click();
    harness.run_steps(2);
    harness.run_steps(2);

    // Locate and click the Proportional strategy option inside the opened submenu.
    harness
        .get_by_label_contains(crate::ui::strings::TABLE_WIDTH_STRATEGY_PROPORTIONAL)
        .click_accesskit();
    harness.run_steps(2);
    harness.run_steps(2);

    let captured = harness.state();
    assert!(
        captured.contains(&crate::ui::strings::TABLE_WIDTH_STRATEGY_EVENT),
        "selecting a strategy in the hamburger menu must fire the `{}` on_click event; got: {:?}",
        crate::ui::strings::TABLE_WIDTH_STRATEGY_EVENT,
        captured
    );
}

/// Tier 4 click test: clicking hamburger menu -> Windows -> Background operations
/// toggles background logs and fires the on_click("background_operations") event.
#[test]
fn test_hamburger_menu_windows_background_operations_click() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;

    let mut harness = stateful_harness(Vec::<&'static str>::new(), |ui, captured| {
        let mut app = create_test_app();
        show_top_panel_capture(&mut app, ui, |event| {
            captured.push(event);
        });
    });
    harness.set_size(egui::vec2(800.0, 600.0));
    harness.run_steps(2);

    // Open hamburger menu
    harness
        .get_by_label(crate::ui::strings::HAMBURGER_MENU_BUTTON)
        .click();
    harness.run_steps(2);
    harness.run_steps(2);

    // Open Windows submenu (egui appends " ⏵" to submenu button labels)
    harness
        .get_by_label_contains(crate::ui::strings::MENU_WINDOWS)
        .click();
    harness.run_steps(2);
    harness.run_steps(2);

    // Click Background operations item
    harness
        .get_by_label_contains(crate::ui::strings::MENU_BACKGROUND_OPERATIONS)
        .click_accesskit();
    harness.run_steps(2);
    harness.run_steps(2);

    let captured = harness.state();
    assert!(
        captured.contains(&crate::ui::strings::BACKGROUND_OPERATIONS_EVENT),
        "toggling background operations in the Windows menu must fire the `{}` event; got: {:?}",
        crate::ui::strings::BACKGROUND_OPERATIONS_EVENT,
        captured
    );
}

/// Tier 4 click test: clicking hamburger menu -> Windows -> Agent debug
/// toggles the agent debug window and fires the on_click("agent_debug") event.
#[test]
fn test_hamburger_menu_windows_agent_debug_click() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;

    let mut harness = stateful_harness(Vec::<&'static str>::new(), |ui, captured| {
        let mut app = create_test_app();
        show_top_panel_capture(&mut app, ui, |event| {
            captured.push(event);
        });
    });
    harness.set_size(egui::vec2(800.0, 600.0));
    harness.run_steps(2);

    // Open hamburger menu
    harness
        .get_by_label(crate::ui::strings::HAMBURGER_MENU_BUTTON)
        .click();
    harness.run_steps(2);
    harness.run_steps(2);

    // Open Windows submenu
    harness
        .get_by_label_contains(crate::ui::strings::MENU_WINDOWS)
        .click();
    harness.run_steps(2);
    harness.run_steps(2);

    // Click Agent debug item
    harness
        .get_by_label_contains(crate::ui::strings::MENU_AGENT_DEBUG)
        .click_accesskit();
    harness.run_steps(2);
    harness.run_steps(2);

    let captured = harness.state();
    assert!(
        captured.contains(&crate::ui::strings::AGENT_DEBUG_EVENT),
        "toggling agent debug in the Windows menu must fire the `{}` event; got: {:?}",
        crate::ui::strings::AGENT_DEBUG_EVENT,
        captured
    );
}

fn create_test_app_with_models() -> FastMdApp {
    let mut config = crate::config::AppConfig::default();
    config.models.insert(
        "model-a".to_string(),
        crate::config::LlmConfig {
            model: "model-a".to_string(),
            api_url: "http://localhost".to_string(),
            api_key: "key".to_string(),
            cost: Some(10),
            use_case: vec!["chat".to_string()],
        },
    );
    config.models.insert(
        "model-b".to_string(),
        crate::config::LlmConfig {
            model: "model-b".to_string(),
            api_url: "http://localhost".to_string(),
            api_key: "key".to_string(),
            cost: Some(20),
            use_case: vec!["chat".to_string()],
        },
    );
    FastMdApp::empty_state(config)
}

/// Tier 4 click test: clicking hamburger menu -> Chat models -> model-b
/// sets selected_chat_model and fires the on_click("chat_model_selection") event.
#[test]
fn test_hamburger_menu_chat_models_selection_click() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;

    let mut harness = stateful_harness(
        (create_test_app_with_models(), Vec::<&'static str>::new()),
        |ui, (app, captured)| {
            show_top_panel_capture(app, ui, |event| {
                captured.push(event);
            });
        },
    );
    harness.set_size(egui::vec2(800.0, 600.0));
    harness.run_steps(2);

    // Open hamburger menu
    harness
        .get_by_label(crate::ui::strings::HAMBURGER_MENU_BUTTON)
        .click();
    harness.run_steps(2);
    harness.run_steps(2);

    // Open Chat models submenu
    harness
        .get_by_label_contains(crate::ui::strings::MENU_CHAT_MODELS)
        .click();
    harness.run_steps(2);
    harness.run_steps(2);

    // Click model-b item
    harness.get_by_label_contains("model-b").click_accesskit();
    harness.run_steps(2);
    harness.run_steps(2);

    let (app, captured) = harness.state();
    assert_eq!(app.config().selected_chat_model.as_deref(), Some("model-b"));
    assert_eq!(
        app.agent().agent_config().selected_chat_model(),
        Some("model-b"),
        "selecting a chat model must update the runtime agent configuration"
    );
    assert!(
        captured.contains(&crate::ui::strings::CHAT_MODEL_SELECTION_EVENT),
        "selecting a chat model in the Chat models menu must fire the `{}` event; got: {:?}",
        crate::ui::strings::CHAT_MODEL_SELECTION_EVENT,
        captured
    );
}

/// Tier 4 click test: clicking hamburger menu -> Chat models -> model-b, then switching back to model-a.
#[test]
fn test_hamburger_menu_chat_models_switch_between_models_click() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;

    let mut harness = stateful_harness(
        (create_test_app_with_models(), Vec::<&'static str>::new()),
        |ui, (app, captured)| {
            show_top_panel_capture(app, ui, |event| {
                captured.push(event);
            });
        },
    );
    harness.set_size(egui::vec2(800.0, 600.0));
    harness.run_steps(2);

    // Open hamburger menu -> Chat models -> click model-b
    harness
        .get_by_label(crate::ui::strings::HAMBURGER_MENU_BUTTON)
        .click();
    harness.run_steps(2);
    harness.run_steps(2);

    harness
        .get_by_label_contains(crate::ui::strings::MENU_CHAT_MODELS)
        .click();
    harness.run_steps(2);
    harness.run_steps(2);

    harness.get_by_label_contains("model-b").click_accesskit();
    harness.run_steps(2);
    harness.run_steps(2);

    let (app, captured) = harness.state();
    assert_eq!(app.config().selected_chat_model.as_deref(), Some("model-b"));
    assert_eq!(
        app.agent().agent_config().selected_chat_model(),
        Some("model-b")
    );
    assert_eq!(
        captured.last().copied(),
        Some(crate::ui::strings::CHAT_MODEL_SELECTION_EVENT)
    );

    // Open hamburger menu -> Chat models -> switch back to model-a
    harness
        .get_by_label(crate::ui::strings::HAMBURGER_MENU_BUTTON)
        .click();
    harness.run_steps(2);
    harness.run_steps(2);

    harness
        .get_by_label_contains(crate::ui::strings::MENU_CHAT_MODELS)
        .click();
    harness.run_steps(2);
    harness.run_steps(2);

    harness.get_by_label_contains("model-a").click_accesskit();
    harness.run_steps(2);
    harness.run_steps(2);

    let (app, _) = harness.state();
    assert_eq!(app.config().selected_chat_model.as_deref(), Some("model-a"));
    assert_eq!(
        app.agent().agent_config().selected_chat_model(),
        Some("model-a")
    );
}

/// Tier 4 click test: the selected chat model's checkbox reports
/// `Toggled::True` and a non-selected model reports `Toggled::False` via
/// AccessKit. This guards against regressing back to a text-prefix `✓` glyph,
/// which egui's bundled default fonts lack (U+2713 renders as tofu / an
/// empty-checkbox-looking missing-glyph box). `ui.checkbox` draws the
/// checkmark as vector strokes, so the toggled state is font-independent.
#[test]
fn test_hamburger_menu_chat_models_selected_item_toggled_state() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use accesskit::Toggled;
    use egui_kittest::kittest::NodeT;
    use egui_kittest::kittest::Queryable;

    let mut harness = stateful_harness(
        (create_test_app_with_models(), Vec::<&'static str>::new()),
        |ui, (app, captured)| {
            show_top_panel_capture(app, ui, |event| {
                captured.push(event);
            });
        },
    );
    harness.set_size(egui::vec2(800.0, 600.0));
    harness.run_steps(2);

    // Open hamburger menu -> Chat models.
    harness
        .get_by_label(crate::ui::strings::HAMBURGER_MENU_BUTTON)
        .click();
    harness.run_steps(2);
    harness.run_steps(2);

    harness
        .get_by_label_contains(crate::ui::strings::MENU_CHAT_MODELS)
        .click();
    harness.run_steps(2);
    harness.run_steps(2);

    // `selected_chat_model` starts `None`; `current_chat_model_key()`
    // returns the lowest-cost chat model, i.e. `Some("model-a")`.
    let model_a = harness.get_by_label_contains("model-a");
    assert_eq!(
        model_a.accesskit_node().toggled(),
        Some(Toggled::True),
        "the selected chat model must report a checked toggled state"
    );

    let model_b = harness.get_by_label_contains("model-b");
    assert_eq!(
        model_b.accesskit_node().toggled(),
        Some(Toggled::False),
        "a non-selected chat model must report an unchecked toggled state"
    );
}

/// Tier 1 test for `apply_chat_model_selection` verifying switching between models and idempotence.
#[test]
fn test_apply_chat_model_selection() {
    let mut app = create_test_app_with_models();
    assert_eq!(app.config().selected_chat_model, None);
    assert_eq!(
        app.config().current_chat_model_key().as_deref(),
        Some("model-a")
    );

    // Switch to model-b
    apply_chat_model_selection(&mut app, "model-b".to_string());
    assert_eq!(app.config().selected_chat_model.as_deref(), Some("model-b"));
    assert_eq!(
        app.config().current_chat_model_key().as_deref(),
        Some("model-b")
    );
    assert_eq!(
        app.agent().agent_config().selected_chat_model(),
        Some("model-b")
    );
    assert_eq!(
        app.agent()
            .agent_config()
            .select_chat_model()
            .unwrap()
            .model,
        "model-b"
    );

    // Idempotent re-selection of same model
    apply_chat_model_selection(&mut app, "model-b".to_string());
    assert_eq!(app.config().selected_chat_model.as_deref(), Some("model-b"));

    // Switch back to model-a
    apply_chat_model_selection(&mut app, "model-a".to_string());
    assert_eq!(app.config().selected_chat_model.as_deref(), Some("model-a"));
    assert_eq!(
        app.config().current_chat_model_key().as_deref(),
        Some("model-a")
    );
    assert_eq!(
        app.agent().agent_config().selected_chat_model(),
        Some("model-a")
    );
    assert_eq!(
        app.agent()
            .agent_config()
            .select_chat_model()
            .unwrap()
            .model,
        "model-a"
    );
}

/// Tier 1 test for `apply_background_logs_toggle`.
#[test]
fn test_apply_background_logs_toggle() {
    let mut app = create_test_app();
    assert!(
        !app.orchestrator
            .background_manager
            .lock()
            .unwrap()
            .show_background_logs
    );
    apply_background_logs_toggle(&mut app, true);
    assert!(
        app.orchestrator
            .background_manager
            .lock()
            .unwrap()
            .show_background_logs
    );
    apply_background_logs_toggle(&mut app, false);
    assert!(
        !app.orchestrator
            .background_manager
            .lock()
            .unwrap()
            .show_background_logs
    );
}

/// Tier 1 test for `apply_agent_debug_toggle`.
#[test]
fn test_apply_agent_debug_toggle() {
    let mut app = create_test_app();
    assert!(!app.orchestrator.agent_panel_state.show_debug_window);
    apply_agent_debug_toggle(&mut app, true);
    assert!(app.orchestrator.agent_panel_state.show_debug_window);
    apply_agent_debug_toggle(&mut app, false);
    assert!(!app.orchestrator.agent_panel_state.show_debug_window);
}

/// Tier 1 test for the batch button click effect. The click sets
/// `app.orchestrator.dialogs.batch_dialog_open` to `true`; the dialog itself
/// resets the flag to `false` when it closes. We verify the
/// effect without driving the egui harness.
#[test]
fn test_apply_batch_button_click_sets_dialog_open() {
    let mut app = create_test_app();
    assert!(
        !app.orchestrator.dialogs.batch_dialog_open,
        "dialog must start closed"
    );
    apply_batch_button_click(&mut app);
    assert!(
        app.orchestrator.dialogs.batch_dialog_open,
        "batch button click must open the batch dialog"
    );
}

/// Tier 1 test for the tools button click effect. The click sets
/// `app.orchestrator.dialogs.tools_dialog_open` to `true`; the dialog itself
/// resets the flag to `false` when it closes.
#[test]
fn test_apply_tools_button_click_sets_dialog_open() {
    let mut app = create_test_app();
    assert!(
        !app.orchestrator.dialogs.tools_dialog_open,
        "dialog must start closed"
    );
    apply_tools_button_click(&mut app);
    assert!(
        app.orchestrator.dialogs.tools_dialog_open,
        "tools button click must open the tools dialog"
    );
}

/// Tier 1 test for the table-width-strategy apply function. Picking a
/// new strategy must:
///   1. Update `app.config().table_width_strategy` to the new value's
///      `to_config()` form.
///   2. Make `app.config().deficit_strategy()` return the new variant
///      (the markdown renderer reads this on every frame, so the
///      change is live the next paint).
///   3. Be a no-op when called with the *current* strategy (so egui's
///      re-fired selected-value events don't trigger redundant disk
///      writes via `save_config`).
///
/// `save_config` is called as a side effect but the test does not
/// assert on it — failures are logged via `tracing::error!` and the
/// function continues, mirroring the `tools_dialog::render_row`
/// policy.
///
/// **IMPORTANT**: the persist callback is supplied by the test, not
/// hard-coded inside the function. Earlier versions of this test
/// called `apply_table_width_strategy_change` (no path), which
/// internally invoked `crate::config::save_config` and wrote to
/// `%APPDATA%\fastmd\config.yaml`, silently clobbering the user's
/// real config on every test run (the original two-arm swap wrote
/// `"proportional"`; a later five-arm cycle wrote `"ratio"` /
/// `"lagrange"` / `"hybrid"`). The current callback-based API lets
/// the test capture the persist call for inspection without touching
/// the real filesystem.
#[test]
fn test_apply_table_width_strategy_change_updates_config() {
    use crate::ui::table_width::DeficitStrategy;

    let mut app = create_test_app();
    let initial = app.orchestrator.config.deficit_strategy();
    // Pick *any* variant that differs from the current one. With five
    // strategies now, a simple two-arm swap is no longer exhaustive —
    // cycle deterministically through the list and pick the next one.
    let target = match initial {
        DeficitStrategy::ProportionalToSlack => DeficitStrategy::BreakpointWaterFill,
        DeficitStrategy::BreakpointWaterFill => DeficitStrategy::WaterFillRatio,
        DeficitStrategy::WaterFillRatio => DeficitStrategy::LagrangePenalty,
        DeficitStrategy::LagrangePenalty => DeficitStrategy::HybridMinPenaltyWaterFill,
        DeficitStrategy::HybridMinPenaltyWaterFill => DeficitStrategy::ProportionalToSlack,
    };

    // (1) and (2): config string and parsed enum both update, and
    // the persist callback receives the post-mutation config. We
    // capture it for inspection rather than writing to disk.
    let mut persisted: Option<crate::config::AppConfig> = None;
    let mut persist_fn = |cfg: &crate::config::AppConfig| {
        persisted = Some(cfg.clone());
        Ok(PathBuf::new())
    };
    apply_table_width_strategy_change(&mut app, target, &mut persist_fn);
    assert_eq!(
        app.orchestrator.config.deficit_strategy(),
        target,
        "deficit_strategy() must reflect the picked variant after the apply"
    );
    assert_eq!(
        app.orchestrator.config.table_width_strategy,
        target.to_config(),
        "in-memory config string must equal target.to_config()"
    );
    let persisted_cfg = persisted.expect("persist must be called when value changes");
    assert_eq!(
        persisted_cfg.table_width_strategy,
        target.to_config(),
        "persisted callback must receive the post-mutation config"
    );

    // (3): no-op when called with the *current* strategy. The persist
    // callback must NOT be called — supply a closure that panics if
    // it is, to assert the short-circuit.
    let before = app.orchestrator.config.table_width_strategy.clone();
    let mut noop_fn = |_cfg: &crate::config::AppConfig| -> Result<PathBuf, String> {
        panic!("persist must not be called when the value is unchanged");
    };
    apply_table_width_strategy_change(&mut app, target, &mut noop_fn);
    let after = app.orchestrator.config.table_width_strategy.clone();
    assert_eq!(
        before, after,
        "re-applying the current strategy must be a no-op (config string unchanged)"
    );
}

/// Tier 1 test for the `strategy_label` mapping. Every `DeficitStrategy`
/// variant must map to a distinct, non-empty label so the dropdown
/// `selected_text` is always defined and the row labels inside the
/// dropdown don't collide.
#[test]
fn test_strategy_label_maps_every_variant() {
    use crate::ui::table_width::DeficitStrategy;

    let proportional = strategy_label(DeficitStrategy::ProportionalToSlack);
    let waterfill = strategy_label(DeficitStrategy::BreakpointWaterFill);
    let ratio = strategy_label(DeficitStrategy::WaterFillRatio);
    let lagrange = strategy_label(DeficitStrategy::LagrangePenalty);
    let hybrid = strategy_label(DeficitStrategy::HybridMinPenaltyWaterFill);
    assert!(
        !proportional.is_empty(),
        "ProportionalToSlack label must not be empty"
    );
    assert!(
        !waterfill.is_empty(),
        "BreakpointWaterFill label must not be empty"
    );
    assert!(!ratio.is_empty(), "WaterFillRatio label must not be empty");
    assert!(
        !lagrange.is_empty(),
        "LagrangePenalty label must not be empty"
    );
    assert!(
        !hybrid.is_empty(),
        "HybridMinPenaltyWaterFill label must not be empty"
    );
    // All five labels must be pairwise distinct so the dropdown can
    // show them without collisions.
    let labels = [proportional, waterfill, ratio, lagrange, hybrid];
    for (i, a) in labels.iter().enumerate() {
        for (j, b) in labels.iter().enumerate() {
            if i != j {
                assert_ne!(
                    a, b,
                    "strategy labels for distinct variants must differ (i={i}, j={j})"
                );
            }
        }
    }
    // Sanity: the labels are the documented user-facing strings.
    assert_eq!(
        proportional,
        crate::ui::strings::TABLE_WIDTH_STRATEGY_PROPORTIONAL
    );
    assert_eq!(
        waterfill,
        crate::ui::strings::TABLE_WIDTH_STRATEGY_WATERFILL
    );
    assert_eq!(ratio, crate::ui::strings::TABLE_WIDTH_STRATEGY_RATIO);
    assert_eq!(lagrange, crate::ui::strings::TABLE_WIDTH_STRATEGY_LAGRANGE);
    assert_eq!(hybrid, crate::ui::strings::TABLE_WIDTH_STRATEGY_HYBRID);
}

#[test]
fn test_build_indexing_status_text_finished() {
    let text = build_indexing_status_text(true, 42);
    assert_eq!(text.text(), "Indexing finished (42 files)");
}

#[test]
fn test_build_indexing_status_text_unfinished() {
    let text = build_indexing_status_text(false, 10);
    assert_eq!(text.text(), "Indexing workspace (found 10 files)...");
}

#[test]
fn test_get_tag_filter_text() {
    assert_eq!(get_tag_filter_text(None), "Filter by Tag: All");
    let tag = "Rust".to_string();
    assert_eq!(get_tag_filter_text(Some(&tag)), "Rust");
}

#[test]
fn test_compute_next_selected_file_no_selected_file() {
    let file_tags = BTreeMap::new();
    assert_eq!(compute_next_selected_file(None, None, &file_tags), None);
}

#[test]
fn test_compute_next_selected_file_no_tag() {
    let mut file_tags = BTreeMap::new();
    let path = PathBuf::from("test.md");
    file_tags.insert(path.clone(), vec!["Rust".to_string()]);

    assert_eq!(
        compute_next_selected_file(Some(&path), None, &file_tags),
        Some(path)
    );
}

#[test]
fn test_compute_next_selected_file_tag_matches() {
    let mut file_tags = BTreeMap::new();
    let path = PathBuf::from("test.md");
    file_tags.insert(path.clone(), vec!["Rust".to_string()]);
    let tag = "Rust".to_string();

    assert_eq!(
        compute_next_selected_file(Some(&path), Some(&tag), &file_tags),
        Some(path)
    );
}

#[test]
fn test_compute_next_selected_file_tag_missing() {
    let mut file_tags = BTreeMap::new();
    let path = PathBuf::from("test.md");
    file_tags.insert(path.clone(), vec!["Rust".to_string()]);
    let tag = "Go".to_string();

    assert_eq!(
        compute_next_selected_file(Some(&path), Some(&tag), &file_tags),
        None
    );
}

#[test]
fn test_compute_next_selected_file_file_not_in_tags() {
    let file_tags = BTreeMap::new();
    let path = PathBuf::from("test.md");
    let tag = "Rust".to_string();

    assert_eq!(
        compute_next_selected_file(Some(&path), Some(&tag), &file_tags),
        None
    );
}

// --- UI / window tests (R-7: merged from `mod ui_tests`) ---

use crate::ui::strings::APP_TITLE;
use crate::ui::test_helpers::assert::assert_no_id_change_in_log;

fn create_test_app() -> FastMdApp {
    FastMdApp::empty_state(crate::config::AppConfig::default())
}

#[test]
fn test_show_top_panel_indexing_unfinished() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();
    app.file_processor_mut().indexing_finished = false;
    let output = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
        show_top_panel(&mut app, ui);
    });
    // The top panel keeps the app title and hamburger button visible.
    // Batch..., Tools..., and the log toggle are in the hamburger menu.
    assert_text_contains(&output.shapes, APP_TITLE);
    assert!(
        !extract_text(&output.shapes)
            .iter()
            .any(|text| text.contains("Show log"))
    );
    assert_text_contains(&output.shapes, crate::ui::strings::HAMBURGER_MENU_BUTTON);
    assert!(!app.file_processor().indexing_finished);
}

#[test]
fn test_show_top_panel_indexing_finished_with_tags() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();
    app.file_processor_mut().indexing_finished = true;
    app.tags_mut().add_tags(
        PathBuf::from("dummy.md"),
        vec!["Rust".to_string(), "Docs".to_string()],
    );

    let output = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
        show_top_panel(&mut app, ui);
    });
    // Header assertion (Q12 borderline case): the toolbar chrome is
    // the stable surface here. The tag combobox content is dynamic
    // so we don't assert on individual tag names.
    assert_text_contains(&output.shapes, APP_TITLE);
    assert!(app.file_processor().indexing_finished);
}

/// Regression: the production UI logged
/// `WARN egui::context: Widget rect ... changed id between passes`
/// for the toolbar row on every frame around the
/// `indexing_finished` transition. The previous revision put the
/// spinner and the tag combobox under mutually-exclusive
/// `if`/`else if` blocks keyed on the bool, so the moment
/// indexing finished a different widget (combobox) replaced the
/// previous one (spinner) at the same rect and egui flagged the
/// whole row. After the fix, both widgets always allocate (via
/// `add_visible` / `set_invisible`) so their ids are stable
/// across the bool flip. The test simulates the transition by
/// flipping `indexing_finished` between two passes and asserts
/// no `changed id between passes` warning is emitted.
#[test]
fn test_show_top_panel_no_id_change_warnings_on_indexing_finished_transition() {
    use std::sync::{Mutex, OnceLock};
    struct Capture {
        msgs: Mutex<Vec<String>>,
    }
    impl log::Log for Capture {
        fn enabled(&self, _: &log::Metadata) -> bool {
            true
        }
        fn log(&self, record: &log::Record) {
            self.msgs
                .lock()
                .unwrap()
                .push(format!("[{}] {}", record.level(), record.args()));
        }
        fn flush(&self) {}
    }
    static LOGGER: OnceLock<Capture> = OnceLock::new();
    static INSTALLED: OnceLock<()> = OnceLock::new();
    let cap = LOGGER.get_or_init(|| Capture {
        msgs: Mutex::new(Vec::new()),
    });
    INSTALLED.get_or_init(|| {
        let _ = log::set_logger(cap);
        log::set_max_level(log::LevelFilter::Trace);
    });
    cap.msgs.lock().unwrap().clear();

    let ctx = egui::Context::default();
    let mut app = create_test_app();
    app.file_processor_mut().indexing_finished = false;
    app.tags_mut().add_tags(
        PathBuf::from("dummy.md"),
        vec!["Rust".to_string(), "Docs".to_string()],
    );

    // Pre-finish: spinner is visible, combobox is hidden but
    // still allocated.
    let _ = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
        show_top_panel(&mut app, ui);
    });
    // Flip the bool and render again — the rects the spinner
    // and combobox live at must stay the same; only their
    // visibility changes.
    app.file_processor_mut().indexing_finished = true;
    let _ = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
        show_top_panel(&mut app, ui);
    });
    // Stabilise on the finished side.
    let _ = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
        show_top_panel(&mut app, ui);
    });

    let msgs = cap.msgs.lock().unwrap().clone();
    // Sanity check that the log capture is actually wired up —
    // an empty `msgs` would silently pass the id-stability check
    // even if the warning fires through a different sink.
    assert!(
        !msgs.is_empty(),
        "log capture is empty — the test is not actually running under the installed log::Log impl"
    );
    assert_no_id_change_in_log(&msgs);
}
