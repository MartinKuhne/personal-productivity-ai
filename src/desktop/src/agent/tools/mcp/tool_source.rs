//! Abstraction over a dynamic tool source.
//!
//! [`DynamicToolSource`] defines the minimal interface the registry
//! depends on for MCP-driven tool discovery and execution. The
//! concrete implementation today is [`McpClientManager`](super::McpClientManager);
//! test doubles and future non-MCP sources can implement the same
//! surface without the registry depending on the concrete type.

use crate::config::AppConfig;

/// Surface the registry needs from whichever backend powers
/// tool discovery and invocation.
pub trait DynamicToolSource: Send + Sync {
    /// Names of the currently-configured servers.
    fn configured_servers(&self) -> Vec<String>;

    /// Run `tools/list` against a single server and return the
    /// descriptors it currently advertises.
    fn discover_tools(&self, server: &str) -> Result<Vec<super::McpToolDescriptor>, String>;

    /// Push an updated [`AppConfig`], dropping sessions for
    /// servers that were removed or changed.
    fn update_config(&self, config: &AppConfig);


    /// Execute a tool call (`tools/call`) against the named server.
    fn call_tool(
        &self,
        server: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, String>;
}
