use eframe::egui;
use crate::ui::FastMdApp;
use crate::background::LogCategory;

pub fn show_background_logs_window(app: &mut FastMdApp, ctx: &egui::Context) {
    if !app.show_background_logs {
        return;
    }

    let mut open = app.show_background_logs;
    
    egui::Window::new("Background Processes")
        .open(&mut open)
        .resizable(true)
        .collapsible(true)
        .default_size([600.0, 400.0])
        .show(ctx, |ui| {
            let mut mgr = app.background_manager.lock().unwrap();

            ui.horizontal(|ui| {
                ui.label("Search:");
                ui.text_edit_singleline(&mut mgr.search_text);
                
                ui.label("Category:");
                egui::ComboBox::from_id_source("category_filter")
                    .selected_text(match mgr.filter_category {
                        Some(c) => c.to_string(),
                        None => "All".to_string(),
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut mgr.filter_category, None, "All");
                        ui.selectable_value(&mut mgr.filter_category, Some(LogCategory::Indexer), "Indexer");
                        ui.selectable_value(&mut mgr.filter_category, Some(LogCategory::Watcher), "Watcher");
                        ui.selectable_value(&mut mgr.filter_category, Some(LogCategory::PdfConverter), "PDF Converter");
                        ui.selectable_value(&mut mgr.filter_category, Some(LogCategory::ImageVision), "Image Vision");
                        ui.selectable_value(&mut mgr.filter_category, Some(LogCategory::LlmTools), "LLM Tools");
                    });

                ui.checkbox(&mut mgr.auto_scroll, "Auto-scroll");
                
                if ui.button("Clear").clicked() {
                    mgr.clear_logs();
                }
            });

            ui.separator();

            let logs: Vec<_> = mgr.get_logs().iter().filter(|log| {
                if let Some(cat) = mgr.filter_category {
                    if log.category != cat {
                        return false;
                    }
                }
                if !mgr.search_text.is_empty() {
                    let search_lower = mgr.search_text.to_lowercase();
                    if !log.message.to_lowercase().contains(&search_lower) {
                        return false;
                    }
                }
                true
            }).cloned().collect();

            let row_height = ui.text_style_height(&egui::TextStyle::Body);
            
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .stick_to_bottom(mgr.auto_scroll)
                .show_rows(ui, row_height, logs.len(), |ui, row_range| {
                    for i in row_range {
                        let log = &logs[i];
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(log.timestamp.format("%H:%M:%S%.3f").to_string())
                                    .color(egui::Color32::DARK_GRAY)
                            );
                            ui.label(
                                egui::RichText::new(format!("[{}]", log.category))
                                    .color(egui::Color32::LIGHT_BLUE)
                            );
                            ui.label(&log.message);
                        });
                    }
                });
        });

    app.show_background_logs = open;
}
