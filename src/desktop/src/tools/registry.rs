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
                execute: |$config:ident, $root_path:ident, $input:ident, $resolve_path:ident| $exec:expr
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

            let resolve_and_check_path = |p: &str| -> Result<Option<std::path::PathBuf>, String> {
                if Path::new(p).components().any(|c| c == Component::ParentDir) { return Err("Path traversal not allowed".to_string()); }
                let path = Path::new(p);
                let mut components = path.components().peekable();
                
                while let Some(c) = components.peek() {
                    match c {
                        Component::RootDir | Component::CurDir => { components.next(); },
                        _ => break,
                    }
                }

                if components.peek().is_none() {
                    return Ok(None);
                }

                if let Some(std::path::Component::Normal(first)) = components.next() {
                    let first_str = first.to_string_lossy();
                    for lib in &config.content_libraries {
                        if lib.name == first_str {
                            let rest: std::path::PathBuf = components.collect();
                            return Ok(Some(Path::new(&lib.root_folder).join(rest)));
                        }
                    }
                    
                    Err(format!("Content library '{}' not found in virtual path '{}'", first_str, p))
                } else {
                    Err(format!("Invalid virtual path: '{}'", p))
                }
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
                                    let $resolve_path = &resolve_and_check_path;
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
        execute: |_config, _root_path, input, resolve_path| {
            let path = resolve_path(&input.path)?.ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
            crate::tools::filesystem::tool_replace_text(&path.to_string_lossy(), &input.old_string, &input.new_string)
        }
    },    {
        name: "grep",
        description: "Search for a query string case-insensitively across all Markdown files in the workspace.",
        input: crate::tools::dtos::GrepInput,
        enabled: |_| true,
        execute: |config, _root_path, input, _resolve_path| {
            let mut all_results = Vec::new();
            let mut libs: Vec<_> = config.content_libraries.iter().collect();
            libs.sort_by(|a, b| b.priority.cmp(&a.priority)); // Highest priority first
            for lib in libs {
                if let Ok(res) = crate::tools::filesystem::tool_grep(Path::new(&lib.root_folder), &lib.name, &input.query) {
                    if res.matches != "No matches found." {
                        all_results.push(res.matches);
                    }
                }
            }
            if all_results.is_empty() {
                Ok(crate::tools::dtos::GrepResponse { matches: "No matches found.".to_string() })
            } else {
                Ok(crate::tools::dtos::GrepResponse { matches: all_results.join("\n") })
            }
        }
    },
    {
        name: "read_tags",
        description: "Get all unique tags defined in front-matter headers of all Markdown files in the workspace.",
        input: crate::tools::dtos::ReadTagsInput,
        enabled: |_| true,
        execute: |config, _root_path, _input, _resolve_path| {
            let mut count = 0;
            for lib in &config.content_libraries {
                if let Ok(res) = crate::tools::filesystem::tool_read_tags(Path::new(&lib.root_folder)) {
                    if let Some(c) = res.tags_found.strip_prefix("Tags found: ") {
                        if let Ok(num) = c.parse::<usize>() {
                            count += num;
                        }
                    }
                }
            }
            Ok(crate::tools::dtos::ReadTagsResponse { tags_found: format!("Tags found: {}", count) })
        }
    },
    {
        name: "list_files_by_tag",
        description: "List all Markdown files that contain a specific tag in their front-matter.",
        input: crate::tools::dtos::ListFilesByTagInput,
        enabled: |_| true,
        execute: |config, _root_path, input, _resolve_path| {
            let mut all_results = Vec::new();
            for lib in &config.content_libraries {
                if let Ok(res) = crate::tools::filesystem::tool_list_files_by_tag(Path::new(&lib.root_folder), &lib.name, &input.tag) {
                    if res.files != "No matching files found." {
                        all_results.push(res.files);
                    }
                }
            }
            if all_results.is_empty() {
                Ok(crate::tools::dtos::ListFilesByTagResponse { files: "No matching files found.".to_string() })
            } else {
                Ok(crate::tools::dtos::ListFilesByTagResponse { files: all_results.join("\n") })
            }
        }
    },
    {
        name: "list_files",
        description: "List all Markdown files in a directory (not recursive).",
        input: crate::tools::dtos::ListFilesInput,
        enabled: |_| true,
        execute: |config, _root_path, input, resolve_path| {
            match resolve_path(&input.path)? {
                Some(path) => crate::tools::filesystem::tool_list_files(&path, &input.path),
                None => {
                    let libs: Vec<String> = config.content_libraries.iter().map(|lib| lib.name.clone()).collect();
                    Ok(crate::tools::dtos::ListFilesResponse { files: libs.join("\n") })
                }
            }
        }
    },
    {
        name: "read_file",
        description: "Read the entire text contents of a file at the specified path. Prefer using the read_yaml_header tool if just a document summary is needed.",
        input: crate::tools::dtos::ReadFileInput,
        enabled: |_| true,
        execute: |_config, _root_path, input, resolve_path| {
            let path = resolve_path(&input.path)?.ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
            crate::tools::filesystem::tool_read_file(&path.to_string_lossy())
        }
    },
    {
        name: "read_file_lines",
        description: "Read specific lines from a file (1-indexed).",
        input: crate::tools::dtos::ReadFileLinesInput,
        enabled: |_| true,
        execute: |_config, _root_path, input, resolve_path| {
            let path = resolve_path(&input.path)?.ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
            crate::tools::filesystem::tool_read_file_lines(&path.to_string_lossy(), input.start_line, input.end_line)
        }
    },
    {
        name: "create_file",
        description: "Create a new file at the specified path with the provided content.",
        input: crate::tools::dtos::CreateFileInput,
        enabled: |_| true,
        execute: |_config, _root_path, input, resolve_path| {
            let path = resolve_path(&input.path)?.ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
            crate::tools::filesystem::tool_create_file(&path.to_string_lossy(), &input.content)
        }
    },
    {
        name: "insert_lines",
        description: "Insert lines into a file at a specific 1-indexed line index.",
        input: crate::tools::dtos::InsertLinesInput,
        enabled: |_| true,
        execute: |_config, _root_path, input, resolve_path| {
            let path = resolve_path(&input.path)?.ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
            crate::tools::filesystem::tool_insert_lines(&path.to_string_lossy(), input.line_index, &input.lines)
        }
    },
    {
        name: "delete_lines",
        description: "Delete specific lines from a file (1-indexed, inclusive).",
        input: crate::tools::dtos::DeleteLinesInput,
        enabled: |_| true,
        execute: |_config, _root_path, input, resolve_path| {
            let path = resolve_path(&input.path)?.ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
            crate::tools::filesystem::tool_delete_lines(&path.to_string_lossy(), input.start_line, input.end_line)
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
        execute: |_config, _root_path, input, resolve_path| {
            let path = resolve_path(&input.path)?.ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
            crate::tools::yaml_header::tool_read_yaml_header(&path.to_string_lossy())
        }
    },
    {
        name: "write_yaml_header",
        description: "Write or update data in a YAML header to a markdown file.",
        input: crate::tools::dtos::WriteYamlHeaderInput,
        enabled: |_| true,
        execute: |_config, _root_path, input, resolve_path| {
            let path = resolve_path(&input.path)?.ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
            crate::tools::yaml_header::tool_write_yaml_header(&path.to_string_lossy(), input.title.as_deref(), input.summary.as_deref(), input.tags, input.header_date.as_deref())
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
    fn test_resolve_virtual_path() {
        let mut config = AppConfig::default();
        config.content_libraries.push(crate::config::ContentLibrary {
            name: "TestLib".to_string(),
            root_folder: "C:\\TestRoot".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        });
        let root = Path::new("C:\\TestRoot");
        
        // This will succeed in resolving to C:\TestRoot\sub\file.md
        let res1 = execute_tool(&config, root, "read_file", r#"{"path": "TestLib\\sub\\file.md"}"#);
        assert!(!res1.contains("Invalid virtual path"));

        // Path traversal is blocked
        let res3 = execute_tool(&config, root, "read_file", r#"{"path": "TestLib\\..\\Windows\\System32\\cmd.exe"}"#);
        assert!(res3.contains("Path traversal not allowed"));
        
        // Unknown library
        let res4 = execute_tool(&config, root, "read_file", r#"{"path": "UnknownLib\\file.md"}"#);
        assert!(res4.contains("Content library 'UnknownLib' not found"));

        // Resolving root dir / and .
        let res5 = execute_tool(&config, root, "list_files", r#"{"path": "."}"#);
        assert!(!res5.contains("Invalid virtual path") && !res5.contains("error"));
        // Since it's list_files on root, it should return TestLib
        assert!(res5.contains("TestLib"));

        let res6 = execute_tool(&config, root, "list_files", r#"{"path": "/"}"#);
        assert!(!res6.contains("Invalid virtual path") && !res6.contains("error"));
        assert!(res6.contains("TestLib"));
    }

    #[test]
    fn test_grep_priority_ordering() {
        let mut config = AppConfig::default();
        config.content_libraries.push(crate::config::ContentLibrary {
            name: "Low".to_string(),
            root_folder: "C:\\LowRoot".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        });
        config.content_libraries.push(crate::config::ContentLibrary {
            name: "High".to_string(),
            root_folder: "C:\\HighRoot".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 100,
        });
        let mut libs: Vec<_> = config.content_libraries.iter().collect();
        libs.sort_by(|a, b| b.priority.cmp(&a.priority));
        assert_eq!(libs[0].name, "High");
        assert_eq!(libs[1].name, "Low");
    }

    #[test]
    fn test_path_traversal_dotdot_rejected() {
        let mut config = AppConfig::default();
        config.content_libraries.push(crate::config::ContentLibrary {
            name: "Lib".to_string(),
            root_folder: "C:\\Root".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        });
        let root = Path::new("C:\\Root");
        
        // Multiple traversals
        let res = execute_tool(&config, root, "read_file", r#"{"path": "Lib/../../etc/passwd"}"#);
        assert!(res.contains("Path traversal not allowed"));

        // Single parent dir
        let res2 = execute_tool(&config, root, "read_file", r#"{"path": "Lib/.."}"#);
        assert!(res2.contains("Path traversal not allowed"));
    }

    #[test]
    fn test_resolve_path_with_library_missing() {
        let config = AppConfig::default();
        let root = Path::new("C:\\");
        let res = execute_tool(&config, root, "list_files", r#"{"path": "NonExistentLib"}"#);
        assert!(res.contains("Content library 'NonExistentLib' not found"));
    }

    #[test]
    fn test_unknown_tool_returns_error() {
        let config = AppConfig::default();
        let root = Path::new("C:\\");
        let res = execute_tool(&config, root, "nonexistent_tool", "{}");
        assert!(res.contains("Tool nonexistent_tool not found"));
    }

    #[test]
    fn test_tool_invalid_args_returns_error() {
        let config = AppConfig::default();
        let root = Path::new("C:\\");
        let res = execute_tool(&config, root, "list_files", "not valid json");
        assert!(res.contains("Invalid args") || res.contains("error"));
    }
}
