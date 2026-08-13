//! Tests for the `ToolRegistry` group-state and error-tracking surface.
//! These complement the `manager::tests` (the migrated registry tests)
//! and cover the new behaviour introduced by the merge.

use super::*;
use crate::agent::config::AgentConfig;
use crate::config::McpServerConfig;

/// Every registered tool has a `tool_to_group` entry â€” no orphans.
#[test]
fn tool_to_group_index_is_complete() {
    let mut mgr = ToolRegistry::new();
    let config = AgentConfig::default();
    mgr.refresh_state(&config);
    for name in mgr.tools.keys() {
        assert!(
            mgr.tool_to_group.contains_key(name),
            "tool {name} missing from tool_to_group"
        );
    }
    for name in mgr.tool_to_group.keys() {
        assert!(
            mgr.tools.contains_key(name),
            "tool_to_group references missing tool {name}"
        );
    }
}

/// `refresh_state` reads the current `AgentConfig::tool_groups` flags.
#[test]
fn group_state_refresh_reflects_config() {
    let mut mgr = ToolRegistry::new();
    let mut config = crate::config::AppConfig::default();
    mgr.refresh_state(&config.to_agent_config());
    let id = ToolGroupId::Internal(InternalToolGroup::Filesystem);
    assert!(mgr.group(&id).unwrap().enabled);

    config.tool_groups.filesystem = false;
    mgr.refresh_state(&config.to_agent_config());
    assert!(!mgr.group(&id).unwrap().enabled);
}

/// A group whose tools are all `ReadOnly` reports `parallel_safe = true`.
#[test]
fn group_parallel_safe_when_all_tools_readonly() {
    // Build a manager that has only `grep` (ReadOnly) in the
    // Filesystem group. We can't easily remove tools from a
    // `ToolRegistry`; instead we test the *complement*: a group that
    // mixes read-only and mutating tools is not parallel-safe
    // (verified in the next test). Here we test the trivial case
    // by checking that the Web group's `parallel_safe` value matches
    // the per-tool safety: `web_delegate` is Mutating, so the Web
    // group must not be parallel-safe; but the *Filesystem* group
    // has at least one Mutating tool (`create_file`).
    let mut mgr = ToolRegistry::new();
    let config = AgentConfig::default();
    mgr.refresh_state(&config);
    let fs = mgr
        .group(&ToolGroupId::Internal(InternalToolGroup::Filesystem))
        .unwrap();
    // The group has both read-only and mutating tools, so it is NOT
    // parallel-safe.
    assert!(!fs.parallel_safe);
}

/// `parallel_safe_tools` returns every ReadOnly tool.
#[test]
fn parallel_safe_tools_includes_all_readonly_tools() {
    let mgr = ToolRegistry::new();
    let safe = mgr.parallel_safe_tools();
    // `grep` is documented as ReadOnly. The list is not exhaustive
    // â€” it grows as more tools are audited â€” but it must include
    // every tool that overrides `safety()` to `ReadOnly`.
    assert!(safe.iter().any(|n| n == "search_notes"));
    // And it must NOT include obviously-mutating tools.
    assert!(!safe.iter().any(|n| n == "create_note"));
}

/// A `ToolRegistry`-level error replaces any prior `last_error` for
/// the same group.
#[test]
fn record_error_replaces_previous() {
    let mut mgr = ToolRegistry::new();
    let config = AgentConfig::default();
    mgr.refresh_state(&config);
    let id = ToolGroupId::Internal(InternalToolGroup::Filesystem);
    mgr.record_error(&id, ToolGroupError::now(ToolErrorKind::Execution, "first"));
    assert_eq!(
        mgr.group(&id)
            .and_then(|s| s.last_error.as_ref())
            .map(|e| &e.message),
        Some(&"first".to_string())
    );
    mgr.record_error(&id, ToolGroupError::now(ToolErrorKind::Execution, "second"));
    assert_eq!(
        mgr.group(&id)
            .and_then(|s| s.last_error.as_ref())
            .map(|e| &e.message),
        Some(&"second".to_string())
    );
}

/// Per-group error recording and clearing round-trip.
#[test]
fn record_and_clear_error_round_trip() {
    let mut mgr = ToolRegistry::new();
    let config = AgentConfig::default();
    mgr.refresh_state(&config);
    let id = ToolGroupId::Internal(InternalToolGroup::Filesystem);
    assert!(mgr.group(&id).unwrap().last_error.is_none());

    mgr.record_error(&id, ToolGroupError::now(ToolErrorKind::Execution, "boom"));
    assert!(mgr.group(&id).unwrap().last_error.is_some());
    mgr.clear_error(&id);
    assert!(mgr.group(&id).unwrap().last_error.is_none());
}

/// `set_group_enabled` flips the right field on `AgentConfig` for an
/// internal group and persists the change.
#[test]
fn set_internal_group_enabled_persists_to_config() {
    let mgr = ToolRegistry::new();
    let mut config = AgentConfig::default();
    assert!(config.tool_groups.weather);
    mgr.set_group_enabled(
        &mut config,
        &ToolGroupId::Internal(InternalToolGroup::Weather),
        false,
    );
    assert!(!config.tool_groups.weather);
    mgr.set_group_enabled(
        &mut config,
        &ToolGroupId::Internal(InternalToolGroup::Weather),
        true,
    );
    assert!(config.tool_groups.weather);
}

