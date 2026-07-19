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
                }
            });
        } else if !app.tabs.is_empty() {
            ui.horizontal(|ui| {
                let mut tab_to_close = None;
                for (i, tab_path) in app.tabs.iter().enumerate() {
                    let is_selected = app.selected_file.as_ref() == Some(tab_path);
                    let title = tab_path.file_name().unwrap_or_default().to_string_lossy();

                    let response = ui.selectable_label(is_selected, title);
                    if response.clicked() {
                        app.selected_file = Some(tab_path.clone());
                    }
                    if response.middle_clicked() {
                        tab_to_close = Some(i);
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
                    });

                    if ui.button("❌").clicked() {
                        tab_to_close = Some(i);
                    }
                    ui.separator();
                }
                if let Some(i) = tab_to_close {
                    app.tabs.remove(i);
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