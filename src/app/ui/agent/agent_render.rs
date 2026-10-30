//! Splits LLM thinking/reasoning blocks from final content and formats tool-call/result messages for the chat UI.
//!
//! Unit tests live in the sibling `agent_render_tests.rs` sidecar.

use crate::agent::events::DelegateToolCall;

/// Split an assistant message into its thinking block and its visible content.
///
/// The model wraps its reasoning in a pair of `U+1F914` (THINKING FACE)
/// delimiters: the text between the **first** pair becomes the thinking block,
/// and the text before the opening delimiter plus the text after the closing
/// delimiter are concatenated back into the returned content.
///
/// Precondition: `text` is the raw assistant message. When it contains fewer
/// than two delimiters the split is a no-op and the result is `("", text)` —
/// an empty thinking block and the message unchanged.
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

/// Render a tool invocation as a markdown block-quote for the chat transcript.
///
/// `func_args_str` is the raw JSON argument string the model emitted. For
/// `create_note` only the `path` argument is surfaced, and only when the
/// arguments parse as a JSON object carrying a string `path`; every other tool
/// gets its arguments pretty-printed with a `> ` prefix on each line, falling
/// back to the raw string when it is not valid JSON.
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

/// Format a list of [`DelegateToolCall`]s into `<span>`-wrapped HTML blocks,
/// rendering delegate sub-agent tool calls as muted gray text,
/// visually distinguishing them from the parent agent's own tool calls
/// (FR-014, SC-006).
pub fn format_delegate_trace(tool_calls: &[DelegateToolCall]) -> String {
    let mut out = String::new();
    for tc in tool_calls {
        if tc.name == "create_note" {
            out.push_str(&format!("<span>**Executing tool `{}`**", tc.name));
            if let Some(path) = tc.args.get("path").and_then(|p| p.as_str()) {
                out.push_str(&format!(" — Path: `{}`", path));
            }
            out.push_str("</span>\n");
            continue;
        }
        let formatted_args = match serde_json::to_string_pretty(&tc.args) {
            Ok(s) => s,
            Err(_) => tc.args.to_string(),
        };
        out.push_str(&format!(
            "<span>**Executing tool `{}`**\n{}\n</span>\n\n",
            tc.name, formatted_args
        ));
    }
    out
}

fn count_from_data(data: &serde_json::Value, field: &str) -> usize {
    data.get(field)
        .and_then(|f| f.as_array())
        .map(|a| a.len())
        .unwrap_or(0)
}

/// Render a tool result as a markdown block-quote for the chat transcript.
///
/// `result` is the tool's JSON envelope, `{"status": "success" | "error", …}`.
/// An `error` envelope short-circuits to its `message` (or `Unknown error` when
/// absent); only a `success` envelope contributes its `data` payload, so a
/// payload attached to any other status is ignored. A string that does not parse
/// as such an envelope is summarised from the raw text: a short single-line
/// result is echoed verbatim, anything longer renders a generic
/// "completed successfully" line. Recognised tools get a purpose-built summary
/// (counts, read lines, paging cursors) built from `data`.
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
            let returned = if content.is_empty() {
                0
            } else {
                content.split('\n').count()
            };

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

/// Split a `search_notes` match line into the path it was found in.
///
/// `search_notes` renders one match per line as `{path}:{line} - {text}`
/// (see `crate::tools::filesystem::tool_search_notes`), so the path ends at
/// the **first `':'` that is followed by an ASCII digit** — the line-number
/// separator.
///
/// Splitting on the first `':'` instead truncates a Windows path to its drive
/// letter (`C:\notes\n.md:12` → `C`), and splitting on the last `.md:`
/// occurrence swallows a `other.md:9` reference quoted inside the matching
/// text. A line without a line-number separator is a bare path.
fn match_path_part(line: &str) -> &str {
    for (idx, _) in line.match_indices(':') {
        if line[idx + 1..].starts_with(|c: char| c.is_ascii_digit()) {
            return &line[..idx];
        }
    }
    line
}

fn format_search_notes_result(func_name: &str, data: &serde_json::Value) -> String {
    let content = data.get("matches").and_then(|f| f.as_str()).unwrap_or("");
    if content == "No matches found." || content.is_empty() {
        return format!("> **Result (`{}`):** 0 file(s) match\n\n", func_name);
    }
    let mut files = std::collections::HashSet::new();
    for line in content.lines() {
        let path = match_path_part(line);
        if !path.is_empty() {
            files.insert(path);
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
    let line_count = if let Some(body) = data.get("body").and_then(|b| b.as_str()) {
        body.lines().count()
    } else {
        let raw = data.get("result").and_then(|f| f.as_str()).unwrap_or("");
        if let Some(fm) = crate::markdown::parse_front_matter(raw) {
            let body = fm
                .body
                .strip_prefix("\r\n\r\n")
                .or_else(|| fm.body.strip_prefix("\n\n"))
                .or_else(|| fm.body.strip_prefix("\r\n"))
                .or_else(|| fm.body.strip_prefix("\n"))
                .unwrap_or(&fm.body);
            if body.trim().is_empty() {
                0
            } else {
                body.lines().count()
            }
        } else if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(raw) {
            parsed
                .get("body")
                .and_then(|b| b.as_str())
                .map(|b| b.lines().count())
                .unwrap_or_else(|| raw.lines().count())
        } else {
            raw.lines().count()
        }
    };

    format!(
        "> **Result (`{}`):** {} line(s) read.\n\n",
        func_name, line_count
    )
}

fn format_search_email_result(func_name: &str, data: &serde_json::Value) -> String {
    let total = data.get("total").and_then(|t| t.as_u64()).unwrap_or(0);
    let cursor = data.get("cursor").and_then(|c| c.as_str()).unwrap_or("");
    let hint = data.get("hint").and_then(|h| h.as_str()).unwrap_or("");
    // Count items on this page from the structured JSON `results`
    // array. The empty result sentinel is an empty array.
    let page_items = data
        .get("results")
        .and_then(|r| r.as_array())
        .map(|a| a.len())
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
// Tests live in the sibling `agent_render_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "agent_render_tests.rs"]
mod tests;
