//! CardDAV contact tool implementations for the tool registry.

use crate::agent::tools::Tool;
use crate::agent::tools::context::ToolContext;
use crate::agent::tools::dtos;
use crate::config::AppConfig;
use std::any::TypeId;

use super::json_schema;
use super::strings;

/// Tool that searches contacts by keyword.
pub(crate) struct SearchContactTool;
impl Tool for SearchContactTool {
    fn name(&self) -> &'static str {
        "search_contact"
    }
    fn description(&self) -> &'static str {
        strings::SEARCH_CONTACT_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::SearchContactInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::SearchContactInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        if !config.tool_groups.contacts {
            return false;
        }
        if config
            .feature_flags
            .get("useDAVForContacts")
            .copied()
            .unwrap_or(false)
        {
            !config.caldav_clients.is_empty()
        } else {
            !config.jmap_clients.is_empty()
        }
    }
    fn safety(&self) -> crate::agent::tools::Safety {
        crate::agent::tools::Safety::ReadOnly
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::SearchContactInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::agent::tools::carddav::tool_search_contact(ctx.config, &input.keyword).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that adds a new contact.
pub(crate) struct AddContactTool;
impl Tool for AddContactTool {
    fn name(&self) -> &'static str {
        "add_contact"
    }
    fn description(&self) -> &'static str {
        strings::ADD_CONTACT_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::AddContactInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::AddContactInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        if !config.tool_groups.contacts {
            return false;
        }
        if config
            .feature_flags
            .get("useDAVForContacts")
            .copied()
            .unwrap_or(false)
        {
            !config.caldav_clients.is_empty()
        } else {
            !config.jmap_clients.is_empty()
        }
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::AddContactInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::agent::tools::carddav::tool_add_contact(ctx.config, &input.contact_json).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that gets contact details by ID.
pub(crate) struct GetContactTool;
impl Tool for GetContactTool {
    fn name(&self) -> &'static str {
        "get_contact"
    }
    fn description(&self) -> &'static str {
        strings::GET_CONTACT_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::GetContactInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::GetContactInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        if !config.tool_groups.contacts {
            return false;
        }
        if config
            .feature_flags
            .get("useDAVForContacts")
            .copied()
            .unwrap_or(false)
        {
            !config.caldav_clients.is_empty()
        } else {
            !config.jmap_clients.is_empty()
        }
    }
    fn safety(&self) -> crate::agent::tools::Safety {
        crate::agent::tools::Safety::ReadOnly
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::GetContactInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::agent::tools::carddav::tool_get_contact(ctx.config, &input.id).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that updates an existing contact.
pub(crate) struct UpdateContactTool;
impl Tool for UpdateContactTool {
    fn name(&self) -> &'static str {
        "update_contact"
    }
    fn description(&self) -> &'static str {
        strings::UPDATE_CONTACT_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::UpdateContactInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::UpdateContactInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        if !config.tool_groups.contacts {
            return false;
        }
        if config
            .feature_flags
            .get("useDAVForContacts")
            .copied()
            .unwrap_or(false)
        {
            !config.caldav_clients.is_empty()
        } else {
            !config.jmap_clients.is_empty()
        }
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::UpdateContactInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::agent::tools::carddav::tool_update_contact(
            ctx.config,
            &input.id,
            &input.contact_json,
        )
        .map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that deletes a contact by ID.
///
/// **Currently disabled.** The function, DTO, and registration are
/// kept so the tool can be re-enabled in a future release; until then
/// `is_enabled` returns `false` so the LLM never sees the tool. To
/// re-enable, replace the `false` literal below with the same
/// backend-presence check used by the other contact tools
/// (`useDAVForContacts` flag → `caldav_clients`; otherwise
/// `jmap_clients`).
pub(crate) struct DeleteContactTool;
impl Tool for DeleteContactTool {
    fn name(&self) -> &'static str {
        "delete_contact"
    }
    fn description(&self) -> &'static str {
        strings::DELETE_CONTACT_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::DeleteContactInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::DeleteContactInput>()
    }
    fn is_enabled(&self, _config: &AppConfig, _: &str) -> bool {
        // See struct doc comment — disabled until explicitly re-enabled.
        false
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::DeleteContactInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::agent::tools::carddav::tool_delete_contact(ctx.config, &input.id).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}
