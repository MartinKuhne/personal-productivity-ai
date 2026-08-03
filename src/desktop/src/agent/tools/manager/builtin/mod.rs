//! Built-in tool implementations and registration logic.

pub(crate) mod browser;
pub(crate) mod caldav;
pub(crate) mod carddav;
pub(crate) mod csv;
pub(crate) mod fs;
pub(crate) mod jmap;
pub(crate) mod strings;
pub(crate) mod trello;
pub(crate) mod weather;
pub(crate) mod web;
pub(crate) mod yaml;

use super::ToolManager;
use super::groups::InternalToolGroup;

/// Generate the JSON Schema for a tool's input DTO.
pub(crate) fn json_schema<T: schemars::JsonSchema>() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(T)).unwrap()
}

/// Register every built-in tool into the given manager, tagged with
/// the group it belongs to.
pub(crate) fn register_all_builtins(mgr: &mut ToolManager) {
    // Web
    mgr.register_builtin(InternalToolGroup::Web, Box::new(web::WebDelegateTool));
    mgr.register_builtin(InternalToolGroup::Web, Box::new(web::WebFetchTool));
    mgr.register_builtin(InternalToolGroup::Web, Box::new(web::WebSearchTool));

    // Browser automation (BRWS-001..008)
    mgr.register_builtin(
        InternalToolGroup::Browser,
        Box::new(browser::BrowserNavigateTool),
    );
    mgr.register_builtin(
        InternalToolGroup::Browser,
        Box::new(browser::BrowserGetPageStateTool),
    );
    mgr.register_builtin(
        InternalToolGroup::Browser,
        Box::new(browser::BrowserClickTool),
    );
    mgr.register_builtin(
        InternalToolGroup::Browser,
        Box::new(browser::BrowserFillInputTool),
    );
    mgr.register_builtin(
        InternalToolGroup::Browser,
        Box::new(browser::BrowserSelectDropdownTool),
    );
    mgr.register_builtin(
        InternalToolGroup::Browser,
        Box::new(browser::BrowserPressKeyTool),
    );
    mgr.register_builtin(
        InternalToolGroup::Browser,
        Box::new(browser::BrowserEvaluateJsTool),
    );
    mgr.register_builtin(
        InternalToolGroup::Browser,
        Box::new(browser::BrowserScreenshotTool),
    );

    // Filesystem
    mgr.register_builtin(InternalToolGroup::Filesystem, Box::new(fs::ReplaceTextTool));
    mgr.register_builtin(InternalToolGroup::Filesystem, Box::new(fs::GrepTool));
    mgr.register_builtin(InternalToolGroup::Filesystem, Box::new(fs::ReadTagsTool));
    mgr.register_builtin(
        InternalToolGroup::Filesystem,
        Box::new(fs::ListFilesByTagTool),
    );
    mgr.register_builtin(InternalToolGroup::Filesystem, Box::new(fs::ListFilesTool));
    mgr.register_builtin(InternalToolGroup::Filesystem, Box::new(fs::ReadFileTool));
    mgr.register_builtin(
        InternalToolGroup::Filesystem,
        Box::new(fs::ReadFileLinesTool),
    );
    mgr.register_builtin(InternalToolGroup::Filesystem, Box::new(fs::CreateFileTool));
    mgr.register_builtin(InternalToolGroup::Filesystem, Box::new(fs::InsertLinesTool));
    mgr.register_builtin(InternalToolGroup::Filesystem, Box::new(fs::DeleteLinesTool));
    mgr.register_builtin(
        InternalToolGroup::Filesystem,
        Box::new(yaml::ReadYamlHeaderTool),
    );
    mgr.register_builtin(
        InternalToolGroup::Filesystem,
        Box::new(yaml::WriteYamlHeaderTool),
    );

    // Calendar (CalDAV)
    mgr.register_builtin(
        InternalToolGroup::Calendar,
        Box::new(caldav::SearchCalendarTool),
    );
    mgr.register_builtin(
        InternalToolGroup::Calendar,
        Box::new(caldav::GetCalendarTool),
    );
    mgr.register_builtin(
        InternalToolGroup::Calendar,
        Box::new(caldav::GetCalendarItemTool),
    );
    mgr.register_builtin(
        InternalToolGroup::Calendar,
        Box::new(caldav::AddCalendarItemTool),
    );
    mgr.register_builtin(
        InternalToolGroup::Calendar,
        Box::new(caldav::UpdateCalendarItemTool),
    );
    mgr.register_builtin(
        InternalToolGroup::Calendar,
        Box::new(caldav::DeleteCalendarItemTool),
    );

    // Email (JMAP)
    mgr.register_builtin(InternalToolGroup::Email, Box::new(jmap::SearchEmailTool));
    mgr.register_builtin(InternalToolGroup::Email, Box::new(jmap::GetEmailByIdTool));
    mgr.register_builtin(InternalToolGroup::Email, Box::new(jmap::SendEmailTool));

    // Contacts (CardDAV)
    mgr.register_builtin(
        InternalToolGroup::Contacts,
        Box::new(carddav::SearchContactTool),
    );
    mgr.register_builtin(
        InternalToolGroup::Contacts,
        Box::new(carddav::AddContactTool),
    );
    mgr.register_builtin(
        InternalToolGroup::Contacts,
        Box::new(carddav::GetContactTool),
    );

    // CSV database
    mgr.register_builtin(InternalToolGroup::CsvDb, Box::new(csv::CsvCreateTool));
    mgr.register_builtin(InternalToolGroup::CsvDb, Box::new(csv::CsvListTool));
    mgr.register_builtin(InternalToolGroup::CsvDb, Box::new(csv::CsvAddRowsTool));
    mgr.register_builtin(InternalToolGroup::CsvDb, Box::new(csv::CsvDeleteRowsTool));
    mgr.register_builtin(InternalToolGroup::CsvDb, Box::new(csv::CsvQueryTool));

    // Weather
    mgr.register_builtin(
        InternalToolGroup::Weather,
        Box::new(weather::GetWeatherTool),
    );

    // Trello
    mgr.register_builtin(
        InternalToolGroup::Trello,
        Box::new(trello::TrelloGetBoardsTool),
    );
    mgr.register_builtin(
        InternalToolGroup::Trello,
        Box::new(trello::TrelloGetBoardTool),
    );
    mgr.register_builtin(
        InternalToolGroup::Trello,
        Box::new(trello::TrelloGetListsTool),
    );
    mgr.register_builtin(
        InternalToolGroup::Trello,
        Box::new(trello::TrelloGetCardsTool),
    );
    mgr.register_builtin(
        InternalToolGroup::Trello,
        Box::new(trello::TrelloCreateCardTool),
    );
    mgr.register_builtin(
        InternalToolGroup::Trello,
        Box::new(trello::TrelloUpdateCardTool),
    );
    mgr.register_builtin(
        InternalToolGroup::Trello,
        Box::new(trello::TrelloDeleteCardTool),
    );
}
