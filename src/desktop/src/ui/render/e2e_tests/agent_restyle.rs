//! US3 restyle test — verifies that tool-call display formatting lives in
//! the UI layer (`ui/render/agent_render.rs`), not in `agent/`.
//!
//! Quickstart scenario 8 / SC-002: a maintainer can change a formatting
//! rule in `agent_render.rs` and see the rendered output change without
//! editing any file under `src/desktop/src/agent/`.

#![cfg(test)]

use crate::agent::events::{AgentEvent, DelegateToolCall};
use crate::ui::agent::transcript::AgentTranscript;
use serde_json::json;
use uuid::Uuid;

/// T033: Tool-call formatting is entirely UI-side. The transcript formats
/// `ToolCallStarted`/`ToolResult` events via `agent_render.rs` functions
/// — no `agent/` module is involved in formatting.
#[test]
fn test_tool_call_formatting_is_ui_side() {
    let id = Uuid::new_v4();
    let mut t = AgentTranscript::new(id);

    t.apply_event(&AgentEvent::ToolCallStarted {
        session_id: id,
        id: "call_1".into(),
        name: "create_note".into(),
        args: json!({"path": "lib/test.md"}),
    });

    // The `> **Executing tool ...**` format comes from
    // `ui::render::agent_render::format_tool_call_message` — a UI-only
    // function. Changing it there changes the rendered output with zero
    // edits to `src/desktop/src/agent/`.
    assert!(
        t.content.contains("> **Executing tool `create_note`**"),
        "Expected UI-formatted tool call, got: {}",
        t.content
    );
    assert!(
        t.content.contains("Path: `lib/test.md`"),
        "Expected create_note path formatting, got: {}",
        t.content
    );
}

/// T033: Tool-result formatting is entirely UI-side.
#[test]
fn test_tool_result_formatting_is_ui_side() {
    let id = Uuid::new_v4();
    let mut t = AgentTranscript::new(id);

    t.apply_event(&AgentEvent::ToolResult {
        session_id: id,
        id: "call_1".into(),
        name: "create_note".into(),
        result: json!({"status": "success", "data": {"size_bytes": 42}}),
    });

    assert!(
        t.content
            .contains("> **Result (`create_note`):** File created (42 B)."),
        "Expected UI-formatted tool result, got: {}",
        t.content
    );
}

/// T034: Structured delegate trace renders from `Vec<DelegateToolCall>`.
/// The `<span>`-wrapped formatting comes from `format_delegate_trace` in
/// `agent_render.rs` — no string `tool_call_trace` involved.
#[test]
fn test_structured_delegate_trace_renders_from_tool_calls() {
    let id = Uuid::new_v4();
    let mut t = AgentTranscript::new(id);

    t.apply_event(&AgentEvent::ToolResult {
        session_id: id,
        id: "call_1".into(),
        name: "web_delegate".into(),
        result: json!({
            "status": "success",
            "data": {
                "result": "Task completed",
                "tool_calls": [
                    DelegateToolCall {
                        name: "web_fetch".to_string(),
                        args: json!({"url": "https://example.com"}),
                        result: json!({"status": "success"}),
                    },
                    DelegateToolCall {
                        name: "browser_navigate".to_string(),
                        args: json!({"url": "https://other.com"}),
                        result: json!({"status": "success"}),
                    },
                ],
            },
        }),
    });

    // Main result formatting
    assert!(
        t.content.contains("web_delegate"),
        "Expected main result formatting, got: {}",
        t.content
    );
    // First delegate sub-call
    assert!(
        t.content.contains("<span>**Executing tool `web_fetch`**"),
        "Expected first delegate trace entry with <span> wrapper, got: {}",
        t.content
    );
    // Second delegate sub-call
    assert!(
        t.content
            .contains("<span>**Executing tool `browser_navigate`**"),
        "Expected second delegate trace entry with <span> wrapper, got: {}",
        t.content
    );
}
