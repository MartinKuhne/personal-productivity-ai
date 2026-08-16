//! Per-family tool enable specs and prompt rules.
//!
//! Each function returns a [`ToolConfigSpec`] expressing what an
//! `AgentConfig` (and, where relevant, the current prompt) must
//! satisfy for the LLM to see a tool from a given family.

use crate::tools::descriptor::{ConfigPredicate, PromptPredicate, ToolConfigSpec};
use crate::tools::registry::groups::{InternalToolGroup, ToolGroupId};

/// Spec for the email family: the email group is on AND at
/// least one JMAP client is configured.
pub fn email_spec() -> ToolConfigSpec {
    let group = ToolGroupId::Internal(InternalToolGroup::Email);
    ToolConfigSpec {
        group: Some(group),
        requires: vec![ConfigPredicate::JmapClientsPresent],
        prompt_rule: None,
    }
}

/// Spec for the calendar family: the calendar group is on AND
/// at least one CalDAV client is configured.
pub fn calendar_spec() -> ToolConfigSpec {
    let group = ToolGroupId::Internal(InternalToolGroup::Calendar);
    ToolConfigSpec {
        group: Some(group),
        requires: vec![ConfigPredicate::CalDavClientsPresent],
        prompt_rule: None,
    }
}

/// Spec for the contacts family: the contacts group is on AND
/// at least one of `caldav_clients` / `jmap_clients` is
/// configured, depending on the `useDAVForContacts` feature
/// flag. The flag-and-presence check is encoded in
/// [`ConfigPredicate::DavOrJmapClients`].
pub fn contacts_spec() -> ToolConfigSpec {
    let group = ToolGroupId::Internal(InternalToolGroup::Contacts);
    ToolConfigSpec {
        group: Some(group),
        requires: vec![ConfigPredicate::DavOrJmapClients],
        prompt_rule: None,
    }
}

/// Spec for the Trello family: the trello group is on AND
/// `trello_client` is configured.
pub fn trello_spec() -> ToolConfigSpec {
    let group = ToolGroupId::Internal(InternalToolGroup::Trello);
    ToolConfigSpec {
        group: Some(group),
        requires: vec![ConfigPredicate::TrelloConfigured],
        prompt_rule: None,
    }
}

/// Spec for the `web_search` tool: the web group is on AND
/// `searxng_url` is configured. (The other web tools —
/// `web_fetch` and `web_delegate` — only need the group flag.)
pub fn web_search_spec() -> ToolConfigSpec {
    let group = ToolGroupId::Internal(InternalToolGroup::Web);
    ToolConfigSpec {
        group: Some(group),
        requires: vec![ConfigPredicate::SearxngConfigured],
        prompt_rule: None,
    }
}

/// The keywords the CSV family looks for in the prompt (TOOL-001).
/// Case-insensitive: the matcher lower-cases the prompt before
/// scanning.
pub fn csv_keywords() -> &'static [&'static str] {
    &[
        "table",
        "csv",
        "database",
        "add_rows",
        "delete_rows",
        "create_csv",
        "list_csv",
        "query",
    ]
}

/// Build the [`ToolConfigSpec`] for the CSV family of tools. The
/// spec is gated on:
/// 1. the `tool_groups.csv_db` flag being on (the group
///    enable), and
/// 2. the current prompt containing at least one of the CSV
///    keywords (TOOL-001).
pub fn csv_prompt_rule() -> ToolConfigSpec {
    let group = ToolGroupId::Internal(InternalToolGroup::CsvDb);
    ToolConfigSpec {
        group: Some(group),
        requires: Vec::new(),
        prompt_rule: Some(PromptPredicate::ContainsAny(csv_keywords())),
    }
}

#[cfg(test)]
#[path = "specs_tests.rs"]
mod tests;
