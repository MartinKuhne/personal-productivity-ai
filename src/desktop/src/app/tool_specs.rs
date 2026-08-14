//! Per-family tool enable specs.
//!
//! Each function returns a [`ToolConfigSpec`] expressing what an
//! `AppConfig` (and, where relevant, the current prompt) must
//! satisfy for the LLM to see a tool from a given family. The
//! tool system in `agent/tools/descriptor.rs` exposes the
//! `ConfigPredicate` enum and the evaluation logic; the per-family
//! *decisions* live here, in the application domain, because
//! "this integration requires that credential" is a product
//! rule, not a tool-system rule.
//!
//! The CSV family's prompt-content rule lives in
//! [`crate::app::batch::prompt_rules`] (it's a prompt rule, not
//! a config rule — different mechanism, different file).

use crate::agent::tools::descriptor::{ConfigPredicate, ToolConfigSpec};
use crate::agent::tools::registry::groups::{InternalToolGroup, ToolGroupId};

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
