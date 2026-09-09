//! CardDAV contact tool implementations for the tool registry.
//!
//! Unit tests live in the sibling `carddav_tests.rs` sidecar.

use crate::tools::Tool;
use crate::tools::context::ToolContext;
use crate::tools::descriptor::{ConfigPredicate, ToolConfigSpec};
use crate::tools::dtos;
use crate::tools::provider::{RegisteredTool, ToolProvider};
use crate::tools::registry::groups::{InternalToolGroup, ToolGroupId};
use fastmd_tool_macros::ToolDescriptor;
use std::sync::Arc;

use super::strings;

/// Tool that searches contacts by keyword.
#[derive(ToolDescriptor)]
#[tool(
    name = "search_contact",
    desc = strings::SEARCH_CONTACT_DESCRIPTION,
    input = dtos::SearchContactInput,
    safety = crate::tools::Safety::ReadOnly,
    group = Contacts,
    config = crate::tools::specs::contacts_spec(),
    execute_with = execute_search_contact,
)]
pub(crate) struct SearchContactTool;
fn execute_search_contact(
    _self: &SearchContactTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::SearchContactInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    crate::lib::dav::card::tool_search_contact(
        &ctx.config,
        &input.keyword,
        input.cursor,
        &ctx.cache(),
        ctx.uuid_gen().as_ref(),
    )
    .map(|r| {
        serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
    })
}

/// Tool that adds a new contact.
#[derive(ToolDescriptor)]
#[tool(
    name = "add_contact",
    desc = strings::ADD_CONTACT_DESCRIPTION,
    input = dtos::AddContactInput,
    safety = crate::tools::Safety::Mutating,
    group = Contacts,
    config = crate::tools::specs::contacts_spec(),
    execute_with = execute_add_contact,
)]
pub(crate) struct AddContactTool;
fn execute_add_contact(
    _self: &AddContactTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::AddContactInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    let contact_json =
        serde_json::to_string(&input).map_err(|e| format!("Failed to serialize input: {}", e))?;
    crate::lib::dav::card::tool_add_contact(&ctx.config, input.client.as_deref(), &contact_json)
        .map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
}

/// Tool that gets contact details by ID.
#[derive(ToolDescriptor)]
#[tool(
    name = "get_contact",
    desc = strings::GET_CONTACT_DESCRIPTION,
    input = dtos::GetContactInput,
    safety = crate::tools::Safety::ReadOnly,
    group = Contacts,
    config = crate::tools::specs::contacts_spec(),
    execute_with = execute_get_contact,
)]
pub(crate) struct GetContactTool;
fn execute_get_contact(
    _self: &GetContactTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::GetContactInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    crate::lib::dav::card::tool_get_contact(&ctx.config, &input.href).map(|r| {
        serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
    })
}

/// Tool that updates an existing contact.
#[derive(ToolDescriptor)]
#[tool(
    name = "update_contact",
    desc = strings::UPDATE_CONTACT_DESCRIPTION,
    input = dtos::UpdateContactInput,
    safety = crate::tools::Safety::Mutating,
    group = Contacts,
    config = crate::tools::specs::contacts_spec(),
    execute_with = execute_update_contact,
)]
pub(crate) struct UpdateContactTool;
fn execute_update_contact(
    _self: &UpdateContactTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::UpdateContactInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    let contact_json =
        serde_json::to_string(&input).map_err(|e| format!("Failed to serialize input: {}", e))?;
    crate::lib::dav::card::tool_update_contact(
        &ctx.config,
        &input.href,
        input.client.as_deref(),
        &contact_json,
    )
    .map(|r| {
        serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
    })
}

/// `ToolConfigSpec` for the disabled [`DeleteContactTool`]. Kept
/// here (not in `agent::tools::specs`) because no other tool uses the
/// `ConfigPredicate::Never` gate — re-enabling the tool means
/// swapping this spec out for [`crate::tools::specs::contacts_spec`].
fn delete_contact_disabled_spec(group: ToolGroupId) -> ToolConfigSpec {
    ToolConfigSpec {
        group: Some(group),
        requires: vec![ConfigPredicate::Never],
        prompt_rule: None,
    }
}

/// Tool that deletes a contact by ID.
///
/// **Currently disabled.** The function, DTO, and registration are
/// kept so the tool can be re-enabled in a future release; until then
/// `is_enabled` returns `false` so the LLM never sees the tool. To
/// re-enable, replace the
/// [`delete_contact_disabled_spec`] below with
/// [`crate::tools::specs::contacts_spec`].
#[derive(ToolDescriptor)]
#[tool(
    name = "delete_contact",
    desc = strings::DELETE_CONTACT_DESCRIPTION,
    input = dtos::DeleteContactInput,
    safety = crate::tools::Safety::Mutating,
    group = Contacts,
    config = delete_contact_disabled_spec(ToolGroupId::Internal(InternalToolGroup::Contacts)),
    execute_with = execute_delete_contact,
)]
pub(crate) struct DeleteContactTool;
fn execute_delete_contact(
    _self: &DeleteContactTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::DeleteContactInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    crate::lib::dav::card::tool_delete_contact(&ctx.config, &input.href, input.client.as_deref())
        .map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
}

/// Self-registering provider for the CardDAV contacts family.
pub(crate) struct CardDavProvider;
impl ToolProvider for CardDavProvider {
    fn id(&self) -> &'static str {
        "carddav"
    }
    fn group(&self) -> ToolGroupId {
        ToolGroupId::Internal(InternalToolGroup::Contacts)
    }
    fn tools(&self) -> Vec<RegisteredTool> {
        vec![
            registered(SearchContactTool),
            registered(AddContactTool),
            registered(GetContactTool),
            registered(UpdateContactTool),
            registered(DeleteContactTool),
        ]
    }
}

fn registered<T: Tool + 'static>(tool: T) -> RegisteredTool {
    RegisteredTool {
        descriptor: Arc::new(tool.descriptor().clone()),
        executor: Arc::new(tool),
    }
}

#[cfg(test)]
#[path = "carddav_tests.rs"]
mod tests;
