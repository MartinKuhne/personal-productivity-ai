//! Tests for `AgentPanelState` default construction.

use super::*;

#[test]
fn test_panel_state_defaults() {
    let state = AgentPanelState::new();
    assert!(!state.show_results);
    assert!(!state.show_debug_window);
    assert!(state.debug_auto_scroll);
    assert!(state.debug_search_text.is_empty());
    assert_eq!(state.debug_json_rows, 8);
    assert!(state.command_input.is_empty());
    assert!(state.scroll_to_id.is_none());
    assert!(state.active_session_id.is_none());
}

#[test]
fn test_panel_state_default_eq_new() {
    assert!(AgentPanelState::default().command_input.is_empty());
}
