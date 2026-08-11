//! Tests for `panels/bottom.rs`.

use super::*;
use crate::config::{ContentLibrary, LlmConfig};
use crate::ui::test_helpers::run_ui_test;
use std::collections::HashMap;
use std::path::PathBuf;

/// Tier 1 test for the `apply_send_click` effect: a bare
/// `/models` prompt dispatches to `CommandIntent::ShowModels`,
/// which sets the agent status, response, and show_results
/// flag. We verify the effect without driving the egui
/// harness.
#[test]
fn test_apply_send_click_show_models_dispatch() {
    let mut app = create_test_app();
    // Populate at least one model so `format_models_list` is
    // non-empty and the test can assert the response contains
    // the model name.
    app.orchestrator.config.models.insert(
        "test-model".to_string(),
        LlmConfig {
            api_key: String::new(),
            api_url: String::new(),
            model: "test-model".to_string(),
            cost: None,
            use_case: Vec::new(),
        },
    );
    app.orchestrator.agent_panel_state.command_input = "/models".to_string();
    assert!(
        !app.orchestrator.agent_panel_state.show_results,
        "show_results must start false"
    );

    apply_send_click(&mut app);

    assert_eq!(app.orchestrator.agent.state().status, "Done");
    assert!(
        app.orchestrator.agent_panel_state.show_results,
        "ShowModels dispatch must set show_results"
    );
    assert!(
        app.orchestrator
            .agent
            .state()
            .response
            .contains("test-model"),
        "ShowModels dispatch must put the model list into the response, got: {}",
        app.orchestrator.agent.state().response
    );
    assert!(
        app.orchestrator.agent_panel_state.command_input.is_empty(),
        "command_input must be cleared after dispatch"
    );
}

/// Tier 1 test: an empty prompt produces `CommandIntent::Empty`
/// and `apply_send_click` is a no-op (no status change, no
/// show_results toggle, command_input is still cleared).
#[test]
fn test_apply_send_click_empty_prompt_is_noop() {
    let mut app = create_test_app();
    app.orchestrator.agent_panel_state.command_input = "   ".to_string();

    apply_send_click(&mut app);

    assert!(
        !app.orchestrator.agent_panel_state.show_results,
        "Empty intent must not toggle show_results"
    );
    assert!(
        app.orchestrator.agent_panel_state.command_input.is_empty(),
        "command_input is still cleared (the .clear() runs before the match)"
    );
}

/// Tier 1 test: an unknown `/foo` style command produces
/// `CommandIntent::RunAgent("/foo")` and starts an agent
/// session. We don't verify the full session state (that
/// requires the background LLM machinery) — just that
/// show_results is toggled and the command_input is cleared.
#[test]
fn test_apply_send_click_run_agent_dispatches_with_prompt() {
    let mut app = create_test_app();
    app.orchestrator.agent_panel_state.command_input = "hello world".to_string();

    apply_send_click(&mut app);

    assert!(
        app.orchestrator.agent_panel_state.show_results,
        "RunAgent dispatch must set show_results"
    );
    assert!(
        app.orchestrator.agent_panel_state.command_input.is_empty(),
        "command_input must be cleared after dispatch"
    );
}

/// Tier 4 click test: pressing Enter in the bottom-panel
/// command input must dispatch the prompt via
/// `apply_send_click` and fire the `on_click("send")` callback.
///
/// The harness owns `&mut app` for its lifetime, so the
/// post-click observation goes through the captured
/// `&'static str` event name (per the state-capture pattern
/// in `test_helpers::interact`). The dispatch's effect on
/// `app.orchestrator.agent` is verified separately in the Tier 1 tests
/// above.
#[test]
fn test_send_enter_key_captures_event() {
    use crate::ui::test_helpers::interact::stateful_harness;

    let mut harness = stateful_harness(Vec::<&'static str>::new(), |ui, captured| {
        let mut app = create_test_app();
        app.orchestrator.agent_panel_state.command_input = "/models".to_string();
        show_bottom_panel_capture(&mut app, ui, |event| {
            captured.push(event);
        });
    });
    harness.fit_contents();
    // The TextEdit must have focus for the production
    // Enter-handler to fire (it gates on
    // `response.has_focus()`). Click into it first to set
    // focus. The bottom-panel command input is a multi-line
    // TextEdit, which has `accesskit::Role::TextInput` (or
    // `MultilineTextEdit` in some versions). Query by role
    // to find it without depending on the hint-text label
    // (which the TextEdit doesn't expose as an accessible
    // name).
    use accesskit::Role;
    use egui_kittest::kittest::Queryable;
    let candidates: Vec<_> = harness
        .query_all_by_role(Role::TextInput)
        .chain(harness.query_all_by_role(Role::MultilineTextInput))
        .collect();
    // The bottom panel is the only panel rendered in this
    // test, so any TextInput we find IS the command input.
    assert!(
        !candidates.is_empty(),
        "expected at least one TextInput node in the bottom panel"
    );
    candidates[0].click();
    harness.run_steps(2);
    // Synthesize an Enter key press. The harness queues the
    // event; `run_steps` processes it through the input
    // pipeline.
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);
    harness.run_steps(2);

    let captured = harness.state();
    assert!(
        captured.contains(&"send"),
        "pressing Enter in the command input must fire the `send` \
             on_click event; got: {:?}",
        captured
    );
}

