//! Bottom command/agent panel — prompt intent parsing (`/models`, agent prompt), agent status, response, and token-usage stats.

use crate::ui::FastMdApp;
use eframe::egui;
use egui::RichText;
use egui::containers::Panel;

/// Enum representing the parsed intent from a user command prompt.
#[derive(Debug, PartialEq)]
pub enum CommandIntent {
    ShowModels,
    ShowDeprecatedModelMessage,
    RunAgent(String),
    Empty,
}

/// Parses the user prompt to determine the intended command.
pub fn parse_command_intent(prompt: &str) -> CommandIntent {
    let trimmed = prompt.trim();
    if trimmed.starts_with("/models") {
        CommandIntent::ShowModels
    } else if trimmed.starts_with("/model ") {
        CommandIntent::ShowDeprecatedModelMessage
    } else if !trimmed.is_empty() {
        CommandIntent::RunAgent(trimmed.to_string())
    } else {
        CommandIntent::Empty
    }
}

/// Computes the prompt prefix based on the selected directory and available content libraries.
pub fn compute_prompt_prefix(
    selected_dir: Option<&std::path::Path>,
    content_libraries: &[crate::config::ContentLibrary],
) -> String {
    if let Some(dir) = selected_dir {
        let rel_str = crate::config::library_display_label(content_libraries, dir)
            .unwrap_or_else(|| dir.to_string_lossy().to_string());
        if rel_str.is_empty() {
            ">".to_string()
        } else {
            format!("{} >", rel_str)
        }
    } else {
        ">".to_string()
    }
}

pub fn format_models_list(
    models: &std::collections::HashMap<String, crate::config::LlmConfig>,
) -> String {
    let mut output = crate::ui::strings::MODELS_LIST_HEADER.to_string();
    let mut sorted_names: Vec<&String> = models.keys().collect();
    sorted_names.sort();

    for name in sorted_names {
        let model_cfg = &models[name];
        let use_cases = model_cfg.use_case.join(", ");
        output.push_str(&format!(
            "- {} [cost: {}, use_case: {}]\n",
            name,
            model_cfg.get_cost(),
            use_cases
        ));
    }
    if models.is_empty() {
        output.push_str(crate::ui::strings::MODELS_LIST_NO_ADDITIONAL);
    }
    output
}

/// Purpose: Applies the side effect of pressing Enter in the
/// command input (or clicking an equivalent submit trigger).
/// Inputs: app (the application state)
/// Outputs: ()
/// Purity: Impure (mutates `app.orchestrator.agent`, `app.orchestrator.config`,
/// `app.orchestrator.selection`).
/// Preconditions: `app.agent().command_input` contains the user's
/// prompt. The `command_input` is consumed and cleared as part of
/// the call.
/// Postconditions: Dispatches based on `parse_command_intent`:
///   * `ShowModels` — sets status to "Done", response to the
///     formatted model list, and show_results to `true`.
///   * `ShowDeprecatedModelMessage` — sets status to "Error" and
///     response to the deprecation message.
///   * `RunAgent(agent_prompt)` — starts an agent session with
///     the prompt and the current selection context, and sets
///     show_results to `true`.
///   * `Empty` — no-op.
///
/// The Enter-key handler in `show_bottom_panel` calls this
/// function. It is extracted so the dispatch can be unit-tested
/// without driving the egui harness.
pub fn apply_send_click(app: &mut FastMdApp) {
    let prompt = app.agent_mut().command_input.trim_end().to_string();
    app.agent_mut().command_input.clear();

    match parse_command_intent(&prompt) {
        CommandIntent::ShowModels => {
            app.agent_mut().set_status("Done".to_string());
            let models_response = format_models_list(&app.orchestrator.config.models);
            app.agent_mut().set_response(models_response);
            app.agent_mut().set_show_results(true);
        }
        CommandIntent::ShowDeprecatedModelMessage => {
            app.agent_mut().set_status("Error".to_string());
            app.agent_mut()
                .set_response(crate::ui::strings::DEPRECATED_MODEL_MESSAGE.to_string());
            app.agent_mut().set_show_results(true);
        }
        CommandIntent::RunAgent(agent_prompt) => {
            let tx = app.orchestrator.tx.clone();
            let file = app.selection().selected_file().cloned();
            let dir = app.selection().selected_dir().cloned();
            let files = app.selection().selected_files().clone();
            let bus = app.orchestrator.file_event_bus.clone();
            app.agent_mut()
                .start_session(tx, agent_prompt, file, dir, files, bus);
            app.agent_mut().set_show_results(true);
        }
        CommandIntent::Empty => {}
    }
}

