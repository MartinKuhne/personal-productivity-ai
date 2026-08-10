//! Agent debug window — scrollable log of raw LLM API traffic with collapsible entry rows.

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
                                ui.centered_and_justified(|ui| {
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
                                render_entry_row(ui, entry, orig_idx);
                            }
                        });
                    }
                });
        });

    app.orchestrator.agent.show_debug_window = open;
}

fn render_entry_row(ui: &mut egui::Ui, entry: &AgentDebugEntry, id_salt: usize) {
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
                        if ui.button("Copy JSON").clicked() {
                            let json_str =
                                serde_json::to_string_pretty(content).unwrap_or_default();
                            ui.ctx().copy_text(json_str);
                        }
                    });
                });

                let mut json_str = serde_json::to_string_pretty(content).unwrap_or_default();
                egui::ScrollArea::vertical()
                    .max_height(1600.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut json_str)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY)
                                .interactive(false),
                        );
                    });
            }
        });

    ui.add_space(2.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::events::debug::{AgentDebugEntry, DebugEntryKind, DebugEntryRow};
    use crate::ui::test_helpers::run_ui_test;
    use chrono::Local;

    fn make_entry(
        turn: usize,
        session: usize,
        kind: DebugEntryKind,
        summary: &str,
    ) -> AgentDebugEntry {
        AgentDebugEntry {
            turn,
            session,
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
            session,
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
        app.orchestrator.agent.show_debug_window = false;

        let _ = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
            show_agent_debug_window(&mut app, ui.ctx());
        });
        assert!(!app.orchestrator.agent.show_debug_window);
    }

    #[test]
    fn test_show_agent_debug_window_open_with_entries() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        app.orchestrator.agent.show_debug_window = true;

        app.orchestrator
            .agent
            .state_mut()
            .debug_entries
            .extend(vec![
                make_boundary(1),
                make_entry(1, 1, DebugEntryKind::Outgoing, "Turn 1 — Outgoing"),
                make_entry(1, 1, DebugEntryKind::Incoming, "Turn 1 — Incoming"),
            ]);

        let _ = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
            show_agent_debug_window(&mut app, ui.ctx());
        });
        assert!(app.orchestrator.agent.show_debug_window);
    }

    #[test]
    fn test_show_agent_debug_window_no_id_change_warnings() {
        use crate::ui::test_helpers::assert::assert_no_id_change_in_shapes;

        let ctx = egui::Context::default();
        let mut app = create_test_app();
        app.orchestrator.agent.show_debug_window = true;

        app.orchestrator
            .agent
            .state_mut()
            .debug_entries
            .extend(vec![
                make_boundary(1),
                make_entry(
                    1,
                    1,
                    DebugEntryKind::Outgoing,
                    "Turn 1 — Outgoing (+2 messages)",
                ),
                make_entry(
                    1,
                    1,
                    DebugEntryKind::Incoming,
                    "Turn 1 — Incoming (assistant OK)",
                ),
                make_entry(
                    1,
                    1,
                    DebugEntryKind::ToolResults,
                    "Turn 1 — Tool results (1)",
                ),
                make_entry(
                    2,
                    1,
                    DebugEntryKind::Outgoing,
                    "Turn 2 — Outgoing (+3 messages)",
                ),
                make_entry(
                    2,
                    1,
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
        app.orchestrator.agent.show_debug_window = true;

        app.orchestrator
            .agent
            .state_mut()
            .debug_entries
            .push(make_entry(1, 1, DebugEntryKind::Outgoing, "Turn 1"));

        assert_eq!(app.orchestrator.agent.state().debug_entries.len(), 1);

        // Clear manually (simulating button click)
        app.orchestrator.agent.state_mut().debug_entries.clear();
        assert_eq!(app.orchestrator.agent.state().debug_entries.len(), 0);

        let _ = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
            show_agent_debug_window(&mut app, ui.ctx());
        });
        assert!(app.orchestrator.agent.show_debug_window);
    }
}
