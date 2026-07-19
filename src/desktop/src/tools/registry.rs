use crate::config::AppConfig;
use std::path::{Component, Path};

macro_rules! define_tools {
    (
        $(
            {
                name: $name:expr,
                description: $desc:expr,
                input: $input_type:ty,
                enabled: $enabled:expr,
                execute: |$config:ident, $root_path:ident, $input:ident, $is_safe:ident| $exec:expr
            }
        ),* $(,)?
    ) => {
        pub fn get_tools_schema(config: &AppConfig) -> serde_json::Value {
            let mut tools = Vec::new();
            $(
                let is_enabled: bool = $enabled(config);
                if is_enabled {
                    tools.push(serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": $name,
                            "description": $desc,
                            "parameters": schemars::schema_for!($input_type)
                        }
                    }));
                }
            )*
            serde_json::Value::Array(tools)
        }

        pub fn execute_tool(config: &AppConfig, root_path: &Path, name: &str, args_str: &str) -> String {
            rustls::crypto::ring::default_provider().install_default().ok();

            let args_compact = args_str.to_string();
            println!("Tool call: {}({})", name, args_compact);
            let start_time = std::time::Instant::now();

            let is_safe_path = |p: &str| -> bool {
                if Path::new(p).components().any(|c| c == Component::ParentDir) { return false; }
                let full_path = root_path.join(p).to_string_lossy().to_lowercase();
                let mut root_str = root_path.to_string_lossy().to_lowercase();
                if !root_str.ends_with('/') && !root_str.ends_with('\\') { root_str.push(std::path::MAIN_SEPARATOR); }
                full_path.starts_with(&root_str) || full_path == root_path.to_string_lossy().to_lowercase()
            };

            let result_raw: Result<serde_json::Value, String> = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                match name {
                    $(
                        $name => {
                            let input_res: Result<$input_type, _> = serde_json::from_str(args_str);
                            match input_res {
                                Ok(parsed_input) => {
                                    let $input = parsed_input;
                                    let $config = config;
                                    let $root_path = root_path;
                                    let $is_safe = &is_safe_path;
                                    let res = $exec;
                                    res.map(|r| serde_json::to_value(r).unwrap())
                                },
                                Err(e) => Err(format!("Invalid args: {}", e)),
                            }
                        }
                    )*
                    _ => Err(format!("Tool {} not found.", name)),
                }
            })) {
                Ok(res) => res,
                Err(e) => {
                    let msg = if let Some(s) = e.downcast_ref::<&str>() { *s } else if let Some(s) = e.downcast_ref::<String>() { s.as_str() } else { "Unknown panic" };
                    Err(format!("Tool {} panicked: {}", name, msg))
                }
            };
            
            let elapsed = start_time.elapsed();
            let response_dto = match result_raw {
                Ok(data) => {
                    println!("Tool '{}' succeeded in {:.2?}.", name, elapsed);
                    crate::tools::dtos::ToolResponse::Success { data }
                },
                Err(err) => {
                    println!("Tool '{}' failed in {:.2?}.", name, elapsed);
                    eprintln!("Tool '{}' error details: {}", name, err);
                    crate::tools::dtos::ToolResponse::Error { message: err }
                }
            };
            
            serde_json::to_string(&response_dto).unwrap_or_else(|_| r#"{"status":"error","message":"Failed to serialize tool response"}"#.to_string())
        }
    };
}

