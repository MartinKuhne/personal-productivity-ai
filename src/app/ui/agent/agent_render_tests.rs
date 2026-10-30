//! Tests for `ui/agent/agent_render.rs`.

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

#[test]
fn test_format_result_get_email_by_id_json_body() {
    let json_email = serde_json::json!({
        "id": "e1",
        "subject": "Status Update",
        "date": "2026-09-09T12:00:00Z",
        "from": ["Alice <alice@example.com>"],
        "to": ["Bob <bob@example.com>"],
        "preview": "Line 1",
        "body": "Line 1\nLine 2\nLine 3\nLine 4\nLine 5"
    });

    let tool_result = serde_json::json!({
        "status": "success",
        "data": {
            "result": serde_json::to_string_pretty(&json_email).unwrap()
        }
    });

    let msg = format_tool_result_message("get_email_by_id", &tool_result.to_string());
    assert!(msg.contains("get_email_by_id"));
    assert!(msg.contains("5 line(s) read"));
    assert!(!msg.contains("13 line(s) read"));
}

#[test]
fn test_format_result_get_email_by_id_empty_body() {
    let json_email = serde_json::json!({
        "id": "e2",
        "body": ""
    });

    let tool_result = serde_json::json!({
        "status": "success",
        "data": {
            "result": serde_json::to_string_pretty(&json_email).unwrap()
        }
    });

    let msg = format_tool_result_message("get_email_by_id", &tool_result.to_string());
    assert!(msg.contains("0 line(s) read"));
}

#[test]
fn test_format_result_get_email_by_id_yaml_frontmatter() {
    let markdown = "---\nid: e1\nsubject: Status Update\ndate: '2026-09-09T12:00:00Z'\nfrom:\n  - Alice <alice@example.com>\nto:\n  - Bob <bob@example.com>\n---\n\nLine 1\nLine 2\nLine 3\nLine 4\nLine 5";
    let tool_result = serde_json::json!({
        "status": "success",
        "data": {
            "result": markdown
        }
    });

    let msg = format_tool_result_message("get_email_by_id", &tool_result.to_string());
    assert!(msg.contains("get_email_by_id"));
    assert!(msg.contains("5 line(s) read"));
    assert!(!msg.contains("12 line(s) read"));
}

#[test]
fn test_format_result_get_email_by_id_yaml_frontmatter_empty_body() {
    let markdown = "---\nid: e2\nsubject: Empty\n---\n";
    let tool_result = serde_json::json!({
        "status": "success",
        "data": {
            "result": markdown
        }
    });

    let msg = format_tool_result_message("get_email_by_id", &tool_result.to_string());
    assert!(msg.contains("0 line(s) read"));
}

// `search_email` paging display: the result header should
// show the cross-page total, the items on this page, and either
// the cursor (more pages) or the "Final page." hint (last page).
// See `format_search_email_result`.

fn search_email_page(items: usize) -> serde_json::Value {
    // Build a synthetic `results` JSON array of N empty objects.
    let entries: Vec<serde_json::Value> = (0..items).map(|_| serde_json::json!({})).collect();
    serde_json::Value::Array(entries)
}

/// A paging-display case for `search_email`: the exact substrings that must
/// appear (and must not appear) in the rendered summary line.
struct SearchEmailCase {
    name: &'static str,
    items: usize,
    total: u64,
    cursor: Option<&'static str>,
    hint: Option<&'static str>,
    expected: &'static [&'static str],
    forbidden: &'static [&'static str],
}

