//! Unit tests for `ConversationLoggerObserver`.

use super::*;
use chrono::TimeZone;

#[test]
fn test_generate_conversation_filename() {
    let dt = Local.with_ymd_and_hms(2026, 8, 24, 14, 30, 45).unwrap();
    let filename = generate_conversation_filename(dt);
    assert_eq!(filename, "2026-08-24 14-30-45.md");
}

#[test]
fn test_heading_formatting() {
    assert_eq!(format_prompt_heading(1), "## Prompt (1)");
    assert_eq!(format_response_heading(1), "## Response (1)");
    assert_eq!(format_prompt_heading(42), "## Prompt (42)");
    assert_eq!(format_response_heading(42), "## Response (42)");
}

#[test]
fn test_format_write_tool_calls() {
    let records = vec![
        MutatingToolCallRecord {
            name: "create_note".to_string(),
            arguments: r#"{"path": "notes/todo.md", "content": "hello"}"#.to_string(),
            result: "Created successfully".to_string(),
        },
        MutatingToolCallRecord {
            name: "patch_note".to_string(),
            arguments: r#"{"path": "notes/todo.md", "old_string": "a", "new_string": "b"}"#
                .to_string(),
            result: "Replaced 1 instance".to_string(),
        },
    ];

    let formatted = format_write_tool_calls(&records);
    assert!(formatted.contains("> **Executing tool `create_note`**"));
    assert!(formatted.contains(r#"> {"path": "notes/todo.md", "content": "hello"}"#));
    assert!(formatted.contains("> **Result (`create_note`):**"));
    assert!(formatted.contains("> Created successfully"));
    assert!(formatted.contains("> **Executing tool `patch_note`**"));
    assert!(formatted.contains("> Replaced 1 instance"));
}

#[test]
fn test_format_turn_entry_single_and_multi_turn() {
    let entry1 = format_turn_entry(1, "Hello assistant", "Hello user!", &[]);
    assert_eq!(
        entry1,
        "## Prompt (1)\n\nHello assistant\n\n## Response (1)\n\nHello user!\n\n"
    );

    let entry2 = format_turn_entry(
        2,
        "Create a note please",
        "Note created.",
        &[MutatingToolCallRecord {
            name: "create_note".to_string(),
            arguments: r#"{"path": "test.md"}"#.to_string(),
            result: "Done".to_string(),
        }],
    );
    assert!(entry2.starts_with("## Prompt (2)\n\nCreate a note please\n\n## Response (2)\n\nNote created.\n\n> **Executing tool `create_note`**"));
}

#[test]
fn test_conversation_logger_observer_multi_turn() {
    let temp = tempfile::tempdir().unwrap();
    let conv_dir = temp.path().join("Conversations");

    let session_id = Uuid::new_v4();
    let observer = ConversationLoggerObserver::new(session_id, conv_dir.clone());

    // Turn 1
    let history_turn_1 = vec![
        serde_json::json!({"role": "user", "content": "Turn 1 prompt"}),
        serde_json::json!({"role": "assistant", "content": "Turn 1 response"}),
    ];
    observer.on_session_finished(history_turn_1);

    assert_eq!(observer.get_turn_number(), 1);
    let log_path = observer.get_log_path().expect("log path must be set");
    assert!(log_path.exists());
    assert_eq!(log_path.parent().unwrap(), conv_dir);

    let content_1 = std::fs::read_to_string(&log_path).unwrap();
    assert!(content_1.contains("## Prompt (1)\n\nTurn 1 prompt"));
    assert!(content_1.contains("## Response (1)\n\nTurn 1 response"));

    // Turn 2 in same session with mutating tool
    observer.on_tool_call_started(
        "call_1".to_string(),
        "create_note".to_string(),
        serde_json::json!({"path": "notes/todo.md"}),
    );
    observer.on_tool_result(
        "call_1".to_string(),
        "create_note".to_string(),
        serde_json::json!("Created note"),
    );

    let history_turn_2 = vec![
        serde_json::json!({"role": "user", "content": "Turn 1 prompt"}),
        serde_json::json!({"role": "assistant", "content": "Turn 1 response"}),
        serde_json::json!({"role": "user", "content": "Turn 2 prompt"}),
        serde_json::json!({"role": "assistant", "content": "Turn 2 response"}),
    ];
    observer.on_session_finished(history_turn_2);

    assert_eq!(observer.get_turn_number(), 2);
    let content_2 = std::fs::read_to_string(&log_path).unwrap();
    assert!(content_2.contains("## Prompt (1)\n\nTurn 1 prompt"));
    assert!(content_2.contains("## Prompt (2)\n\nTurn 2 prompt"));
    assert!(content_2.contains("## Response (2)\n\nTurn 2 response"));
    assert!(content_2.contains("> **Executing tool `create_note`**"));
}

#[test]
fn test_conversation_logger_observer_read_only_tool_not_logged() {
    let temp = tempfile::tempdir().unwrap();
    let conv_dir = temp.path().join("Conversations");

    let session_id = Uuid::new_v4();
    let observer = ConversationLoggerObserver::new(session_id, conv_dir);

    observer.on_tool_call_started(
        "call_read".to_string(),
        "read_note".to_string(),
        serde_json::json!({"path": "notes/todo.md"}),
    );
    observer.on_tool_result(
        "call_read".to_string(),
        "read_note".to_string(),
        serde_json::json!("File content"),
    );

    let history = vec![
        serde_json::json!({"role": "user", "content": "Read note please"}),
        serde_json::json!({"role": "assistant", "content": "Here is the content"}),
    ];
    observer.on_session_finished(history);

    let log_path = observer.get_log_path().expect("log path must be set");
    let content = std::fs::read_to_string(&log_path).unwrap();
    assert!(!content.contains("read_note"));
    assert!(!content.contains("Executing tool"));
}

#[test]
fn test_conversation_logger_observer_mutating_tool_logged_at_end_of_response() {
    let temp = tempfile::tempdir().unwrap();
    let conv_dir = temp.path().join("Conversations");

    let session_id = Uuid::new_v4();
    let observer = ConversationLoggerObserver::new(session_id, conv_dir);

    observer.on_tool_call_started(
        "call_mut".to_string(),
        "patch_note".to_string(),
        serde_json::json!({"path": "notes/todo.md", "old": "a", "new": "b"}),
    );
    observer.on_tool_result(
        "call_mut".to_string(),
        "patch_note".to_string(),
        serde_json::json!("Patched"),
    );

    let history = vec![
        serde_json::json!({"role": "user", "content": "Patch note"}),
        serde_json::json!({"role": "assistant", "content": "I patched the note."}),
    ];
    observer.on_session_finished(history);

    let log_path = observer.get_log_path().expect("log path must be set");
    let content = std::fs::read_to_string(&log_path).unwrap();

    let resp_idx = content.find("## Response (1)").unwrap();
    let text_idx = content.find("I patched the note.").unwrap();
    let tool_idx = content.find("> **Executing tool `patch_note`**").unwrap();

    assert!(resp_idx < text_idx);
    assert!(text_idx < tool_idx);
}
