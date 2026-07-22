//! Splits LLM thinking/reasoning blocks from final content and formats tool-call/result messages for the chat UI.

pub fn split_thinking_and_content(text: &str) -> (String, String) {
    let delim = "\u{1f914}";
    if let Some(start_idx) = text.find(delim) {
        if let Some(offset) = text[start_idx + delim.len()..].find(delim) {
            let end_idx = start_idx + delim.len() + offset;
            let thinking = text[start_idx + delim.len()..end_idx].to_string();
            let content = format!("{}{}", &text[..start_idx], &text[end_idx + delim.len()..]);
            return (thinking, content);
        }
    }
    (String::new(), text.to_string())
}

pub fn format_tool_call_message(func_name: &str, func_args_str: &str) -> String {
    if func_name == "create_file" {
        let mut msg = format!("> **Executing tool `{}`**\n", func_name);
        if let Ok(args_val) = serde_json::from_str::<serde_json::Value>(func_args_str) {
            if let Some(path) = args_val.get("path").and_then(|p| p.as_str()) {
                msg.push_str(&format!("> Path: `{}`\n", path));
            }
        }
        return msg;
    }
    let formatted_args = match serde_json::from_str::<serde_json::Value>(func_args_str) {
        Ok(val) => serde_json::to_string_pretty(&val).unwrap_or_else(|_| func_args_str.to_string()),
        Err(_) => func_args_str.to_string(),
    };
    let quoted = formatted_args
        .lines()
        .map(|line| format!("> {}", line))
        .collect::<Vec<_>>()
        .join("\n");
    format!("> **Executing tool `{}`**\n{}\n", func_name, quoted)
}

fn count_from_data(data: &serde_json::Value, field: &str) -> usize {
    data.get(field)
        .and_then(|f| f.as_array())
        .map(|a| a.len())
        .unwrap_or(0)
}

pub fn format_tool_result_message(func_name: &str, result: &str) -> String {
    let mut is_error = false;
    let mut error_msg = String::new();
    let mut result_data = serde_json::Value::Null;
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(result) {
        if let Some(status) = parsed.get("status").and_then(|s| s.as_str()) {
            if status == "error" {
                is_error = true;
                error_msg = parsed
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("Unknown error")
                    .to_string();
            } else if status == "success" {
                if let Some(data) = parsed.get("data") {
                    result_data = data.clone();
                }
            }
        }
    }
    if is_error {
        return format!("> **Result Error:** {}\n\n", error_msg);
    }
    match func_name {
        "create_file" => {
            let size = result_data
                .get("size_bytes")
                .and_then(|s| s.as_u64())
                .unwrap_or(0);
            format!("> **Result:** File created ({} B).\n\n", size)
        }
        "list_files" | "list_files_by_tag" => {
            let count = count_from_data(&result_data, "files");
            let total = result_data
                .get("total")
                .and_then(|t| t.as_u64())
                .unwrap_or(count as u64);
            format!(
                "> **Result:** {} files returned (total: {}).\n\n",
                count, total
            )
        }
        "read_tags" => {
            let count = count_from_data(&result_data, "tags");
            format!("> **Result:** {} tag(s) found.\n\n", count)
        }
        "read_file" | "read_file_lines" => {
            let content = result_data
                .get("content")
                .and_then(|f| f.as_str())
                .unwrap_or("");
            format!(
                "> **Result:** {} line(s) read.\n\n",
                content.lines().count()
            )
        }
        "web_fetch" => {
            let content = result_data
                .get("content")
                .and_then(|f| f.as_str())
                .unwrap_or("");
            let total_lines = result_data
                .get("total_lines")
                .and_then(|f| f.as_u64())
                .unwrap_or(0);
            let from_cache = result_data
                .get("from_cache")
                .and_then(|f| f.as_bool())
                .unwrap_or(false);
            let cache_tag = if from_cache { " (cached)" } else { "" };
            let returned = content.lines().count();
            format!(
                "> **Result:** {} of {} markdown lines returned{}. Use limit/offset to read other sections.\n\n",
                returned, total_lines, cache_tag
            )
        }
        "web_search" => {
            let content = result_data
                .get("results")
                .and_then(|f| f.as_str())
                .unwrap_or("");
            let count = content
                .split("\n\n")
                .filter(|s| !s.trim().is_empty())
                .count();
            format!("> **Result:** {} search results returned.\n\n", count)
        }
        "grep" => format_grep_result(&result_data),
        "get_email_by_id" => format_email_by_id_result(&result_data),
        "search_email" => format_search_email_result(&result_data),
        name if name.starts_with("search_") => format_generic_search_result(&result_data),
        _ if result.len() < 100 && result.lines().count() <= 1 => {
            format!("> **Result:** {}\n\n", result)
        }
        _ => {
            let action = func_name.replace('_', " ");
            format!(
                "> **Result:** Tool '{}' completed successfully.\n\n",
                action
            )
        }
    }
}

