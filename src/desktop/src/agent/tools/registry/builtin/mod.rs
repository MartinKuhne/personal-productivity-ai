//! Built-in tool implementations and registration logic.

pub(crate) mod caldav;
pub(crate) mod carddav;
pub(crate) mod csv;
pub(crate) mod fs;
pub(crate) mod jmap;
pub(crate) mod weather;
pub(crate) mod web;
pub(crate) mod yaml;

use super::ToolRegistry;

/// Generate the JSON Schema for a tool's input DTO.
pub(crate) fn json_schema<T: schemars::JsonSchema>() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(T)).unwrap()
}

/// Register all built-in tools into the given registry instance.
pub(crate) fn register_all_builtins(registry: &mut ToolRegistry) {
    registry.register(Box::new(web::WebDelegateTool));
    registry.register(Box::new(fs::ReplaceTextTool));
    registry.register(Box::new(fs::GrepTool));
    registry.register(Box::new(fs::ReadTagsTool));
    registry.register(Box::new(fs::ListFilesByTagTool));
    registry.register(Box::new(fs::ListFilesTool));
    registry.register(Box::new(fs::ReadFileTool));
    registry.register(Box::new(fs::ReadFileLinesTool));
    registry.register(Box::new(fs::CreateFileTool));
    registry.register(Box::new(fs::InsertLinesTool));
    registry.register(Box::new(fs::DeleteLinesTool));
    registry.register(Box::new(web::WebFetchTool));
    registry.register(Box::new(yaml::ReadYamlHeaderTool));
    registry.register(Box::new(yaml::WriteYamlHeaderTool));
    registry.register(Box::new(web::WebSearchTool));
    registry.register(Box::new(caldav::SearchCalendarTool));
    registry.register(Box::new(caldav::GetCalendarTool));
    registry.register(Box::new(caldav::GetCalendarItemTool));
    registry.register(Box::new(caldav::AddCalendarItemTool));
    registry.register(Box::new(caldav::UpdateCalendarItemTool));
    registry.register(Box::new(caldav::DeleteCalendarItemTool));
    registry.register(Box::new(jmap::SearchEmailTool));
    registry.register(Box::new(jmap::GetEmailByIdTool));
    registry.register(Box::new(jmap::SendEmailTool));
    registry.register(Box::new(carddav::SearchContactTool));
    registry.register(Box::new(carddav::AddContactTool));
    registry.register(Box::new(carddav::GetContactTool));
    registry.register(Box::new(csv::CsvCreateTool));
    registry.register(Box::new(csv::CsvListTool));
    registry.register(Box::new(csv::CsvAddRowsTool));
    registry.register(Box::new(csv::CsvDeleteRowsTool));
    registry.register(Box::new(csv::CsvQueryTool));
    registry.register(Box::new(weather::GetWeatherTool));
}