define_tools! {

    {
        name: "web_delegate",
        description: "Delegate complex web research to a sub-agent. This protects your context window. Give clear instructions and it will return summarized information using web search and fetch tools.",
        input: crate::tools::dtos::WebDelegateInput,
        enabled: |_| true,
        execute: |config, _root_path, input, _is_safe| crate::tools::web::tool_web_delegate(config, &input.instruction)
    },
    {
        name: "replace_text",
        description: "Replace exact occurrences of old_string with new_string in a file.",
        input: crate::tools::dtos::ReplaceTextInput,
        enabled: |_| true,
        execute: |_config, root_path, input, is_safe| {
            if !is_safe(&input.path) { return Err("Invalid path".to_string()); }
            crate::tools::filesystem::tool_replace_text(&root_path.join(input.path).to_string_lossy(), &input.old_string, &input.new_string)
        }
    },    {
        name: "grep",
        description: "Search for a query string case-insensitively across all Markdown files in the workspace.",
        input: crate::tools::dtos::GrepInput,
        enabled: |_| true,
        execute: |_config, root_path, input, _is_safe| crate::tools::filesystem::tool_grep(root_path, &input.query)
    },
    {
        name: "read_tags",
        description: "Get all unique tags defined in front-matter headers of all Markdown files in the workspace.",
        input: crate::tools::dtos::ReadTagsInput,
        enabled: |_| true,
        execute: |_config, root_path, _input, _is_safe| crate::tools::filesystem::tool_read_tags(root_path)
    },
    {
        name: "list_files_by_tag",
        description: "List all Markdown files that contain a specific tag in their front-matter.",
        input: crate::tools::dtos::ListFilesByTagInput,
        enabled: |_| true,
        execute: |_config, root_path, input, _is_safe| crate::tools::filesystem::tool_list_files_by_tag(root_path, &input.tag)
    },
    {
        name: "list_files",
        description: "List all Markdown files in a directory (not recursive).",
        input: crate::tools::dtos::ListFilesInput,
        enabled: |_| true,
        execute: |_config, root_path, input, is_safe| {
            if !is_safe(&input.path) { return Err("Invalid path".to_string()); }
            crate::tools::filesystem::tool_list_files(&root_path.join(input.path))
        }
    },
    {
        name: "read_file",
        description: "Read the entire text contents of a file at the specified path.",
        input: crate::tools::dtos::ReadFileInput,
        enabled: |_| true,
        execute: |_config, root_path, input, is_safe| {
            if !is_safe(&input.path) { return Err("Invalid path".to_string()); }
            crate::tools::filesystem::tool_read_file(&root_path.join(input.path).to_string_lossy())
        }
    },
    {
        name: "read_file_lines",
        description: "Read specific lines from a file (1-indexed).",
        input: crate::tools::dtos::ReadFileLinesInput,
        enabled: |_| true,
        execute: |_config, root_path, input, is_safe| {
            if !is_safe(&input.path) { return Err("Invalid path".to_string()); }
            crate::tools::filesystem::tool_read_file_lines(&root_path.join(input.path).to_string_lossy(), input.start_line, input.end_line)
        }
    },
    {
        name: "create_file",
        description: "Create a new file at the specified path with the provided content.",
        input: crate::tools::dtos::CreateFileInput,
        enabled: |_| true,
        execute: |_config, root_path, input, is_safe| {
            if !is_safe(&input.path) { return Err("Invalid path".to_string()); }
            crate::tools::filesystem::tool_create_file(&root_path.join(input.path).to_string_lossy(), &input.content)
        }
    },
    {
        name: "insert_lines",
        description: "Insert lines into a file at a specific 1-indexed line index.",
        input: crate::tools::dtos::InsertLinesInput,
        enabled: |_| true,
        execute: |_config, root_path, input, is_safe| {
            if !is_safe(&input.path) { return Err("Invalid path".to_string()); }
            crate::tools::filesystem::tool_insert_lines(&root_path.join(input.path).to_string_lossy(), input.line_index, &input.lines)
        }
    },
    {
        name: "delete_lines",
        description: "Delete specific lines from a file (1-indexed, inclusive).",
        input: crate::tools::dtos::DeleteLinesInput,
        enabled: |_| true,
        execute: |_config, root_path, input, is_safe| {
            if !is_safe(&input.path) { return Err("Invalid path".to_string()); }
            crate::tools::filesystem::tool_delete_lines(&root_path.join(input.path).to_string_lossy(), input.start_line, input.end_line)
        }
    },
    {
        name: "web_fetch",
        description: "Fetch content from a URL.",
        input: crate::tools::dtos::WebFetchInput,
        enabled: |_| true,
        execute: |_config, _root_path, input, _is_safe| crate::tools::web::tool_web_fetch(&input.url)
    },
    {
        name: "read_yaml_header",
        description: "Parse a YAML header from a markdown file and return its content representation. Tip: Use this to read a document's summary before reading the full file if you are not sure the full contents are needed, to protect context.",
        input: crate::tools::dtos::ReadYamlHeaderInput,
        enabled: |_| true,
        execute: |_config, root_path, input, is_safe| {
            if !is_safe(&input.path) { return Err("Invalid path".to_string()); }
            crate::tools::yaml_header::tool_read_yaml_header(&root_path.join(input.path).to_string_lossy())
        }
    },
    {
        name: "write_yaml_header",
        description: "Write or update data in a YAML header to a markdown file.",
        input: crate::tools::dtos::WriteYamlHeaderInput,
        enabled: |_| true,
        execute: |_config, root_path, input, is_safe| {
            if !is_safe(&input.path) { return Err("Invalid path".to_string()); }
            crate::tools::yaml_header::tool_write_yaml_header(&root_path.join(input.path).to_string_lossy(), input.title.as_deref(), input.summary.as_deref(), input.tags, input.header_date.as_deref())
        }
    },
    {
        name: "web_search",
        description: "Search the web using SearXNG.",
        input: crate::tools::dtos::WebSearchInput,
        enabled: |config: &AppConfig| config.searxng_url.is_some(),
        execute: |config, _root_path, input, _is_safe| {
            if let Some(url) = &config.searxng_url {
                crate::tools::web::tool_web_search(url, &input.query)
            } else {
                Err("web_search tool is disabled (no SearXNG URL configured).".to_string())
            }
        }
    },
    {
        name: "search_calendar",
        description: "Search the calendar by keyword.",
        input: crate::tools::dtos::SearchCalendarInput,
        enabled: |config: &AppConfig| !config.caldav_clients.is_empty(),
        execute: |config, _root_path, input, _is_safe| crate::tools::caldav::tool_search_calendar(config, &input.keyword)
    },
    {
        name: "get_calendar",
        description: "Get calendar items by date range.",
        input: crate::tools::dtos::GetCalendarInput,
        enabled: |config: &AppConfig| !config.caldav_clients.is_empty(),
        execute: |config, _root_path, input, _is_safe| crate::tools::caldav::tool_get_calendar(config, &input.start_date, &input.end_date)
    },
    {
        name: "get_calendar_item",
        description: "Get a specific calendar item by its full href. IMPORTANT: Use the exact, full 'href' value returned by search or get tools (e.g., '/dav/calendars/user/.../item.ics'). Do not use just the UUID.",
        input: crate::tools::dtos::GetCalendarItemInput,
        enabled: |config: &AppConfig| !config.caldav_clients.is_empty(),
        execute: |config, _root_path, input, _is_safe| crate::tools::caldav::tool_get_calendar_item(config, &input.href)
    },
    {
        name: "add_calendar_item",
        description: "Add a new calendar item.",
        input: crate::tools::dtos::AddCalendarItemInput,
        enabled: |config: &AppConfig| !config.caldav_clients.is_empty(),
        execute: |config, _root_path, input, _is_safe| crate::tools::caldav::tool_add_calendar_item(config, &input.item_json)
    },
    {
        name: "update_calendar_item",
        description: "Update a calendar item.",
        input: crate::tools::dtos::UpdateCalendarItemInput,
        enabled: |config: &AppConfig| !config.caldav_clients.is_empty(),
        execute: |config, _root_path, input, _is_safe| crate::tools::caldav::tool_update_calendar_item(config, &input.id, &input.update_json)
    },
    {
        name: "delete_calendar_item",
        description: "Delete a calendar item.",
        input: crate::tools::dtos::DeleteCalendarItemInput,
        enabled: |config: &AppConfig| !config.caldav_clients.is_empty(),
        execute: |config, _root_path, input, _is_safe| crate::tools::caldav::tool_delete_calendar_item(config, &input.id)
    },
    {
        name: "search_email",
        description: "Search email by keyword.",
        input: crate::tools::dtos::SearchEmailInput,
        enabled: |config: &AppConfig| !config.jmap_clients.is_empty(),
        execute: |config, _root_path, input, _is_safe| crate::tools::jmap::tool_search_email(config, &input.keyword)
    },
    {
        name: "get_email_by_id",
        description: "Get email by id.",
        input: crate::tools::dtos::GetEmailByIdInput,
        enabled: |config: &AppConfig| !config.jmap_clients.is_empty(),
        execute: |config, _root_path, input, _is_safe| crate::tools::jmap::tool_get_email_by_id(config, &input.id)
    },
    {
        name: "get_email",
        description: "Get email by date range, sender, recipient, unread status, or flagged status.",
        input: crate::tools::dtos::GetEmailInput,
        enabled: |config: &AppConfig| !config.jmap_clients.is_empty(),
        execute: |config, _root_path, input, _is_safe| crate::tools::jmap::tool_get_email(config, input.start_date.as_deref(), input.end_date.as_deref(), input.sender.as_deref(), input.recipient.as_deref(), input.is_unread, input.is_flagged)
    },
    {
        name: "send_email",
        description: "Send an email.",
        input: crate::tools::dtos::SendEmailInput,
        enabled: |config: &AppConfig| !config.jmap_clients.is_empty(),
        execute: |config, _root_path, input, _is_safe| crate::tools::jmap::tool_send_email(config, &input.to, &input.subject, &input.body)
    },
    {
        name: "search_contact",
        description: "Search contacts by keyword.",
        input: crate::tools::dtos::SearchContactInput,
        enabled: |config: &AppConfig| !config.jmap_clients.is_empty(),
        execute: |config, _root_path, input, _is_safe| crate::tools::jmap::tool_search_contact(config, &input.keyword)
    },
    {
        name: "add_contact",
        description: "Add a new contact.",
        input: crate::tools::dtos::AddContactInput,
        enabled: |config: &AppConfig| !config.jmap_clients.is_empty(),
        execute: |config, _root_path, input, _is_safe| crate::tools::jmap::tool_add_contact(config, &input.contact_json)
    },
    {
        name: "get_contact",
        description: "Get contact by id.",
        input: crate::tools::dtos::GetContactInput,
        enabled: |config: &AppConfig| !config.jmap_clients.is_empty(),
        execute: |config, _root_path, input, _is_safe| crate::tools::jmap::tool_get_contact(config, &input.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_is_safe_path() {
        let config = AppConfig::default();
        let root = Path::new("C:\\TestRoot");
        
        let res1 = execute_tool(&config, root, "read_file", r#"{"path": "C:\\TestRoot\\sub\\file.md"}"#);
        assert!(!res1.contains("Invalid path"));

        let res2 = execute_tool(&config, root, "read_file", r#"{"path": "c:\\testroot\\sub\\file.md"}"#);
        assert!(!res2.contains("Invalid path"));

        let res3 = execute_tool(&config, root, "read_file", r#"{"path": "C:\\TestRoot\\..\\Windows\\System32\\cmd.exe"}"#);
        assert!(res3.contains("Invalid path"));
        
        let res4 = execute_tool(&config, root, "read_file", r#"{"path": "C:\\Windows\\System32\\cmd.exe"}"#);
        assert!(res4.contains("Invalid path"));
    }
}
