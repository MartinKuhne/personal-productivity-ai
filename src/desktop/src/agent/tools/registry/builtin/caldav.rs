//! CalDAV calendar tool implementations for the tool registry.

use crate::config::AppConfig;
use crate::tools::Tool;
use crate::tools::context::ToolContext;
use crate::tools::dtos;
use std::any::TypeId;

use super::json_schema;

/// Tool that searches calendar items by keyword.
pub(crate) struct SearchCalendarTool;
impl Tool for SearchCalendarTool {
    fn name(&self) -> &'static str {
        "search_calendar"
    }
    fn description(&self) -> &'static str {
        "Search the calendar by keyword."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::SearchCalendarInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::SearchCalendarInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.calendar && !config.caldav_clients.is_empty()
    }
    fn safety(&self) -> crate::tools::Safety {
        crate::tools::Safety::ReadOnly
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::SearchCalendarInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::tools::caldav::tool_search_calendar(ctx.config, &input.keyword).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that gets calendar items by date range.
pub(crate) struct GetCalendarTool;
impl Tool for GetCalendarTool {
    fn name(&self) -> &'static str {
        "get_calendar"
    }
    fn description(&self) -> &'static str {
        "Get calendar items by date range."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::GetCalendarInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::GetCalendarInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.calendar && !config.caldav_clients.is_empty()
    }
    fn safety(&self) -> crate::tools::Safety {
        crate::tools::Safety::ReadOnly
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::GetCalendarInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::tools::caldav::tool_get_calendar(ctx.config, &input.start_date, &input.end_date).map(
            |r| {
                serde_json::to_value(r)
                    .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
            },
        )
    }
}

/// Tool that gets a specific calendar item by its full href.
pub(crate) struct GetCalendarItemTool;
impl Tool for GetCalendarItemTool {
    fn name(&self) -> &'static str {
        "get_calendar_item"
    }
    fn description(&self) -> &'static str {
        "Get a specific calendar item by its full href. IMPORTANT: Use the exact, full 'href' value returned by search or get tools (e.g., '/dav/calendars/user/.../item.ics'). Do not use just the UUID."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::GetCalendarItemInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::GetCalendarItemInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.calendar && !config.caldav_clients.is_empty()
    }
    fn safety(&self) -> crate::tools::Safety {
        crate::tools::Safety::ReadOnly
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::GetCalendarItemInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::tools::caldav::tool_get_calendar_item(ctx.config, &input.href).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that adds a new calendar item.
pub(crate) struct AddCalendarItemTool;
impl Tool for AddCalendarItemTool {
    fn name(&self) -> &'static str {
        "add_calendar_item"
    }
    fn description(&self) -> &'static str {
        "Add a new calendar item."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::AddCalendarItemInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::AddCalendarItemInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.calendar && !config.caldav_clients.is_empty()
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::AddCalendarItemInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::tools::caldav::tool_add_calendar_item(ctx.config, &input.item_json).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that updates an existing calendar item.
pub(crate) struct UpdateCalendarItemTool;
impl Tool for UpdateCalendarItemTool {
    fn name(&self) -> &'static str {
        "update_calendar_item"
    }
    fn description(&self) -> &'static str {
        "Update a calendar item."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::UpdateCalendarItemInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::UpdateCalendarItemInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.calendar && !config.caldav_clients.is_empty()
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::UpdateCalendarItemInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::tools::caldav::tool_update_calendar_item(ctx.config, &input.id, &input.update_json)
            .map(|r| {
                serde_json::to_value(r)
                    .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
            })
    }
}

/// Tool that deletes a calendar item.
pub(crate) struct DeleteCalendarItemTool;
impl Tool for DeleteCalendarItemTool {
    fn name(&self) -> &'static str {
        "delete_calendar_item"
    }
    fn description(&self) -> &'static str {
        "Delete a calendar item."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::DeleteCalendarItemInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::DeleteCalendarItemInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.calendar && !config.caldav_clients.is_empty()
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::DeleteCalendarItemInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::tools::caldav::tool_delete_calendar_item(ctx.config, &input.id).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}
