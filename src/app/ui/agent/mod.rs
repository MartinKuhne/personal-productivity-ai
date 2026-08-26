//! UI agent view models — transcript accumulator and panel state.
//!
//! Unit tests live in the sibling `mod_tests.rs` sidecar (per submodule).

pub mod conversation_logger;
pub mod panel_state;
pub mod transcript;

pub use conversation_logger::ConversationLoggerObserver;
pub use panel_state::AgentPanelState;
