//! Splits LLM thinking/reasoning blocks from final content and formats tool-call/result messages for the chat UI.
//!
//! Unit tests live in the sibling `response_formatter_tests.rs` sidecar.

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
    if func_name == "create_note" {
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

/// Like [`format_tool_call_message`] but produces HTML `<span>` blocks
/// instead of markdown blockquotes. The HTML path renders as muted gray
/// text via [`InlineElem::Html`], visually distinguishing delegate
/// sub-agent tool calls from the parent agent's own.
pub fn format_delegate_tool_call_message(func_name: &str, func_args_str: &str) -> String {
    if func_name == "create_note" {
        let mut msg = format!("<span>**Executing tool `{}`**", func_name);
        if let Ok(args_val) = serde_json::from_str::<serde_json::Value>(func_args_str)
            && let Some(path) = args_val.get("path").and_then(|p| p.as_str())
        {
            msg.push_str(&format!(" — Path: `{}`", path));
        }
        msg.push_str("</span>\n");
        return msg;
    }
    let formatted_args = match serde_json::from_str::<serde_json::Value>(func_args_str) {
        Ok(val) => serde_json::to_string_pretty(&val).unwrap_or_else(|_| func_args_str.to_string()),
        Err(_) => func_args_str.to_string(),
    };
    format!(
        "<span>**Executing tool `{}`**\n{}\n</span>\n",
        func_name, formatted_args
    )
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
        "create_note" => {
            let size = result_data
                .get("size_bytes")
                .and_then(|s| s.as_u64())
                .unwrap_or(0);
            format!(
                "> **Result (`{}`):** File created ({} B).\n\n",
                func_name, size
            )
        }
        "list_notes" | "list_notes_by_tag" => {
            let count = count_from_data(&result_data, "files");
            let total = result_data
                .get("total")
                .and_then(|t| t.as_u64())
                .unwrap_or(count as u64);
            format!(
                "> **Result (`{}`):** {} notes returned (total: {}).\n\n",
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
        "read_note" | "window_note" => {
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
            let cursor = result_data
                .get("cursor")
                .and_then(|c| c.as_str())
                .unwrap_or("");
            let hint = result_data
                .get("hint")
                .and_then(|h| h.as_str())
                .unwrap_or("");
            let from_cache = result_data
                .get("from_cache")
                .and_then(|f| f.as_bool())
                .unwrap_or(false);
            let cache_tag = if from_cache { " (cached)" } else { "" };
            let returned = content.lines().count();

            let mut msg = format!(
                "> **Result (`{}`):** {} of {} markdown lines returned{}.",
                func_name, returned, total_lines, cache_tag
            );

            if !cursor.is_empty() {
                // More pages remain. Surface the cursor so the LLM (and a
                // human operator inspecting the transcript) can see what to
                // pass back.
                msg.push_str(&format!(" More pages remain. Cursor: `{cursor}`."));
            } else if !hint.is_empty() {
                // Final page or empty result; the hint already ends with a
                // period ("Final page.").
                msg.push_str(&format!(" {hint}"));
            } else {
                // No cursor, no hint: all content fits on a single page.
                msg.push_str(" All content on this page.");
            }
            msg.push_str("\n\n");
            msg
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
        "search_notes" => format_search_notes_result(func_name, &result_data),
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

fn format_search_notes_result(func_name: &str, data: &serde_json::Value) -> String {
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

// ---------------------------------------------------------------------------
// Tests live in the sibling `response_formatter_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "response_formatter_tests.rs"]
mod tests;