#[test]
fn test_format_result_search_email_paging_display() {
    let cases = [
        SearchEmailCase {
            // 189 total, 100 items on this page, cursor present.
            name: "first page with cursor",
            items: 100,
            total: 189,
            cursor: Some("550e8400-e29b-41d4-a716-446655440000"),
            hint: None,
            expected: &[
                "search_email",
                "189 item(s) found",
                "Page: 100 item(s)",
                "More pages remain",
                "550e8400-e29b-41d4-a716-446655440000",
            ],
            // No "Final page." on a non-final page.
            forbidden: &["Final page"],
        },
        SearchEmailCase {
            // 189 total, 89 items on this page, no cursor, final-page hint.
            name: "final page with hint",
            items: 89,
            total: 189,
            cursor: None,
            hint: Some("Final page."),
            expected: &[
                "search_email",
                "189 item(s) found",
                "Page: 89 item(s)",
                "Final page",
            ],
            // No "More pages remain" on the final page.
            forbidden: &["More pages remain"],
        },
        SearchEmailCase {
            // 5 total, 5 items on this page, no cursor, no hint.
            name: "single page without paging",
            items: 5,
            total: 5,
            cursor: None,
            hint: None,
            expected: &[
                "5 item(s) found",
                "Page: 5 item(s)",
                "All results on this page",
            ],
            forbidden: &["More pages remain", "Final page"],
        },
        SearchEmailCase {
            // Empty result: hint carries the human-readable message; `results`
            // is an empty structured array.
            name: "empty result",
            items: 0,
            total: 0,
            cursor: None,
            hint: Some("No matching emails found."),
            expected: &[
                "0 item(s) found",
                // An empty `results` array yields a page count of 0.
                "Page: 0 item(s)",
                "No matching emails found",
            ],
            forbidden: &[],
        },
    ];

    for case in cases {
        let result = serde_json::json!({
            "status": "success",
            "data": {
                "results": search_email_page(case.items),
                "total": case.total,
                "cursor": case.cursor,
                "hint": case.hint,
            }
        })
        .to_string();

        let msg = format_tool_result_message("search_email", &result);
        for needle in case.expected {
            assert!(
                msg.contains(needle),
                "[{}] expected {msg:?} to contain {needle:?}",
                case.name
            );
        }
        for needle in case.forbidden {
            assert!(
                !msg.contains(needle),
                "[{}] expected {msg:?} not to contain {needle:?}",
                case.name
            );
        }
    }
}

// `web_fetch` cursor paging display: the result header should
// show the total lines, lines on this page, and either
// the cursor (more pages) or the "Final page." hint (last page).
// See `format_tool_result_message` for "web_fetch" branch.

/// A paging-display case for the `web_fetch` branch: the exact substrings that
/// must appear (and must not appear) in the rendered summary line.
struct WebFetchCase {
    name: &'static str,
    content: &'static str,
    total_lines: u64,
    cursor: Option<&'static str>,
    hint: Option<&'static str>,
    from_cache: bool,
    expected: &'static [&'static str],
    forbidden: &'static [&'static str],
}

#[test]
fn test_format_result_web_fetch_paging_display() {
    let cases = [
        WebFetchCase {
            // 500 total lines, 100 lines on this page, cursor present.
            name: "first page with cursor",
            content: "line1\nline2\nline3",
            total_lines: 500,
            cursor: Some("550e8400-e29b-41d4-a716-446655440000"),
            hint: None,
            from_cache: false,
            expected: &[
                "web_fetch",
                "3 of 500 markdown lines returned",
                "More pages remain",
                "550e8400-e29b-41d4-a716-446655440000",
            ],
            // No "Final page." on a non-final page.
            forbidden: &["Final page"],
        },
        WebFetchCase {
            // 150 total lines, 50 lines on this page, no cursor, final-page hint.
            name: "final page with hint",
            content: "line1\nline2",
            total_lines: 150,
            cursor: None,
            hint: Some("Final page."),
            from_cache: true,
            expected: &[
                "web_fetch",
                "2 of 150 markdown lines returned",
                "(cached)",
                "Final page",
            ],
            // No "More pages remain" on the final page.
            forbidden: &["More pages remain"],
        },
        WebFetchCase {
            // 5 total lines, 5 lines on this page, no cursor, no hint.
            name: "single page without paging",
            content: "line1\nline2\nline3\nline4\nline5",
            total_lines: 5,
            cursor: None,
            hint: None,
            from_cache: false,
            expected: &["5 of 5 markdown lines returned", "All content on this page"],
            forbidden: &["More pages remain", "Final page"],
        },
    ];

    for case in cases {
        let result = serde_json::json!({
            "status": "success",
            "data": {
                "content": case.content,
                "total_lines": case.total_lines,
                "cursor": case.cursor,
                "hint": case.hint,
                "from_cache": case.from_cache,
            }
        })
        .to_string();

        let msg = format_tool_result_message("web_fetch", &result);
        for needle in case.expected {
            assert!(
                msg.contains(needle),
                "[{}] expected {msg:?} to contain {needle:?}",
                case.name
            );
        }
        for needle in case.forbidden {
            assert!(
                !msg.contains(needle),
                "[{}] expected {msg:?} not to contain {needle:?}",
                case.name
            );
        }
    }
}

