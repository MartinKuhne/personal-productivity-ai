use crate::ui::FastMdApp;
use eframe::egui;
use egui::RichText;

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
    let mut output = "Available Models:\n".to_string();
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
        output.push_str("No additional models configured.\n");
    }
    output
}

pub fn show_bottom_panel(app: &mut FastMdApp, ctx: &egui::Context) {
    egui::TopBottomPanel::bottom("bottom_panel")
        .resizable(true)
        .min_height(32.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let prompt_prefix = compute_prompt_prefix(
                    app.selected_dir.as_deref(),
                    &app.content_libraries,
                );
                ui.label(RichText::new(prompt_prefix).monospace().strong());

                let text_width = ui.available_width() - 130.0;
                let response = ui.add_sized(
                    egui::vec2(text_width, ui.available_height()),
                    egui::TextEdit::multiline(&mut app.command_input)
                        .desired_width(f32::INFINITY)
                        .hint_text("Type command (Enter to submit, Shift+Enter for new line)"),
                );

                let mut submit = false;
                if response.has_focus()
                    && ctx.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift)
                {
                    submit = true;
                }

                ui.vertical(|ui| {
                    ui.menu_button("⚡ Quick Tasks", |ui| {
                        if ui.button("Format Markdown").clicked() {
                            let now = chrono::Local::now();
                            let date_str = now.to_rfc3339();
                            app.command_input = crate::ui::generate_format_prompt(&date_str);
                            submit = true;
                            ui.close_menu();
                        }
                    });

                    if app.agent.state().running
                        && ui
                            .button(RichText::new("⏹ Stop").color(egui::Color32::RED))
                            .clicked()
                    {
                        app.agent.cancel();
                    }
                });

                if submit {
                    let prompt = app.command_input.trim_end().to_string();
                    app.command_input.clear();

                    match parse_command_intent(&prompt) {
                        CommandIntent::ShowModels => {
                            app.agent.set_status("Done".to_string());
                            app.agent.set_response(format_models_list(&app.config.models));
                            app.show_agent_results = true;
                        }
                        CommandIntent::ShowDeprecatedModelMessage => {
                            app.agent.set_status("Error".to_string());
                            app.agent.set_response(
                                "The /model command is deprecated. Models are now automatically selected based on use case and cost.".to_string(),
                            );
                            app.show_agent_results = true;
                        }
                        CommandIntent::RunAgent(agent_prompt) => {
                            app.agent.start_session(
                                app.tx.clone(),
                                agent_prompt,
                                app.selected_file.clone(),
                                app.selected_dir.clone(),
                                app.selected_files.clone(),
                                app.file_event_bus.clone(),
                            );
                            app.show_agent_results = true;
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
}

#[cfg(test)]
mod ui_tests {
    use super::*;

    fn create_test_app() -> FastMdApp {
        FastMdApp::empty_state(crate::config::AppConfig::default())
    }

    #[test]
    fn test_show_bottom_panel_render() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        app.command_input = "test input".to_string();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            show_bottom_panel(&mut app, ctx);
        });
        assert_eq!(app.command_input, "test input");
    }

    #[test]
    fn test_show_bottom_panel_stop_agent() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        app.agent.state_mut().running = true;

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            show_bottom_panel(&mut app, ctx);
        });

        assert!(app.agent.state().running);
    }
}
