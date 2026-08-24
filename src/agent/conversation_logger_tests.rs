//! Tests for `agent/conversation_logger.rs`.

use super::*;
use tempfile::tempdir;

#[test]
fn test_generate_conversation_filename() {
    let dt = chrono::Local::now();
    let filename = generate_conversation_filename(dt);
    assert!(filename.ends_with(".md"));
    // Format check: YYYY-MM-DD HH-MM-SS.md (length is 22 chars)
    assert_eq!(filename.len(), 22);
    assert_eq!(&filename[4..5], "-");
    assert_eq!(&filename[7..8], "-");
    assert_eq!(&filename[10..11], " ");
    assert_eq!(&filename[13..14], "-");
    assert_eq!(&filename[16..17], "-");
}

#[test]
fn test_heading_formatting() {
    assert_eq!(format_prompt_heading(1), "## Prompt (1)");
    assert_eq!(format_prompt_heading(42), "## Prompt (42)");
    assert_eq!(format_response_heading(1), "## Response (1)");
    assert_eq!(format_response_heading(99), "## Response (99)");
}

#[test]
fn test_format_write_tool_calls() {
    let records = vec![
        ToolCallRecord {
            call_id: "call_1".to_string(),
            name: "create_note".to_string(),
            arguments: r##"{"path": "notes/todo.md", "content": "# Todo"}"##.to_string(),
            result: r#"{"status": "success", "data": {"size_bytes": 10}}"#.to_string(),
        },
        ToolCallRecord {
            call_id: "call_2".to_string(),
            name: "patch_note".to_string(),
            arguments: r#"{"path": "notes/todo.md", "old_string": "a", "new_string": "b"}"#
                .to_string(),
            result: "Replaced 1 instance".to_string(),
        },
    ];

    let formatted = format_write_tool_calls(&records);
    assert!(formatted.contains("> **Executing tool `create_note`**"));
    assert!(formatted.contains("> path: `notes/todo.md`"));
    assert!(formatted.contains("> **Result (`create_note`):**"));
    assert!(formatted.contains("> **Executing tool `patch_note`**"));
    assert!(formatted.contains("> **Result (`patch_note`):** Replaced 1 instance"));
}

#[test]
fn test_format_turn_entry_single_and_multi_turn() {
    let entry1 = format_turn_entry(1, "What is the weather?", "It is sunny.", &[]);
    assert_eq!(
        entry1,
        "## Prompt (1)\n\nWhat is the weather?\n\n## Response (1)\n\nIt is sunny.\n"
    );

    let tools = vec![ToolCallRecord {
        call_id: "1".to_string(),
        name: "create_note".to_string(),
        arguments: r#"{"path": "test.md"}"#.to_string(),
        result: "File created".to_string(),
    }];
    let entry2 = format_turn_entry(2, "Create a file", "Done!", &tools);
    assert!(entry2.starts_with("\n\n## Prompt (2)\n\nCreate a file\n\n## Response (2)\n\nDone!\n\n> **Executing tool `create_note`**"));
}

#[test]
fn test_logger_multi_turn_logging() {
    let dir = tempdir().unwrap();
    let conv_dir = dir.path().join("Conversations");
    let logger = ConversationLogger::new();
    let session_id = Uuid::new_v4();

    // Turn 1
    let path1 = logger
        .log_turn(
            session_id,
            "Hello, who are you?",
            "I am FastMD Assistant.",
            &[],
            &conv_dir,
        )
        .unwrap();

    assert!(path1.exists());
    let content1 = std::fs::read_to_string(&path1).unwrap();
    assert!(content1.contains("## Prompt (1)\n\nHello, who are you?"));
    assert!(content1.contains("## Response (1)\n\nI am FastMD Assistant."));

    // Turn 2 in same session
    let write_tools = vec![ToolCallRecord {
        call_id: "c1".to_string(),
        name: "create_note".to_string(),
        arguments: r#"{"path": "sample.md"}"#.to_string(),
        result: "created".to_string(),
    }];
    let path2 = logger
        .log_turn(
            session_id,
            "Create a note please",
            "I have created the note.",
            &write_tools,
            &conv_dir,
        )
        .unwrap();

    // Same file path should be used
    assert_eq!(path1, path2);

    let content2 = std::fs::read_to_string(&path2).unwrap();
    assert!(content2.contains("## Prompt (1)"));
    assert!(content2.contains("## Response (1)"));
    assert!(content2.contains("## Prompt (2)\n\nCreate a note please"));
    assert!(content2.contains("## Response (2)\n\nI have created the note."));
    assert!(content2.contains("> **Executing tool `create_note`**"));
}

#[test]
fn test_logger_read_only_tool_not_logged() {
    // When no write tools are supplied (e.g. only search_notes was executed),
    // the formatted output does not include any tool execution block.
    let entry = format_turn_entry(1, "Find notes about work", "Found 3 notes.", &[]);
    assert!(!entry.contains("Executing tool"));
    assert_eq!(
        entry,
        "## Prompt (1)\n\nFind notes about work\n\n## Response (1)\n\nFound 3 notes.\n"
    );
}

#[test]
fn test_logger_write_tools_appended_at_end_of_response() {
    let write_tools = vec![ToolCallRecord {
        call_id: "w1".to_string(),
        name: "patch_note".to_string(),
        arguments: r#"{"path": "file.md"}"#.to_string(),
        result: "1 replacement".to_string(),
    }];
    let entry = format_turn_entry(1, "Update file", "I modified the file.", &write_tools);
    let prompt_idx = entry.find("## Prompt (1)").unwrap();
    let resp_idx = entry.find("## Response (1)").unwrap();
    let text_idx = entry.find("I modified the file.").unwrap();
    let tool_idx = entry.find("> **Executing tool `patch_note`**").unwrap();

    assert!(prompt_idx < resp_idx);
    assert!(resp_idx < text_idx);
    assert!(text_idx < tool_idx);
}