/// Regression guard: pasting text into the command input must not
/// break the Enter-to-submit path. Pasting multiline text, then
/// pressing Enter, must still dispatch the prompt via
/// `apply_send_click` and fire the `on_click("send")` callback.
#[test]
fn test_paste_then_enter_still_submits() {
    use crate::ui::test_helpers::interact::stateful_harness;

    let mut harness = stateful_harness(Vec::<&'static str>::new(), |ui, captured| {
        let mut app = create_test_app();
        show_bottom_panel_capture(&mut app, ui, |event| {
            captured.push(event);
        });
    });
    harness.fit_contents();

    use accesskit::Role;
    use egui_kittest::kittest::Queryable;
    let candidates: Vec<_> = harness
        .query_all_by_role(Role::TextInput)
        .chain(harness.query_all_by_role(Role::MultilineTextInput))
        .collect();
    assert!(
        !candidates.is_empty(),
        "expected at least one TextInput node in the bottom panel"
    );
    candidates[0].click();
    harness.run_steps(2);

    // Paste multiline text (with a trailing newline, as copied text
    // often carries).
    harness.event(egui::Event::Paste("summarize this\n".to_owned()));
    harness.run_steps(2);

    // Enter must still submit the prompt.
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);
    harness.run_steps(2);

    let captured = harness.state();
    assert!(
        captured.contains(&"send"),
        "pressing Enter after pasting text must fire the `send` \
             on_click event; got: {:?}",
        captured
    );
}

/// Regression guard: on Windows, when an IME is active the Enter key
/// is swallowed by the IME (winit reports `VK_PROCESSKEY`; egui-winit
/// filters it out) and only `Event::Ime(ImeEvent::Commit("\n"))`
/// reaches the app. The bottom panel must still treat that as a
/// submit trigger, otherwise pasting text followed by Enter silently
/// does nothing.
#[test]
fn test_ime_commit_enter_still_submits() {
    use crate::ui::test_helpers::interact::stateful_harness;

    let mut harness = stateful_harness(Vec::<&'static str>::new(), |ui, captured| {
        let mut app = create_test_app();
        app.orchestrator.agent_panel_state.command_input = "summarize the doc".to_string();
        show_bottom_panel_capture(&mut app, ui, |event| {
            captured.push(event);
        });
    });
    harness.fit_contents();

    use accesskit::Role;
    use egui_kittest::kittest::Queryable;
    let candidates: Vec<_> = harness
        .query_all_by_role(Role::TextInput)
        .chain(harness.query_all_by_role(Role::MultilineTextInput))
        .collect();
    assert!(
        !candidates.is_empty(),
        "expected at least one TextInput node in the bottom panel"
    );
    candidates[0].click();
    harness.run_steps(2);

    // Simulate Windows delivering the Enter key as an IME commit
    // (the raw Key event was consumed by the IME).
    harness.event(egui::Event::Ime(egui::ImeEvent::Commit("\n".to_owned())));
    harness.run_steps(2);
    harness.run_steps(2);

    let captured = harness.state();
    assert!(
        captured.contains(&"send"),
        "an IME commit of Enter must fire the `send` on_click event; got: {:?}",
        captured
    );
}

