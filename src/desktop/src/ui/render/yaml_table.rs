//! YAML front-matter rendering — paints a YAML key/value mapping as a
//! two-column table with alternating row backgrounds. Falls through
//! silently when the input is not a YAML mapping (string, sequence,
//! null) because the front-matter editor only shows key/value rows.
//!
//! Public API: [`render_yaml_table`] is the only entry point. It is
//! the largest single rendering function in the module and the
//! most layout-sensitive (regression tests for word-wrap, row
//! height, and viewport overflow all live in `super`'s `e2e_tests`).
//!
//! Caching: the parsed key/value pairs and row heights are memoized
//! in egui's temp data keyed by a hash of the YAML content, so
//! re-rendering the same front matter on subsequent frames is nearly free.

use crate::markdown::parse_yaml_to_pairs;
use eframe::egui;
use egui::RichText;
use std::hash::{Hash, Hasher};

/// Width of the key column in the YAML metadata table. The value
/// column takes whatever's left over, with a 12px inter-column gap.
const YAML_KEY_COLUMN_WIDTH: f32 = 110.0;

/// Cached YAML table data to avoid re-parsing and re-measuring on every frame.
#[derive(Clone, Debug)]
struct YamlTableCache {
    /// Hash of the YAML content.
    content_hash: u64,
    /// Parsed key/value pairs.
    pairs: Vec<(String, String)>,
    /// Cached row heights.
    row_heights: Vec<f32>,
}

pub fn render_yaml_table(ui: &mut egui::Ui, yaml: &serde_norway::Value) {
    if let Some(pairs) = parse_yaml_to_pairs(yaml) {
        // Compute hash of YAML content for cache invalidation.
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        pairs.hash(&mut hasher);
        let content_hash = hasher.finish();

        let table_id = ui.make_persistent_id("yaml_table");
        let cache_id = table_id.with("cache");

        // Try to get cached data.
        let cached: Option<YamlTableCache> = ui
            .data(|d| d.get_temp(cache_id))
            .filter(|c: &YamlTableCache| c.content_hash == content_hash);

        let (pairs, _cached_row_heights) = if let Some(cache) = cached {
            (cache.pairs, Some(cache.row_heights))
        } else {
            // No valid cache — we'll measure row heights during render.
            (pairs, None)
        };

        egui::Frame::NONE
            .fill(egui::Color32::from_rgb(24, 24, 27))
            .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_gray(40)))
            .inner_margin(8.0)
            .corner_radius(4.0)
            .show(ui, |ui| {
                let available_width = ui.available_width();
                let key_col_width = YAML_KEY_COLUMN_WIDTH.min((available_width - 20.0).max(40.0));
                let value_col_width = (available_width - key_col_width - 12.0).max(40.0);

                let mut new_row_heights = Vec::with_capacity(pairs.len());

                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(12.0, 4.0);
                    let clip = ui.clip_rect();
                    let viewport_margin = 400.0_f32;

                    for (row_idx, (k, v)) in pairs.iter().enumerate() {
                        let cached_h = _cached_row_heights.as_ref().and_then(|h| h.get(row_idx)).copied();
                        let top_y = ui.cursor().min.y;
                        
                        // If we have a cached height and it's off-screen, skip layout
                        if let Some(h) = cached_h {
                            if clip.is_positive() && (top_y > clip.max.y + viewport_margin || top_y + h < clip.min.y - viewport_margin) {
                                ui.add_space(h);
                                new_row_heights.push(h);
                                continue;
                            }
                        }

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

                        // Cache row height for next frame.
                        new_row_heights.push(row_response.response.rect.height());
                    }
                });

                // Store cache for next frame.
                let cache = YamlTableCache {
                    content_hash,
                    pairs: pairs.clone(),
                    row_heights: new_row_heights,
                };
                ui.data_mut(|d| d.insert_temp(cache_id, cache));
            });
        ui.add_space(8.0);
    }
}