#[test]
fn test_format_result_web_fetch_preserves_count_with_trailing_empty_line() {
    // 64 lines where line 64 is an empty line (common between markdown blocks).
    // Joining with "\n" creates a trailing newline. The count must be 64, not 63.
    let mut lines: Vec<&str> = (0..63).map(|_| "paragraph text").collect();
    lines.push("");
    let content = lines.join("\n");

    let result = serde_json::json!({
        "status": "success",
        "data": {
            "content": content,
            "total_lines": 3089,
            "cursor": "c_4a8b2f9c",
            "hint": null,
            "from_cache": false
        }
    })
    .to_string();

    let msg = format_tool_result_message("web_fetch", &result);
    assert!(msg.contains("64 of 3089 markdown lines returned"));
    assert!(msg.contains("More pages remain"));
    assert!(msg.contains("c_4a8b2f9c"));
}

// ---------------------------------------------------------------------------
// `split_thinking_and_content` — delimiter position matrix
// ---------------------------------------------------------------------------

#[test]
fn test_split_thinking_delimiter_positions() {
    let cases: Vec<(&str, &str, &str, &str)> = vec![
        ("empty input", "", "", ""),
        (
            "delimiter pair at the very start",
            "\u{1f914}thinking\u{1f914}",
            "thinking",
            "",
        ),
        (
            "lone trailing delimiter is kept verbatim",
            "no closing\u{1f914}",
            "",
            "no closing\u{1f914}",
        ),
        (
            "only the first delimiter pair is stripped",
            "A\u{1f914}B\u{1f914}C\u{1f914}D",
            "B",
            "AC\u{1f914}D",
        ),
        (
            "thinking may span several lines",
            "out\u{1f914}l1\nl2\u{1f914}rest",
            "l1\nl2",
            "outrest",
        ),
    ];

    for (name, input, want_thinking, want_content) in cases {
        let (thinking, content) = split_thinking_and_content(input);
        assert_eq!(thinking, want_thinking, "[{name}] thinking text");
        assert_eq!(content, want_content, "[{name}] remaining content");
    }
}

// ---------------------------------------------------------------------------
// `format_tool_call_message`
// ---------------------------------------------------------------------------

#[test]
fn test_format_tool_call_create_note_path_variants() {
    let cases: Vec<(&str, &str, &str)> = vec![
        (
            "a string path is rendered inline",
            r#"{"path":"lib/test.md"}"#,
            "> **Executing tool `create_note`**\n> Path: `lib/test.md`\n",
        ),
        (
            "missing path key",
            "{}",
            "> **Executing tool `create_note`**\n",
        ),
        (
            "a non-string path is ignored",
            r#"{"path":42}"#,
            "> **Executing tool `create_note`**\n",
        ),
        (
            "arguments that are not JSON are ignored",
            "not json",
            "> **Executing tool `create_note`**\n",
        ),
        (
            "empty argument string",
            "",
            "> **Executing tool `create_note`**\n",
        ),
    ];

    for (name, args, expected) in cases {
        assert_eq!(
            format_tool_call_message("create_note", args),
            expected,
            "[{name}]"
        );
    }
}

#[test]
fn test_format_tool_call_generic_args_are_quoted_line_by_line() {
    let cases: Vec<(&str, &str, &str)> = vec![
        (
            "an object is pretty printed",
            r#"{"pattern":"hello"}"#,
            "> **Executing tool `search_notes`**\n> {\n>   \"pattern\": \"hello\"\n> }\n",
        ),
        (
            "an array is pretty printed",
            r#"["a","b"]"#,
            "> **Executing tool `search_notes`**\n> [\n>   \"a\",\n>   \"b\"\n> ]\n",
        ),
        (
            "invalid JSON is echoed verbatim",
            "not json",
            "> **Executing tool `search_notes`**\n> not json\n",
        ),
        (
            "a scalar JSON argument keeps its JSON quoting",
            "\"plain\"",
            "> **Executing tool `search_notes`**\n> \"plain\"\n",
        ),
        (
            "no arguments produces an empty quote block",
            "",
            "> **Executing tool `search_notes`**\n\n",
        ),
    ];

    for (name, args, expected) in cases {
        assert_eq!(
            format_tool_call_message("search_notes", args),
            expected,
            "[{name}]"
        );
    }
}