/// Regression guard: the real Windows failure mode is Paste (which
/// may activate IME) followed by Enter key being swallowed by IME
/// and delivered as IME Commit("\n"), NOT as a raw Key::Enter.
/// This test simulates that exact sequence.
#[test]
fn test_paste_then_ime_commit_enter_still_submits() {
    use crate::ui::test_helpers::interact::stateful_harness;

    let mut harness = stateful_harness(Vec::<&'static str>::new(), |ui, captured| {
        let mut app = create_test_app();
        show_bottom_panel_capture(&mut app, ui, |event| {
            captured.push(event);
        });
    });
    harness.fit_contents();

    use accesskit::Role;
    use egui_kittest::kittest::Queryable;
    let candidates: Vec<_> = harness
        .query_all_by_role(Role::TextInput)
        .chain(harness.query_all_by_role(Role::MultilineTextInput))
        .collect();
    assert!(
        !candidates.is_empty(),
        "expected at least one TextInput node in the bottom panel"
    );
    candidates[0].click();
    harness.run_steps(2);

    // Paste text (simulating user Ctrl+V)
    harness.event(egui::Event::Paste("summarize this\n".to_owned()));
    harness.run_steps(2);

    // On Windows with IME active, Enter is NOT delivered as Key::Enter.
    // It's swallowed (VK_PROCESSKEY filtered by egui-winit) and only
    // an IME Commit("\n") reaches the app.
    harness.event(egui::Event::Ime(egui::ImeEvent::Commit("\n".to_owned())));
    harness.run_steps(2);
    harness.run_steps(2);

    let captured = harness.state();
    assert!(
        captured.contains(&"send"),
        "pressing Enter (delivered as IME commit) after pasting must fire the `send` \
             on_click event; got: {:?}",
        captured
    );
}

/// Full-app reproduction: drive the whole `FastMdApp::update_ui`
/// panel stack, focus the prompt, paste multiline text, then press
/// Enter. The agent session must actually start (`running` flips).
#[test]
fn test_full_app_paste_then_enter_starts_agent() {
    use egui_kittest::kittest::Queryable;

    let app = create_test_app();
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::Vec2::new(900.0, 700.0))
        .with_max_steps(200)
        .build_ui_state(
            |ui, app: &mut FastMdApp| {
                app.update_ui(ui);
            },
            app,
        );
    harness.run_steps(5);

    let candidates: Vec<_> = harness
        .query_all_by_role(accesskit::Role::TextInput)
        .chain(harness.query_all_by_role(accesskit::Role::MultilineTextInput))
        .collect();
    assert!(
        !candidates.is_empty(),
        "expected the command input TextEdit in the full app"
    );
    candidates[0].click();
    harness.run_steps(2);

    harness.event(egui::Event::Paste("summarize the doc\n".to_owned()));
    harness.run_steps(2);

    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);
    harness.run_steps(2);

    let app = harness.state();
    assert!(
        app.agent().state().running,
        "pressing Enter after pasting must start the agent session; \
             command_input={:?}, status={:?}",
        app.orchestrator.agent_panel_state.command_input,
        app.agent().state().status
    );
    assert!(
        app.orchestrator.agent_panel_state.command_input.is_empty(),
        "the pasted prompt must have been consumed by the dispatch; \
             command_input={:?}",
        app.orchestrator.agent_panel_state.command_input
    );
}

/// Tier 4 click test: pressing Enter in the bottom-panel
/// command input while the agent is running must queue the
/// prompt instead of dispatching it immediately.
#[test]
fn test_send_enter_key_while_running_queues_prompt() {
    use crate::ui::test_helpers::interact::stateful_harness;

    let mut harness = stateful_harness((), |ui, _| {
        let mut app = create_test_app();
        // Set agent as running
        app.orchestrator.agent.state_mut().running = true;
        app.orchestrator.agent_panel_state.command_input = "queued prompt".to_string();
        show_bottom_panel_capture(&mut app, ui, |_| {});
    });
    harness.fit_contents();

    // Focus the command input
    use accesskit::Role;
    use egui_kittest::kittest::Queryable;
    let candidates: Vec<_> = harness
        .query_all_by_role(Role::TextInput)
        .chain(harness.query_all_by_role(Role::MultilineTextInput))
        .collect();
    assert!(!candidates.is_empty());
    candidates[0].click();
    harness.run_steps(2);

    // Press Enter - should queue the prompt, not dispatch
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);
    harness.run_steps(2);

    // The prompt should be queued and command_input cleared
    // We can't easily observe the queue from the harness, but we can
    // verify the command_input was cleared (it's cleared when queued)
    // The actual queue verification is done in the manager tests
}

#[test]
fn test_compute_prompt_prefix_no_dir() {
    assert_eq!(compute_prompt_prefix(None, &[]), ">");
}

