//! Tests for `agent/response_formatter.rs`.

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
    let msg = format_tool_call_message("create_note", r#"{"path":"lib/test.md"}"#);
    assert!(msg.contains("create_note"));
    assert!(msg.contains("lib/test.md"));
}

#[test]
fn test_format_tool_call_other() {
    let msg = format_tool_call_message("search_notes", r#"{"pattern":"hello"}"#);
    assert!(msg.contains("search_notes"));
    assert!(msg.contains("hello"));
}

#[test]
fn test_format_result_error() {
    let result = r#"{"status":"error","message":"not found"}"#;
    let msg = format_tool_result_message("read_note", result);
    assert!(msg.contains("Error"));
    assert!(msg.contains("read_note"));
    assert!(msg.contains("not found"));
}

#[test]
fn test_format_result_create_file() {
    let result = r#"{"status":"success","data":{"size_bytes":42}}"#;
    let msg = format_tool_result_message("create_note", result);
    assert!(msg.contains("create_note"));
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
    let msg = format_tool_result_message("read_note", result);
    assert!(msg.contains("read_note"));
    assert!(msg.contains("2 line(s)"));
}

#[test]
fn test_format_result_grep_no_matches() {
    let result = r#"{"status":"success","data":{"matches":"No matches found."}}"#;
    let msg = format_tool_result_message("search_notes", result);
    assert!(msg.contains("search_notes"));
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

// `web_fetch` cursor paging display: the result header should
// show the total lines, lines on this page, and either
// the cursor (more pages) or the "Final page." hint (last page).
// See `format_tool_result_message` for "web_fetch" branch.

#[test]
fn test_format_result_web_fetch_first_page_with_cursor() {
    // 500 total lines, 100 lines on this page, cursor present.
    let result = serde_json::json!({
        "status": "success",
        "data": {
            "content": "line1\nline2\nline3",
            "total_lines": 500,
            "cursor": "550e8400-e29b-41d4-a716-446655440000",
            "hint": null,
            "from_cache": false
        }
    })
    .to_string();
    let msg = format_tool_result_message("web_fetch", &result);
    assert!(msg.contains("web_fetch"));
    assert!(msg.contains("3 of 500 markdown lines returned"));
    assert!(msg.contains("More pages remain"));
    assert!(msg.contains("550e8400-e29b-41d4-a716-446655440000"));
    // No "Final page." on a non-final page.
    assert!(!msg.contains("Final page"));
}

#[test]
fn test_format_result_web_fetch_final_page_with_hint() {
    // 150 total lines, 50 lines on this page, no cursor, final-page hint.
    let result = serde_json::json!({
        "status": "success",
        "data": {
            "content": "line1\nline2",
            "total_lines": 150,
            "cursor": null,
            "hint": "Final page.",
            "from_cache": true
        }
    })
    .to_string();
    let msg = format_tool_result_message("web_fetch", &result);
    assert!(msg.contains("web_fetch"));
    assert!(msg.contains("2 of 150 markdown lines returned"));
    assert!(msg.contains("(cached)"));
    assert!(msg.contains("Final page"));
    // No "More pages remain" on the final page.
    assert!(!msg.contains("More pages remain"));
}

#[test]
fn test_format_result_web_fetch_single_page_no_paging() {
    // 5 total lines, 5 lines on this page, no cursor, no hint.
    let result = serde_json::json!({
        "status": "success",
        "data": {
            "content": "line1\nline2\nline3\nline4\nline5",
            "total_lines": 5,
            "cursor": null,
            "hint": null,
            "from_cache": false
        }
    })
    .to_string();
    let msg = format_tool_result_message("web_fetch", &result);
    assert!(msg.contains("5 of 5 markdown lines returned"));
    assert!(msg.contains("All content on this page"));
    assert!(!msg.contains("More pages remain"));
    assert!(!msg.contains("Final page"));
}
