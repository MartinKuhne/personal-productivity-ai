use crate::ui::FastMdApp;
use crate::ui::render::{render_markdown, render_yaml_table};
use eframe::egui;
use egui::RichText;

pub fn show_center_panel(app: &mut FastMdApp, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        if app.show_agent_results {
            ui.horizontal(|ui| {
                ui.heading(
                    RichText::new("🤖 FastMD Agent Session")
                        .size(18.0)
                        .strong()
                        .color(egui::Color32::from_rgb(100, 200, 255)),
                );
                ui.separator();
                if ui.button("Back to Document").clicked() {
                    app.show_agent_results = false;
                    app.agent_history = None;
                }
            });
            ui.separator();

            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Status: {}", app.agent_status)).strong());
                if app.agent_running {
                    ui.spinner();
                }
            });
            ui.add_space(8.0);

            egui::ScrollArea::vertical().id_source("agent_thinking_scroll").stick_to_bottom(true).show(ui, |ui| {
                if !app.agent_thinking.is_empty() {
                    ui.collapsing("Thinking Process", |ui| {
                        ui.label(
                            egui::RichText::new(&app.agent_thinking)
                                .italics()
                                .color(egui::Color32::from_rgb(160, 160, 160)),
                        );
                    });
                    ui.add_space(8.0);
                }

                if !app.agent_response.is_empty() {
                    ui.heading("Response");
                    ui.separator();
                    render_markdown(ui, &app.agent_response, &mut app.agent_scroll_to_id);
                    if app.agent_running {
                        ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                    }
                }
            });
        } else if !app.tabs.is_empty() {
            ui.horizontal(|ui| {
                enum TabAction {
                    Close(usize),
                    CloseOthers(usize),
                    CloseAll,
                }
                let mut tab_action = None;

                for (i, tab_path) in app.tabs.iter().enumerate() {
                    let is_selected = app.selected_file.as_ref() == Some(tab_path);
                    let title = tab_path.file_name().unwrap_or_default().to_string_lossy();

                    let response = ui.selectable_label(is_selected, title);
                    if response.clicked() {
                        app.selected_file = Some(tab_path.clone());
                    }
                    if response.middle_clicked() {
                        tab_action = Some(TabAction::Close(i));
                    }
                    response.context_menu(|ui| {
                        if ui.button("Edit").clicked() {
                            if app.inline_editor_enabled {
                                if let Ok(content) = std::fs::read_to_string(tab_path) {
                                    app.editor_state.open(tab_path, &content);
                                }
                            } else {
                                let _ = std::process::Command::new("cmd")
                                    .args(["/c", "start", "", &tab_path.to_string_lossy()])
                                    .spawn();
                            }
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Close").clicked() {
                            tab_action = Some(TabAction::Close(i));
                            ui.close_menu();
                        }
                        if ui.button("Close Others").clicked() {
                            tab_action = Some(TabAction::CloseOthers(i));
                            ui.close_menu();
                        }
                        if ui.button("Close All").clicked() {
                            tab_action = Some(TabAction::CloseAll);
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Copy Path").clicked() {
                            ui.output_mut(|o| o.copied_text = tab_path.to_string_lossy().to_string());
                            ui.close_menu();
                        }
                        if ui.button("Show in File Explorer").clicked() {
                            #[cfg(target_os = "windows")]
                            {
                                use std::os::windows::process::CommandExt;
                                let _ = std::process::Command::new("explorer")
                                    .raw_arg(format!("/select,\"{}\"", tab_path.to_string_lossy()))
                                    .spawn();
                            }
                            #[cfg(not(target_os = "windows"))]
                            {
                                let _ = std::process::Command::new("explorer")
                                    .arg(tab_path)
                                    .spawn();
                            }
                            ui.close_menu();
                        }
                        if ui.button("Open in Editor").clicked() {
                            let _ = std::process::Command::new("cmd")
                                .args(["/c", "start", "", &tab_path.to_string_lossy()])
                                .spawn();
                            ui.close_menu();
                        }
                        if ui.button("Format Markdown").clicked() {
                            let now = chrono::Local::now();
                            let date_str = now.to_rfc3339();
                            let prompt = format!("Format the current document into correct markdown and use this template for the yaml front matter. Focus ONLY on the currently active file, and DO NOT use list_files or search for other files.\n```yaml\n---\ntitle: A brief title\nsummary: A three sentence summary of the contents\ntags: [\"tag1\",\"tag2\"]\nheader-date: {}\n---\n```", date_str);
                            app.submit_prompt = Some(prompt);
                            app.selected_file = Some(tab_path.clone());
                            ui.close_menu();
                        }
                    });

                    if ui.button("❌").clicked() {
                        tab_action = Some(TabAction::Close(i));
                    }
                    ui.separator();
                }

                if let Some(action) = tab_action {
                    match action {
                        TabAction::Close(i) => {
                            if i < app.tabs.len() {
                                app.tabs.remove(i);
                            }
                        }
                        TabAction::CloseOthers(i) => {
                            if i < app.tabs.len() {
                                let keep = app.tabs[i].clone();
                                app.tabs.clear();
                                app.tabs.push(keep);
                            }
                        }
                        TabAction::CloseAll => {
                            app.tabs.clear();
                        }
                    }

                    if let Some(selected) = &app.selected_file {
                        if !app.tabs.contains(selected) {
                            app.selected_file = app.tabs.last().cloned();
                        }
                    } else if !app.tabs.is_empty() {
                        app.selected_file = app.tabs.last().cloned();
                    } else {
                        app.selected_file = None;
                    }
                }
            });
            ui.separator();

            if let Some(selected_path) = &app.selected_file {
                ui.horizontal(|ui| {
                    ui.heading(
                        RichText::new(
                            selected_path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy(),
                        )
                        .size(18.0)
                        .strong(),
                    );
                    ui.label(
                        RichText::new(format!("({})", selected_path.to_string_lossy()))
                            .size(11.0)
                            .italics()
                            .color(egui::Color32::GRAY),
                    );
                });
                ui.separator();

                egui::ScrollArea::vertical().id_source("main_markdown_scroll").show(ui, |ui| {
                    if let Some(yaml) = &app.current_yaml {
                        render_yaml_table(ui, yaml);
                    }

                    render_markdown(ui, &app.current_markdown, &mut app.scroll_to_header_id);
                });
            }
        } else {
            ui.centered_and_justified(|ui| {
                ui.label(
                    RichText::new(
                        "Select a markdown file from the left pane to view its content",
                    )
                    .size(15.0)
                    .italics()
                    .color(egui::Color32::GRAY),
                );
            });
        }
    });
}