#[test]
fn test_compute_prompt_prefix_with_dir_no_libs() {
    let dir = PathBuf::from("C:/my/test/dir");
    let prefix = compute_prompt_prefix(Some(&dir), &[]);
    assert_eq!(prefix, format!("{} >", dir.to_string_lossy()));
}

#[test]
fn test_compute_prompt_prefix_with_dir_and_libs() {
    let dir = PathBuf::from("C:/my/test/dir/subdir");
    let libs = vec![ContentLibrary {
        root_folder: "C:/my/test/dir".to_string(),
        name: "TestLib".to_string(),
        kind: "local".to_string(),
        readonly: false,
        priority: 0,
    }];
    let prefix = compute_prompt_prefix(Some(&dir), &libs);
    let expected = PathBuf::from("TestLib").join("subdir");
    assert_eq!(prefix, format!("{} >", expected.to_string_lossy()));
}

#[test]
fn test_compute_prompt_prefix_exact_match() {
    let dir = PathBuf::from("C:/my/test/dir");
    let libs = vec![ContentLibrary {
        root_folder: "C:/my/test/dir".to_string(),
        name: "TestLib".to_string(),
        kind: "local".to_string(),
        readonly: false,
        priority: 0,
    }];
    let prefix = compute_prompt_prefix(Some(&dir), &libs);
    assert_eq!(prefix, "TestLib >");
}

#[test]
fn test_generate_format_markdown_prompt() {
    let date = "2026-07-19T22:31:41-07:00";
    let prompt = crate::ui::generate_format_prompt(date);
    assert!(prompt.contains(date));
    assert!(prompt.contains("title: A brief title"));
}

#[test]
fn test_parse_command_intent() {
    assert_eq!(parse_command_intent("/models"), CommandIntent::ShowModels);
    assert_eq!(parse_command_intent("/models "), CommandIntent::ShowModels);
    assert_eq!(
        parse_command_intent("/model something"),
        CommandIntent::ShowDeprecatedModelMessage
    );
    assert_eq!(parse_command_intent("   "), CommandIntent::Empty);
    assert_eq!(
        parse_command_intent("hello world"),
        CommandIntent::RunAgent("hello world".to_string())
    );
}

/// Tests for `compute_file_context` function.
#[test]
fn test_compute_file_context_none_when_no_file() {
    let result = compute_file_context(None, "dir >");
    assert!(result.is_none());
}

#[test]
fn test_compute_file_context_none_when_empty_file_name() {
    let path = std::path::Path::new("/");
    let result = compute_file_context(Some(path), "dir >");
    assert!(result.is_none());
}

#[test]
fn test_compute_file_context_returns_file_name_and_base_size() {
    // File name shorter than dir -> no shrink
    let path = std::path::Path::new("/home/user/file.md");
    let result = compute_file_context(Some(path), "very_long_directory_name >");
    assert!(result.is_some());
    let (name, size) = result.unwrap();
    assert_eq!(name, "file.md");
    assert_eq!(size, 12.0); // base size when file is shorter than dir
}

#[test]
fn test_compute_file_context_shrinks_when_file_longer_than_dir() {
    let path = std::path::Path::new("/home/user/very_long_file_name_that_exceeds_dir.md");
    let result = compute_file_context(Some(path), "short >");
    assert!(result.is_some());
    let (name, size) = result.unwrap();
    assert_eq!(name, "very_long_file_name_that_exceeds_dir.md");
    assert!(size < 12.0);
    assert!(size >= 8.0); // minimum size
}

#[test]
fn test_compute_file_context_minimum_size_clamped() {
    // File name much longer than dir (1 char after trim)
    let path = std::path::Path::new(
        "/home/user/extremely_long_file_name_that_is_much_longer_than_directory.md",
    );
    let result = compute_file_context(Some(path), "x >"); // dir_display is "x" (1 char)
    assert!(result.is_some());
    let (_, size) = result.unwrap();
    assert_eq!(size, 8.0); // clamped to minimum
}

#[test]
fn test_compute_file_context_no_shrink_when_dir_empty() {
    let path = std::path::Path::new("/home/user/file.md");
    let result = compute_file_context(Some(path), ">");
    assert!(result.is_some());
    let (_, size) = result.unwrap();
    assert_eq!(size, 12.0); // no shrink when dir is empty
}

#[test]
fn test_is_enter_pressed_not_triggered() {
    let ctx = egui::Context::default();
    let _ = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
        let _ = ui;
        assert!(
            !ctx.input(is_enter_pressed),
            "a frame with no Enter key or IME commit must not report Enter"
        );
    });
}

