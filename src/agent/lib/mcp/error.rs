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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn from_jsonrpc_uses_provided_fields() {
        let err = McpError::from_jsonrpc(
            "srv",
            &json!({"code": -32601, "message": "Method not found", "data": {"x": 1}}),
        );
        assert_eq!(err.code, -32601);
        assert_eq!(err.message, "Method not found");
        assert_eq!(err.data, Some(json!({"x": 1})));
        assert_eq!(err.context.as_deref(), Some("server 'srv'"));
    }

    #[test]
    fn from_jsonrpc_missing_code_defaults_to_minus_one() {
        let err = McpError::from_jsonrpc("srv", &json!({"message": "hi"}));
        assert_eq!(err.code, -1);
    }

    #[test]
    fn from_jsonrpc_missing_message_defaults() {
        let err = McpError::from_jsonrpc("srv", &json!({"code": 1}));
        assert_eq!(err.message, "Unknown JSON-RPC error");
    }

    #[test]
    fn from_jsonrpc_absent_data_is_none() {
        let err = McpError::from_jsonrpc("srv", &json!({"code": 1, "message": "m"}));
        assert_eq!(err.data, None);
    }

    #[test]
    fn from_jsonrpc_non_object_value_yields_all_defaults() {
        let err = McpError::from_jsonrpc("srv", &json!(42));
        assert_eq!(err.code, -1);
        assert_eq!(err.message, "Unknown JSON-RPC error");
        assert_eq!(err.data, None);
        assert_eq!(err.context.as_deref(), Some("server 'srv'"));
    }

    #[test]
    fn transport_builds_code_minus_one_with_context() {
        let err = McpError::transport("HTTP 500", "boom");
        assert_eq!(err.code, -1);
        assert_eq!(err.message, "boom");
        assert_eq!(err.data, None);
        assert_eq!(err.context.as_deref(), Some("HTTP 500"));
    }

    #[test]
    fn display_with_context() {
        let err = McpError {
            code: -1,
            message: "boom".to_string(),
            data: None,
            context: Some("HTTP 500".to_string()),
        };
        assert_eq!(err.to_string(), "HTTP 500 (code -1): boom");
    }

    #[test]
    fn display_without_context() {
        let err = McpError {
            code: -32000,
            message: "oops".to_string(),
            data: None,
            context: None,
        };
        assert_eq!(err.to_string(), "code -32000: oops");
    }

    #[test]
    fn error_trait_and_string_conversion() {
        let err = McpError::transport("spawn", "failed");
        let boxed: Box<dyn std::error::Error> = Box::new(err.clone());
        assert_eq!(boxed.to_string(), "spawn (code -1): failed");
        let s: String = err.into();
        assert_eq!(s, "spawn (code -1): failed");
    }
}
