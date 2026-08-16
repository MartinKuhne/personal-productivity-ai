//! Unit tests for the [`super::descriptor`] module — `ToolDescriptor`,
//! `ToolConfigSpec`, `ConfigPredicate`, `PromptPredicate`, and
//! `group_enabled`.

use super::descriptor::{
    ConfigPredicate, LatencyClass, PromptPredicate, SessionRequirement, ToolConfigSpec,
    ToolDescriptor, ToolProfile, group_enabled,
};
use crate::config::AgentConfig;
use crate::config::{
    CalDavClient, JmapClient, McpServerConfig, McpServerEntry, ToolGroupsConfig, TrelloClient,
};
use crate::tools::Safety;
use crate::tools::registry::groups::{InternalToolGroup, ToolGroupId};
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
    let mut c = AgentConfig {
        tool_groups: ToolGroupsConfig {
            filesystem: false,
            ..ToolGroupsConfig::default()
        },
        ..AgentConfig::default()
    };
    let spec = ToolConfigSpec::group_only(ToolGroupId::Internal(InternalToolGroup::Filesystem));
    assert!(!spec.is_enabled_for(&c, ""));
    c.tool_groups.filesystem = true;
    assert!(spec.is_enabled_for(&c, ""));
}

#[test]
fn test_searxng_predicate() {
    let mut c = AgentConfig::default();
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
    let mut c = AgentConfig::default();
    let spec = crate::tools::specs::trello_spec();
    assert!(!spec.is_enabled_for(&c, ""));
    c.trello_client = Some(trello());
    assert!(spec.is_enabled_for(&c, ""));
}

#[test]
fn test_jmap_clients_present_predicate() {
    let mut c = AgentConfig::default();
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
    let mut c = AgentConfig::default();
    c.jmap_clients.insert("j".into(), jmap());
    c.caldav_clients.insert("d".into(), caldav());
    let spec = crate::tools::specs::contacts_spec();
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
    assert!(!spec.is_enabled_for(&AgentConfig::default(), ""));
}

#[test]
fn test_group_enabled_for_internal_groups() {
    let mut c = AgentConfig::default();
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
    let mut c = AgentConfig::default();
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

// ---------------------------------------------------------------------------
// ToolProfile — defaults, custom values, LatencyClass, SessionRequirement.
// ---------------------------------------------------------------------------

#[test]
fn test_tool_profile_defaults_match_spec() {
    // Spec: idempotent=false (the conservative default — true only
    // after a tool has been audited), latency_class=Fast, no user
    // confirmation, no session sharing.
    let p = ToolProfile::default();
    assert!(!p.idempotent);
    assert_eq!(p.latency_class, LatencyClass::Fast);
    assert!(!p.requires_user_confirmation);
    assert_eq!(p.session, SessionRequirement::None);
}

#[test]
fn test_tool_profile_defaults_helper_is_default() {
    // The named `defaults()` constructor is just an alias for
    // `Default::default` — locked in here so the two can't drift.
    assert_eq!(ToolProfile::defaults(), ToolProfile::default());
}

#[test]
fn test_tool_profile_constructor_attaches_default_profile() {
    // `ToolDescriptor::new` must populate `profile` (the field was
    // added late; constructors that forget to set it cause
    // read-then-uninit bugs that are silent at compile time).
    let d = ToolDescriptor::new::<DummyInput>(
        "dummy",
        "test",
        Safety::ReadOnly,
        ToolConfigSpec::group_only(ToolGroupId::Internal(InternalToolGroup::Filesystem)),
        ToolGroupId::Internal(InternalToolGroup::Filesystem),
    );
    assert_eq!(d.profile, ToolProfile::default());
}

#[test]
fn test_tool_profile_constructor_with_json_schema_attaches_default_profile() {
    let d = ToolDescriptor::with_json_schema(
        "mcp",
        "from wire",
        serde_json::json!({"type": "object"}),
        Safety::Mutating,
        ToolConfigSpec::group_only(ToolGroupId::Mcp("srv".to_string())),
        ToolGroupId::Mcp("srv".to_string()),
    );
    assert_eq!(d.profile, ToolProfile::default());
}

#[test]
fn test_tool_profile_with_profile_replaces() {
    // Builder-style: `with_profile` swaps the default out for a
    // caller-supplied one and returns the descriptor for chaining.
    let custom = ToolProfile {
        idempotent: false,
        latency_class: LatencyClass::Interactive,
        requires_user_confirmation: true,
        session: SessionRequirement::None,
    };
    let d = ToolDescriptor::new::<DummyInput>(
        "send_email",
        "send a message",
        Safety::Mutating,
        ToolConfigSpec::group_only(ToolGroupId::Internal(InternalToolGroup::Email)),
        ToolGroupId::Internal(InternalToolGroup::Email),
    )
    .with_profile(custom.clone());
    assert_eq!(d.profile, custom);
    assert!(d.profile.requires_user_confirmation);
    assert_eq!(d.profile.latency_class, LatencyClass::Interactive);
}

#[test]
fn test_latency_class_default_is_fast() {
    // `Default` for the enum is `Fast` — every other variant is
    // opt-in, so a tool that forgets to declare its latency still
    // claims the cheapest class.
    assert_eq!(LatencyClass::default(), LatencyClass::Fast);
}

#[test]
fn test_latency_class_variants_distinct() {
    // The three classes are distinct values; the descriptor uses
    // `PartialEq` to drive per-class UI affordances so the
    // comparison must be meaningful.
    assert_ne!(LatencyClass::Fast, LatencyClass::Interactive);
    assert_ne!(LatencyClass::Interactive, LatencyClass::Slow);
    assert_ne!(LatencyClass::Fast, LatencyClass::Slow);
}

#[test]
fn test_session_requirement_default_is_none() {
    assert_eq!(SessionRequirement::default(), SessionRequirement::None);
}

#[test]
fn test_session_requirement_shared_is_distinct_by_name() {
    // Two `Shared` variants with the same name compare equal —
    // a future per-session serialiser treats them as the same lock.
    let a = SessionRequirement::Shared(std::borrow::Cow::Borrowed("browser"));
    let b = SessionRequirement::Shared(std::borrow::Cow::Borrowed("browser"));
    let c = SessionRequirement::Shared(std::borrow::Cow::Borrowed("ssh"));
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert_ne!(a, SessionRequirement::None);
}
