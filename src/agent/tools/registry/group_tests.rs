//! Tests for the ToolRegistry group-state and error-tracking surface.

use super::*;
use crate::config::AgentConfig;
use crate::config::AgentConfigBuilder;
use crate::config::McpServerConfig;

#[test]
fn group_state_refresh_reflects_config() {
    let mgr = ToolRegistry::new();
    let config = crate::config::AgentConfig::default();
    let id = ToolGroupId::Internal(InternalToolGroup::Filesystem);
    assert!(mgr.group(&id, &config).unwrap().enabled);

    let mut tool_groups = config.tool_groups().clone();
    tool_groups.filesystem = false;
    let config = crate::config::AgentConfigBuilder::new()
        .with_tool_groups(tool_groups)
        .build();
    assert!(!mgr.group(&id, &config).unwrap().enabled);
}

#[test]
fn group_parallel_safe_when_all_tools_readonly() {
    let mgr = ToolRegistry::new();
    let config = AgentConfig::default();
    let fs = mgr
        .group(
            &ToolGroupId::Internal(InternalToolGroup::Filesystem),
            &config,
        )
        .unwrap();
    assert!(!fs.parallel_safe);
}

#[test]
fn parallel_safe_tools_includes_all_readonly_tools() {
    let mgr = ToolRegistry::new();
    let safe = mgr.parallel_safe_tools();
    assert!(safe.iter().any(|n| n == "search_notes"));
    assert!(!safe.iter().any(|n| n == "create_note"));
}

#[test]
fn record_error_replaces_previous() {
    let mut mgr = ToolRegistry::new();
    let id = ToolGroupId::Internal(InternalToolGroup::Filesystem);
    mgr.record_error(&id, ToolGroupError::now(ToolErrorKind::Execution, "first"));
    let config = AgentConfig::default();
    assert_eq!(
        mgr.group(&id, &config)
            .and_then(|s| s.last_error)
            .map(|e| e.message),
        Some("first".to_string())
    );
    mgr.record_error(&id, ToolGroupError::now(ToolErrorKind::Execution, "second"));
    assert_eq!(
        mgr.group(&id, &config)
            .and_then(|s| s.last_error)
            .map(|e| e.message),
        Some("second".to_string())
    );
}

#[test]
fn record_and_clear_error_round_trip() {
    let mut mgr = ToolRegistry::new();
    let config = AgentConfig::default();
    let id = ToolGroupId::Internal(InternalToolGroup::Filesystem);
    assert!(mgr.group(&id, &config).unwrap().last_error.is_none());

    mgr.record_error(
        &id,
        ToolGroupError::now(crate::tools::registry::ToolErrorKind::Execution, "boom"),
    );
    assert!(mgr.group(&id, &config).unwrap().last_error.is_some());

    mgr.clear_error(&id);
    assert!(mgr.group(&id, &config).unwrap().last_error.is_none());
}

#[test]
fn set_internal_group_enabled_persists_to_config() {
    let mgr = ToolRegistry::new();
    let mut config = AgentConfig::default();
    assert!(config.tool_groups.filesystem);

    mgr.set_group_enabled(
        &mut config,
        &ToolGroupId::Internal(InternalToolGroup::Filesystem),
        false,
    );
    assert!(!config.tool_groups.filesystem);
}

#[test]
fn set_mcp_group_enabled_persists_to_config() {
    let mgr = ToolRegistry::new();
    let mut config = AgentConfigBuilder::new()
        .with_mcp_servers(std::collections::HashMap::from([(
            "github".to_string(),
            crate::config::McpServerEntry {
                enabled: true,
                config: McpServerConfig::Stdio {
                    command: "cmd".to_string(),
                    args: vec![],
                    env: std::collections::HashMap::new(),
                },
            },
        )]))
        .build();

    assert!(config.mcp_servers.get("github").unwrap().enabled);

    mgr.set_group_enabled(&mut config, &ToolGroupId::Mcp("github".to_string()), false);
    assert!(!config.mcp_servers.get("github").unwrap().enabled);
}

#[test]
fn mcp_manager_auth_override() {
    let mgr = ToolRegistry::new();
    assert!(!mgr.mcp_manager().needs_auth_now("github"));
    mgr.mcp_manager().mark_needs_auth("github", true);
    assert!(mgr.mcp_manager().needs_auth_now("github"));
    mgr.mcp_manager().mark_needs_auth("github", false);
    assert!(!mgr.mcp_manager().needs_auth_now("github"));
}
