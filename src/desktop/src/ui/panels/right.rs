use crate::ui::FastMdApp;
use eframe::egui;
use egui::RichText;

pub fn show_right_panel(app: &mut FastMdApp, ctx: &egui::Context) {
    if !app.toc.is_empty() && app.selected_file.is_some() {
        egui::SidePanel::right("toc_panel")
            .width_range(150.0..=250.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.heading(
                    RichText::new("Table of Contents")
                        .size(14.0)
                        .strong()
                        .color(egui::Color32::from_rgb(100, 200, 255)),
                );
                ui.add_space(4.0);

                egui::ScrollArea::vertical().id_source("right_toc_scroll").show(ui, |ui| {
                    for entry in &app.toc {
                        let indent = match entry.level {
                            1 => 0.0,
                            2 => 10.0,
                            3 => 20.0,
                            _ => 0.0,
                        };
                        ui.horizontal(|ui| {
                            ui.add_space(indent);
                            let label = egui::RichText::new(&entry.title)
                                .size(13.0 - (entry.level as f32 * 0.5));
                            if ui.selectable_label(false, label).clicked() {
                                app.scroll_to_header_id = Some(entry.id);
                            }
                        });
                    }
                });
            });
    }
}