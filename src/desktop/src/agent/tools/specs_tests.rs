//! Unit tests for the per-family tool enable specs and prompt rules.

use super::*;
use crate::config::{
    AgentConfig, CalDavClient, JmapClient, McpServerConfig, McpServerEntry, TrelloClient,
};

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

#[test]
fn test_csv_prompt_rule_gates_on_prompt_keyword() {
    let spec = csv_prompt_rule();
    let mut c = AgentConfig::default();
    c.tool_groups.csv_db = true;
    // Negative cases — no keyword in the prompt.
    assert!(!spec.is_enabled_for(&c, "what is the weather today"));
    assert!(!spec.is_enabled_for(&c, "summarise my notes"));
    // Positive cases — the tool name itself or a synonym.
    assert!(spec.is_enabled_for(&c, "show me the csv database"));
    assert!(spec.is_enabled_for(&c, "add_rows to my csv"));
    assert!(spec.is_enabled_for(&c, "TABLE of contents"));
}

#[test]
fn test_csv_prompt_rule_gates_on_group_flag() {
    let spec = csv_prompt_rule();
    let mut c = AgentConfig::default();
    c.tool_groups.csv_db = false;
    // Even with a matching keyword, the group flag must also be on.
    assert!(!spec.is_enabled_for(&c, "show me the csv database"));
    c.tool_groups.csv_db = true;
    assert!(spec.is_enabled_for(&c, "show me the csv database"));
}

#[test]
fn test_csv_prompt_rule_case_insensitive() {
    let spec = csv_prompt_rule();
    let mut c = AgentConfig::default();
    c.tool_groups.csv_db = true;
    assert!(spec.is_enabled_for(&c, "query the CSV"));
    assert!(spec.is_enabled_for(&c, "what is a TABLE"));
}