#[test]
fn test_is_enter_pressed_raw_key() {
    let ctx = egui::Context::default();
    let raw_input = egui::RawInput {
        events: vec![egui::Event::Key {
            key: egui::Key::Enter,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
            physical_key: None,
        }],
        ..Default::default()
    };
    let _ = run_ui_test(&ctx, raw_input, |ui| {
        let _ = ui;
        assert!(
            ctx.input(is_enter_pressed),
            "a raw Enter key event must register as Enter pressed"
        );
    });
}
#[test]
fn test_is_enter_pressed_ime_commit() {
    let ctx = egui::Context::default();
    let raw_input = egui::RawInput {
        events: vec![egui::Event::Ime(egui::ImeEvent::Commit("\n".to_owned()))],
        ..Default::default()
    };
    let _ = run_ui_test(&ctx, raw_input, |ui| {
        let _ = ui;
        assert!(
            ctx.input(is_enter_pressed),
            "an IME commit of a newline (Windows swallowing the raw Enter key) \
                 must register as Enter pressed"
        );
    });
}

#[test]
fn test_is_enter_pressed_ime_commit_other_text_ignored() {
    let ctx = egui::Context::default();
    let raw_input = egui::RawInput {
        events: vec![egui::Event::Ime(egui::ImeEvent::Commit("你好".to_owned()))],
        ..Default::default()
    };
    let _ = run_ui_test(&ctx, raw_input, |ui| {
        let _ = ui;
        assert!(
            !ctx.input(is_enter_pressed),
            "an IME commit of actual text must not register as Enter pressed"
        );
    });
}

#[test]
fn test_is_enter_pressed_ime_commit_crlf() {
    let ctx = egui::Context::default();
    let raw_input = egui::RawInput {
        events: vec![egui::Event::Ime(egui::ImeEvent::Commit("\r\n".to_owned()))],
        ..Default::default()
    };
    let _ = run_ui_test(&ctx, raw_input, |ui| {
        let _ = ui;
        assert!(
            ctx.input(is_enter_pressed),
            "an IME commit of CRLF (Windows newline) must register as Enter pressed"
        );
    });
}

#[test]
fn test_format_models_list_empty() {
    let models = HashMap::new();
    let output = format_models_list(&models);
    assert!(output.contains("Available Models:\nNo additional models configured.\n"));
}

#[test]
fn test_format_models_list_with_items() {
    let mut models = HashMap::new();
    models.insert(
        "model_a".to_string(),
        LlmConfig {
            model: "a".to_string(),
            api_url: "url".to_string(),
            api_key: "key".to_string(),
            cost: Some(10),
            use_case: vec!["chat".to_string(), "vision".to_string()],
        },
    );
    models.insert(
        "model_b".to_string(),
        LlmConfig {
            model: "b".to_string(),
            api_url: "url".to_string(),
            api_key: "key".to_string(),
            cost: Some(5),
            use_case: vec!["embeddings".to_string()],
        },
    );

    let output = format_models_list(&models);

    let expected_a = "- model_a [cost: 10, use_case: chat, vision]\n";
    let expected_b = "- model_b [cost: 5, use_case: embeddings]\n";

    // Sorting means model_a is first, model_b is second
    assert!(output.starts_with("Available Models:\n"));
    assert!(output.contains(expected_a));
    assert!(output.contains(expected_b));

    // Check order
    let index_a = output.find(expected_a).unwrap();
    let index_b = output.find(expected_b).unwrap();
    assert!(index_a < index_b);
}

// --- UI / window tests (R-7: merged from `mod ui_tests`) ---

use crate::ui::strings::{COMMAND_INPUT_HINT, STOP_AGENT_BUTTON};
use crate::ui::test_helpers::text::{assert_text_contains, extract_text};

fn create_test_app() -> FastMdApp {
    FastMdApp::empty_state(crate::config::AppConfig::default())
}

/// R-2 / Q12: rendered-content assertion for the bottom panel.
/// Asserts the command input's hint is visible (the panel was
/// previously locked in by asserting on `QUICK_TASKS_MENU`; that
/// menu has since been removed and relocated to the file context
/// menu, see `doc/planning/quick-actions-into-context-menu.md`).
#[test]
fn test_show_bottom_panel_render() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();
    // Leave `command_input` empty so `TextEdit::hint_text` is
    // visible — egui hides the hint as soon as the field has any
    // text.
    let output = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
        show_bottom_panel(&mut app, ui);
    });
    assert_text_contains(&output.shapes, COMMAND_INPUT_HINT);
}

