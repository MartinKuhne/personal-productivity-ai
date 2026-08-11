//! Agent debug window — scrollable log of raw LLM API traffic with collapsible entry rows.

use crate::bus::events::debug::{AgentDebugEntry, DebugEntryKind, DebugEntryRow};
use crate::ui::FastMdApp;
use eframe::egui;

pub fn show_agent_debug_window(app: &mut FastMdApp, ctx: &egui::Context) {
    if !app.orchestrator.agent_panel_state.show_debug_window {
        return;
    }

    let mut open = app.orchestrator.agent_panel_state.show_debug_window;

    egui::Window::new(crate::ui::strings::AGENT_DEBUG_WINDOW)
        .open(&mut open)
        .resizable(true)
        .collapsible(true)
        .default_size([600.0, 1600.0])
        .show(ctx, |ui| {
            let entries = app.orchestrator.agent.state().debug_entries.clone();

            ui.horizontal(|ui| {
                ui.label(crate::ui::strings::SEARCH_LABEL);
                ui.text_edit_singleline(&mut app.orchestrator.agent_panel_state.debug_search_text);

                ui.checkbox(
                    &mut app.orchestrator.agent_panel_state.debug_auto_scroll,
                    crate::ui::strings::AUTO_SCROLL_CHECKBOX,
                );

                ui.label(crate::ui::strings::DEBUG_JSON_ROWS_LABEL);
                egui::ComboBox::from_id_salt("debug_json_rows")
                    .selected_text(format!(
                        "{}",
                        app.orchestrator.agent_panel_state.debug_json_rows
                    ))
                    .show_ui(ui, |ui| {
                        for rows in [8, 16, 24, 32, 64] {
                            if ui
                                .selectable_value(
                                    &mut app.orchestrator.agent_panel_state.debug_json_rows,
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

            let search_lower = app
                .orchestrator
                .agent_panel_state
                .debug_search_text
                .to_lowercase();
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

            let mut omit_tools_for_idx = std::collections::HashSet::new();
            let mut seen_tools_this_session = false;
            for (orig_idx, entry) in &filtered {
                match entry.row_type {
                    DebugEntryRow::SessionBoundary => {
                        seen_tools_this_session = false;
                    }
                    DebugEntryRow::Entry => {
                        if let Some(content) = &entry.content {
                            if let Some(obj) = content.as_object() {
                                if obj.contains_key("tools") {
                                    if seen_tools_this_session {
                                        omit_tools_for_idx.insert(*orig_idx);
                                    } else {
                                        seen_tools_this_session = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let row_height = ui.text_style_height(&egui::TextStyle::Body);

            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .stick_to_bottom(app.orchestrator.agent_panel_state.debug_auto_scroll)
                .show_rows(ui, row_height, filtered.len(), |ui, row_range| {
                    for i in row_range {
                        let (orig_idx, entry) = filtered[i];
                        ui.push_id(orig_idx, |ui| match entry.row_type {
                            DebugEntryRow::SessionBoundary => {
                                ui.separator();
                                ui.centered_and_justified(|ui| {
                                    ui.label(
                                        egui::RichText::new(&entry.summary)
                                            .color(egui::Color32::GRAY)
                                            .size(12.0),
                                    );
                                });
                                ui.separator();
                            }
                            DebugEntryRow::Entry => {
                                let mut display_entry = (*entry).clone();
                                if omit_tools_for_idx.contains(&orig_idx) {
                                    if let Some(content) = &mut display_entry.content {
                                        if let Some(obj) = content.as_object_mut() {
                                            obj.insert(
                                                "tools".to_string(),
                                                serde_json::Value::String(
                                                    "[omitted - see earlier turn]".to_string(),
                                                ),
                                            );
                                        }
                                    }
                                }

                                if let Some(content) = &mut display_entry.content {
                                    unescape_json_strings(content);
                                }

                                render_entry_row(
                                    ui,
                                    &display_entry,
                                    orig_idx,
                                    app.orchestrator.agent_panel_state.debug_json_rows,
                                );
                            }
                        });
                    }
                });
        });

    app.orchestrator.agent_panel_state.show_debug_window = open;
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
/// visible height matches `json_rows` lines and scrolls horizontally for
/// wide content.
fn render_json_text_area(
    ui: &mut egui::Ui,
    content: &serde_json::Value,
    json_rows: usize,
) -> egui::Response {
    let mut json_str = serde_json::to_string_pretty(content).unwrap_or_default();
    let line_height = ui.text_style_height(&egui::TextStyle::Monospace);
    let max_h = line_height * json_rows as f32;
    ui.allocate_ui(egui::vec2(ui.available_width(), max_h), |ui| {
        egui::ScrollArea::both().max_height(max_h).show(ui, |ui| {
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

/// Recursively un-escapes double-escaped JSON strings and external data envelopes
/// in the debug entry payload, making them pretty-printable in the UI.
fn unescape_json_strings(val: &mut serde_json::Value) {
    match val {
        serde_json::Value::Object(map) => {
            for (_, v) in map.iter_mut() {
                unescape_json_strings(v);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr.iter_mut() {
                unescape_json_strings(v);
            }
        }
        serde_json::Value::String(s) => {
            let s_trimmed = s.trim();
            if (s_trimmed.starts_with('{') && s_trimmed.ends_with('}'))
                || (s_trimmed.starts_with('[') && s_trimmed.ends_with(']'))
            {
                if let Ok(mut parsed) = serde_json::from_str::<serde_json::Value>(s_trimmed) {
                    unescape_json_strings(&mut parsed);
                    *val = parsed;
                    return;
                }
            }

            if s_trimmed.starts_with("<<<EXTERNAL_DATA>>>") && s_trimmed.ends_with("<<<END_EXTERNAL_DATA>>>") {
                let inner = &s_trimmed["<<<EXTERNAL_DATA>>>".len()..s_trimmed.len() - "<<<END_EXTERNAL_DATA>>>".len()];
                let inner = inner.trim();
                if let Some((prov, data)) = inner.split_once('\n') {
                    if prov.starts_with("provenance=") {
                        let data_trimmed = data.trim();
                        if (data_trimmed.starts_with('{') && data_trimmed.ends_with('}'))
                            || (data_trimmed.starts_with('[') && data_trimmed.ends_with(']'))
                        {
                            if let Ok(mut parsed) = serde_json::from_str::<serde_json::Value>(data_trimmed) {
                                unescape_json_strings(&mut parsed);
                                *val = serde_json::json!({
                                    "EXTERNAL_DATA": {
                                        "provenance": prov,
                                        "data": parsed
                                    }
                                });
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::events::debug::{AgentDebugEntry, DebugEntryKind, DebugEntryRow};
    use crate::ui::test_helpers::run_ui_test;
    use chrono::Local;

    fn make_entry(turn: usize, kind: DebugEntryKind, summary: &str) -> AgentDebugEntry {
        AgentDebugEntry {
            turn,
            timestamp: Local::now(),
            kind,
            summary: summary.to_string(),
            content: Some(serde_json::json!({"key": "value"})),
            row_type: DebugEntryRow::Entry,
        }
    }

    fn make_boundary(session: usize) -> AgentDebugEntry {
        AgentDebugEntry {
            turn: 0,
            timestamp: Local::now(),
            kind: DebugEntryKind::Outgoing,
            summary: format!("Session {}", session),
            content: None,
            row_type: DebugEntryRow::SessionBoundary,
        }
    }

    fn create_test_app() -> FastMdApp {
        FastMdApp::empty_state(crate::config::AppConfig::default())
    }

    #[test]
    fn test_show_agent_debug_window_closed() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        app.orchestrator.agent_panel_state.show_debug_window = false;

        let _ = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
            show_agent_debug_window(&mut app, ui.ctx());
        });
        assert!(!app.orchestrator.agent_panel_state.show_debug_window);
    }

    #[test]
    fn test_show_agent_debug_window_open_with_entries() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        app.orchestrator.agent_panel_state.show_debug_window = true;

        app.orchestrator
            .agent
            .state_mut()
            .debug_entries
            .extend(vec![
                make_boundary(1),
                make_entry(1, DebugEntryKind::Outgoing, "Turn 1 — Outgoing"),
                make_entry(1, DebugEntryKind::Incoming, "Turn 1 — Incoming"),
            ]);

        let _ = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
            show_agent_debug_window(&mut app, ui.ctx());
        });
        assert!(app.orchestrator.agent_panel_state.show_debug_window);
    }

    #[test]
    fn test_show_agent_debug_window_no_id_change_warnings() {
        use crate::ui::test_helpers::assert::assert_no_id_change_in_shapes;

        let ctx = egui::Context::default();
        let mut app = create_test_app();
        app.orchestrator.agent_panel_state.show_debug_window = true;

        app.orchestrator
            .agent
            .state_mut()
            .debug_entries
            .extend(vec![
                make_boundary(1),
                make_entry(
                    1,
                    DebugEntryKind::Outgoing,
                    "Turn 1 — Outgoing (+2 messages)",
                ),
                make_entry(
                    1,
                    DebugEntryKind::Incoming,
                    "Turn 1 — Incoming (assistant OK)",
                ),
                make_entry(1, DebugEntryKind::ToolResults, "Turn 1 — Tool results (1)"),
                make_entry(
                    2,
                    DebugEntryKind::Outgoing,
                    "Turn 2 — Outgoing (+3 messages)",
                ),
                make_entry(
                    2,
                    DebugEntryKind::Incoming,
                    "Turn 2 — Incoming (assistant OK)",
                ),
            ]);

        let _ = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
            show_agent_debug_window(&mut app, ui.ctx());
        });
        let output = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
            show_agent_debug_window(&mut app, ui.ctx());
        });

        let shapes: Vec<egui::Shape> = output.shapes.into_iter().map(|cs| cs.shape).collect();
        assert_no_id_change_in_shapes(&shapes);
    }

    #[test]
    fn test_clear_button_removes_entries() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        app.orchestrator.agent_panel_state.show_debug_window = true;

        app.orchestrator
            .agent
            .state_mut()
            .debug_entries
            .push(make_entry(1, DebugEntryKind::Outgoing, "Turn 1"));

        assert_eq!(app.orchestrator.agent.state().debug_entries.len(), 1);

        // Clear manually (simulating button click)
        app.orchestrator.agent.state_mut().debug_entries.clear();
        assert_eq!(app.orchestrator.agent.state().debug_entries.len(), 0);

        let _ = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
            show_agent_debug_window(&mut app, ui.ctx());
        });
        assert!(app.orchestrator.agent_panel_state.show_debug_window);
    }
}
