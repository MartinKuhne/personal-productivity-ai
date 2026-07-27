//! LLM-callable tools — the `Tool` trait, tool registry, and implementations for filesystem, web, calendar, contacts, email, CSV, and weather.

pub mod blocking;
pub mod caldav;
pub mod carddav;
pub mod context;
pub mod csv_db;
pub mod dtos;
pub mod filesystem;
pub mod jmap;
pub mod mcp;
pub mod registry;
pub mod weather;
pub mod web;
pub mod yaml_header;

use crate::config::AppConfig;
use context::ToolContext;
use std::any::TypeId;

/// Whether a tool mutates user state and therefore cannot be dispatched
/// in parallel with other mutating tools.
///
/// `Tool::safety` lets [`crate::agent::tool_executor::ToolExecutor`]
/// classify tools without matching on string names. The default
/// classification is [`Safety::Mutating`] (the conservative choice for
/// any tool that has not yet been audited).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Safety {
    /// Read-only / idempotent. Multiple safe tool calls may run in
    /// parallel within a single agent turn.
    ReadOnly,
    /// Mutates user-visible state (filesystem, calendar, contacts, email,
    /// ...). Calls run sequentially in the order the LLM emitted them.
    Mutating,
}

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_type(&self) -> TypeId;
    fn parameters_schema(&self) -> serde_json::Value;

    fn is_enabled(&self, config: &AppConfig, prompt: &str) -> bool;
    /// Classification used by the agent loop to decide parallel vs.
    /// sequential dispatch. Default is `Mutating`; tools that cannot
    /// cause side effects override this to `ReadOnly`.
    fn safety(&self) -> Safety {
        Safety::Mutating
    }
    fn execute(&self, ctx: &ToolContext, input_json: &str) -> Result<serde_json::Value, String>;
}

pub use context::ToolContext as ToolContextType;
pub use registry::{execute_tool, get_tools_schema};