// ---------------------------------------------------------------------------
// `format_delegate_trace`
// ---------------------------------------------------------------------------

/// A delegate sub-agent call with a null result, for trace-rendering tests.
fn delegate_call(name: &str, args: serde_json::Value) -> DelegateToolCall {
    DelegateToolCall {
        name: name.to_string(),
        args,
        result: serde_json::Value::Null,
    }
}

#[test]
fn test_format_delegate_trace_is_empty_for_no_calls() {
    assert_eq!(
        format_delegate_trace(&[]),
        "",
        "an empty trace renders nothing"
    );
}

#[test]
fn test_format_delegate_trace_renders_each_call_in_order() {
    let out = format_delegate_trace(&[
        delegate_call("create_note", serde_json::json!({ "path": "notes/a.md" })),
        delegate_call("create_note", serde_json::json!({})),
        delegate_call(
            "web_fetch",
            serde_json::json!({ "url": "https://example.com" }),
        ),
    ]);

    let expected = concat!(
        "<span>**Executing tool `create_note`** — Path: `notes/a.md`</span>\n",
        "<span>**Executing tool `create_note`**</span>\n",
        "<span>**Executing tool `web_fetch`**\n",
        "{\n",
        "  \"url\": \"https://example.com\"\n",
        "}\n",
        "</span>\n\n",
    );
    assert_eq!(out, expected, "each call must render in the input order");
}

// ---------------------------------------------------------------------------
// `format_tool_result_message` — branches not covered above
// ---------------------------------------------------------------------------

/// Wrap a success payload the way the tool layer returns it.
fn success(data: serde_json::Value) -> String {
    serde_json::json!({ "status": "success", "data": data }).to_string()
}

#[test]
fn test_format_result_list_notes_counts() {
    let cases: Vec<(&str, &str, serde_json::Value, &str)> = vec![
        (
            "files array with an explicit total",
            "list_notes",
            serde_json::json!({ "files": ["a", "b"], "total": 5 }),
            "> **Result (`list_notes`):** 2 notes returned (total: 5).\n\n",
        ),
        (
            "total falls back to the page count",
            "list_notes_by_tag",
            serde_json::json!({ "files": ["a", "b"] }),
            "> **Result (`list_notes_by_tag`):** 2 notes returned (total: 2).\n\n",
        ),
        (
            "a non-array files field counts as zero",
            "list_notes",
            serde_json::json!({ "files": "nope", "total": 5 }),
            "> **Result (`list_notes`):** 0 notes returned (total: 5).\n\n",
        ),
        (
            "empty data",
            "list_notes",
            serde_json::json!({}),
            "> **Result (`list_notes`):** 0 notes returned (total: 0).\n\n",
        ),
    ];

    for (name, func, data, expected) in cases {
        assert_eq!(
            format_tool_result_message(func, &success(data)),
            expected,
            "[{name}]"
        );
    }
}

#[test]
fn test_format_result_window_note_counts_lines() {
    let cases: Vec<(&str, serde_json::Value, &str)> = vec![
        (
            "three content lines",
            serde_json::json!({ "content": "a\nb\nc" }),
            "3 line(s) read",
        ),
        (
            "missing content counts zero",
            serde_json::json!({}),
            "0 line(s) read",
        ),
        (
            "non-string content counts zero",
            serde_json::json!({ "content": 7 }),
            "0 line(s) read",
        ),
    ];

    for (name, data, needle) in cases {
        let msg = format_tool_result_message("window_note", &success(data));
        assert!(
            msg.contains(needle),
            "[{name}] expected {msg:?} to contain {needle:?}"
        );
        assert!(
            msg.contains("window_note"),
            "[{name}] the tool name must be echoed"
        );
    }
}

