//! Tests for [`McpError`] (`agent::lib::mcp::error`).

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
