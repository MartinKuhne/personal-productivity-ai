//! MCP (Model Context Protocol) client integration — client manager, transports, and tool adapters.

use crate::config::{AppConfig, McpServerConfig};
use crate::tools::context::ToolContext;
use crate::tools::{Safety, Tool};
use std::any::TypeId;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Adapter implementing [`Tool`] for an external MCP server tool.
pub struct McpToolAdapter {
    server_name: String,
    name: String,
    description: String,
    parameters: serde_json::Value,
    manager: Arc<McpClientManager>,
}

impl McpToolAdapter {
    /// Constructs a new [`McpToolAdapter`].
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
        config.mcp_servers.contains_key(&self.server_name)
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

/// Manager for MCP server client connections, transport execution, and tool dispatching.
pub struct McpClientManager {
    servers: RwLock<HashMap<String, McpServerConfig>>,
}

impl Default for McpClientManager {
    fn default() -> Self {
        Self::new()
    }
}

impl McpClientManager {
    /// Creates a new [`McpClientManager`].
    pub fn new() -> Self {
        Self {
            servers: RwLock::new(HashMap::new()),
        }
    }

    /// Update manager configuration with active MCP servers.
    pub fn update_config(&self, config: &AppConfig) {
        if let Ok(mut guard) = self.servers.write() {
            *guard = config.mcp_servers.clone();
        }
    }

    /// Execute a tool call (`tools/call`) on the specified MCP server.
    pub fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let cfg = {
            let guard = self
                .servers
                .read()
                .map_err(|e| format!("Failed to read MCP server config: {}", e))?;
            guard
                .get(server_name)
                .cloned()
                .ok_or_else(|| format!("MCP server '{}' is not configured.", server_name))?
        };

        tracing::info!(
            server = %server_name,
            tool = %tool_name,
            "Dispatching tool call to MCP server"
        );

        let start = std::time::Instant::now();
        let result = match cfg {
            McpServerConfig::Stdio { command, args, env } => {
                Self::call_stdio(server_name, &command, &args, &env, tool_name, arguments)
            }
            McpServerConfig::Sse { url, headers } => {
                Self::call_sse(server_name, &url, &headers, tool_name, arguments)
            }
        };

        let elapsed = start.elapsed();
        match &result {
            Ok(_) => {
                tracing::info!(
                    server = %server_name,
                    tool = %tool_name,
                    elapsed = ?elapsed,
                    "MCP tool execution completed successfully"
                );
            }
            Err(err) => {
                tracing::error!(
                    server = %server_name,
                    tool = %tool_name,
                    elapsed = ?elapsed,
                    error = %err,
                    "MCP tool execution failed"
                );
            }
        }

        result
    }