#[test]
fn test_format_result_web_search_counts_blank_line_separated_blocks() {
    let cases: Vec<(&str, serde_json::Value, &str)> = vec![
        (
            "three blocks",
            serde_json::json!({ "results": "r1\n\nr2\n\nr3" }),
            "> **Result (`web_search`):** 3 search results returned.\n\n",
        ),
        (
            "whitespace-only blocks are skipped",
            serde_json::json!({ "results": "  \n\n real \n\n" }),
            "> **Result (`web_search`):** 1 search results returned.\n\n",
        ),
        (
            "an empty payload has no results",
            serde_json::json!({ "results": "" }),
            "> **Result (`web_search`):** 0 search results returned.\n\n",
        ),
        (
            "a missing results key has no results",
            serde_json::json!({}),
            "> **Result (`web_search`):** 0 search results returned.\n\n",
        ),
        (
            "a non-string results field is ignored",
            serde_json::json!({ "results": ["a", "b"] }),
            "> **Result (`web_search`):** 0 search results returned.\n\n",
        ),
    ];

    for (name, data, expected) in cases {
        assert_eq!(
            format_tool_result_message("web_search", &success(data)),
            expected,
            "[{name}]"
        );
    }
}

#[test]
fn test_format_result_search_notes_counts_distinct_files() {
    let cases: Vec<(&str, serde_json::Value, &str)> = vec![
        (
            "matches in the same file are counted once",
            serde_json::json!({
                "matches": "notes/a.md:3: hit\nnotes/a.md:9: hit\nnotes/b.md:1: hit",
                "total": 3
            }),
            "> **Result (`search_notes`):** 2 file(s) match (3 match(es) total)\n\n",
        ),
        (
            "the .markdown extension is recognised",
            serde_json::json!({ "matches": "notes/a.markdown:3: hit", "total": 1 }),
            "> **Result (`search_notes`):** 1 file(s) match (1 match(es) total)\n\n",
        ),
        (
            "other extensions split at the line-number separator",
            serde_json::json!({ "matches": "notes/a.txt:3: hit", "total": 1 }),
            "> **Result (`search_notes`):** 1 file(s) match (1 match(es) total)\n\n",
        ),
        (
            "total defaults to the number of files",
            serde_json::json!({ "matches": "notes/a.md:3: hit" }),
            "> **Result (`search_notes`):** 1 file(s) match (1 match(es) total)\n\n",
        ),
        (
            "the no-match sentinel short-circuits",
            serde_json::json!({ "matches": "No matches found.", "total": 0 }),
            "> **Result (`search_notes`):** 0 file(s) match\n\n",
        ),
        (
            "an empty payload short-circuits even with a total",
            serde_json::json!({ "matches": "", "total": 9 }),
            "> **Result (`search_notes`):** 0 file(s) match\n\n",
        ),
    ];

    for (name, data, expected) in cases {
        assert_eq!(
            format_tool_result_message("search_notes", &success(data)),
            expected,
            "[{name}]"
        );
    }
}

#[test]
fn test_format_result_search_notes_appends_the_truncation_note() {
    let msg = format_tool_result_message(
        "search_notes",
        &success(serde_json::json!({
            "matches": "notes/a.md:3: hit",
            "total": 250,
            "truncated": true,
        })),
    );

    assert!(
        msg.starts_with("> **Result (`search_notes`):** 1 file(s) match (250 match(es) total)\n\n"),
        "the summary must come first: {msg:?}"
    );
    assert!(
        msg.contains("> **Note:** the result was truncated to 200 matches."),
        "the truncation note must be rendered: {msg:?}"
    );
    assert!(
        msg.ends_with("\n\n"),
        "the note must close the block: {msg:?}"
    );
}

