//! Agent debug window — scrollable log of raw LLM API traffic with collapsible entry rows.
//!
//! Unit tests live in the sibling `agent_debug_window_tests.rs` sidecar.

use crate::bus::events::debug::{AgentDebugEntry, DebugEntryKind, DebugEntryRow};
use crate::ui::FastMdApp;
use eframe::egui;

pub fn show_agent_debug_window(app: &mut FastMdApp, ctx: &egui::Context) {
    if !app.orchestrator.agent.show_debug_window {
        return;
    }

    let mut open = app.orchestrator.agent.show_debug_window;

    egui::Window::new(crate::ui::strings::AGENT_DEBUG_WINDOW)
        .open(&mut open)
        .resizable(true)
        .collapsible(true)
        .default_size([600.0, 1600.0])
        .show(ctx, |ui| {
            let entries = app.orchestrator.agent.state().debug_entries.clone();

            ui.horizontal(|ui| {
                ui.label(crate::ui::strings::SEARCH_LABEL);
                ui.text_edit_singleline(&mut app.orchestrator.agent.debug_search_text);

                ui.checkbox(
                    &mut app.orchestrator.agent.debug_auto_scroll,
                    crate::ui::strings::AUTO_SCROLL_CHECKBOX,
                );

                ui.label(crate::ui::strings::DEBUG_JSON_ROWS_LABEL);
                egui::ComboBox::from_id_salt("debug_json_rows")
                    .selected_text(format!("{}", app.orchestrator.agent.debug_json_rows))
                    .show_ui(ui, |ui| {
                        for rows in [8, 16, 24, 32, 64] {
                            if ui
                                .selectable_value(
                                    &mut app.orchestrator.agent.debug_json_rows,
                                    rows,
                                    format!("{}", rows),
                                )
                                .clicked()
                            {}
                        }
                    });

                if ui.button(crate::ui::strings::CLEAR_BUTTON).clicked() {
                    app.orchestrator.agent.state_mut().debug_entries.clear();
                }
            });

            ui.separator();

            let search_lower = app.orchestrator.agent.debug_search_text.to_lowercase();
            let filtered: Vec<(usize, &AgentDebugEntry)> = entries
                .iter()
                .enumerate()
                .filter(|(_i, entry)| {
                    if matches!(entry.row_type, DebugEntryRow::SessionBoundary) {
                        return true;
                    }
                    if search_lower.is_empty() {
                        return true;
                    }
                    entry.summary.to_lowercase().contains(&search_lower)
                })
                .collect();

            let row_height = ui.text_style_height(&egui::TextStyle::Body);

            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .stick_to_bottom(app.orchestrator.agent.debug_auto_scroll)
                .show_rows(ui, row_height, filtered.len(), |ui, row_range| {
                    for i in row_range {
                        let (orig_idx, entry) = filtered[i];
                        ui.push_id(orig_idx, |ui| match entry.row_type {
                            DebugEntryRow::SessionBoundary => {
                                ui.add_space(4.0);
                                ui.separator();
                                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                                    ui.label(
                                        egui::RichText::new(&entry.summary)
                                            .color(egui::Color32::GRAY)
                                            .size(12.0),
                                    );
                                });
                                ui.separator();
                                ui.add_space(4.0);
                            }
                            DebugEntryRow::Entry => {
                                render_entry_row(
                                    ui,
                                    entry,
                                    orig_idx,
                                    app.orchestrator.agent.debug_json_rows,
                                );
                            }
                        });
                    }
                });
        });

    app.orchestrator.agent.show_debug_window = open;
}

fn render_entry_row(ui: &mut egui::Ui, entry: &AgentDebugEntry, id_salt: usize, json_rows: usize) {
    let kind_label = match entry.kind {
        DebugEntryKind::Outgoing => crate::ui::strings::DEBUG_KIND_OUTGOING,
        DebugEntryKind::Incoming => crate::ui::strings::DEBUG_KIND_INCOMING,
        DebugEntryKind::ToolResults => crate::ui::strings::DEBUG_KIND_TOOL_RESULTS,
    };

    let header_text = format!(
        "{}  [{}]  {}",
        entry.timestamp.format("%H:%M:%S%.3f"),
        kind_label,
        entry.summary,
    );

    egui::CollapsingHeader::new(header_text)
        .id_salt(("entry_header", id_salt))
        .default_open(false)
        .show(ui, |ui| {
            if let Some(ref content) = entry.content {
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(crate::ui::strings::DEBUG_COPY_JSON_BUTTON)
                            .clicked()
                        {
                            let json_str =
                                serde_json::to_string_pretty(content).unwrap_or_default();
                            ui.ctx().copy_text(json_str);
                        }
                    });
                });

                render_json_text_area(ui, content, json_rows);
            }
        });

    ui.add_space(2.0);
}

/// Render the pretty-printed JSON content in a scrollable text area whose
/// visible height matches `json_rows` lines.
///
/// Wraps the `ScrollArea` in `ui.allocate_ui` so the inner area always
/// receives `max_h` of available height, even when the parent is a
/// `ScrollArea::show_rows` viewport that constrains `available_size`.
/// Without `allocate_ui`, the viewport's `max_rect` caps the ScrollArea's
/// visible area well below `max_h`, leaving only ~4 lines visible.
fn render_json_text_area(
    ui: &mut egui::Ui,
    content: &serde_json::Value,
    json_rows: usize,
) -> egui::Response {
    let mut json_str = serde_json::to_string_pretty(content).unwrap_or_default();
    let line_height = ui.text_style_height(&egui::TextStyle::Monospace);
    let max_h = line_height * json_rows as f32;
    ui.allocate_ui(egui::vec2(ui.available_width(), max_h), |ui| {
        egui::ScrollArea::vertical()
            .max_height(max_h)
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut json_str)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .desired_rows(json_rows)
                        .interactive(false),
                );
            });
    })
    .response
}

#[cfg(test)]
#[path = "agent_debug_window_tests.rs"]
mod tests;
