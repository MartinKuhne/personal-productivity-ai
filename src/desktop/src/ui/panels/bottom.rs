use crate::ui::FastMdApp;
use eframe::egui;
use egui::RichText;

pub fn show_bottom_panel(app: &mut FastMdApp, ctx: &egui::Context) {
    egui::TopBottomPanel::bottom("bottom_panel")
        .resizable(true)
        .min_height(32.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let prompt_prefix = if let Some(dir) = &app.selected_dir {
                    let rel = dir
                        .strip_prefix(&app.root_path)
                        .unwrap_or(dir)
                        .to_string_lossy();
                    if rel.is_empty() {
                        ">".to_string()
                    } else {
                        format!("{} >", rel)
                    }
                } else {
                    ">".to_string()
                };
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
                            app.command_input = format!("Format the current document into correct markdown and use this template for the yaml front matter. Focus ONLY on the currently active file, and DO NOT use list_files or search for other files.\n```yaml\n---\ntitle: A brief title\nsummary: A three sentence summary of the contents\ntags: [\"tag1\",\"tag2\"]\nheader-date: {}\n---\n```", date_str);
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
                    
                    if prompt.starts_with("/models") {
                        let config = crate::config::load_config();
                        let mut output = "Available Models:\n".to_string();
                        for (name, model_cfg) in &config.models {
                            output.push_str(&format!("- {}: {}\n", name, model_cfg.model));
                        }
                        if config.models.is_empty() {
                            output.push_str("No additional models configured.\n");
                        }
                        app.agent_status = "Done".to_string();
                        app.agent_response = output;
                        app.show_agent_results = true;
                    } else if prompt.starts_with("/model ") {
                        let model_name = prompt.trim_start_matches("/model ").trim();
                        let mut config = crate::config::load_config();
                        if let Some(model_cfg) = config.models.get(model_name) {
                            config.model = model_cfg.model.clone();
                            config.api_url = model_cfg.api_url.clone();
                            config.api_key = model_cfg.api_key.clone();
                            
                            let config_path = crate::config::get_config_path();
                            if let Ok(yaml_str) = serde_yaml::to_string(&config) {
                                let _ = std::fs::write(&config_path, yaml_str);
                                app.agent_status = "Done".to_string();
                                app.agent_response = format!("Switched to model: {}", model_name);
                            } else {
                                app.agent_status = "Error".to_string();
                                app.agent_response = "Failed to save configuration.".to_string();
                            }
                        } else {
                            app.agent_status = "Error".to_string();
                            app.agent_response = format!("Model '{}' not found in configuration.", model_name);
                        }
                        app.show_agent_results = true;
                    } else if !prompt.trim().is_empty() {
                        app.agent_status = "Initializing agent...".to_string();
                        app.agent_thinking.clear();
                        if app.agent_history.is_none() || !app.show_agent_results {
                            app.agent_response.clear();
                            app.agent_history = None;
                        } else {
                            app.agent_response.push_str(&format!("> **User:** {}\n\n", prompt));
                        }
                        app.show_agent_results = true;
                        app.agent_running = true;

                        let cancel_flag =
                            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                        app.agent_cancel_flag = Some(cancel_flag.clone());
                        let tx_gui_agent = app.tx.clone();
                        let root_path_agent = app.root_path.clone();
                        let active_file_agent = app.selected_file.clone();
                        let active_dir_agent = app.selected_dir.clone();
                        let history = app.agent_history.clone();
                        let current_response = app.agent_response.clone();

                        crate::agent::run_agent(
                            tx_gui_agent,
                            root_path_agent,
                            active_file_agent,
                            active_dir_agent,
                            prompt,
                            cancel_flag,
                            history,
                            current_response,
                        );
                    }
                }
            });
        });
}