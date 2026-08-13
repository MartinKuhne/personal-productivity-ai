//! CalDAV calendar tool implementations for the tool registry.

use crate::agent::tools::Tool;
use crate::agent::tools::context::ToolContext;
use crate::agent::tools::descriptor::ToolDescriptor;
use crate::agent::tools::dtos;
use crate::agent::tools::provider::{RegisteredTool, ToolProvider};
use crate::agent::tools::registry::groups::{InternalToolGroup, ToolGroupId};
use std::sync::{Arc, OnceLock};

use super::strings;

fn build_calendar_descriptor<I>(
    name: &'static str,
    description: &'static str,
    safety: crate::agent::tools::Safety,
) -> ToolDescriptor
where
    I: schemars::JsonSchema + 'static,
{
    let group = ToolGroupId::Internal(InternalToolGroup::Calendar);
    ToolDescriptor::new::<I>(
        name,
        description,
        safety,
        crate::app::tool_specs::calendar_spec(),
        group,
    )
}

/// Tool that searches calendar items by keyword.
pub(crate) struct SearchCalendarTool;
impl Tool for SearchCalendarTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static D: OnceLock<ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            build_calendar_descriptor::<dtos::SearchCalendarInput>(
                "search_calendar",
                strings::SEARCH_CALENDAR_DESCRIPTION,
                crate::agent::tools::Safety::ReadOnly,
            )
        })
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::SearchCalendarInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::integrations::dav::cal::tool_search_calendar(&ctx.config, &input.keyword).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that gets calendar items by date range.
pub(crate) struct GetCalendarTool;
impl Tool for GetCalendarTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static D: OnceLock<ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            build_calendar_descriptor::<dtos::GetCalendarInput>(
                "get_calendar",
                strings::GET_CALENDAR_DESCRIPTION,
                crate::agent::tools::Safety::ReadOnly,
            )
        })
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::GetCalendarInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::integrations::dav::cal::tool_get_calendar(
            &ctx.config,
            &input.start_date,
            &input.end_date,
        )
        .map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that gets a specific calendar item by its full href.
pub(crate) struct GetCalendarItemTool;
impl Tool for GetCalendarItemTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static D: OnceLock<ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            build_calendar_descriptor::<dtos::GetCalendarItemInput>(
                "get_calendar_item",
                strings::GET_CALENDAR_ITEM_DESCRIPTION,
                crate::agent::tools::Safety::ReadOnly,
            )
        })
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::GetCalendarItemInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::integrations::dav::cal::tool_get_calendar_item(&ctx.config, &input.href).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that adds a new calendar item.
pub(crate) struct AddCalendarItemTool;
impl Tool for AddCalendarItemTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static D: OnceLock<ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            build_calendar_descriptor::<dtos::AddCalendarItemInput>(
                "add_calendar_item",
                strings::ADD_CALENDAR_ITEM_DESCRIPTION,
                crate::agent::tools::Safety::Mutating,
            )
        })
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::AddCalendarItemInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let item_json = serde_json::to_string(&input)
            .map_err(|e| format!("Failed to serialize input: {}", e))?;
        crate::integrations::dav::cal::tool_add_calendar_item(&ctx.config, &item_json).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that updates an existing calendar item.
pub(crate) struct UpdateCalendarItemTool;
impl Tool for UpdateCalendarItemTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static D: OnceLock<ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            build_calendar_descriptor::<dtos::UpdateCalendarItemInput>(
                "update_calendar_item",
                strings::UPDATE_CALENDAR_ITEM_DESCRIPTION,
                crate::agent::tools::Safety::Mutating,
            )
        })
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::UpdateCalendarItemInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let update_json = serde_json::to_string(&input)
            .map_err(|e| format!("Failed to serialize input: {}", e))?;
        crate::integrations::dav::cal::tool_update_calendar_item(
            &ctx.config,
            &input.id,
            &update_json,
        )
        .map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that deletes a calendar item.
pub(crate) struct DeleteCalendarItemTool;
impl Tool for DeleteCalendarItemTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static D: OnceLock<ToolDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            build_calendar_descriptor::<dtos::DeleteCalendarItemInput>(
                "delete_calendar_item",
                strings::DELETE_CALENDAR_ITEM_DESCRIPTION,
                crate::agent::tools::Safety::Mutating,
            )
        })
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::DeleteCalendarItemInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::integrations::dav::cal::tool_delete_calendar_item(&ctx.config, &input.id).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
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