pub fn show_bottom_panel(app: &mut FastMdApp, parent_ui: &mut egui::Ui) {
    show_bottom_panel_capture(app, parent_ui, |_| {});
}

/// Tier 4 test variant of [`show_bottom_panel`]. The `on_click`
/// callback is invoked after every dispatch trigger (Enter key on
/// the command input) with a stable event name. The production
/// caller ([`show_bottom_panel`]) passes a no-op closure; the
/// test caller in `tests::test_send_enter_key_captures_event`
/// passes a closure that pushes the event into the harness's
/// persistent state.
pub fn show_bottom_panel_capture(
    app: &mut FastMdApp,
    parent_ui: &mut egui::Ui,
    mut on_click: impl FnMut(&'static str),
) {
    // egui 0.35 unified `TopBottomPanel`/`SidePanel` into `Panel`,
    // and panels now allocate within a parent `&mut Ui` (using
    // `show_inside`) rather than from a `&Context`.
    Panel::bottom("bottom_panel")
        .resizable(true)
        // egui 0.35: `min_height` was replaced by `min_size` (and the
        // same `size` parameter is used for all four sides).
        .min_size(32.0)
        .show(parent_ui, |ui| {
            // Some branches below still need the context for input
            // polling — pull it from the inner Ui.
            let ctx = ui.ctx().clone();
            ui.horizontal(|ui| {
                let prompt_prefix = compute_prompt_prefix(
                    app.selection().selected_dir().map(|p| p.as_path()),
                    app.content_libraries(),
                );
                ui.label(RichText::new(prompt_prefix).monospace().strong());

                // Lay the Stop button out right-to-left first so it hugs
                // the panel's right edge, then measure where it ends to
                // give the prompt every remaining pixel. This replaces the
                // old fixed 130px reserve, which left dead space between
                // the prompt and the button.
                let row = ui.available_rect_before_wrap();
                let mut stop_clicked = false;
                let mut prompt_right = row.max.x;
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if app.agent().state().running {
                        let response = ui.button(
                            RichText::new(crate::ui::strings::STOP_AGENT_BUTTON)
                                .color(egui::Color32::RED),
                        );
                        stop_clicked = response.clicked();
                        prompt_right = response.rect.left();
                    }
                });

                let prompt_rect = egui::Rect::from_min_max(
                    row.min,
                    egui::pos2(prompt_right.max(row.min.x), row.max.y),
                );
                let response = ui.put(
                    prompt_rect,
                    egui::TextEdit::multiline(&mut app.agent_mut().command_input)
                        .desired_width(f32::INFINITY)
                        .hint_text(crate::ui::strings::COMMAND_INPUT_HINT),
                );

                let mut submit = false;
                if response.has_focus()
                    && ctx.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift)
                {
                    submit = true;
                }

                // The bottom panel used to host a `⚡ Quick Tasks`
                // menu button whose only item was `Format Markdown`.
                // That entry point duplicated the file context
                // menu's [Format Markdown] action, so the button
                // was removed. Quick Actions now live in the
                // file context menu — see
                // `doc/planning/quick-actions-into-context-menu.md`.

                if stop_clicked {
                    app.agent_mut().cancel();
                }

                if submit {
                    apply_send_click(app);
                    on_click("send");
                }
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ContentLibrary, LlmConfig};
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
        app.orchestrator.agent.command_input = "/models".to_string();
        assert!(
            !app.orchestrator.agent.show_results(),
            "show_results must start false"
        );

        apply_send_click(&mut app);

        assert_eq!(app.orchestrator.agent.state().status, "Done");
        assert!(
            app.orchestrator.agent.show_results(),
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
            app.orchestrator.agent.command_input.is_empty(),
            "command_input must be cleared after dispatch"
        );
    }

    /// Tier 1 test: an empty prompt produces `CommandIntent::Empty`
    /// and `apply_send_click` is a no-op (no status change, no
    /// show_results toggle, command_input is still cleared).
    #[test]
    fn test_apply_send_click_empty_prompt_is_noop() {
        let mut app = create_test_app();
        app.orchestrator.agent.command_input = "   ".to_string();

        apply_send_click(&mut app);

        assert!(
            !app.orchestrator.agent.show_results(),
            "Empty intent must not toggle show_results"
        );
        assert!(
            app.orchestrator.agent.command_input.is_empty(),
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
        app.orchestrator.agent.command_input = "hello world".to_string();

        apply_send_click(&mut app);

        assert!(
            app.orchestrator.agent.show_results(),
            "RunAgent dispatch must set show_results"
        );
        assert!(
            app.orchestrator.agent.command_input.is_empty(),
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
            app.orchestrator.agent.command_input = "/models".to_string();
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
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
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
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
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

        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
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
}