/// D1c: the file key was cut at the **first** `':'`, so every Windows path
/// collapsed to its drive letter, and the `.md:` probe cut at the **last**
/// occurrence, so a `other.md:9` reference quoted inside a matching line was
/// taken for the path. Both made the distinct-file count wrong.
#[test]
fn defect_01c_search_notes_counts_windows_paths_and_quoted_references_correctly() {
    let cases: Vec<(&str, serde_json::Value, &str)> = vec![
        (
            "two drive-letter matches in different files are two files",
            serde_json::json!({
                "matches": "C:\\notes\\a.txt:1 - hit\nC:\\notes\\b.txt:2 - hit",
                "total": 2
            }),
            "> **Result (`search_notes`):** 2 file(s) match (2 match(es) total)\n\n",
        ),
        (
            "a drive-letter path without a line number survives",
            serde_json::json!({
                "matches": "C:\\notes\\a.md\nC:\\notes\\b.md",
                "total": 2
            }),
            "> **Result (`search_notes`):** 2 file(s) match (2 match(es) total)\n\n",
        ),
        (
            "a drive-letter path with a line number still counts once",
            serde_json::json!({
                "matches": "C:\\notes\\n.md:12 - hit\nC:\\notes\\n.md:31 - hit",
                "total": 2
            }),
            "> **Result (`search_notes`):** 1 file(s) match (2 match(es) total)\n\n",
        ),
        (
            "a POSIX path with a line number still counts once",
            serde_json::json!({
                "matches": "/srv/notes/a.md:3 - hit\n/srv/notes/a.md:9 - hit",
                "total": 2
            }),
            "> **Result (`search_notes`):** 1 file(s) match (2 match(es) total)\n\n",
        ),
        (
            "a .md:9 reference quoted inside the text is not a second file",
            serde_json::json!({
                "matches": "Docs/a.md:1 - see Docs/a.md:9\nDocs/a.md:9 - the real line",
                "total": 2
            }),
            "> **Result (`search_notes`):** 1 file(s) match (2 match(es) total)\n\n",
        ),
        (
            "a malformed line without a path contributes no file",
            serde_json::json!({ "matches": ":3 - no path at all", "total": 1 }),
            "> **Result (`search_notes`):** 0 file(s) match (1 match(es) total)\n\n",
        ),
    ];

    for (name, data, expected) in cases {
        assert_eq!(
            format_tool_result_message("search_notes", &success(data)),
            expected,
            "[{name}]"
        );
    }
}

/// D1c: the path part of a match line ends at the first `':'` that is followed
/// by an ASCII digit — the line-number separator of `{path}:{line} - {text}`.
#[test]
fn defect_01c_match_path_part_splits_at_the_line_number_separator() {
    let cases: Vec<(&str, &str)> = vec![
        ("Docs/a.md:3 - hit", "Docs/a.md"),
        ("notes/a.markdown:3: hit", "notes/a.markdown"),
        ("notes/a.txt:3: hit", "notes/a.txt"),
        (r"C:\notes\n.md:12 - text", r"C:\notes\n.md"),
        (r"C:\notes\n.md", r"C:\notes\n.md"),
        (r"C:\notes\data.csv:12 - hit", r"C:\notes\data.csv"),
        ("/srv/notes/a.md:3 - hit", "/srv/notes/a.md"),
        ("Docs/a.md:3 - see Docs/b.md:9", "Docs/a.md"),
        ("no colon at all", "no colon at all"),
        ("path:x - not a line number", "path:x - not a line number"),
        (":3 - a malformed line has no path", ""),
    ];

    for (line, expected) in cases {
        assert_eq!(match_path_part(line), expected, "[{line:?}]");
    }
}

