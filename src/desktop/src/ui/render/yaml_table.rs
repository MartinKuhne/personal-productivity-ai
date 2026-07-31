//! YAML front-matter rendering — paints a YAML key/value mapping as a
//! two-column table with alternating row backgrounds. Falls through
//! silently when the input is not a YAML mapping (string, sequence,
//! null) because the front-matter editor only shows key/value rows.
//!
//! Public API: [`render_yaml_table`] is the only entry point. It is
//! the largest single rendering function in the module and the
//! most layout-sensitive (regression tests for word-wrap, row
//! height, and viewport overflow all live in `super`'s `e2e_tests`).

use crate::markdown::parse_yaml_to_pairs;
use eframe::egui;
use egui::RichText;

/// Width of the key column in the YAML metadata table. The value
/// column takes whatever's left over, with a 12px inter-column gap.
const YAML_KEY_COLUMN_WIDTH: f32 = 110.0;

pub fn render_yaml_table(ui: &mut egui::Ui, yaml: &serde_yml::Value) {
    if let Some(pairs) = parse_yaml_to_pairs(yaml) {
        let table_id = ui.make_persistent_id("yaml_table");
        egui::Frame::NONE
            .fill(egui::Color32::from_rgb(24, 24, 27))
            .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_gray(40)))
            .inner_margin(8.0)
            .corner_radius(4.0)
            .show(ui, |ui| {
                // Capture available width *inside* the frame so it accounts
                // for the inner_margin. The previous code captured it
                // before entering the frame, making set_min_width exceed
                // the content rect by ~16px and forcing a permanent
                // horizontal scrollbar.
                let available_width = ui.available_width();
                // Reserve a fixed width for the key column and let the
                // value column take the remainder. Without explicit widths, a
                // cell might expand past the panel width.
                // By giving each cell an explicit width, the value cell knows its
                // wrap budget and the total table width matches
                // `available_width` exactly, so no horizontal scrolling
                // is needed.
                let key_col_width = YAML_KEY_COLUMN_WIDTH.min((available_width - 20.0).max(40.0));
                // `12.0` matches the column spacing `12.0` below.
                let value_col_width = (available_width - key_col_width - 12.0).max(40.0);

                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(12.0, 4.0);
                    for (row_idx, (k, v)) in pairs.into_iter().enumerate() {
                        let is_striped = row_idx % 2 == 1;
                        let bg_idx = if is_striped {
                            Some(ui.painter().add(egui::Shape::Noop))
                        } else {
                            None
                        };

                        let row_response = ui.push_id((table_id, "yaml_row", row_idx), |ui| {
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                                // Key cell: set key_col_width and align top.
                                ui.push_id("key", |ui| {
                                    ui.with_layout(
                                        egui::Layout::left_to_right(egui::Align::Min),
                                        |ui| {
                                            ui.set_min_width(key_col_width);
                                            ui.set_max_width(key_col_width);
                                            ui.label(
                                                RichText::new(k)
                                                    .strong()
                                                    .color(egui::Color32::from_rgb(150, 200, 255)),
                                            );
                                        },
                                    );
                                });
                                // Value cell: set value_col_width and wrap text inside.
                                ui.push_id("value", |ui| {
                                    ui.with_layout(
                                        egui::Layout::left_to_right(egui::Align::Min),
                                        |ui| {
                                            ui.set_min_width(value_col_width);
                                            ui.set_max_width(value_col_width);
                                            ui.add(
                                                egui::Label::new(
                                                    RichText::new(v)
                                                        .color(egui::Color32::from_gray(220)),
                                                )
                                                .wrap(),
                                            );
                                        },
                                    );
                                });
                            })
                        });

                        if let Some(idx) = bg_idx {
                            ui.painter().set(
                                idx,
                                egui::Shape::rect_filled(
                                    row_response.response.rect,
                                    0.0,
                                    ui.visuals().faint_bg_color,
                                ),
                            );
                        }
                    }
                });
            });
        ui.add_space(8.0);
    }
}
