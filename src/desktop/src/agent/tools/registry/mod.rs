//! Tool registry — registers all available tools, dispatches execution by name, and produces the JSON-Schema tool list for the LLM.

pub(crate) mod builtin;
pub(crate) mod pagination;
#[cfg(test)]
mod tests;

pub use pagination::paginate_in_range;

use crate::config::AppConfig;
use crate::agent::tools::Tool;
use crate::agent::tools::context::ToolContext;
use crate::agent::tools::mcp::DynamicToolSource;
use std::collections::HashMap;
use std::sync::Arc;

/// Central registry of available agent tools, both built-in and dynamic MCP tools.
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
    /// Names of tools auto-registered from MCP server discovery.
    auto_mcp_tools: HashMap<String, String>,
    /// Provider for dynamic MCP tools.
    pub mcp_manager: Arc<dyn DynamicToolSource>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    /// Create a new tool registry with all built-in tools registered.
    pub fn new() -> Self {
        let mut registry = Self {
            tools: HashMap::new(),
            auto_mcp_tools: HashMap::new(),
            mcp_manager: Arc::new(crate::agent::tools::mcp::McpClientManager::new()),
        };
        registry.register_all();
        registry
    }

    /// Register a tool instance into the registry.
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }

    /// Register a dynamic MCP server tool into this registry.
    pub fn register_mcp_tool(
        &mut self,
        server_name: impl Into<String>,
        tool_name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) {
        let adapter = crate::agent::tools::mcp::McpToolAdapter::from_dynamic_source(
            server_name,
            tool_name,
            description,
            parameters,
            self.mcp_manager.clone(),
        );
        self.register(Box::new(adapter));
    }

    /// Execute a tool by name with the given JSON argument string.
    pub fn execute(
        &self,
        ctx: &ToolContext,
        name: &str,
        args: &str,
    ) -> Result<serde_json::Value, String> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| format!("Tool {} not found.", name))?;
        tool.execute(ctx, args)
    }

    /// Look up a tool by name and return its [`crate::agent::tools::Safety`] classification.
    pub fn safety_of(&self, name: &str) -> crate::agent::tools::Safety {
        self.tools
            .get(name)
            .map(|t| t.safety())
            .unwrap_or(crate::agent::tools::Safety::Mutating)
    }

    /// Build the JSON Schema tool list for enabled tools given the application config and prompt.
    pub fn get_schema(&self, config: &AppConfig, prompt: &str) -> serde_json::Value {
        let mut tools = Vec::new();
        for tool in self.tools.values() {
            if tool.is_enabled(config, prompt) {
                let mut params = tool.parameters_schema();
                if params.get("properties").is_none() {
                    params["properties"] = serde_json::Value::Object(Default::default());
                }
                tools.push(serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": tool.name(),
                        "description": tool.description(),
                        "parameters": params
                    }
                }));
            }
        }
        serde_json::Value::Array(tools)
    }

    /// Refresh tools auto-registered from configured MCP servers.
    pub fn refresh_mcp_tools(&mut self) {
        for name in self.auto_mcp_tools.keys() {
            self.tools.remove(name);
        }
        self.auto_mcp_tools.clear();

        let server_names = self.mcp_manager.configured_servers();
        for server_name in server_names {
            match self.mcp_manager.discover_tools(&server_name) {
                Ok(tools) => {
                    for tool in tools {
                        self.register_mcp_tool(
                            server_name.clone(),
                            tool.name.clone(),
                            tool.description,
                            tool.input_schema,
                        );
                        self.auto_mcp_tools.insert(tool.name, server_name.clone());
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        server = %server_name,
                        error = %e,
                        "Failed to discover tools from MCP server; skipping"
                    );
                }
            }
        }
    }

    fn register_all(&mut self) {
        builtin::register_all_builtins(self);
    }
}

static TOOL_REGISTRY: std::sync::LazyLock<std::sync::RwLock<ToolRegistry>> =
    std::sync::LazyLock::new(|| std::sync::RwLock::new(ToolRegistry::new()));

/// Register a dynamic MCP tool into the global registry.
pub fn register_mcp_tool(
    server_name: impl Into<String>,
    tool_name: impl Into<String>,
    description: impl Into<String>,
    parameters: serde_json::Value,
) {
    if let Ok(mut registry) = TOOL_REGISTRY.write() {
        registry.register_mcp_tool(server_name, tool_name, description, parameters);
    }
}

/// Retrieve the JSON Schema for all active tools.
pub fn get_tools_schema(config: &AppConfig, prompt: &str) -> serde_json::Value {
    let mut registry = TOOL_REGISTRY.write().unwrap_or_else(|e| e.into_inner());
    registry.mcp_manager.update_config(config);
    registry.refresh_mcp_tools();
    registry.get_schema(config, prompt)
}

/// Look up a tool's [`crate::agent::tools::Safety`] classification by name.
pub fn safety_of(name: &str) -> crate::agent::tools::Safety {
    let registry = TOOL_REGISTRY.read().unwrap();
    registry.safety_of(name)
}

/// Initialize the MCP subsystem on app start.
pub fn init_mcp_on_startup(config: &AppConfig) -> usize {
    let Ok(mut registry) = TOOL_REGISTRY.write() else {
        return 0;
    };
    registry.mcp_manager.update_config(config);
    let ok = registry.mcp_manager.ping_all_servers();
    registry.refresh_mcp_tools();
    ok
}

/// Execute a named tool with JSON arguments and return a serialized [`crate::agent::tools::dtos::ToolResponse`].
pub fn execute_tool(ctx: &ToolContext, name: &str, args_str: &str) -> String {
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    let debug_mode = ctx
        .config
        .feature_flags
        .get("toolCallDebugMode")
        .copied()
        .unwrap_or(false);

    tracing::info!(name = "tool.registry.call", tool_name = %name, args = %args_str, "Executing tool call");
    let start_time = std::time::Instant::now();

    let result_raw: Result<serde_json::Value, String> =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let registry = TOOL_REGISTRY.read().unwrap();
            registry.mcp_manager.update_config(ctx.config);
            registry.execute(ctx, name, args_str)
        }))
        .unwrap_or_else(|e| {
            let msg = if let Some(s) = e.downcast_ref::<&str>() {
                *s
            } else if let Some(s) = e.downcast_ref::<String>() {
                s.as_str()
            } else {
                "Unknown panic"
            };
            Err(format!("Tool {} panicked: {}", name, msg))
        });

    let elapsed = start_time.elapsed();
    let response_dto = match result_raw {
        Ok(data) => {
            if debug_mode {
                let data_str = serde_json::to_string(&data)
                    .unwrap_or_else(|_| "<serialization error>".to_string());
                tracing::info!(name = "tool.registry.success", tool_name = %name, elapsed = ?elapsed, data = %data_str, "Tool execution succeeded");
            } else {
                tracing::info!(name = "tool.registry.success", tool_name = %name, elapsed = ?elapsed, "Tool execution succeeded");
            }
            crate::agent::tools::dtos::ToolResponse::Success { data }
        }
        Err(err) => {
            tracing::error!(name = "tool.registry.failed", tool_name = %name, elapsed = ?elapsed, error = %err, "Tool execution failed. Operator should verify tool inputs.");
            crate::agent::tools::dtos::ToolResponse::Error { message: err }
        }
    };

    serde_json::to_string(&response_dto).unwrap_or_else(|_| {
        r#"{"status":"error","message":"Failed to serialize tool response"}"#.to_string()
    })
}
