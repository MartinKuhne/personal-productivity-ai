//! Unit tests for the [`super::descriptor`] module — `ToolDescriptor`,
//! `ToolConfigSpec`, `ConfigPredicate`, `PromptPredicate`, and
//! `group_enabled`.

use super::descriptor::{
    ConfigPredicate, PromptPredicate, ToolConfigSpec, ToolDescriptor, group_enabled,
};
use crate::agent::tools::Safety;
use crate::agent::tools::registry::groups::{InternalToolGroup, ToolGroupId};
use crate::config::{
    AppConfig, CalDavClient, JmapClient, McpServerConfig, McpServerEntry, ToolGroupsConfig,
    TrelloClient,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, JsonSchema)]
struct DummyInput {
    q: String,
}

fn jmap() -> JmapClient {
    JmapClient {
        url: "http://localhost".to_string(),
        token: "t".to_string(),
    }
}

fn caldav() -> CalDavClient {
    CalDavClient {
        url: "http://localhost".to_string(),
        username: "u".to_string(),
        password: "p".to_string(),
    }
}

fn trello() -> TrelloClient {
    TrelloClient {
        token: "t".to_string(),
        api_key: "k".to_string(),
    }
}

fn mcp_entry(enabled: bool) -> McpServerEntry {
    McpServerEntry {
        enabled,
        config: McpServerConfig::Sse {
            url: "http://localhost".to_string(),
            headers: Default::default(),
            oauth: None,
        },
    }
}

#[test]
fn test_group_only_spec_gates_on_group_flag() {
    let mut c = AppConfig {
        tool_groups: ToolGroupsConfig {
            filesystem: false,
            ..ToolGroupsConfig::default()
        },
        ..AppConfig::default()
    };
    let spec = ToolConfigSpec::group_only(ToolGroupId::Internal(InternalToolGroup::Filesystem));
    assert!(!spec.is_enabled_for(&c, ""));
    c.tool_groups.filesystem = true;
    assert!(spec.is_enabled_for(&c, ""));
}

#[test]
fn test_searxng_predicate() {
    let mut c = AppConfig::default();
    c.tool_groups.web = false;
    c.searxng_url = None;
    let spec = ToolConfigSpec {
        group: Some(ToolGroupId::Internal(InternalToolGroup::Web)),
        requires: vec![ConfigPredicate::SearxngConfigured],
        prompt_rule: None,
    };
    // Group off → disabled regardless of URL.
    assert!(!spec.is_enabled_for(&c, ""));
    c.tool_groups.web = true;
    // Group on, URL missing → still disabled.
    assert!(!spec.is_enabled_for(&c, ""));
    c.searxng_url = Some("http://localhost:8090".to_string());
    // Group on, URL present → enabled.
    assert!(spec.is_enabled_for(&c, ""));
}

#[test]
fn test_trello_predicate() {
    let mut c = AppConfig::default();
    let spec = crate::app::tool_specs::trello_spec();
    assert!(!spec.is_enabled_for(&c, ""));
    c.trello_client = Some(trello());
    assert!(spec.is_enabled_for(&c, ""));
}

#[test]
fn test_jmap_clients_present_predicate() {
    let mut c = AppConfig::default();
    let spec = ToolConfigSpec {
        group: Some(ToolGroupId::Internal(InternalToolGroup::Email)),
        requires: vec![ConfigPredicate::JmapClientsPresent],
        prompt_rule: None,
    };
    assert!(!spec.is_enabled_for(&c, ""));
    c.jmap_clients.insert("p".into(), jmap());
    assert!(spec.is_enabled_for(&c, ""));
}

#[test]
fn test_dav_or_jmap_predicate_follows_feature_flag() {
    let mut c = AppConfig::default();
    c.jmap_clients.insert("j".into(), jmap());
    c.caldav_clients.insert("d".into(), caldav());
    let spec = crate::app::tool_specs::contacts_spec();
    // Default: feature flag off → JMAP.
    assert!(spec.is_enabled_for(&c, ""));
    // Flag on → CalDAV.
    c.feature_flags
        .insert("useDAVForContacts".to_string(), true);
    assert!(spec.is_enabled_for(&c, ""));
    // No clients on the chosen side → disabled.
    c.jmap_clients.clear();
    c.caldav_clients.clear();
    c.feature_flags
        .insert("useDAVForContacts".to_string(), false);
    assert!(!spec.is_enabled_for(&c, ""));
}

#[test]
fn test_never_predicate_is_always_false() {
    let spec = ToolConfigSpec {
        group: Some(ToolGroupId::Internal(InternalToolGroup::Contacts)),
        requires: vec![ConfigPredicate::Never],
        prompt_rule: None,
    };
    assert!(!spec.is_enabled_for(&AppConfig::default(), ""));
}

#[test]
fn test_group_enabled_for_internal_groups() {
    let mut c = AppConfig::default();
    c.tool_groups.filesystem = true;
    assert!(group_enabled(
        &c,
        &ToolGroupId::Internal(InternalToolGroup::Filesystem)
    ));
    assert!(!group_enabled(
        &c,
        &ToolGroupId::Internal(InternalToolGroup::Browser)
    ));
}

#[test]
fn test_group_enabled_for_mcp_servers() {
    let mut c = AppConfig::default();
    c.mcp_servers.insert("srv".to_string(), mcp_entry(false));
    let id = ToolGroupId::Mcp("srv".to_string());
    assert!(!group_enabled(&c, &id));
    c.mcp_servers.insert("srv".to_string(), mcp_entry(true));
    assert!(group_enabled(&c, &id));
}

#[test]
fn test_prompt_predicate_case_insensitive() {
    let p = PromptPredicate::ContainsAny(&["TABLE", "csv"]);
    assert!(p.matches("show me the CSV"));
    assert!(!p.matches("show me the json"));
}

#[test]
fn test_tool_descriptor_new_generates_schema() {
    let d = ToolDescriptor::new::<DummyInput>(
        "dummy",
        "test tool",
        Safety::ReadOnly,
        ToolConfigSpec::group_only(ToolGroupId::Internal(InternalToolGroup::Filesystem)),
        ToolGroupId::Internal(InternalToolGroup::Filesystem),
    );
    assert_eq!(d.name, "dummy");
    assert_eq!(d.safety, Safety::ReadOnly);
    assert!(d.parameters_schema.get("properties").is_some());
}

#[test]
fn test_tool_descriptor_with_json_schema_uses_value() {
    let schema = serde_json::json!({"type": "object", "properties": {}});
    let d = ToolDescriptor::with_json_schema(
        "mcp_tool",
        "from the wire",
        schema.clone(),
        Safety::Mutating,
        ToolConfigSpec::group_only(ToolGroupId::Mcp("srv".to_string())),
        ToolGroupId::Mcp("srv".to_string()),
    );
    assert_eq!(d.parameters_schema, schema);
    assert_eq!(d.safety, Safety::Mutating);
}
