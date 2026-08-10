//! Agent subsystem — agent implementation, context, LLM client, session manager, prompt builder, response formatter, and tool executor.
//!
//! Requirements: see [`SPEC.md`](SPEC.md) (AGENT-001..AGENT-023) for the full specification.

pub mod agent_impl;
pub mod context;
pub mod datamark;
pub mod error;
pub mod events;
pub mod llm_client;
pub mod manager;
pub mod prompt_builder;
pub mod response_formatter;
pub mod tool_executor;
pub mod tools;

#[cfg(test)]
#[path = "agent_impl_tests.rs"]
mod agent_impl_tests;

pub use agent_impl::*;
pub use context::AgentContext;
pub use manager::AgentSessionManager;