    fn call_stdio(
        server_name: &str,
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        tracing::debug!(
            server = %server_name,
            command = %command,
            tool = %tool_name,
            "Calling stdio MCP transport"
        );

        if command.trim().is_empty() {
            return Err(format!(
                "Stdio MCP server '{}' has empty command path.",
                server_name
            ));
        }

        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments
            }
        });

        let payload_str = serde_json::to_string(&payload)
            .map_err(|e| format!("Failed to serialize JSON-RPC payload: {}", e))?;

        let mut cmd = std::process::Command::new(command);
        cmd.args(args);
        for (k, v) in env {
            cmd.env(k, v);
        }
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            tracing::error!(
                server = %server_name,
                command = %command,
                error = %e,
                "Failed to spawn stdio MCP subprocess"
            );
            format!("Failed to spawn MCP subprocess '{}': {}", command, e)
        })?;

        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = writeln!(stdin, "{}", payload_str);
        }

        let output = child.wait_with_output().map_err(|e| {
            tracing::error!(
                server = %server_name,
                error = %e,
                "Error waiting for stdio MCP process output"
            );
            format!("Error executing MCP process output: {}", e)
        })?;

        if !output.status.success() {
            let err_text = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "MCP server '{}' exited with status {}: {}",
                server_name, output.status, err_text
            ));
        }

        let stdout_str = String::from_utf8_lossy(&output.stdout);
        Self::parse_jsonrpc_response(server_name, tool_name, &stdout_str)
    }

    fn call_sse(
        server_name: &str,
        url: &str,
        headers: &HashMap<String, String>,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        tracing::debug!(
            server = %server_name,
            url = %url,
            tool = %tool_name,
            "Calling SSE MCP transport"
        );

        if url.trim().is_empty() {
            return Err(format!(
                "SSE MCP server '{}' has empty endpoint URL.",
                server_name
            ));
        }

        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments
            }
        });

        let mut req = ureq::post(url).set("Content-Type", "application/json");
        for (k, v) in headers {
            req = req.set(k, v);
        }

        let response_str = match req.send_json(payload) {
            Ok(resp) => resp
                .into_string()
                .map_err(|e| format!("Failed to read response body: {}", e))?,
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                return Err(format!(
                    "MCP SSE server '{}' returned HTTP {}: {}",
                    server_name, code, body
                ));
            }
            Err(e) => {
                return Err(format!(
                    "MCP SSE transport network error for '{}': {}",
                    server_name, e
                ));
            }
        };

        Self::parse_jsonrpc_response(server_name, tool_name, &response_str)
    }

    fn parse_jsonrpc_response(
        server_name: &str,
        tool_name: &str,
        response_text: &str,
    ) -> Result<serde_json::Value, String> {
        let json: serde_json::Value = serde_json::from_str(response_text.trim()).map_err(|e| {
            tracing::error!(
                server = %server_name,
                tool = %tool_name,
                response = %response_text,
                error = %e,
                "Invalid JSON-RPC response from MCP server"
            );
            format!(
                "Invalid JSON-RPC response from MCP server '{}': {}",
                server_name, e
            )
        })?;

        if let Some(err) = json.get("error") {
            let msg = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown JSON-RPC error");
            let code = err.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
            return Err(format!(
                "MCP server '{}' returned error (code {}): {}",
                server_name, code, msg
            ));
        }

        if let Some(result) = json.get("result") {
            Ok(result.clone())
        } else {
            Ok(json)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_tool_adapter_metadata_and_safety() {
        let manager = Arc::new(McpClientManager::new());
        let adapter = McpToolAdapter::new(
            "test_server",
            "test_tool",
            "A test tool",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                }
            }),
            manager,
        );

        assert_eq!(adapter.server_name(), "test_server");
        assert_eq!(adapter.name(), "test_tool");
        assert_eq!(adapter.description(), "A test tool");
        assert_eq!(adapter.safety(), Safety::Mutating);
        assert_eq!(adapter.parameters_schema()["type"].as_str(), Some("object"));

        let mut config = AppConfig::default();
        assert!(!adapter.is_enabled(&config, "prompt"));

        config.mcp_servers.insert(
            "test_server".to_string(),
            McpServerConfig::Stdio {
                command: "echo".to_string(),
                args: vec![],
                env: HashMap::new(),
            },
        );
        assert!(adapter.is_enabled(&config, "prompt"));
    }

    #[test]
    fn test_mcp_client_manager_unconfigured_server_error() {
        let manager = McpClientManager::new();
        let result = manager.call_tool("unknown_server", "tool_name", serde_json::json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("is not configured"));
    }

    #[test]
    fn test_mcp_client_manager_empty_command_or_url() {
        let manager = McpClientManager::new();
        let mut config = AppConfig::default();
        config.mcp_servers.insert(
            "empty_stdio".to_string(),
            McpServerConfig::Stdio {
                command: "  ".to_string(),
                args: vec![],
                env: HashMap::new(),
            },
        );
        config.mcp_servers.insert(
            "empty_sse".to_string(),
            McpServerConfig::Sse {
                url: "".to_string(),
                headers: HashMap::new(),
            },
        );
        manager.update_config(&config);

        let stdio_res = manager.call_tool("empty_stdio", "my_tool", serde_json::json!({}));
        assert!(stdio_res.is_err());
        assert!(stdio_res.unwrap_err().contains("empty command path"));

        let sse_res = manager.call_tool("empty_sse", "my_tool", serde_json::json!({}));
        assert!(sse_res.is_err());
        assert!(sse_res.unwrap_err().contains("empty endpoint URL"));
    }

    #[test]
    fn test_parse_jsonrpc_response_success_and_error() {
        let valid_resp =
            r#"{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"hello"}]}}"#;
        let parsed = McpClientManager::parse_jsonrpc_response("srv", "tool", valid_resp).unwrap();
        assert_eq!(parsed["content"][0]["text"].as_str(), Some("hello"));

        let err_resp =
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"Method not found"}}"#;
        let err_res = McpClientManager::parse_jsonrpc_response("srv", "tool", err_resp);
        assert!(err_res.is_err());
        let err_msg = err_res.unwrap_err();
        assert!(err_msg.contains("-32601"));
        assert!(err_msg.contains("Method not found"));

        let invalid_json = "not json";
        let invalid_res = McpClientManager::parse_jsonrpc_response("srv", "tool", invalid_json);
        assert!(invalid_res.is_err());
        assert!(
            invalid_res
                .unwrap_err()
                .contains("Invalid JSON-RPC response")
        );
    }
}
