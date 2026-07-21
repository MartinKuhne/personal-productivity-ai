//! Agent subsystem: LLM orchestration, tool execution, session management.
//!
//! Modules:
//! - `agent_impl`: Core `run_agent` function and LLM interaction loop
//! - `manager`: `AgentSessionManager` for UI state management
//! - `context`: `AgentContext` struct consolidating agent parameters
//! - `llm_client` (coming soon): LLM API abstraction
//! - `tool_executor` (coming soon): Tool execution orchestration
//! - `prompt_builder` (coming soon): System prompt construction
//! - `response_formatter` (coming soon): UI response formatting

pub mod agent_impl;
pub mod manager;
pub mod context;

// Re-exports for convenient access
pub use agent_impl::*;
pub use context::AgentContext;
pub use manager::AgentSessionManager;
