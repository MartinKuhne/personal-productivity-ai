use crate::config::AgentConfig;
// Tests for [`McpToolAdapter`] — the LLM-tool-loop adapter that
// exposes MCP-discovered tools to the agent tool registry.
//
// Sidecar file for `crate::tools::mcp`. Extracted from the
// original `tests.rs` when the protocol layer moved to
// `crate::lib::mcp`; only the adapter-side tests live here.
// Protocol-layer tests now live in
// `crate::lib::mcp::tests`.

use super::*;
use crate::config::{McpServerConfig, McpServerEntry};
use crate::lib::mcp::McpClients;
use crate::tools::{Safety, Tool};
use std::collections::HashMap;
use std::sync::Arc;

#[test]
fn test_mcp_tool_adapter_metadata_and_safety() {
    let manager = Arc::new(McpClients::new());
    let adapter = McpToolAdapter::new(
        "test_server",
        "test_tool",
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
    assert_eq!(adapter.description(), "[test_server] A test tool");
    assert_eq!(adapter.safety(), Safety::Mutating);
    assert_eq!(adapter.parameters_schema()["type"].as_str(), Some("object"));

    let mut config = AgentConfig::default();
    assert!(!adapter.is_enabled(&config, "prompt"));

    config.mcp_servers.insert(
        "test_server".to_string(),
        McpServerConfig::Stdio {
            command: "echo".to_string(),
            args: vec![],
            env: HashMap::new(),
        }
        .into(),
    );
    assert!(adapter.is_enabled(&config, "prompt"));
}

#[test]
fn test_mcp_tool_adapter_description_carries_server_prefix() {
    let manager = Arc::new(McpClients::new());
    let adapter = McpToolAdapter::new(
        "my_server",
        "my_server/remote_tool",
        "remote_tool",
        "Does work.",
        serde_json::json!({"type": "object", "properties": {}}),
        manager,
    );

    assert_eq!(adapter.prefixed_description(), "[my_server] Does work.");
    assert_eq!(adapter.description(), "[my_server] Does work.");
    assert_eq!(
        adapter.descriptor().description.as_ref(),
        "[my_server] Does work."
    );
}

#[test]
fn test_mcp_tool_adapter_empty_description_gets_prefix_and_fallback() {
    let manager = Arc::new(McpClients::new());
    let adapter = McpToolAdapter::new(
        "my_server",
        "my_server/remote_tool",
        "remote_tool",
        "   ",
        serde_json::json!({"type": "object", "properties": {}}),
        manager,
    );

    assert_eq!(
        adapter.prefixed_description(),
        "[my_server] No description."
    );
}

#[test]
fn test_mcp_tool_adapter_disabled_when_entry_disabled() {
    // Regression for CONFIG-012: an entry with `enabled: false`
    // must cause the adapter to be disabled even though the
    // server is present in the config map.
    let manager = Arc::new(McpClients::new());
    let adapter = McpToolAdapter::new(
        "test_server",
        "test_tool",
        "test_tool",
        "A test tool",
        serde_json::json!({"type": "object", "properties": {}}),
        manager,
    );

    let mut config = AgentConfig::default();
    config.mcp_servers.insert(
        "test_server".to_string(),
        McpServerEntry {
            enabled: false,
            config: McpServerConfig::Stdio {
                command: "echo".to_string(),
                args: vec![],
                env: HashMap::new(),
            },
        },
    );
    assert!(!adapter.is_enabled(&config, "prompt"));
}
