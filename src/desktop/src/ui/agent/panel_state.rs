//! UI-owned panel state for the agent panel — widget toggles, scroll target,
//! and command input that live on the UI layer, not the agent domain.
//!
//! Unit tests live in the sibling `panel_state_tests.rs` sidecar.

use uuid::Uuid;

/// Pure UI view state for the agent panel, owned by `AppOrchestrator`.
///
/// Fields previously lived on `AgentSessionManager` / `AgentState` (the
/// agent-domain structs). Moving them here enforces SC-007: the agent
/// layer has no UI widget state (migration step 6, FR-013).
pub struct AgentPanelState {
    /// Whether the agent results panel is visible.
    pub show_results: bool,
    /// Whether the debug window is open.
    pub show_debug_window: bool,
    /// Auto-scroll the debug window to the bottom.
    pub debug_auto_scroll: bool,
    /// Search filter text for the debug window.
    pub debug_search_text: String,
    /// The user's command input text.
    pub command_input: String,
    /// Stable string id of the heading the agent-results panel should
    /// scroll to. The UI layer maps it to an `egui::Id` at render time.
    pub scroll_to_id: Option<String>,
    /// Number of JSON rows to show in the debug window's expanded entry view.
    pub debug_json_rows: usize,
    /// Active session identity (UI tracks which session to render).
    pub active_session_id: Option<Uuid>,
}

impl Default for AgentPanelState {
    fn default() -> Self {
        Self {
            show_results: false,
            show_debug_window: false,
            debug_auto_scroll: true,
            debug_search_text: String::new(),
            debug_json_rows: 8,
            command_input: String::new(),
            scroll_to_id: None,
            active_session_id: None,
        }
    }
}

impl AgentPanelState {
    /// Create a new `AgentPanelState` with default values.
    pub fn new() -> Self {
        Self::default()
    }
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `panel_state_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "panel_state_tests.rs"]
mod tests;
