//! Typed error type for MCP protocol errors.

use std::fmt;

/// A JSON-RPC `error` object returned by an MCP server, plus any
/// transport-level context that wrapped the round-trip.
#[derive(Debug, Clone)]
pub struct McpError {
    /// JSON-RPC error code (e.g. -32601 Method not found).
    pub code: i64,
    /// Human-readable error message from the server.
    pub message: String,
    /// Optional structured `data` payload from the server.
    pub data: Option<serde_json::Value>,
    /// Optional transport/context prefix (e.g. "spawn", "HTTP 500", "stdio").
    pub context: Option<String>,
}

impl McpError {
    /// Build an error from a JSON-RPC error object body.
    pub fn from_jsonrpc(server: &str, value: &serde_json::Value) -> Self {
        let code = value.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
        let message = value
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown JSON-RPC error")
            .to_owned();
        let data = value.get("data").cloned();
        Self {
            code,
            message,
            data,
            context: Some(format!("server '{server}'")),
        }
    }

    /// Build a transport-level error (no JSON-RPC error object).
    pub fn transport(context: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: -1,
            message: message.into(),
            data: None,
            context: Some(context.into()),
        }
    }
}

impl fmt::Display for McpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.context {
            Some(ctx) => write!(f, "{} (code {}): {}", ctx, self.code, self.message),
            None => write!(f, "code {}: {}", self.code, self.message),
        }
    }
}

impl std::error::Error for McpError {}

impl From<McpError> for String {
    fn from(err: McpError) -> Self {
        err.to_string()
    }
}
