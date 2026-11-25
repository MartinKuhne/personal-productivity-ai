//! Shared fixtures for the agent panel's test sidecars.
//!
//! Test-only. `base_vm` was defined byte-identically in three sidecars
//! (`mod_tests.rs`, `panel_state_tests.rs`, `transcript_tests.rs`); it lives
//! here so the idle-state fixture has one definition. The body is verbatim
//! from those sidecars — no field changed.

use hoarfrost::widget::text_input_model::TextInputModel;

use super::panel_state::AgentPanelViewModel;

/// An agent panel view model in its idle state: visible, empty, not running,
/// with the command input cleared and auto-scroll on.
///
/// This is the starting point every panel sidecar mutates to reach the state
/// its test needs; nothing here depends on files, the bus or agent threads
/// ([RUST-006]).
pub(crate) fn base_vm() -> AgentPanelViewModel {
    AgentPanelViewModel {
        visible: true,
        turns: Vec::new(),
        thinking: String::new(),
        thinking_expanded: true,
        status: "Idle".to_string(),
        running: false,
        queued_prompts: Vec::new(),
        command_input: TextInputModel::empty().with_clearable(false),
        auto_scroll: true,
    }
}