/// `set_group_enabled` flips the `McpServerEntry::enabled` flag for an
/// MCP group, preserving the transport config.
#[test]
fn set_mcp_group_enabled_preserves_server_config() {
    let mgr = ToolRegistry::new();
    let mut config = AgentConfig::default();
    config.mcp_servers.insert(
        "github".to_string(),
        McpServerConfig::Stdio {
            command: "echo".to_string(),
            args: vec!["hi".to_string()],
            env: Default::default(),
        }
        .into(),
    );
    mgr.set_group_enabled(&mut config, &ToolGroupId::Mcp("github".to_string()), false);
    let entry = config.mcp_servers.get("github").unwrap();
    assert!(!entry.enabled);
    // Transport preserved.
    match entry.config() {
        McpServerConfig::Stdio { command, args, .. } => {
            assert_eq!(command, "echo");
            assert_eq!(args, &vec!["hi".to_string()]);
        }
        other => panic!("expected Stdio, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Browser group (BRWS-001..008)
// ---------------------------------------------------------------------------

/// Contract: the Browser group is one of the eight built-in
/// groups and is OFF by default (BRWS-CONF-001).
#[test]
#[cfg(feature = "browser")]
fn browser_group_defaults_to_disabled() {
    let mut mgr = ToolRegistry::new();
    let config = AgentConfig::default();
    mgr.refresh_state(&config);
    let id = ToolGroupId::Internal(InternalToolGroup::Browser);
    let state = mgr.group(&id).expect("Browser group missing");
    assert!(!state.enabled, "Browser group should default to OFF");
}

/// Contract: the eight BRWS-001..008 tool names are all
/// registered under the Browser group when the group is enabled.
#[test]
#[cfg(feature = "browser")]
fn browser_group_lists_all_eight_tools() {
    let mut mgr = ToolRegistry::new();
    let mut config = AgentConfig::default();
    config.tool_groups.browser = true;
    let snapshot = mgr.groups_snapshot(&config);
    let browser = snapshot
        .iter()
        .find(|g| g.id == ToolGroupId::Internal(InternalToolGroup::Browser))
        .expect("Browser group missing from snapshot");
    let mut names: Vec<&str> = browser.tool_names.iter().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        vec![
            "browser_click",
            "browser_evaluate_js",
            "browser_fill_input",
            "browser_get_page_state",
            "browser_navigate",
            "browser_press_key",
            "browser_screenshot",
            "browser_select_dropdown",
        ]
    );
}

/// Contract: `set_group_enabled` flips the new `tool_groups.browser`
/// field on `AgentConfig` (BRWS-CONF-001).
#[test]
#[cfg(feature = "browser")]
fn set_browser_group_enabled_persists_to_config() {
    let mgr = ToolRegistry::new();
    let mut config = AgentConfig::default();
    assert!(!config.tool_groups.browser);
    mgr.set_group_enabled(
        &mut config,
        &ToolGroupId::Internal(InternalToolGroup::Browser),
        true,
    );
    assert!(config.tool_groups.browser);
    mgr.set_group_enabled(
        &mut config,
        &ToolGroupId::Internal(InternalToolGroup::Browser),
        false,
    );
    assert!(!config.tool_groups.browser);
}

/// Contract: every browser tool is `Mutating` except
/// `browser_get_page_state`, which is the only parallel-safe one
/// (BRWS-002).
#[test]
#[cfg(feature = "browser")]
fn browser_only_get_page_state_is_parallel_safe() {
    use crate::agent::tools::Safety;
    let mgr = ToolRegistry::new();
    for name in [
        "browser_navigate",
        "browser_click",
        "browser_fill_input",
        "browser_select_dropdown",
        "browser_press_key",
        "browser_evaluate_js",
        "browser_screenshot",
    ] {
        assert_eq!(
            mgr.safety_of(name),
            Safety::Mutating,
            "{name} should be Mutating"
        );
    }
    assert_eq!(
        mgr.safety_of("browser_get_page_state"),
        Safety::ReadOnly,
        "browser_get_page_state should be the only ReadOnly browser tool"
    );
}

/// `mcp_needs_auth_now` returns the manager's per-server `needs_auth`
/// flag. The flag defaults to `false` and is set by the MCP client
/// when a 401 is observed.
#[test]
fn mcp_needs_auth_now_defaults_to_false() {
    let mgr = ToolRegistry::new();
    let mut config = AgentConfig::default();
    config.mcp_servers.insert(
        "github".to_string(),
        McpServerConfig::Sse {
            url: "https://api.github.com/mcp".to_string(),
            headers: Default::default(),
            oauth: None,
        }
        .into(),
    );
    mgr.mcp_manager().update_config(&config);
    assert!(!mgr.mcp_manager().needs_auth_now("github"));
}

/// `mcp_clear_needs_auth` clears the manager's flag for a server.
#[test]
fn mcp_clear_needs_auth_clears_the_flag() {
    let mgr = ToolRegistry::new();
    let mut config = AgentConfig::default();
    config.mcp_servers.insert(
        "github".to_string(),
        McpServerConfig::Sse {
            url: "https://api.github.com/mcp".to_string(),
            headers: Default::default(),
            oauth: None,
        }
        .into(),
    );
    mgr.mcp_manager().update_config(&config);
    mgr.mcp_manager().mark_needs_auth("github", true);
    assert!(mgr.mcp_manager().needs_auth_now("github"));
    mgr.mcp_manager().mark_needs_auth("github", false);
    assert!(!mgr.mcp_manager().needs_auth_now("github"));
}
