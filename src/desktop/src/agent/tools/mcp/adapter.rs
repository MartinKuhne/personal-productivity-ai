//! LLM-tool-loop adapter for MCP-discovered tools.
//!
//! [`McpToolAdapter`] implements the [`Tool`] trait so the LLM can
//! invoke any tool advertised by an MCP server. The actual protocol
//! work (transports, sessions, OAuth) is delegated to
//! [`crate::lib::mcp::McpClients`].

use std::sync::OnceLock;

use crate::lib::mcp::{DynamicToolSource, McpClients};
use crate::tools::Tool;
use crate::tools::context::ToolContext;
use crate::tools::descriptor::{ToolConfigSpec, ToolDescriptor};
use crate::tools::dispatcher::ToolError;
use crate::tools::registry::groups::ToolGroupId;

/// Adapter implementing [`Tool`] for an external MCP server tool.
pub struct McpToolAdapter {
    server_name: String,
    name: String,
    description: String,
    parameters: serde_json::Value,
    manager: std::sync::Arc<dyn DynamicToolSource>,
    /// Lazily-initialised descriptor. The `Box` is leaked inside
    /// the `OnceLock` so the returned `&ToolDescriptor` has a
    /// `'static`-ish lifetime tied to the adapter instance. MCP
    /// tools live for the lifetime of the process, so the leak
    /// is bounded.
    descriptor_cell: OnceLock<Box<ToolDescriptor>>,
}

impl McpToolAdapter {
    /// Constructs a new [`McpToolAdapter`].
    pub fn new(
        server_name: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
        manager: std::sync::Arc<McpClients>,
    ) -> Self {
        Self {
            server_name: server_name.into(),
            name: name.into(),
            description: description.into(),
            parameters,
            manager,
            descriptor_cell: OnceLock::new(),
        }
    }

    /// Construct from any [`DynamicToolSource`] back-end.
    pub fn from_dynamic_source(
        server_name: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
        manager: std::sync::Arc<dyn DynamicToolSource>,
    ) -> Self {
        Self {
            server_name: server_name.into(),
            name: name.into(),
            description: description.into(),
            parameters,
            manager,
            descriptor_cell: OnceLock::new(),
        }
    }

    /// Return the server name providing this tool.
    pub fn server_name(&self) -> &str {
        &self.server_name
    }
}

impl Tool for McpToolAdapter {
    fn descriptor(&self) -> &ToolDescriptor {
        self.descriptor_cell
            .get_or_init(|| {
                Box::new(ToolDescriptor::with_json_schema(
                    self.name.clone(),
                    self.description.clone(),
                    self.parameters.clone(),
                    crate::tools::Safety::Mutating,
                    // The group's enable flag is the single source
                    // of truth for "is this MCP server enabled?";
                    // see [`crate::tools::descriptor::group_enabled`].
                    ToolConfigSpec::group_only(ToolGroupId::Mcp(self.server_name.clone())),
                    ToolGroupId::Mcp(self.server_name.clone()),
                ))
            })
            .as_ref()
    }

    fn execute(
        &self,
        _ctx: &ToolContext,
        input_json: &str,
    ) -> Result<serde_json::Value, ToolError> {
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
                ToolError::new(format!(
                    "Invalid JSON parameters for MCP tool {}: {}",
                    self.name, e
                ))
            })?
        };

        self.manager
            .call_tool(&self.server_name, &self.name, args)
            .map_err(ToolError::new)
    }
}