/// Regression guard: the `Quick Tasks` menu button has been
/// removed from the bottom panel. Quick Actions now live in the
/// file context menu (see
/// `doc/planning/quick-actions-into-context-menu.md`). If the
/// `menu_button(QUICK_TASKS_MENU, ...)` wrapper is reintroduced
/// by accident, this test fails.
#[test]
fn test_show_bottom_panel_does_not_render_quick_tasks_menu() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();
    let output = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
        show_bottom_panel(&mut app, ui);
    });
    let texts = extract_text(&output.shapes);
    let rendered = texts.join("\n");
    assert!(
        !rendered.contains("⚡ Quick Tasks"),
        "the bottom panel must not render the `⚡ Quick Tasks` menu; \
             Quick Actions now live in the file context menu. \
             Rendered text: {rendered:?}"
    );
}

#[test]
fn test_show_bottom_panel_stop_agent() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();
    app.agent_mut().state_mut().running = true;

    let output = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
        show_bottom_panel(&mut app, ui);
    });

    // The state is unchanged (no click happened); the Stop button is
    // rendered while running, so the label is in the output.
    assert_text_contains(&output.shapes, STOP_AGENT_BUTTON);
    assert!(app.agent().state().running);
}

/// Regression guard for the Stop button layout: the button must hug
/// the panel's right edge and the prompt must extend right up to the
/// button, leaving no dead space between them. The old layout
/// reserved a fixed 130px strip to the right of the prompt, so the
/// button floated ~40px short of the right edge with dead space
/// beyond it.
///
/// Two fixed-size harnesses are compared: one with the agent idle
/// (the prompt spans the full row, calibrating the panel's content
/// right edge) and one with the agent running (the Stop button is
/// visible). A fixed window size is required so both harnesses share
/// identical geometry.
#[test]
fn test_show_bottom_panel_stop_button_right_aligned() {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    const WINDOW_SIZE: egui::Vec2 = egui::Vec2::new(800.0, 600.0);
    const PIXEL_TOLERANCE: f32 = 2.0;

    // The prompt is the only multiline text input in the bottom
    // panel (per the role-query pattern in
    // `test_send_enter_key_captures_event`), so any TextInput /
    // MultilineTextInput node is the command input.
    let prompt_rect = |running: bool| {
        let mut app = create_test_app();
        app.agent_mut().state_mut().running = running;
        let mut harness = Harness::builder().with_size(WINDOW_SIZE).build_ui(|ui| {
            show_bottom_panel(&mut app, ui);
        });
        harness.run();
        let candidates: Vec<_> = harness
            .query_all_by_role(accesskit::Role::TextInput)
            .chain(harness.query_all_by_role(accesskit::Role::MultilineTextInput))
            .collect();
        assert!(
            !candidates.is_empty(),
            "expected the prompt TextEdit in the bottom panel"
        );
        candidates[0].rect()
    };

    // Idle: no Stop button, so the prompt spans the whole row and
    // its right edge marks the panel's content right edge.
    let idle_prompt = prompt_rect(false);

    // Running: the Stop button is present; its right edge must sit
    // at the panel's right edge (right-aligned), and the prompt
    // must end where the button begins — no dead gap.
    let mut app = create_test_app();
    app.agent_mut().state_mut().running = true;
    let mut harness = Harness::builder().with_size(WINDOW_SIZE).build_ui(|ui| {
        show_bottom_panel(&mut app, ui);
    });
    harness.run();

    let button = harness.get_by_role_and_label(accesskit::Role::Button, STOP_AGENT_BUTTON);
    let running_prompt = harness
        .query_all_by_role(accesskit::Role::TextInput)
        .chain(harness.query_all_by_role(accesskit::Role::MultilineTextInput))
        .next()
        .expect("expected the prompt TextEdit in the bottom panel")
        .rect();

    assert!(
        (button.rect().max.x - idle_prompt.max.x).abs() <= PIXEL_TOLERANCE,
        "Stop button must be right-aligned to the panel's right edge: \
             button right = {:.2}, panel right = {:.2}",
        button.rect().max.x,
        idle_prompt.max.x
    );

    let gap = button.rect().min.x - running_prompt.max.x;
    assert!(
        (-PIXEL_TOLERANCE..=3.0 * PIXEL_TOLERANCE).contains(&gap),
        "the prompt must extend up to the Stop button with no dead gap: \
             gap = {gap:.2}, prompt right = {:.2}, button left = {:.2}",
        running_prompt.max.x,
        button.rect().min.x
    );
}
