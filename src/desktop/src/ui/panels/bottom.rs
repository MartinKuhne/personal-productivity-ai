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
/// Inputs: `prompt` (the trimmed string from the user).
/// Outputs: Returns a `CommandIntent`.
/// Purity: Pure.
/// Preconditions: `prompt` is a string slice.
/// Postconditions: Returns `ShowModels`, `ShowDeprecatedModelMessage`, `RunAgent` (if not empty), or `Empty`.
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
/// Inputs: `selected_dir` (optional path), `content_libraries` (list of content libraries).
/// Outputs: Returns a string representing the prompt prefix.
/// Purity: Pure.
/// Preconditions: `selected_dir` should be a valid path if Some.
/// Postconditions: Returns `>` if no dir is selected, or if the relative path is empty, otherwise `relative/path >`.
pub fn compute_prompt_prefix(
    selected_dir: Option<&std::path::Path>,
    content_libraries: &[crate::config::ContentLibrary],
) -> String {
    if let Some(dir) = selected_dir {
        let mut rel_str = dir.to_string_lossy().to_string();
        for lib in content_libraries {
            if let Ok(rel) = dir.strip_prefix(std::path::Path::new(&lib.root_folder)) {
                let lib_path = std::path::Path::new(&lib.name).join(rel);
                rel_str = lib_path.to_string_lossy().to_string();
                break;
            }
        }
        if rel_str.is_empty() {
            ">".to_string()
        } else {
            format!("{} >", rel_str)
        }
    } else {
        ">".to_string()
    }
}

/// Generates the prompt for formatting the current document into correct markdown.
/// Inputs: `current_date` (the current date string in RFC3339 format).
/// Outputs: Returns a string containing the prompt.
/// Purity: Pure.
/// Preconditions: `current_date` must be a valid string.
/// Postconditions: Returns a formatted string containing the YAML template.
pub fn generate_format_markdown_prompt(current_date: &str) -> String {
    format!("Format the current document into correct markdown and use this template for the yaml front matter. Focus ONLY on the currently active file, and DO NOT use list_files or search for other files.\n```yaml\n---\ntitle: A brief title\nsummary: A three sentence summary of the contents\ntags: [\"tag1\",\"tag2\"]\nheader-date: {}\n---\n```", current_date)
}

/// Formats the available models into a human-readable string.
/// Inputs: `models` (hash map of available models).
/// Outputs: Returns a string with the models list.
/// Purity: Pure.
/// Preconditions: `models` hash map may be empty or contain `LlmConfig`.
/// Postconditions: Returns a formatted string listing models deterministically sorted by name.
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
                let prompt_prefix = compute_prompt_prefix(app.selected_dir.as_deref(), &app.content_libraries);
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
                            app.command_input = generate_format_markdown_prompt(&date_str);
                            submit = true;
                            ui.close_menu();
                        }
                    });

                    if app.agent_running
                        && ui
                            .button(RichText::new("⏹ Stop").color(egui::Color32::RED))
                            .clicked()
                    {
                        if let Some(flag) = &app.agent_cancel_flag {
                            flag.store(true, std::sync::atomic::Ordering::SeqCst);
                        }
                        app.agent_running = false;
                        app.agent_status = "Aborted by user.".to_string();
                    }
                });

                if submit {
                    let prompt = app.command_input.trim_end().to_string();
                    app.command_input.clear();
                    
                    match parse_command_intent(&prompt) {
                        CommandIntent::ShowModels => {
                            app.agent_status = "Done".to_string();
                            app.agent_response = format_models_list(&app.config.models);
                            app.show_agent_results = true;
                        }
                        CommandIntent::ShowDeprecatedModelMessage => {
                            app.agent_status = "Error".to_string();
                            app.agent_response = "The /model command is deprecated. Models are now automatically selected based on use case and cost.".to_string();
                            app.show_agent_results = true;
                        }
                        CommandIntent::RunAgent(agent_prompt) => {
                            app.agent_status = "Initializing agent...".to_string();
                            app.agent_thinking.clear();
                            if app.agent_history.is_none() || !app.show_agent_results {
                                app.agent_response.clear();
                                app.agent_history = None;
                            } else {
                                app.agent_response.push_str(&format!("> **User:** {}\n\n", agent_prompt));
                            }
                            app.show_agent_results = true;
                            app.agent_running = true;

                            let cancel_flag =
                                std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                            app.agent_cancel_flag = Some(cancel_flag.clone());
                            let tx_gui_agent = app.tx.clone();
                            let active_file_agent = app.selected_file.clone();
                            let active_dir_agent = app.selected_dir.clone();
                            let history = app.agent_history.clone();
                            let current_response = app.agent_response.clone();
                            let selected_files_agent = app.selected_files.clone();

                            crate::agent::run_agent(
                                app.config.clone(),
                                tx_gui_agent,
                                active_file_agent,
                                active_dir_agent,
                                selected_files_agent,
                                agent_prompt,
                                cancel_flag,
                                history,
                                current_response,
                            );
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
        let expected = PathBuf::from("TestLib").join("");
        assert_eq!(prefix, format!("{} >", expected.to_string_lossy()));
    }

    #[test]
    fn test_generate_format_markdown_prompt() {
        let date = "2026-07-19T22:31:41-07:00";
        let prompt = generate_format_markdown_prompt(date);
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
    use std::collections::{BTreeMap, BTreeSet, HashSet};
    use std::sync::{Arc, Mutex};
    use crate::background::BackgroundProcessManager;

    fn create_test_app() -> FastMdApp {
        let (tx, rx) = std::sync::mpsc::channel();
        let config = crate::config::AppConfig::default();
        FastMdApp {
            content_libraries: vec![],
            rx,
            tx,
            all_files: vec![],
            all_dirs: vec![],
            file_tags: BTreeMap::new(),
            all_tags: BTreeSet::new(),
            selected_tag: None,
            indexing_finished: false,
            indexing_finished_handled: false,
            left_panel_width: None,
            selected_file: None,
            selected_files: HashSet::new(),
            selected_dir: None,
            expanded_dirs: HashSet::new(),
            loaded_path: None,
            current_yaml: None,
            current_markdown: String::new(),
            tabs: vec![],
            move_dialog_open: false,
            file_to_move: None,
            selected_move_folder: None,
            create_dir_dialog_open: false,
            create_dir_parent: None,
            create_dir_name: String::new(),
            rename_dialog_open: false,
            file_to_rename: None,
            rename_new_name: String::new(),
            command_input: String::new(),
            toc: vec![],
            scroll_to_header_id: None,
            _watcher: None,
            show_agent_results: false,
            agent_running: false,
            agent_status: String::new(),
            agent_thinking: String::new(),
            agent_response: String::new(),
            agent_scroll_to_id: None,
            agent_cancel_flag: None,
            agent_history: None,
            left_panel_reset_count: 0,
            submit_prompt: None,
            editor_state: crate::editor::EditorState::default(),
            inline_editor_enabled: true,
            background_manager: Arc::new(Mutex::new(BackgroundProcessManager::new())),
            show_background_logs: false,
            config,
        }
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
        app.agent_running = true;
        let cancel_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        app.agent_cancel_flag = Some(cancel_flag.clone());

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            show_bottom_panel(&mut app, ctx);
        });
        assert!(app.agent_running);
    }
}

