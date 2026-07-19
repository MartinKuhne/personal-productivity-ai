use crate::ui::FastMdApp;
use eframe::egui;
use egui::RichText;

pub fn show_top_panel(app: &mut FastMdApp, ctx: &egui::Context) {
    egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.heading(
                RichText::new("⚡ FastMD Viewer")
                    .strong()
                    .color(egui::Color32::from_rgb(100, 200, 255)),
            );
            ui.separator();
            ui.checkbox(&mut app.show_background_logs, "Show log");
            ui.separator();

            if !app.indexing_finished {
                ui.spinner();
                ui.label(
                    RichText::new(format!(
                        "Indexing workspace (found {} files)...",
                        app.all_files.len()
                    ))
                    .italics(),
                );
            } else {
                ui.label(
                    RichText::new(format!(
                        "Indexing finished ({} files)",
                        app.all_files.len()
                    ))
                    .color(egui::Color32::from_rgb(100, 255, 100)),
                );

                ui.separator();
                egui::ComboBox::from_id_source("tag_combobox")
                    .selected_text(app.selected_tag.as_deref().unwrap_or("Filter by Tag: All"))
                    .show_ui(ui, |ui| {
                        let mut changed = ui
                            .selectable_value(&mut app.selected_tag, None, "All")
                            .changed();
                        for tag in &app.all_tags {
                            changed |= ui
                                .selectable_value(
                                    &mut app.selected_tag,
                                    Some(tag.clone()),
                                    tag,
                                )
                                .changed();
                        }
                        if changed {
                            if let Some(selected) = &app.selected_file {
                                if let Some(active_tag) = &app.selected_tag {
                                    if let Some(tags) = app.file_tags.get(selected) {
                                        if !tags.contains(active_tag) {
                                            app.selected_file = None;
                                        }
                                    } else {
                                        app.selected_file = None;
                                    }
                                }
                            }
                        }
                    });
            }
        });
    });
}