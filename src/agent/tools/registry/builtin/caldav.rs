//! CalDAV calendar tool implementations for the tool registry.

use crate::tools::Tool;
use crate::tools::context::ToolContext;
use crate::tools::dtos;
use crate::tools::provider::{RegisteredTool, ToolProvider};
use crate::tools::registry::groups::{InternalToolGroup, ToolGroupId};
use fastmd_tool_macros::ToolDescriptor;
use std::sync::Arc;

use super::strings;

/// Tool that searches calendar items by keyword.
#[derive(ToolDescriptor)]
#[tool(
    name = "search_calendar",
    desc = strings::SEARCH_CALENDAR_DESCRIPTION,
    input = dtos::SearchCalendarInput,
    safety = crate::tools::Safety::ReadOnly,
    group = Calendar,
    config = crate::tools::specs::calendar_spec(),
    execute_with = execute_search_calendar,
)]
pub(crate) struct SearchCalendarTool;
fn execute_search_calendar(
    _self: &SearchCalendarTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::SearchCalendarInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    crate::lib::dav::cal::tool_search_calendar(&ctx.config, &input.keyword).map(|r| {
        serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
    })
}

/// Tool that gets calendar items by date range.
#[derive(ToolDescriptor)]
#[tool(
    name = "get_calendar",
    desc = strings::GET_CALENDAR_DESCRIPTION,
    input = dtos::GetCalendarInput,
    safety = crate::tools::Safety::ReadOnly,
    group = Calendar,
    config = crate::tools::specs::calendar_spec(),
    execute_with = execute_get_calendar,
)]
pub(crate) struct GetCalendarTool;
fn execute_get_calendar(
    _self: &GetCalendarTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::GetCalendarInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    crate::lib::dav::cal::tool_get_calendar(&ctx.config, &input.start_date, &input.end_date).map(
        |r| serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()})),
    )
}

/// Tool that gets a specific calendar item by its full href.
#[derive(ToolDescriptor)]
#[tool(
    name = "get_calendar_item",
    desc = strings::GET_CALENDAR_ITEM_DESCRIPTION,
    input = dtos::GetCalendarItemInput,
    safety = crate::tools::Safety::ReadOnly,
    group = Calendar,
    config = crate::tools::specs::calendar_spec(),
    execute_with = execute_get_calendar_item,
)]
pub(crate) struct GetCalendarItemTool;
fn execute_get_calendar_item(
    _self: &GetCalendarItemTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::GetCalendarItemInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    crate::lib::dav::cal::tool_get_calendar_item(&ctx.config, &input.href).map(|r| {
        serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
    })
}

/// Tool that adds a new calendar item.
#[derive(ToolDescriptor)]
#[tool(
    name = "add_calendar_item",
    desc = strings::ADD_CALENDAR_ITEM_DESCRIPTION,
    input = dtos::AddCalendarItemInput,
    safety = crate::tools::Safety::Mutating,
    group = Calendar,
    config = crate::tools::specs::calendar_spec(),
    execute_with = execute_add_calendar_item,
)]
pub(crate) struct AddCalendarItemTool;
fn execute_add_calendar_item(
    _self: &AddCalendarItemTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::AddCalendarItemInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    let item_json =
        serde_json::to_string(&input).map_err(|e| format!("Failed to serialize input: {}", e))?;
    crate::lib::dav::cal::tool_add_calendar_item(&ctx.config, &item_json).map(|r| {
        serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
    })
}

/// Tool that updates an existing calendar item.
#[derive(ToolDescriptor)]
#[tool(
    name = "update_calendar_item",
    desc = strings::UPDATE_CALENDAR_ITEM_DESCRIPTION,
    input = dtos::UpdateCalendarItemInput,
    safety = crate::tools::Safety::Mutating,
    group = Calendar,
    config = crate::tools::specs::calendar_spec(),
    execute_with = execute_update_calendar_item,
)]
pub(crate) struct UpdateCalendarItemTool;
fn execute_update_calendar_item(
    _self: &UpdateCalendarItemTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::UpdateCalendarItemInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    let update_json =
        serde_json::to_string(&input).map_err(|e| format!("Failed to serialize input: {}", e))?;
    crate::lib::dav::cal::tool_update_calendar_item(&ctx.config, &input.id, &update_json).map(|r| {
        serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
    })
}

/// Tool that deletes a calendar item.
#[derive(ToolDescriptor)]
#[tool(
    name = "delete_calendar_item",
    desc = strings::DELETE_CALENDAR_ITEM_DESCRIPTION,
    input = dtos::DeleteCalendarItemInput,
    safety = crate::tools::Safety::Mutating,
    group = Calendar,
    config = crate::tools::specs::calendar_spec(),
    execute_with = execute_delete_calendar_item,
)]
pub(crate) struct DeleteCalendarItemTool;
fn execute_delete_calendar_item(
    _self: &DeleteCalendarItemTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::DeleteCalendarItemInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    crate::lib::dav::cal::tool_delete_calendar_item(&ctx.config, &input.id).map(|r| {
        serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
    })
}

/// Self-registering provider for the CalDAV calendar family.
pub(crate) struct CalDavProvider;
impl ToolProvider for CalDavProvider {
    fn id(&self) -> &'static str {
        "caldav"
    }
    fn group(&self) -> ToolGroupId {
        ToolGroupId::Internal(InternalToolGroup::Calendar)
    }
    fn tools(&self) -> Vec<RegisteredTool> {
        vec![
            registered(SearchCalendarTool),
            registered(GetCalendarTool),
            registered(GetCalendarItemTool),
            registered(AddCalendarItemTool),
            registered(UpdateCalendarItemTool),
            registered(DeleteCalendarItemTool),
        ]
    }
}

fn registered<T: Tool + 'static>(tool: T) -> RegisteredTool {
    RegisteredTool {
        descriptor: Arc::new(tool.descriptor().clone()),
        executor: Arc::new(tool),
    }
}
