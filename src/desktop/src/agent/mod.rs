//! Agent subsystem — agent implementation, context, LLM client, session manager, prompt builder, response formatter, and tool executor.

pub mod agent_impl;
pub mod context;
pub mod llm_client;
pub mod manager;
pub mod prompt_builder;
pub mod response_formatter;
pub mod tool_executor;

#[cfg(test)]
#[path = "agent_impl_tests.rs"]
mod agent_impl_tests;

pub use agent_impl::*;
pub use context::AgentContext;
pub use manager::AgentSessionManager;
