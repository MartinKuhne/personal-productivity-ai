//! UI agent view models — transcript accumulator, display formatting and
//! panel state.
//!
//! `agent_render` was relocated here from `ui/render/` by [TASK-029b]: it
//! is framework-free display formatting consumed by [`transcript`].
//!
//! Unit tests live in the sibling `mod_tests.rs` sidecar (per submodule).

pub(crate) mod agent_render;
pub(crate) mod conversation_logger;
pub(crate) mod panel_state;
pub mod transcript;

pub use conversation_logger::ConversationLoggerObserver;
pub use panel_state::AgentPanelState;
