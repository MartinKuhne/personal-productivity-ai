//! LLM-tool-loop adapter for MCP-discovered tools.
//!
//! [`McpToolAdapter`] implements the [`Tool`] trait so the LLM can
//! invoke any tool advertised by an MCP server. The actual protocol
//! work (transports, sessions, OAuth) is delegated to
//! [`crate::integrations::mcp::McpClientManager`].

use std::any::TypeId;
use std::sync::Arc;

use crate::agent::tools::context::ToolContext;
use crate::agent::tools::{Safety, Tool};
use crate::config::AppConfig;
use crate::integrations::mcp::{DynamicToolSource, McpClientManager};

/// Adapter implementing [`Tool`] for an external MCP server tool.
pub struct McpToolAdapter {
    server_name: String,
    name: String,
    description: String,
    parameters: serde_json::Value,
    manager: Arc<dyn DynamicToolSource>,
}

impl McpToolAdapter {
    /// Constructs a new [`McpToolAdapter`].
    ///
    /// The public signature is pinned to [`McpClientManager`] so
    /// existing callers do not need to change; the adapter widens
    /// the concrete type internally so it can also be constructed
    /// from any [`DynamicToolSource`] back-end.
    pub fn new(
        server_name: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
        manager: Arc<McpClientManager>,
    ) -> Self {
        Self {
            server_name: server_name.into(),
            name: name.into(),
            description: description.into(),
            parameters,
            manager,
        }
    }

    /// Construct from any [`DynamicToolSource`] back-end.
    /// Preferred by the registry after the P0-1 split.
    pub fn from_dynamic_source(
        server_name: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
        manager: Arc<dyn DynamicToolSource>,
    ) -> Self {
        Self {
            server_name: server_name.into(),
            name: name.into(),
            description: description.into(),
            parameters,
            manager,
        }
    }

    /// Return the server name providing this tool.
    pub fn server_name(&self) -> &str {
        &self.server_name
    }
}

impl Tool for McpToolAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_type(&self) -> TypeId {
        TypeId::of::<serde_json::Value>()
    }

    fn parameters_schema(&self) -> serde_json::Value {
        self.parameters.clone()
    }

    fn is_enabled(&self, config: &AppConfig, _prompt: &str) -> bool {
        config
            .mcp_servers
            .get(&self.server_name)
            .is_some_and(|entry| entry.is_enabled())
    }

    fn safety(&self) -> Safety {
        Safety::Mutating
    }

    fn execute(&self, _ctx: &ToolContext, input_json: &str) -> Result<serde_json::Value, String> {
        let args: serde_json::Value = if input_json.trim().is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(input_json).map_err(|e| {
                tracing::error!(
                    server = %self.server_name,
                    tool = %self.name,
                    error = %e,
                    "Malformed JSON parameters for MCP tool call"
                );
                format!("Invalid JSON parameters for MCP tool {}: {}", self.name, e)
            })?
        };

        self.manager.call_tool(&self.server_name, &self.name, args)
    }
}
