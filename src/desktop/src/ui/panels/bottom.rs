//! Bottom command/agent panel Ã¢â‚¬â€ prompt intent parsing (`/models`, agent prompt), agent status, response, and token-usage stats.

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

pub fn show_bottom_panel(app: &mut FastMdApp, parent_ui: &mut egui::Ui) {
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

                let text_width = (ui.available_width() - 130.0).max(0.0);
                let response = ui.add_sized(
                    egui::vec2(text_width, ui.available_height()),
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

                ui.vertical(|ui| {
                    ui.menu_button(crate::ui::strings::QUICK_TASKS_MENU, |ui| {
                        if ui
                            .button(crate::ui::strings::FORMAT_MARKDOWN_ACTION)
                            .clicked()
                        {
                            let now = chrono::Local::now();
                            let date_str = now.to_rfc3339();
                            app.agent_mut().command_input =
                                crate::ui::generate_format_prompt(&date_str);
                            submit = true;
                            ui.close();
                        }
                    });

                    if app.agent().state().running
                        && ui
                            .button(
                                RichText::new(crate::ui::strings::STOP_AGENT_BUTTON)
                                    .color(egui::Color32::RED),
                            )
                            .clicked()
                    {
                        app.agent_mut().cancel();
                    }
                });

                if submit {
                    let prompt = app.agent_mut().command_input.trim_end().to_string();
                    app.agent_mut().command_input.clear();

                    match parse_command_intent(&prompt) {
                        CommandIntent::ShowModels => {
                            app.agent_mut().set_status("Done".to_string());
                            let models_response = format_models_list(&app.config.models);
                            app.agent_mut().set_response(models_response);
                            app.agent_mut().set_show_results(true);
                        }
                        CommandIntent::ShowDeprecatedModelMessage => {
                            app.agent_mut().set_status("Error".to_string());
                            app.agent_mut().set_response(
                                crate::ui::strings::DEPRECATED_MODEL_MESSAGE.to_string(),
                            );
                            app.agent_mut().set_show_results(true);
                        }
                        CommandIntent::RunAgent(agent_prompt) => {
                            let tx = app.tx.clone();
                            let file = app.selection().selected_file().cloned();
                            let dir = app.selection().selected_dir().cloned();
                            let files = app.selection().selected_files().clone();
                            let bus = app.file_event_bus.clone();
                            app.agent_mut()
                                .start_session(tx, agent_prompt, file, dir, files, bus);
                            app.agent_mut().set_show_results(true);
                        }
                        CommandIntent::Empty => {}
                    }
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

    use crate::ui::strings::{QUICK_TASKS_MENU, STOP_AGENT_BUTTON};
    use crate::ui::test_helpers::text::assert_text_contains;

    fn create_test_app() -> FastMdApp {
        FastMdApp::empty_state(crate::config::AppConfig::default())
    }

    #[test]
    fn test_show_bottom_panel_render() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        app.agent_mut().command_input = "test input".to_string();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            show_bottom_panel(&mut app, ui);
        });
        // R-2 / Q12: replace the tautological state check with a
        // rendered-content assertion. The Quick Tasks menu is the
        // stable header for the bottom panel.
        //
        // Note: we do not assert on COMMAND_INPUT_HINT because
        // `TextEdit::hint_text` is hidden once the user has typed
        // anything (egui's standard behavior). The hint is only
        // visible when the field is empty; an empty-field test
        // would catch a regression there.
        assert_text_contains(&output.shapes, QUICK_TASKS_MENU);
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
}
