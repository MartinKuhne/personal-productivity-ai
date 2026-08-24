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

/// End-to-end test using an OpenAI wiremock server to prove that user prompts and
/// chat model responses are logged to timestamped markdown files in Conversations (VFS-110..113).
#[test]
fn test_e2e_openai_wiremock_chat_log_generation_multi_turn() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");

    let mock_server = runtime.block_on(MockServer::start());

    // Response for Turn 1
    let response_turn_1 = serde_json::json!({
        "id": "chatcmpl-turn-1",
        "object": "chat.completion",
        "created": 1677652288,
        "model": "gpt-4o",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "First turn assistant response."
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 15,
            "completion_tokens": 8,
            "total_tokens": 23
        }
    });

    // Response for Turn 2
    let response_turn_2 = serde_json::json!({
        "id": "chatcmpl-turn-2",
        "object": "chat.completion",
        "created": 1677652290,
        "model": "gpt-4o",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "Second turn assistant response."
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 30,
            "completion_tokens": 10,
            "total_tokens": 40
        }
    });

    runtime.block_on(
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_json(response_turn_1),
            )
            .up_to_n_times(1)
            .mount(&mock_server),
    );

    let temp = tempfile::tempdir().unwrap();
    let conv_dir = temp.path().join("Conversations");

    let session_id = Uuid::new_v4();
    let logger_observer = std::sync::Arc::new(ConversationLoggerObserver::new(
        session_id,
        conv_dir.clone(),
    ));

    let mut models = std::collections::HashMap::new();
    models.insert(
        "default".to_string(),
        fastmd_agent::config::LlmConfig {
            model: "gpt-4o".to_string(),
            api_url: mock_server.uri(),
            api_key: "test-openai-key".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        },
    );

    let agent_config = fastmd_agent::config::AgentConfigBuilder::new()
        .with_models(models)
        .build();

    // Turn 1
    let ctx1 = fastmd_agent::context::AgentContextBuilder::new(
        agent_config.clone(),
        session_id,
        "Turn 1 user prompt".to_string(),
    )
    .with_system_prompts(vec!["You are FastMD Agent.".to_string()])
    .with_observer(logger_observer.clone())
    .build();

    let handle1 = std::thread::spawn(move || {
        fastmd_agent::run_agent(ctx1);
    });
    handle1.join().unwrap();

    // Verify after Turn 1:
    assert_eq!(logger_observer.get_turn_number(), 1);
    let log_path = logger_observer.get_log_path().expect("log file must exist");
    assert!(log_path.exists());
    assert_eq!(log_path.parent().unwrap(), conv_dir.as_path());

    // Verify filename format YYYY-MM-DD HH-MM-SS.md
    let filename = log_path.file_name().unwrap().to_str().unwrap();
    assert!(filename.ends_with(".md"));
    assert_eq!(filename.len(), 22); // "YYYY-MM-DD HH-MM-SS.md" is 22 chars

    let content_turn_1 = std::fs::read_to_string(&log_path).unwrap();
    assert!(content_turn_1.contains("## Prompt (1)\n\nTurn 1 user prompt"));
    assert!(content_turn_1.contains("## Response (1)\n\nFirst turn assistant response."));

    // Register Turn 2 mock response
    runtime.block_on(
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_json(response_turn_2),
            )
            .mount(&mock_server),
    );

    // Turn 2
    let ctx2 = fastmd_agent::context::AgentContextBuilder::new(
        agent_config,
        session_id,
        "Turn 2 user prompt".to_string(),
    )
    .with_system_prompts(vec!["You are FastMD Agent.".to_string()])
    .with_observer(logger_observer.clone())
    .build();

    let handle2 = std::thread::spawn(move || {
        fastmd_agent::run_agent(ctx2);
    });
    handle2.join().unwrap();

    // Verify after Turn 2:
    assert_eq!(logger_observer.get_turn_number(), 2);
    let log_path_2 = logger_observer.get_log_path().unwrap();
    assert_eq!(
        log_path, log_path_2,
        "Same session must append to the same log file"
    );

    let entries: Vec<_> = std::fs::read_dir(&conv_dir).unwrap().collect();
    assert_eq!(
        entries.len(),
        1,
        "Exactly one log file must exist in Conversations directory"
    );

    let content_turn_2 = std::fs::read_to_string(&log_path).unwrap();
    assert!(content_turn_2.contains("## Prompt (1)\n\nTurn 1 user prompt"));
    assert!(content_turn_2.contains("## Response (1)\n\nFirst turn assistant response."));
    assert!(content_turn_2.contains("## Prompt (2)\n\nTurn 2 user prompt"));
    assert!(content_turn_2.contains("## Response (2)\n\nSecond turn assistant response."));
}

