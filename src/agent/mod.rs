//! Agent subsystem — agent implementation, context, LLM client, session manager, and tool executor.
//!
//! Requirements: see [`SPEC.md`](SPEC.md) (AGENT-001..AGENT-023) for the full specification.

#![allow(special_module_name)]

pub mod agent_impl;
pub mod config;
pub mod context;
pub mod datamark;
pub mod error;
pub mod events;
#[allow(special_module_name)]
pub mod lib;
pub mod llm_client;
pub mod session;
pub mod tool_context;
pub mod tool_executor;
pub mod tools;
pub mod utils;
pub mod vfs;

#[cfg(test)]
#[path = "agent_impl_tests.rs"]
mod agent_impl_tests;

#[cfg(test)]
#[path = "tool_context_tests.rs"]
mod tool_context_tests;

// Property-based tests for the datamark envelope. Phase 1 of the
// fuzzing plan (`doc/planning/fuzzing.md`); this is the single
// highest-value new proptest in the project because the envelope
// is the security boundary between the LLM and untrusted tool
// output.
#[cfg(test)]
#[path = "datamark_proptests.rs"]
mod datamark_proptests;

pub use agent_impl::*;
pub use context::AgentContext;
pub use session::AgentSession;
pub use tool_context::AgentToolContext;
pub use tool_executor::{ToolCallRecord, ToolExecutor, ToolExecutorBuilder};
