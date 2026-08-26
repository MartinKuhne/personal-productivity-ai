//! Abstraction over a dynamic tool source.
//!
//! [`DynamicToolSource`] defines the minimal interface the registry
//! depends on for MCP-driven tool discovery and execution. The
//! concrete implementation today is [`McpClients`](super::McpClients);
//! test doubles and future non-MCP sources can implement the same
//! surface without the registry depending on the concrete type.

use crate::config::AgentConfig;

/// Surface the registry needs from whichever backend powers
/// tool discovery and invocation.
///
/// The discovery and call methods are `async` because the MCP
/// HTTP transport uses [`reqwest::Client`] (async). Stdio
/// sessions do blocking I/O inside the same async path; the
/// caller bridges with [`crate::tools::blocking::block_on`]
/// when invoking from a synchronous context.
#[async_trait::async_trait]
pub trait DynamicToolSource: Send + Sync {
    /// Names of the currently-configured servers.
    fn configured_servers(&self) -> Vec<String>;

    /// Run `tools/list` against a single server and return the
    /// descriptors it currently advertises.
    async fn discover_tools(&self, server: &str) -> Result<Vec<super::McpToolDescriptor>, String>;

    /// Push an updated [`AgentConfig`], dropping sessions for
    /// servers that were removed or changed.
    fn update_config(&self, config: &AgentConfig);

    /// Execute a tool call (`tools/call`) against the named server.
    async fn call_tool(
        &self,
        server: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, String>;
}