/// End-to-end test with OpenAI wiremock verifying that mutating tool calls (VFS-114)
/// executed by the agent are recorded in blockquotes at the end of the response section.
#[test]
fn test_e2e_openai_wiremock_chat_log_with_mutating_tool() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");

    let mock_server = runtime.block_on(MockServer::start());

    // Call 1: OpenAI returns tool call `create_note`
    let tool_call_response = serde_json::json!({
        "id": "chatcmpl-tool",
        "object": "chat.completion",
        "created": 1677652288,
        "model": "gpt-4o",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_create_123",
                    "type": "function",
                    "function": {
                        "name": "create_note",
                        "arguments": "{\"path\":\"Notes/todo.md\",\"content\":\"# Todo list\"}"
                    }
                }]
            },
            "finish_reason": "tool_calls"
        }],
        "usage": {
            "prompt_tokens": 20,
            "completion_tokens": 15,
            "total_tokens": 35
        }
    });

    // Call 2: OpenAI returns final assistant message after tool result
    let final_response = serde_json::json!({
        "id": "chatcmpl-final",
        "object": "chat.completion",
        "created": 1677652290,
        "model": "gpt-4o",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "I have created your todo note."
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 40,
            "completion_tokens": 8,
            "total_tokens": 48
        }
    });

    runtime.block_on(
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_json(tool_call_response),
            )
            .up_to_n_times(1)
            .mount(&mock_server),
    );

    let temp = tempfile::tempdir().unwrap();
    let conv_dir = temp.path().join("Conversations");
    let notes_dir = temp.path().join("Notes");
    std::fs::create_dir_all(&notes_dir).unwrap();

    let session_id = Uuid::new_v4();
    let logger_observer = std::sync::Arc::new(ConversationLoggerObserver::new(
        session_id,
        conv_dir.clone(),
    ));

    let mut models = std::collections::HashMap::new();
    models.insert(
        "default".to_string(),
        fastmd_agent::config::LlmConfig {
            model: "gpt-4o".to_string(),
            api_url: mock_server.uri(),
            api_key: "test-openai-key".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        },
    );

    let mut content_libraries = Vec::new();
    content_libraries.push(crate::config::ContentLibrary {
        name: "Notes".to_string(),
        root_folder: notes_dir.to_string_lossy().to_string(),
        kind: "text".to_string(),
        readonly: false,
        priority: 0,
    });

    let agent_config = fastmd_agent::config::AgentConfigBuilder::new()
        .with_models(models)
        .with_content_libraries(content_libraries)
        .build();

    let ctx = fastmd_agent::context::AgentContextBuilder::new(
        agent_config,
        session_id,
        "Create a todo note for me".to_string(),
    )
    .with_system_prompts(vec!["You are FastMD Agent.".to_string()])
    .with_observer(logger_observer.clone())
    .build();

    // Mount second response before running
    runtime.block_on(
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_json(final_response),
            )
            .mount(&mock_server),
    );

    let handle = std::thread::spawn(move || {
        fastmd_agent::run_agent(ctx);
    });
    handle.join().unwrap();

    let log_path = logger_observer.get_log_path().expect("log file must exist");
    assert!(log_path.exists());

    let content = std::fs::read_to_string(&log_path).unwrap();
    assert!(content.contains("## Prompt (1)\n\nCreate a todo note for me"));
    assert!(content.contains("## Response (1)\n\nI have created your todo note."));
    assert!(content.contains("> **Executing tool `create_note`**"));
    assert!(content.contains("Notes/todo.md"));
    assert!(content.contains("# Todo list"));
    assert!(content.contains("> **Result (`create_note`):**"));
}
