use crate::config::AppConfig;
use serde_json::Value;
use std::path::{Component, Path};

pub fn execute_tool(config: &AppConfig, root_path: &Path, name: &str, args_str: &str) -> String {
    // Ensure rustls crypto provider is installed (ignore error if already installed)
    rustls::crypto::ring::default_provider().install_default().ok();

    let args: Value = serde_json::from_str(args_str).unwrap_or(Value::Null);
    let args_compact = serde_json::to_string(&args).unwrap_or_else(|_| args_str.to_string());
    println!("Tool call: {}({})", name, args_compact);
    let start_time = std::time::Instant::now();
    let is_safe_path = |p: &str| -> bool {
        if Path::new(p).components().any(|c| c == Component::ParentDir) {
            return false;
        }
        let full_path = root_path.join(p).to_string_lossy().to_lowercase();
        let mut root_str = root_path.to_string_lossy().to_lowercase();
        if !root_str.ends_with('/') && !root_str.ends_with('\\') {
            root_str.push(std::path::MAIN_SEPARATOR);
        }
        full_path.starts_with(&root_str) || full_path == root_path.to_string_lossy().to_lowercase()
    };
    let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        match name {
        "grep" => {
            let query = args.get("query").and_then(|q| q.as_str()).unwrap_or("");
            crate::tools::filesystem::tool_grep(root_path, query)
        }
        "read_tags" => crate::tools::filesystem::tool_read_tags(root_path),
        "list_files_by_tag" => {
            let tag = args.get("tag").and_then(|t| t.as_str()).unwrap_or("");
            crate::tools::filesystem::tool_list_files_by_tag(root_path, tag)
        }
        "list_files" => {
            let path = args.get("path").and_then(|p| p.as_str()).unwrap_or("");
            if !is_safe_path(path) { return "Error: Invalid path".to_string(); }
            crate::tools::filesystem::tool_list_files(&root_path.join(path))
        }
        "read_file" => {
            let path = args.get("path").and_then(|p| p.as_str()).unwrap_or("");
            if !is_safe_path(path) { return "Error: Invalid path".to_string(); }
            crate::tools::filesystem::tool_read_file(&root_path.join(path).to_string_lossy())
        }
        "read_file_lines" => {
            let path = args.get("path").and_then(|p| p.as_str()).unwrap_or("");
            if !is_safe_path(path) { return "Error: Invalid path".to_string(); }
            let start = args.get("start_line").and_then(|s| s.as_u64()).unwrap_or(1) as usize;
            let end = args.get("end_line").and_then(|e| e.as_u64()).unwrap_or(1) as usize;
            crate::tools::filesystem::tool_read_file_lines(&root_path.join(path).to_string_lossy(), start, end)
        }
        "create_file" => {
            let path = args.get("path").and_then(|p| p.as_str()).unwrap_or("");
            if !is_safe_path(path) { return "Error: Invalid path".to_string(); }
            let content = args.get("content").and_then(|c| c.as_str()).unwrap_or("");
            crate::tools::filesystem::tool_create_file(&root_path.join(path).to_string_lossy(), content)
        }
        "insert_lines" => {
            let path = args.get("path").and_then(|p| p.as_str()).unwrap_or("");
            if !is_safe_path(path) { return "Error: Invalid path".to_string(); }
            let line_index = args.get("line_index").and_then(|l| l.as_u64()).unwrap_or(1) as usize;
            let lines: Vec<String> = args
                .get("lines")
                .and_then(|l| l.as_array())
                .map(|a| {
                    a.iter()
                        .map(|v| v.as_str().unwrap_or("").to_string())
                        .collect()
                })
                .unwrap_or_default();
            crate::tools::filesystem::tool_insert_lines(&root_path.join(path).to_string_lossy(), line_index, &lines)
        }
        "delete_lines" => {
            let path = args.get("path").and_then(|p| p.as_str()).unwrap_or("");
            if !is_safe_path(path) { return "Error: Invalid path".to_string(); }
            let start = args.get("start_line").and_then(|s| s.as_u64()).unwrap_or(1) as usize;
            let end = args.get("end_line").and_then(|e| e.as_u64()).unwrap_or(1) as usize;
            crate::tools::filesystem::tool_delete_lines(&root_path.join(path).to_string_lossy(), start, end)
        }
        "web_fetch" => {
            let url = args.get("url").and_then(|u| u.as_str()).unwrap_or("");
            crate::tools::web::tool_web_fetch(url)
        }
        "read_yaml_header" => {
            let path = args.get("path").and_then(|p| p.as_str()).unwrap_or("");
            if !is_safe_path(path) { return "Error: Invalid path".to_string(); }
            crate::tools::yaml_header::tool_read_yaml_header(&root_path.join(path).to_string_lossy())
        }
        "write_yaml_header" => {
            let path = args.get("path").and_then(|p| p.as_str()).unwrap_or("");
            if !is_safe_path(path) { return "Error: Invalid path".to_string(); }
            let title = args.get("title").and_then(|t| t.as_str());
            let summary = args.get("summary").and_then(|s| s.as_str());
            let tags = args.get("tags").and_then(|t| t.as_array()).map(|arr| {
                arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<Vec<_>>()
            });
            let header_date = args.get("header-date").and_then(|h| h.as_str());
            crate::tools::yaml_header::tool_write_yaml_header(&root_path.join(path).to_string_lossy(), title, summary, tags, header_date)
        }
        "web_search" => {
            if let Some(url) = &config.searxng_url {
                let query = args.get("query").and_then(|q| q.as_str()).unwrap_or("");
                crate::tools::web::tool_web_search(url, query)
            } else {
                "web_search tool is disabled (no SearXNG URL configured).".to_string()
            }
        }
        "search_calendar" => {
            let keyword = args.get("keyword").and_then(|k| k.as_str()).unwrap_or("");
            crate::tools::caldav::tool_search_calendar(config, keyword)
        }
        "get_calendar" => {
            let start = args.get("start_date").and_then(|d| d.as_str()).unwrap_or("");
            let end = args.get("end_date").and_then(|d| d.as_str()).unwrap_or("");
            crate::tools::caldav::tool_get_calendar(config, start, end)
        }
        "get_calendar_item" => {
            let id = args.get("href").and_then(|h| h.as_str()).unwrap_or("");
            crate::tools::caldav::tool_get_calendar_item(config, id)
        }
        "add_calendar_item" => {
            let item = args.get("item_json").and_then(|i| i.as_str()).unwrap_or("");
            crate::tools::caldav::tool_add_calendar_item(config, item)
        }
        "update_calendar_item" => {
            let id = args.get("id").and_then(|id| id.as_str()).unwrap_or("");
            let update_json = args.get("update_json").and_then(|u| u.as_str()).unwrap_or("");
            crate::tools::caldav::tool_update_calendar_item(config, id, update_json)
        }
        "delete_calendar_item" => {
            let id = args.get("id").and_then(|id| id.as_str()).unwrap_or("");
            crate::tools::caldav::tool_delete_calendar_item(config, id)
        }
        "search_email" => {
            let keyword = args.get("keyword").and_then(|k| k.as_str()).unwrap_or("");
            crate::tools::jmap::tool_search_email(config, keyword)
        }
        "get_email_by_id" => {
            let id = args.get("id").and_then(|id| id.as_str()).unwrap_or("");
            crate::tools::jmap::tool_get_email_by_id(config, id)
        }
        "get_email" => {
            let start_date = args.get("start_date").and_then(|d| d.as_str());
            let end_date = args.get("end_date").and_then(|d| d.as_str());
            let sender = args.get("sender").and_then(|s| s.as_str());
            let recipient = args.get("recipient").and_then(|r| r.as_str());
            let is_unread = args.get("is_unread").and_then(|u| u.as_bool());
            let is_flagged = args.get("is_flagged").and_then(|f| f.as_bool());
            crate::tools::jmap::tool_get_email(config, start_date, end_date, sender, recipient, is_unread, is_flagged)
        }
        "send_email" => {
            let to = args.get("to").and_then(|t| t.as_str()).unwrap_or("");
            let subject = args.get("subject").and_then(|s| s.as_str()).unwrap_or("");
            let body = args.get("body").and_then(|b| b.as_str()).unwrap_or("");
            crate::tools::jmap::tool_send_email(config, to, subject, body)
        }
        "search_contact" => {
            let keyword = args.get("keyword").and_then(|k| k.as_str()).unwrap_or("");
            crate::tools::jmap::tool_search_contact(config, keyword)
        }
        "get_contact" => {
            let id = args.get("id").and_then(|id| id.as_str()).unwrap_or("");
            crate::tools::jmap::tool_get_contact(config, id)
        }
        "add_contact" => {
            let contact_json = args.get("contact_json").and_then(|c| c.as_str()).unwrap_or("");
            crate::tools::jmap::tool_add_contact(config, contact_json)
        }
        _ => format!("Error: Tool {} not found.", name),
        }
    })) {
        Ok(res) => res,
        Err(e) => {
            let msg = if let Some(s) = e.downcast_ref::<&str>() {
                *s
            } else if let Some(s) = e.downcast_ref::<String>() {
                s.as_str()
            } else {
                "Unknown panic"
            };
            format!("Error: Tool {} panicked: {}", name, msg)
        }
    };
    
    let elapsed = start_time.elapsed();
    let is_error = result.starts_with("Error") || result.starts_with("error");
    if is_error {
        println!("Tool '{}' failed in {:.2?}.", name, elapsed);
        eprintln!("Tool '{}' error details: {}", name, result);
    } else {
        println!("Tool '{}' succeeded in {:.2?}.", name, elapsed);
    }
    result
}

