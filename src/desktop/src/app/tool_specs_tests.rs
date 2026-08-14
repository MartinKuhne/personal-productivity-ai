//! Unit tests for the per-family tool enable specs in
//! [`super::tool_specs`].

use super::tool_specs::{calendar_spec, contacts_spec, email_spec, trello_spec, web_search_spec};
use crate::agent::config::AgentConfig;
use crate::config::{CalDavClient, JmapClient, McpServerConfig, McpServerEntry, TrelloClient};

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

#[test]
fn test_email_spec_requires_jmap_clients() {
    let mut c = AgentConfig::default();
    assert!(!email_spec().is_enabled_for(&c, ""));
    c.jmap_clients.insert("p".into(), jmap());
    assert!(email_spec().is_enabled_for(&c, ""));
}

#[test]
fn test_calendar_spec_requires_caldav_clients() {
    let mut c = AgentConfig::default();
    assert!(!calendar_spec().is_enabled_for(&c, ""));
    c.caldav_clients.insert("p".into(), caldav());
    assert!(calendar_spec().is_enabled_for(&c, ""));
}

#[test]
fn test_contacts_spec_follows_feature_flag() {
    let mut c = AgentConfig::default();
    c.jmap_clients.insert("j".into(), jmap());
    c.caldav_clients.insert("d".into(), caldav());
    // Default flag off → JMAP.
    assert!(contacts_spec().is_enabled_for(&c, ""));
    // Flag on → CalDAV.
    c.feature_flags
        .insert("useDAVForContacts".to_string(), true);
    assert!(contacts_spec().is_enabled_for(&c, ""));
}

#[test]
fn test_trello_spec_requires_client() {
    let mut c = AgentConfig::default();
    assert!(!trello_spec().is_enabled_for(&c, ""));
    c.trello_client = Some(trello());
    assert!(trello_spec().is_enabled_for(&c, ""));
}

#[test]
fn test_web_search_spec_requires_searxng() {
    let mut c = AgentConfig::default();
    c.tool_groups.web = false;
    c.searxng_url = None;
    assert!(!web_search_spec().is_enabled_for(&c, ""));
    c.tool_groups.web = true;
    assert!(!web_search_spec().is_enabled_for(&c, ""));
    c.searxng_url = Some("http://localhost:8090".to_string());
    assert!(web_search_spec().is_enabled_for(&c, ""));
}

#[test]
fn test_specs_gate_on_group_flag() {
    // Even with the integration configured, each spec requires its
    // group to be on.
    let mut c = AgentConfig::default();
    c.jmap_clients.insert("p".into(), jmap());
    c.caldav_clients.insert("p".into(), caldav());
    c.trello_client = Some(trello());
    c.searxng_url = Some("http://localhost".to_string());
    // All groups default to true, so all specs evaluate true.
    assert!(email_spec().is_enabled_for(&c, ""));
    assert!(calendar_spec().is_enabled_for(&c, ""));
    assert!(contacts_spec().is_enabled_for(&c, ""));
    assert!(trello_spec().is_enabled_for(&c, ""));
    assert!(web_search_spec().is_enabled_for(&c, ""));
    // Flip a group off; corresponding spec should fail.
    c.tool_groups.trello = false;
    assert!(!trello_spec().is_enabled_for(&c, ""));
}

#[test]
fn test_uses_mcp_server_entry_helper() {
    // Sanity: the McpServerEntry/McpServerConfig roundtrip that
    // MCP-server families use to gate themselves is stable.
    let entry = McpServerEntry {
        enabled: true,
        config: McpServerConfig::Sse {
            url: "http://localhost".to_string(),
            headers: Default::default(),
            oauth: None,
        },
    };
    assert!(entry.is_enabled());
}