#[test]
fn test_format_result_error_without_a_message_uses_the_placeholder() {
    for (name, payload) in [
        ("no message field", r#"{"status":"error"}"#),
        ("a non-string message", r#"{"status":"error","message":42}"#),
    ] {
        assert_eq!(
            format_tool_result_message("read_note", payload),
            "> **Result Error (`read_note`):** Unknown error\n\n",
            "[{name}]"
        );
    }
}

#[test]
fn test_format_result_success_without_data_renders_empty_payloads() {
    assert_eq!(
        format_tool_result_message("create_note", r#"{"status":"success"}"#),
        "> **Result (`create_note`):** File created (0 B).\n\n",
        "a missing size is reported as zero bytes"
    );
}

#[test]
fn test_format_result_ignores_data_when_status_is_not_success() {
    let payload =
        serde_json::json!({ "status": "pending", "data": { "content": "a\nb" } }).to_string();

    assert_eq!(
        format_tool_result_message("read_note", &payload),
        "> **Result (`read_note`):** 0 line(s) read.\n\n",
        "only status=success lifts the data payload"
    );
}

#[test]
fn test_format_result_unknown_tool_echo_limit() {
    let exactly_99 = "x".repeat(99);
    let exactly_100 = "x".repeat(100);

    assert_eq!(
        format_tool_result_message("some_tool", &exactly_99),
        format!("> **Result (`some_tool`):** {exactly_99}\n\n"),
        "a 99-byte single-line result is echoed verbatim"
    );
    assert_eq!(
        format_tool_result_message("some_tool", &exactly_100),
        "> **Result (`some_tool`):** Tool 'some tool' completed successfully.\n\n",
        "a 100-byte result exceeds the echo limit"
    );
    assert_eq!(
        format_tool_result_message("some_tool", "line1\nline2"),
        "> **Result (`some_tool`):** Tool 'some tool' completed successfully.\n\n",
        "a multi-line result is never echoed verbatim"
    );
}

#[test]
fn test_format_result_get_email_by_id_prefers_the_body_field() {
    assert_eq!(
        format_tool_result_message(
            "get_email_by_id",
            &success(serde_json::json!({ "body": "a\nb\nc" })),
        ),
        "> **Result (`get_email_by_id`):** 3 line(s) read.\n\n",
        "a structured body is counted directly"
    );
    assert_eq!(
        format_tool_result_message(
            "get_email_by_id",
            &success(serde_json::json!({ "body": "" })),
        ),
        "> **Result (`get_email_by_id`):** 0 line(s) read.\n\n",
        "an empty body is zero lines"
    );
    assert_eq!(
        format_tool_result_message("get_email_by_id", &success(serde_json::json!({}))),
        "> **Result (`get_email_by_id`):** 0 line(s) read.\n\n",
        "a payload without body or result is zero lines"
    );
}

#[test]
fn test_format_result_get_email_by_id_front_matter_separators() {
    let cases = [
        (
            "LF blank line after the front matter",
            "---\nid: e1\n---\n\nLine 1\nLine 2",
            "2 line(s) read",
        ),
        (
            "CRLF blank line after the front matter",
            "---\nid: e1\n---\r\n\r\nLine 1\nLine 2",
            "2 line(s) read",
        ),
        (
            "a single LF separator",
            "---\nid: e1\n---\nLine 1\nLine 2",
            "2 line(s) read",
        ),
        (
            "a single CRLF separator",
            "---\nid: e1\n---\r\nLine 1\nLine 2",
            "2 line(s) read",
        ),
        (
            "a whitespace-only body counts as empty",
            "---\nid: e1\n---\n   \n",
            "0 line(s) read",
        ),
    ];

    for (name, raw, needle) in cases {
        let msg = format_tool_result_message(
            "get_email_by_id",
            &success(serde_json::json!({ "result": raw })),
        );
        assert!(
            msg.contains(needle),
            "[{name}] expected {msg:?} to contain {needle:?}"
        );
    }
}

#[test]
fn test_format_result_generic_search_fallbacks() {
    let cases: Vec<(&str, serde_json::Value, &str)> = vec![
        (
            "results is already a JSON array",
            serde_json::json!({ "results": [{ "a": 1 }, { "b": 2 }] }),
            "2 item(s) found",
        ),
        (
            "results is a JSON string holding an array",
            serde_json::json!({ "results": "[{\"a\":1}]" }),
            "1 item(s) found",
        ),
        (
            "results is a JSON string holding a nested results array",
            serde_json::json!({ "results": "{\"results\":[1,2,3]}" }),
            "3 item(s) found",
        ),
        (
            "results is not JSON",
            serde_json::json!({ "results": "not json" }),
            "0 item(s) found",
        ),
        (
            "results is an object without a nested array",
            serde_json::json!({ "results": { "a": 1 } }),
            "0 item(s) found",
        ),
        (
            "no results key at all",
            serde_json::json!({ "other": 1 }),
            "0 item(s) found",
        ),
    ];

    for (name, data, needle) in cases {
        let msg = format_tool_result_message("search_calendar", &success(data));
        assert!(
            msg.contains(needle),
            "[{name}] expected {msg:?} to contain {needle:?}"
        );
    }

    let bare_array = serde_json::json!({ "status": "success", "data": [1, 2, 3] }).to_string();
    assert!(
        format_tool_result_message("search_calendar", &bare_array).contains("3 item(s) found"),
        "a bare array payload is counted directly"
    );
}