pub fn get_tools_schema(config: &AppConfig) -> Value {
    let mut tools = serde_json::json!([
      {
        "type": "function",
        "function": {
          "name": "grep",
          "description": "Search for a query string case-insensitively across all Markdown files in the workspace.",
          "parameters": {
            "type": "object",
            "properties": {
              "query": { "type": "string", "description": "The search term." }
            },
            "required": ["query"]
          }
        }
      },
      {
        "type": "function",
        "function": {
          "name": "read_tags",
          "description": "Get all unique tags defined in front-matter headers of all Markdown files in the workspace.",
          "parameters": { "type": "object", "properties": {} }
        }
      },
      {
        "type": "function",
        "function": {
          "name": "list_files_by_tag",
          "description": "List all Markdown files that contain a specific tag in their front-matter.",
          "parameters": {
            "type": "object",
            "properties": {
              "tag": { "type": "string", "description": "The tag to filter by." }
            },
            "required": ["tag"]
          }
        }
      },
      {
        "type": "function",
        "function": {
          "name": "list_files",
          "description": "List all Markdown files in a directory (not recursive).",
          "parameters": {
            "type": "object",
            "properties": {
              "path": { "type": "string", "description": "The path to the directory." }
            },
            "required": ["path"]
          }
        }
      },
      {
        "type": "function",
        "function": {
          "name": "read_file",
          "description": "Read the entire text contents of a file at the specified path.",
          "parameters": {
            "type": "object",
            "properties": {
              "path": { "type": "string", "description": "The path to the file." }
            },
            "required": ["path"]
          }
        }
      },
      {
        "type": "function",
        "function": {
          "name": "read_file_lines",
          "description": "Read specific lines from a file (1-indexed).",
          "parameters": {
            "type": "object",
            "properties": {
              "path": { "type": "string", "description": "The path to the file." },
              "start_line": { "type": "integer", "description": "The start line index (inclusive, 1-indexed)." },
              "end_line": { "type": "integer", "description": "The end line index (inclusive, 1-indexed)." }
            },
            "required": ["path", "start_line", "end_line"]
          }
        }
      },
      {
        "type": "function",
        "function": {
          "name": "create_file",
          "description": "Create a new file at the specified path with the provided content.",
          "parameters": {
            "type": "object",
            "properties": {
              "path": { "type": "string", "description": "The path of the file to create." },
              "content": { "type": "string", "description": "The content to write to the file." }
            },
            "required": ["path", "content"]
          }
        }
      },
      {
        "type": "function",
        "function": {
          "name": "insert_lines",
          "description": "Insert lines into a file at a specific 1-indexed line index.",
          "parameters": {
            "type": "object",
            "properties": {
              "path": { "type": "string", "description": "The path to the file." },
              "line_index": { "type": "integer", "description": "The 1-indexed position to insert lines at (the lines will be inserted right before this line)." },
              "lines": { "type": "array", "items": { "type": "string" }, "description": "The lines of text to insert." }
            },
            "required": ["path", "line_index", "lines"]
          }
        }
      },
      {
        "type": "function",
        "function": {
          "name": "delete_lines",
          "description": "Delete specific lines from a file (1-indexed, inclusive).",
          "parameters": {
            "type": "object",
            "properties": {
              "path": { "type": "string", "description": "The path to the file." },
              "start_line": { "type": "integer", "description": "The start line (1-indexed)." },
              "end_line": { "type": "integer", "description": "The end line (1-indexed)." }
            },
            "required": ["path", "start_line", "end_line"]
          }
        }
      },
      {
        "type": "function",
        "function": {
          "name": "web_fetch",
          "description": "Fetch content from a URL.",
          "parameters": {
            "type": "object",
            "properties": {
              "url": { "type": "string", "description": "The URL to fetch." }
            },
            "required": ["url"]
          }
        }
      },
      {
        "type": "function",
        "function": {
          "name": "read_yaml_header",
          "description": "Parse a YAML header from a markdown file and return its content representation.",
          "parameters": {
            "type": "object",
            "properties": {
              "path": { "type": "string", "description": "The path to the markdown file." }
            },
            "required": ["path"]
          }
        }
      },
      {
        "type": "function",
        "function": {
          "name": "write_yaml_header",
          "description": "Write or update data in a YAML header to a markdown file.",
          "parameters": {
            "type": "object",
            "properties": {
              "path": { "type": "string", "description": "The path to the file." },
              "title": { "type": "string", "description": "A brief title" },
              "summary": { "type": "string", "description": "A three sentence summary of the contents" },
              "tags": { "type": "array", "items": { "type": "string" }, "description": "Array of tags" },
              "header-date": { "type": "string", "description": "RFC 3339 timestamp" }
            },
            "required": ["path"]
          }
        }
      }
    ]).as_array().unwrap().clone();

    if config.searxng_url.is_some() {
        tools.push(serde_json::json!({
            "type": "function",
            "function": {
                "name": "web_search",
                "description": "Search the web using SearXNG.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "The search query." }
                    },
                    "required": ["query"]
                }
            }
        }));
    }

    if !config.caldav_clients.is_empty() {
        tools.push(serde_json::json!({
            "type": "function",
            "function": {
                "name": "search_calendar",
                "description": "Search the calendar by keyword.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "keyword": { "type": "string", "description": "The search keyword." }
                    },
                    "required": ["keyword"]
                }
            }
        }));
        tools.push(serde_json::json!({
            "type": "function",
            "function": {
                "name": "get_calendar",
                "description": "Get calendar items by date range.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "start_date": { "type": "string", "description": "Start date in ISO format." },
                        "end_date": { "type": "string", "description": "End date in ISO format." }
                    },
                    "required": ["start_date", "end_date"]
                }
            }
        }));
        tools.push(serde_json::json!({
            "type": "function",
            "function": {
                "name": "get_calendar_item",
                "description": "Get a specific calendar item by its full href. IMPORTANT: Use the exact, full 'href' value returned by search or get tools (e.g., '/dav/calendars/user/.../item.ics'). Do not use just the UUID.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "href": { "type": "string", "description": "The exact full href of the calendar item." }
                    },
                    "required": ["href"]
                }
            }
        }));
        tools.push(serde_json::json!({
            "type": "function",
            "function": {
                "name": "add_calendar_item",
                "description": "Add a new calendar item.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "item_json": { "type": "string", "description": "JSON representation of the calendar item. Must be a JSON string with keys: 'summary', 'start', 'end', 'description', 'location'. Format newlines in description properly using standard JSON encoding (e.g. \\n)." }
                    },
                    "required": ["item_json"]
                }
            }
        }));
        tools.push(serde_json::json!({
            "type": "function",
            "function": { "name": "update_calendar_item", "description": "Update a calendar item.", "parameters": { "type": "object", "properties": { "id": { "type": "string" }, "update_json": { "type": "string", "description": "JSON string containing updated keys: 'summary', 'start', 'end', 'description', 'location'. Format newlines properly using standard JSON encoding." } }, "required": ["id", "update_json"] } }
        }));
        tools.push(serde_json::json!({
            "type": "function",
            "function": { "name": "delete_calendar_item", "description": "Delete a calendar item.", "parameters": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] } }
        }));
    }

    if !config.jmap_clients.is_empty() {
        tools.push(serde_json::json!({
            "type": "function",
            "function": { "name": "search_email", "description": "Search email by keyword.", "parameters": { "type": "object", "properties": { "keyword": { "type": "string" } }, "required": ["keyword"] } }
        }));
        tools.push(serde_json::json!({
            "type": "function",
            "function": { "name": "get_email_by_id", "description": "Get email by id.", "parameters": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] } }
        }));
        tools.push(serde_json::json!({
            "type": "function",
            "function": { "name": "get_email", "description": "Get email by date range, sender, recipient, unread status, or flagged status.", "parameters": { "type": "object", "properties": { "start_date": { "type": "string" }, "end_date": { "type": "string" }, "sender": { "type": "string" }, "recipient": { "type": "string" }, "is_unread": { "type": "boolean" }, "is_flagged": { "type": "boolean" } } } }
        }));
        tools.push(serde_json::json!({
            "type": "function",
            "function": { "name": "send_email", "description": "Send an email.", "parameters": { "type": "object", "properties": { "to": { "type": "string" }, "subject": { "type": "string" }, "body": { "type": "string" } }, "required": ["to", "subject", "body"] } }
        }));
        tools.push(serde_json::json!({
            "type": "function",
            "function": { "name": "search_contact", "description": "Search contacts by keyword.", "parameters": { "type": "object", "properties": { "keyword": { "type": "string" } }, "required": ["keyword"] } }
        }));
        tools.push(serde_json::json!({
            "type": "function",
            "function": { "name": "add_contact", "description": "Add a new contact.", "parameters": { "type": "object", "properties": { "contact_json": { "type": "string" } }, "required": ["contact_json"] } }
        }));
        tools.push(serde_json::json!({
            "type": "function",
            "function": { "name": "get_contact", "description": "Get contact by id.", "parameters": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] } }
        }));
    }

    serde_json::Value::Array(tools)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_is_safe_path() {
        let config = AppConfig::default();
        let root = Path::new("C:\\TestRoot");
        
        // This relies on execute_tool to exercise the internal closure.
        // If it returns "Error: Invalid path", it failed the check.
        // If it returns "Error reading file" or similar, it passed the check.
        
        // Case-sensitive exact match
        let res1 = execute_tool(&config, root, "read_file", r#"{"path": "C:\\TestRoot\\sub\\file.md"}"#);
        assert!(!res1.contains("Error: Invalid path"));

        // Case-insensitive match on Windows
        let res2 = execute_tool(&config, root, "read_file", r#"{"path": "c:\\testroot\\sub\\file.md"}"#);
        assert!(!res2.contains("Error: Invalid path"));

        // Path traversal should fail
        let res3 = execute_tool(&config, root, "read_file", r#"{"path": "C:\\TestRoot\\..\\Windows\\System32\\cmd.exe"}"#);
        assert!(res3.contains("Error: Invalid path"));
        
        // Outside path should fail
        let res4 = execute_tool(&config, root, "read_file", r#"{"path": "C:\\Windows\\System32\\cmd.exe"}"#);
        assert!(res4.contains("Error: Invalid path"));
    }
}