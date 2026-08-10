//! Tests for `ui/agent/transcript.rs`.

use super::*;
use crate::agent::events::{AgentEvent, AgentStatus, ToolSideEffect};
use serde_json::json;
use uuid::Uuid;

fn test_session_id() -> Uuid {
    Uuid::new_v4()
}

#[test]
fn test_new_transcript_is_empty() {
    let id = test_session_id();
    let t = AgentTranscript::new(id);
    assert!(t.blocks.is_empty());
    assert!(t.thinking.is_empty());
    assert!(t.content.is_empty());
    assert_eq!(t.session_id, id);
}

#[test]
fn test_reset_clears_all() {
    let id = test_session_id();
    let mut t = AgentTranscript::new(id);
    t.content = "hello".to_string();
    t.thinking = "think".to_string();
    t.blocks.push(TranscriptBlock::Content {
        text: "hello".to_string(),
    });
    t.reset();
    assert!(t.blocks.is_empty());
    assert!(t.thinking.is_empty());
    assert!(t.content.is_empty());
}

#[test]
fn test_content_delta_accumulates() {
    let id = test_session_id();
    let mut t = AgentTranscript::new(id);
    t.apply_event(&AgentEvent::ContentDelta {
        session_id: id,
        text: "Hello ".into(),
    });
    t.apply_event(&AgentEvent::ContentDelta {
        session_id: id,
        text: "world\n\n".into(),
    });
    assert_eq!(t.content, "Hello world\n\n");
    assert_eq!(t.blocks.len(), 1);
    match &t.blocks[0] {
        TranscriptBlock::Content { text } => assert_eq!(text, "Hello world\n\n"),
        _ => panic!("expected Content block"),
    }
}

#[test]
fn test_thinking_accumulates() {
    let id = test_session_id();
    let mut t = AgentTranscript::new(id);
    t.apply_event(&AgentEvent::Thinking {
        session_id: id,
        text: "Reasoning ".into(),
    });
    t.apply_event(&AgentEvent::Thinking {
        session_id: id,
        text: "step 1".into(),
    });
    assert_eq!(t.thinking, "Reasoning step 1");
    assert_eq!(t.blocks.len(), 1);
    match &t.blocks[0] {
        TranscriptBlock::Thinking { text } => assert_eq!(text, "Reasoning step 1"),
        _ => panic!("expected Thinking block"),
    }
}

#[test]
fn test_tool_call_started_then_result_pairs() {
    let id = test_session_id();
    let mut t = AgentTranscript::new(id);
    t.apply_event(&AgentEvent::ToolCallStarted {
        session_id: id,
        id: "call_1".into(),
        name: "create_note".into(),
        args: json!({"path": "lib/test.md"}),
    });
    assert!(t.content.contains("Executing tool `create_note`"));
    assert_eq!(t.blocks.len(), 1);
    match &t.blocks[0] {
        TranscriptBlock::ToolCall {
            id, name, result, ..
        } => {
            assert_eq!(id, "call_1");
            assert_eq!(name, "create_note");
            assert!(result.is_none());
        }
        _ => panic!("expected ToolCall block"),
    }
    t.apply_event(&AgentEvent::ToolResult {
        session_id: id,
        id: "call_1".into(),
        name: "create_note".into(),
        result: json!({"status": "success", "data": {"size_bytes": 42}}),
    });
    assert!(t.content.contains("File created (42 B)"));
    match &t.blocks[0] {
        TranscriptBlock::ToolCall { result, .. } => assert!(result.is_some()),
        _ => panic!("expected ToolCall block"),
    }
}

#[test]
fn test_web_delegate_result_formatted() {
    let id = test_session_id();
    let mut t = AgentTranscript::new(id);
    t.apply_event(&AgentEvent::ToolResult {
        session_id: id,
        id: "call_1".into(),
        name: "web_delegate".into(),
        result: json!({
            "status": "success",
            "data": {
                "result": "Done",
                "tool_call_trace": ">> trace text",
            },
        }),
    });
    // The tool result is formatted via format_tool_result_message, which
    // includes the raw JSON in the short-result branch. No separate
    // trace-append (the old agent_impl.rs:431-437 top-level lookup is dead
    // code — trace is under `data`, not top-level).
    assert!(t.content.contains("web_delegate"));
}

