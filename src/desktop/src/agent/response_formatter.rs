//! Splits LLM thinking/reasoning blocks from final content and formats tool-call/result messages for the chat UI.

pub fn split_thinking_and_content(text: &str) -> (String, String) {
    let delim = "\u{1f914}";
    if let Some(start_idx) = text.find(delim)
        && let Some(offset) = text[start_idx + delim.len()..].find(delim)
    {
        let end_idx = start_idx + delim.len() + offset;
        let thinking = text[start_idx + delim.len()..end_idx].to_string();
        let content = format!("{}{}", &text[..start_idx], &text[end_idx + delim.len()..]);
        return (thinking, content);
    }
    (String::new(), text.to_string())
}

pub fn format_tool_call_message(func_name: &str, func_args_str: &str) -> String {
    if func_name == "create_file" {
        let mut msg = format!("> **Executing tool `{}`**\n", func_name);
        if let Ok(args_val) = serde_json::from_str::<serde_json::Value>(func_args_str)
            && let Some(path) = args_val.get("path").and_then(|p| p.as_str())
        {
            msg.push_str(&format!("> Path: `{}`\n", path));
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
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(result)
        && let Some(status) = parsed.get("status").and_then(|s| s.as_str())
    {
        if status == "error" {
            is_error = true;
            error_msg = parsed
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error")
                .to_string();
        } else if status == "success"
            && let Some(data) = parsed.get("data")
        {
            result_data = data.clone();
        }
    }
    if is_error {
        return format!("> **Result Error (`{}`):** {}\n\n", func_name, error_msg);
    }
    match func_name {
        "create_file" => {
            let size = result_data
                .get("size_bytes")
                .and_then(|s| s.as_u64())
                .unwrap_or(0);
            format!(
                "> **Result (`{}`):** File created ({} B).\n\n",
                func_name, size
            )
        }
        "list_files" | "list_files_by_tag" => {
            let count = count_from_data(&result_data, "files");
            let total = result_data
                .get("total")
                .and_then(|t| t.as_u64())
                .unwrap_or(count as u64);
            format!(
                "> **Result (`{}`):** {} files returned (total: {}).\n\n",
                func_name, count, total
            )
        }
        "read_tags" => {
            let count = count_from_data(&result_data, "tags");
            format!(
                "> **Result (`{}`):** {} tag(s) found.\n\n",
                func_name, count
            )
        }
        "read_file" | "read_file_lines" => {
            let content = result_data
                .get("content")
                .and_then(|f| f.as_str())
                .unwrap_or("");
            format!(
                "> **Result (`{}`):** {} line(s) read.\n\n",
                func_name,
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
                "> **Result (`{}`):** {} of {} markdown lines returned{}. To read other sections, set offset to the number of lines already retrieved (e.g., if you received 100 lines starting at offset 0, next use offset=100). Track cumulative coverage to avoid gaps.\n\n",
                func_name, returned, total_lines, cache_tag
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
            format!(
                "> **Result (`{}`):** {} search results returned.\n\n",
                func_name, count
            )
        }
        "grep" => format_grep_result(func_name, &result_data),
        "get_email_by_id" => format_email_by_id_result(func_name, &result_data),
        "search_email" => format_search_email_result(func_name, &result_data),
        name if name.starts_with("search_") => {
            format_generic_search_result(func_name, &result_data)
        }
        _ if result.len() < 100 && result.lines().count() <= 1 => {
            format!("> **Result (`{}`):** {}\n\n", func_name, result)
        }
        _ => {
            let action = func_name.replace('_', " ");
            format!(
                "> **Result (`{}`):** Tool '{}' completed successfully.\n\n",
                func_name, action
            )
        }
    }
}

fn format_grep_result(func_name: &str, data: &serde_json::Value) -> String {
    let content = data.get("matches").and_then(|f| f.as_str()).unwrap_or("");
    if content == "No matches found." || content.is_empty() {
        return format!("> **Result (`{}`):** 0 file(s) match\n\n", func_name);
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
    let total = data
        .get("total")
        .and_then(|f| f.as_u64())
        .unwrap_or(files.len() as u64);
    let truncated = data
        .get("truncated")
        .and_then(|f| f.as_bool())
        .unwrap_or(false);
    let mut msg = format!(
        "> **Result (`{}`):** {} file(s) match ({} match(es) total)\n\n",
        func_name,
        files.len(),
        total
    );
    if truncated {
        msg.push_str(
            "> **Note:** the result was truncated to 200 matches. Refine the query with narrower terms or delegate to a sub-agent to analyse a specific file.\n\n",
        );
    }
    msg
}

fn format_email_by_id_result(func_name: &str, data: &serde_json::Value) -> String {
    let content = data.get("result").and_then(|f| f.as_str()).unwrap_or("");
    format!(
        "> **Result (`{}`):** {} line(s) read.\n\n",
        func_name,
        content.lines().count()
    )
}

fn format_search_email_result(func_name: &str, data: &serde_json::Value) -> String {
    let total = data.get("total").and_then(|t| t.as_u64()).unwrap_or(0);
    let cursor = data.get("cursor").and_then(|c| c.as_str()).unwrap_or("");
    let hint = data.get("hint").and_then(|h| h.as_str()).unwrap_or("");
    // Count items on this page by parsing the JSON-serialized
    // `results` array. If parsing fails (e.g. the "No matching
    // emails found." sentinel on empty results), fall back to 0.
    let page_items = data
        .get("results")
        .and_then(|r| r.as_str())
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        .and_then(|v| v.as_array().map(|a| a.len()))
        .unwrap_or(0);

    let mut msg = format!(
        "> **Result (`{}`):** {} item(s) found. Page: {} item(s).",
        func_name, total, page_items
    );

    if !cursor.is_empty() {
        // More pages remain. Surface the cursor so the LLM (and a
        // human operator inspecting the transcript) can see what to
        // pass back.
        msg.push_str(&format!(" More pages remain. Cursor: `{cursor}`."));
    } else if !hint.is_empty() {
        // Final page or empty result; the hint already ends with a
        // period ("Final page." / "No matching emails found.").
        msg.push_str(&format!(" {hint}"));
    } else {
        // No cursor, no hint: all items fit on a single page.
        msg.push_str(" All results on this page.");
    }
    msg.push_str("\n\n");
    msg
}

fn format_generic_search_result(func_name: &str, data: &serde_json::Value) -> String {
    let extract_len = |val: &serde_json::Value| -> Option<usize> {
        if let Some(arr) = val.as_array() {
            return Some(arr.len());
        }
        if let Some(arr) = val.get("results").and_then(|r| r.as_array()) {
            return Some(arr.len());
        }
        None
    };

    let count = data
        .get("results")
        .and_then(|r| {
            if let Some(s) = r.as_str() {
                serde_json::from_str::<serde_json::Value>(s).ok()
            } else {
                Some(r.clone())
            }
        })
        .and_then(|v| extract_len(&v))
        .unwrap_or_else(|| extract_len(data).unwrap_or(0));

    format!(
        "> **Result (`{}`):** {} item(s) found\n\n",
        func_name, count
    )
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
        assert!(msg.contains("read_file"));
        assert!(msg.contains("not found"));
    }

    #[test]
    fn test_format_result_create_file() {
        let result = r#"{"status":"success","data":{"size_bytes":42}}"#;
        let msg = format_tool_result_message("create_file", result);
        assert!(msg.contains("create_file"));
        assert!(msg.contains("42 B"));
    }

    #[test]
    fn test_format_result_read_tags() {
        let result = r#"{"status":"success","data":{"tags":["a","b"]}}"#;
        let msg = format_tool_result_message("read_tags", result);
        assert!(msg.contains("read_tags"));
        assert!(msg.contains("2 tag(s)"));
    }

    #[test]
    fn test_format_result_read_file() {
        let result = r#"{"status":"success","data":{"content":"line1\nline2"}}"#;
        let msg = format_tool_result_message("read_file", result);
        assert!(msg.contains("read_file"));
        assert!(msg.contains("2 line(s)"));
    }

    #[test]
    fn test_format_result_grep_no_matches() {
        let result = r#"{"status":"success","data":{"matches":"No matches found."}}"#;
        let msg = format_tool_result_message("grep", result);
        assert!(msg.contains("grep"));
        assert!(msg.contains("0 file(s)"));
    }

    #[test]
    fn test_format_result_generic_search() {
        let result = r#"{"status":"success","data":{"results":"[{\"a\":1}]"}}"#;
        let msg = format_tool_result_message("search_calendar", result);
        assert!(msg.contains("search_calendar"));
        assert!(msg.contains("1 item(s)"));
    }

    #[test]
    fn test_format_result_search_contact_nested_object() {
        let contact_json = r#"{"results":[{"href":"/logan.vcf","name":"Logan"}],"errors":[]}"#;
        let result = serde_json::json!({
            "status": "success",
            "data": {
                "results": contact_json
            }
        })
        .to_string();
        let msg = format_tool_result_message("search_contact", &result);
        assert!(msg.contains("search_contact"));
        assert!(msg.contains("1 item(s)"));
    }

    #[test]
    fn test_format_result_unknown_tool_long_result() {
        let result = "x".repeat(200);
        let msg = format_tool_result_message("some_tool", &result);
        assert!(msg.contains("some_tool"));
        assert!(msg.contains("some tool"));
        assert!(msg.contains("completed successfully"));
    }

    #[test]
    fn test_format_result_unknown_tool_short_result() {
        let msg = format_tool_result_message("some_tool", "ok");
        assert!(msg.contains("some_tool"));
        assert!(msg.contains("ok"));
    }

    #[test]
    fn test_format_result_get_email_by_id() {
        let result = r#"{"status":"success","data":{"result":"line1\nline2\nline3"}}"#;
        let msg = format_tool_result_message("get_email_by_id", result);
        assert!(msg.contains("get_email_by_id"));
        assert!(msg.contains("3 line(s) read"));
    }

    // `search_email` paging display: the result header should
    // show the cross-page total, the items on this page, and either
    // the cursor (more pages) or the "Final page." hint (last page).
    // See `format_search_email_result`.

    fn search_email_page(items: usize) -> String {
        // Build a synthetic `results` JSON array of N empty objects.
        let entries: Vec<serde_json::Value> = (0..items).map(|_| serde_json::json!({})).collect();
        serde_json::Value::Array(entries).to_string()
    }

    #[test]
    fn test_format_result_search_email_first_page_with_cursor() {
        // 189 total, 100 items on this page, cursor present.
        let result = serde_json::json!({
            "status": "success",
            "data": {
                "results": search_email_page(100),
                "total": 189,
                "cursor": "550e8400-e29b-41d4-a716-446655440000"
            }
        })
        .to_string();
        let msg = format_tool_result_message("search_email", &result);
        assert!(msg.contains("search_email"));
        assert!(msg.contains("189 item(s) found"));
        assert!(msg.contains("Page: 100 item(s)"));
        assert!(msg.contains("More pages remain"));
        assert!(msg.contains("550e8400-e29b-41d4-a716-446655440000"));
        // No "Final page." on a non-final page.
        assert!(!msg.contains("Final page"));
    }

    #[test]
    fn test_format_result_search_email_final_page_with_hint() {
        // 189 total, 89 items on this page, no cursor, final-page hint.
        let result = serde_json::json!({
            "status": "success",
            "data": {
                "results": search_email_page(89),
                "total": 189,
                "cursor": null,
                "hint": "Final page."
            }
        })
        .to_string();
        let msg = format_tool_result_message("search_email", &result);
        assert!(msg.contains("search_email"));
        assert!(msg.contains("189 item(s) found"));
        assert!(msg.contains("Page: 89 item(s)"));
        assert!(msg.contains("Final page"));
        // No "More pages remain" on the final page.
        assert!(!msg.contains("More pages remain"));
    }

    #[test]
    fn test_format_result_search_email_single_page_no_paging() {
        // 5 total, 5 items on this page, no cursor, no hint.
        let result = serde_json::json!({
            "status": "success",
            "data": {
                "results": search_email_page(5),
                "total": 5
            }
        })
        .to_string();
        let msg = format_tool_result_message("search_email", &result);
        assert!(msg.contains("5 item(s) found"));
        assert!(msg.contains("Page: 5 item(s)"));
        assert!(msg.contains("All results on this page"));
        assert!(!msg.contains("More pages remain"));
        assert!(!msg.contains("Final page"));
    }

    #[test]
    fn test_format_result_search_email_empty_result() {
        // Empty result: hint carries the human-readable message.
        let result = serde_json::json!({
            "status": "success",
            "data": {
                "results": "No matching emails found.",
                "total": 0,
                "cursor": null,
                "hint": "No matching emails found."
            }
        })
        .to_string();
        let msg = format_tool_result_message("search_email", &result);
        assert!(msg.contains("0 item(s) found"));
        // The page count falls back to 0 when `results` is not a
        // JSON array (it's the "No matching emails found." sentinel).
        assert!(msg.contains("Page: 0 item(s)"));
        assert!(msg.contains("No matching emails found"));
    }
}
