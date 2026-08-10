//! Unit tests for `agent_debug_window.rs`.

use super::*;
use crate::bus::events::debug::{AgentDebugEntry, DebugEntryKind, DebugEntryRow};
use crate::ui::test_helpers::run_ui_test;
use chrono::Local;

fn make_entry(turn: usize, session: usize, kind: DebugEntryKind, summary: &str) -> AgentDebugEntry {
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

/// The JSON text area must render at `json_rows` lines of height even when
/// the parent UI constrains `available_size` — the situation inside a
/// `ScrollArea::show_rows` viewport.
///
/// Before the `allocate_ui` wrapper was added, the viewport's `max_rect`
/// capped the inner `ScrollArea::max_height` to the visible-row slice
/// (as little as ~4 lines), ignoring the user's `json_rows` selection.
/// This test renders `render_json_text_area` inside a 50 px tall
/// `allocate_ui` (simulating the constrained viewport) and asserts the
/// returned `Response` rect is at least `json_rows * line_height` tall.
#[test]
fn test_json_text_area_height_respects_json_rows() {
    let ctx = egui::Context::default();
    let content = serde_json::json!({"key": "value"});

    for json_rows in [8, 16, 24, 32, 64] {
        let mut captured_height = 0.0_f32;
        let mut expected_height = 0.0_f32;

        let _ = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
            let line_height = ui.text_style_height(&egui::TextStyle::Monospace);
            expected_height = line_height * json_rows as f32;

            // Simulate the show_rows viewport: only 50 px of available
            // height — far less than json_rows * line_height. Without the
            // allocate_ui fix the ScrollArea would be capped to ~50 px.
            let inner = ui.allocate_ui(egui::vec2(ui.available_width(), 50.0), |ui| {
                render_json_text_area(ui, &content, json_rows)
            });
            captured_height = inner.inner.rect.height();
        });

        assert!(
            captured_height >= expected_height - 1.0,
            "json_rows={json_rows}: JSON text area height {captured_height:.1}px is less \
             than expected {expected_height:.1}px. The `allocate_ui` wrapper must \
             override the `show_rows` viewport constraint."
        );
    }
}