#[test]
fn test_ignores_events_for_other_session() {
    let id = test_session_id();
    let other = test_session_id();
    let mut t = AgentTranscript::new(id);
    t.apply_event(&AgentEvent::ContentDelta {
        session_id: other,
        text: "should be ignored".into(),
    });
    assert!(t.content.is_empty());
    assert!(t.blocks.is_empty());
}

#[test]
fn test_content_then_tool_call_creates_separate_blocks() {
    let id = test_session_id();
    let mut t = AgentTranscript::new(id);
    t.apply_event(&AgentEvent::ContentDelta {
        session_id: id,
        text: "Some content\n\n".into(),
    });
    t.apply_event(&AgentEvent::ToolCallStarted {
        session_id: id,
        id: "call_1".into(),
        name: "search_notes".into(),
        args: json!({"pattern": "test"}),
    });
    assert_eq!(t.blocks.len(), 2);
    assert!(matches!(&t.blocks[0], TranscriptBlock::Content { .. }));
    assert!(matches!(&t.blocks[1], TranscriptBlock::ToolCall { .. }));
}

#[test]
fn test_tool_side_effect_ignored_by_transcript() {
    let id = test_session_id();
    let mut t = AgentTranscript::new(id);
    t.apply_event(&AgentEvent::ToolSideEffect {
        session_id: id,
        effect: ToolSideEffect::FileCreated {
            path: std::path::PathBuf::from("/tmp/test.md"),
            tags: vec!["tag1".into()],
        },
    });
    assert!(t.content.is_empty());
    assert!(t.blocks.is_empty());
}

#[test]
fn test_status_events_ignored_by_transcript() {
    let id = test_session_id();
    let mut t = AgentTranscript::new(id);
    t.apply_event(&AgentEvent::Status {
        session_id: id,
        status: AgentStatus::AwaitingLlm,
    });
    t.apply_event(&AgentEvent::Status {
        session_id: id,
        status: AgentStatus::Done,
    });
    assert!(t.content.is_empty());
    assert!(t.blocks.is_empty());
}

/// T017: No-regression test — verifies that the transcript's `content`
/// buffer matches the pre-refactor `full_response` accumulation for a
/// prompt that triggers thinking + content + tool call + tool result +
/// final content (quickstart scenario 3, SC-002).
#[test]
fn test_transcript_matches_pre_refactor_full_response() {
    use crate::ui::render::agent_render::{format_tool_call_message, format_tool_result_message};
    let id = test_session_id();
    let mut t = AgentTranscript::new(id);

    // Simulate the event flow from a typical agent session:
    // 1. Thinking (reasoning_content)
    t.apply_event(&AgentEvent::Thinking {
        session_id: id,
        text: "Let me analyze the request.\n".into(),
    });
    // 2. Content (first response chunk)
    t.apply_event(&AgentEvent::ContentDelta {
        session_id: id,
        text: "Here's what I found:\n\n".into(),
    });
    // 3. Tool call started
    t.apply_event(&AgentEvent::ToolCallStarted {
        session_id: id,
        id: "call_1".into(),
        name: "search_notes".into(),
        args: json!({"query": "test"}),
    });
    // 4. Tool result
    t.apply_event(&AgentEvent::ToolResult {
        session_id: id,
        id: "call_1".into(),
        name: "search_notes".into(),
        result: json!({"status": "success", "data": {"matches": 3}}),
    });
    // 5. Final content
    t.apply_event(&AgentEvent::ContentDelta {
        session_id: id,
        text: "Based on the search results, here's the answer.\n\n".into(),
    });

    // Build the expected content the way the old `full_response` would have:
    let mut expected = String::new();
    // handle_content pushed content + "\n\n" (ContentDelta includes it)
    expected.push_str("Here's what I found:\n\n");
    // process_turn pushed format_tool_call_message + "\n\n"
    let tc_msg = format_tool_call_message("search_notes", r#"{"query":"test"}"#);
    expected.push_str(&tc_msg);
    expected.push_str("\n\n");
    // process_tool_results pushed format_tool_result_message (no extra "\n\n")
    let tr_msg = format_tool_result_message(
        "search_notes",
        r#"{"status":"success","data":{"matches":3}}"#,
    );
    expected.push_str(&tr_msg);
    // handle_content pushed final content + "\n\n"
    expected.push_str("Based on the search results, here's the answer.\n\n");

    assert_eq!(
        t.content, expected,
        "transcript content must match pre-refactor full_response accumulation"
    );
    assert_eq!(
        t.thinking, "Let me analyze the request.\n",
        "transcript thinking must match"
    );
}