fn format_grep_result(data: &serde_json::Value) -> String {
    let content = data.get("matches").and_then(|f| f.as_str()).unwrap_or("");
    if content == "No matches found." || content.is_empty() {
        return "> **Result:** 0 file(s) match\n\n".to_string();
    }
    let mut files = std::collections::HashSet::new();
    for line in content.lines() {
        if let Some(idx) = line.rfind(".md:") {
            files.insert(&line[..idx + 3]);
        } else if let Some(idx) = line.rfind(".markdown:") {
            files.insert(&line[..idx + 9]);
        } else if let Some(idx) = line.find(':') {
            files.insert(&line[..idx]);
        }
    }
    format!("> **Result:** {} file(s) match\n\n", files.len())
}

fn format_email_by_id_result(data: &serde_json::Value) -> String {
    let extract = |field: &str| -> String {
        data.get("result")
            .and_then(|v| v.as_str())
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
            .and_then(|val| val.as_array().and_then(|a| a.first().cloned()))
            .and_then(|obj| {
                obj.get(field)
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_default()
    };
    let subject = extract("subject");
    let date = extract("date");
    if !subject.is_empty() || !date.is_empty() {
        format!("> **Result:** {} - {}\n\n", date, subject)
    } else {
        "> **Result:** Email content retrieved.\n\n".to_string()
    }
}

fn format_search_email_result(data: &serde_json::Value) -> String {
    let total = data.get("total").and_then(|t| t.as_u64()).unwrap_or(0);
    let hint = data.get("hint").and_then(|h| h.as_str()).unwrap_or("");
    if !hint.is_empty() {
        format!("> **Result:** {} item(s) found. {}\n\n", total, hint)
    } else {
        format!("> **Result:** {} item(s) found\n\n", total)
    }
}

fn format_generic_search_result(data: &serde_json::Value) -> String {
    let count = data
        .get("results")
        .and_then(|r| r.as_str())
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        .and_then(|v| v.as_array().cloned())
        .map(|a| a.len())
        .unwrap_or_else(|| data.as_array().map(|a| a.len()).unwrap_or(0));
    format!("> **Result:** {} item(s) found\n\n", count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_thinking_no_delimiter() {
        let (t, c) = split_thinking_and_content("Hello world");
        assert!(t.is_empty());
        assert_eq!(c, "Hello world");
    }

    #[test]
    fn test_split_thinking_with_delimiter() {
        let (t, c) = split_thinking_and_content("Before\u{1f914}thinking\u{1f914}After");
        assert_eq!(t, "thinking");
        assert_eq!(c, "BeforeAfter");
    }

    #[test]
    fn test_split_thinking_only_opening() {
        let (t, c) = split_thinking_and_content("Before\u{1f914}no closing");
        assert!(t.is_empty());
        assert_eq!(c, "Before\u{1f914}no closing");
    }

    #[test]
    fn test_split_thinking_empty_thinking() {
        let (t, c) = split_thinking_and_content("Before\u{1f914}\u{1f914}After");
        assert!(t.is_empty());
        assert_eq!(c, "BeforeAfter");
    }

    #[test]
    fn test_format_tool_call_create_file() {
        let msg = format_tool_call_message("create_file", r#"{"path":"lib/test.md"}"#);
        assert!(msg.contains("create_file"));
        assert!(msg.contains("lib/test.md"));
    }

    #[test]
    fn test_format_tool_call_other() {
        let msg = format_tool_call_message("grep", r#"{"pattern":"hello"}"#);
        assert!(msg.contains("grep"));
        assert!(msg.contains("hello"));
    }

    #[test]
    fn test_format_result_error() {
        let result = r#"{"status":"error","message":"not found"}"#;
        let msg = format_tool_result_message("read_file", result);
        assert!(msg.contains("Error"));
        assert!(msg.contains("not found"));
    }

    #[test]
    fn test_format_result_create_file() {
        let result = r#"{"status":"success","data":{"size_bytes":42}}"#;
        let msg = format_tool_result_message("create_file", result);
        assert!(msg.contains("42 B"));
    }

    #[test]
    fn test_format_result_read_tags() {
        let result = r#"{"status":"success","data":{"tags":["a","b"]}}"#;
        let msg = format_tool_result_message("read_tags", result);
        assert!(msg.contains("2 tag(s)"));
    }

    #[test]
    fn test_format_result_read_file() {
        let result = r#"{"status":"success","data":{"content":"line1\nline2"}}"#;
        let msg = format_tool_result_message("read_file", result);
        assert!(msg.contains("2 line(s)"));
    }

    #[test]
    fn test_format_result_grep_no_matches() {
        let result = r#"{"status":"success","data":{"matches":"No matches found."}}"#;
        let msg = format_tool_result_message("grep", result);
        assert!(msg.contains("0 file(s)"));
    }

    #[test]
    fn test_format_result_generic_search() {
        let result = r#"{"status":"success","data":{"results":"[{\"a\":1}]"}}"#;
        let msg = format_tool_result_message("search_calendar", result);
        assert!(msg.contains("1 item(s)"));
    }

    #[test]
    fn test_format_result_unknown_tool_long_result() {
        let result = "x".repeat(200);
        let msg = format_tool_result_message("some_tool", &result);
        assert!(msg.contains("some tool"));
        assert!(msg.contains("completed successfully"));
    }

    #[test]
    fn test_format_result_unknown_tool_short_result() {
        let msg = format_tool_result_message("some_tool", "ok");
        assert!(msg.contains("ok"));
    }
}
