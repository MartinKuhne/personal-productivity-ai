//! CardDAV contact tool implementations for the tool registry.

use crate::config::AppConfig;
use crate::agent::tools::Tool;
use crate::agent::tools::context::ToolContext;
use crate::agent::tools::dtos;
use std::any::TypeId;

use super::json_schema;

/// Tool that searches contacts by keyword.
pub(crate) struct SearchContactTool;
impl Tool for SearchContactTool {
    fn name(&self) -> &'static str {
        "search_contact"
    }
    fn description(&self) -> &'static str {
        "Search contacts by keyword."
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
        "Add a new contact."
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
        "Get contact by id."
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
