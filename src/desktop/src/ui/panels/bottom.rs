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
                    let mut rel_str = dir.to_string_lossy().to_string();
                    for lib in &app.content_libraries {
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
                        let mut output = "Available Models:\n".to_string();
                        for (name, model_cfg) in &app.config.models {
                            let use_cases = model_cfg.use_case.join(", ");
                            output.push_str(&format!(
                                "- {} [cost: {}, use_case: {}]\n",
                                name, model_cfg.get_cost(), use_cases
                            ));
                        }
                        if app.config.models.is_empty() {
                            output.push_str("No additional models configured.\n");
                        }
                        app.agent_status = "Done".to_string();
                        app.agent_response = output;
                        app.show_agent_results = true;
                    } else if prompt.starts_with("/model ") {
                        app.agent_status = "Error".to_string();
                        app.agent_response = "The /model command is deprecated. Models are now automatically selected based on use case and cost.".to_string();
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
                        let active_file_agent = app.selected_file.clone();
                        let active_dir_agent = app.selected_dir.clone();
                        let history = app.agent_history.clone();
                        let current_response = app.agent_response.clone();

                        crate::agent::run_agent(
                            app.config.clone(),
                            tx_gui_agent,
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