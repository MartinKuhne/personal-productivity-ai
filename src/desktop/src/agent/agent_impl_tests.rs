//! Integration tests for `run_agent` — mock HTTP server simulating LLM responses, verifying tool calls, streaming, and cancellation.

use crate::agent_impl::run_agent;
use crate::config::AgentConfig;
use crate::config::AgentConfigBuilder;
use crate::config::LlmConfig;
use crate::context::AgentContext;
use crate::events::{
    AgentDebugEntry, AgentObserverEvent, AgentStatus, DebugEntryKind, DebugEntryRow,
    RecordingObserver,
};
use std::sync::Arc;

fn make_agent_config(port: u16) -> AgentConfig {
    use std::collections::HashMap;
    let mut models = HashMap::new();
    models.insert(
        "test".to_string(),
        LlmConfig {
            model: "test".to_string(),
            api_url: format!("http://127.0.0.1:{}", port),
            api_key: "valid-key".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        },
    );
    AgentConfigBuilder::new().with_models(models).build()
}

struct DummyBrowser;
impl crate::tools::browser::BrowserAutomationExt for DummyBrowser {
    fn navigate(&self, _url: &str) -> Result<(String, String), String> {
        Ok(("http://localhost".to_string(), "Mock".to_string()))
    }
    fn get_page_state(&self) -> Result<(String, String, String, usize), String> {
        Ok((
            "http://localhost".to_string(),
            "Mock".to_string(),
            "[]".to_string(),
            0,
        ))
    }
    fn click(&self, _selector: &str) -> Result<(), String> {
        Ok(())
    }
    fn fill_input(&self, _selector: &str, _text: &str) -> Result<(), String> {
        Ok(())
    }
    fn select_dropdown(&self, _selector: &str, _value: &str) -> Result<(), String> {
        Ok(())
    }
    fn press_key(&self, _key: &str) -> Result<(), String> {
        Ok(())
    }
    fn evaluate_js(&self, _script: &str) -> Result<serde_json::Value, String> {
        Ok(serde_json::Value::Null)
    }
    fn screenshot(
        &self,
        _filename: &str,
        _full_page: bool,
    ) -> Result<(std::path::PathBuf, Vec<u8>), String> {
        Ok((std::path::PathBuf::from("screenshot.png"), Vec::new()))
    }
    fn save_storage(&self) -> Result<(), String> {
        Ok(())
    }
    fn resolve_screenshot_path(&self, filename: &str) -> Result<std::path::PathBuf, String> {
        Ok(std::path::PathBuf::from(filename))
    }
}

fn make_ctx(config: AgentConfig) -> (AgentContext, Arc<RecordingObserver>) {
    let default_browser: std::sync::Arc<dyn crate::tools::browser::BrowserAutomationExt> =
        std::sync::Arc::new(DummyBrowser);
    let recorded = Arc::new(RecordingObserver::new());
    let session_id = uuid::Uuid::new_v4();
    let ctx = crate::context::AgentContextBuilder::new(config, session_id, "Hello".to_string())
        .with_file_observer(std::sync::Arc::new(
            crate::tools::observer::DefaultFileObserver,
        ))
        .with_observer(recorded.clone())
        .with_system_prompts(Vec::new())
        .with_extension(Arc::new(crate::tools::browser::BrowserExt(default_browser)))
        .build();
    (ctx, recorded)
}

/// Collect observer events until `SessionFinished` is seen or `timeout` elapses.
fn collect_observer_events(
    recorded: &RecordingObserver,
    timeout: std::time::Duration,
) -> Vec<AgentObserverEvent> {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        let events = recorded.events();
        if events
            .iter()
            .any(|ev| matches!(ev, AgentObserverEvent::SessionFinished(..)))
        {
            return events;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    recorded.events()
}

/// Spawn a one-shot HTTP server on a random localhost port and bind
/// the given `body` as the response to the first incoming request.
fn spawn_one_shot_http_server(body: &[u8]) -> u16 {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let body = body.to_vec();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0; 8192];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(&body);
            let _ = stream.flush();
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    });
    port
}

/// Build a minimal HTTP/1.1 response with the given status line and body.
fn http_response(status_line: &str, body: &str) -> Vec<u8> {
    format!(
        "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

#[test]
fn test_run_agent_missing_api_key() {
    use std::collections::HashMap;
    let mut models = HashMap::new();
    models.insert(
        "test".to_string(),
        LlmConfig {
            model: "test".to_string(),
            api_url: "http://localhost".to_string(),
            api_key: "".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        },
    );
    let config = AgentConfigBuilder::new().with_models(models).build();
    let (ctx, recorded) = make_ctx(config);
    run_agent(ctx);
    let events = collect_observer_events(&recorded, std::time::Duration::from_secs(2));
    assert!(
        events.iter().any(|e| matches!(
            e,
            AgentObserverEvent::Failed(error) if error.contains("API key not set")
        )),
        "expected Failed with API key message; got: {:?}",
        events
    );
}

#[test]
fn test_run_agent_network_error() {
    use std::collections::HashMap;
    let mut models = HashMap::new();
    models.insert(
        "test".to_string(),
        LlmConfig {
            model: "test".to_string(),
            api_url: "http://127.0.0.1:0".to_string(),
            api_key: "valid-key".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        },
    );
    let config = AgentConfigBuilder::new().with_models(models).build();
    let (ctx, recorded) = make_ctx(config);
    run_agent(ctx);
    let events = collect_observer_events(&recorded, std::time::Duration::from_secs(30));
    assert!(
        events.iter().any(|e| matches!(
            e,
            AgentObserverEvent::Failed(error) if error.contains("Network error") || error.contains("timed out")
        )),
        "expected Failed with network error; got: {:?}",
        events
    );
}

#[test]
fn test_run_agent_invalid_json_response() {
    let port = spawn_one_shot_http_server(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\n{");
    let (ctx, recorded) = make_ctx(make_agent_config(port));
    run_agent(ctx);
    let events = collect_observer_events(&recorded, std::time::Duration::from_secs(5));
    assert!(
        events.iter().any(|e| matches!(
            e,
            AgentObserverEvent::Failed(error) if error.contains("Failed to parse")
        )),
        "expected Failed with parse error; got: {:?}",
        events
    );
}

#[test]
fn test_run_agent_http_status_error() {
    let port = spawn_one_shot_http_server(
        b"HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\n\r\nbad request",
    );
    let (ctx, recorded) = make_ctx(make_agent_config(port));
    run_agent(ctx);
    let events = collect_observer_events(&recorded, std::time::Duration::from_secs(5));
    assert!(
        events.iter().any(|e| matches!(
            e,
            AgentObserverEvent::Failed(error) if error.contains("HTTP 400 error")
        )),
        "expected Failed with HTTP 400 error; got: {:?}",
        events
    );
}

#[test]
fn test_run_agent_missing_choices() {
    let port = spawn_one_shot_http_server(&http_response("HTTP/1.1 200 OK", "{}"));
    let (ctx, recorded) = make_ctx(make_agent_config(port));
    run_agent(ctx);
    let events = collect_observer_events(&recorded, std::time::Duration::from_secs(5));
    assert!(
        events.iter().any(|e| matches!(
            e,
            AgentObserverEvent::Failed(error) if error.contains("Invalid response schema")
        )),
        "expected Failed with schema error; got: {:?}",
        events
    );
}

#[test]
fn test_run_agent_emits_done_status_on_natural_completion() {
    let body = serde_json::json!({
        "id": "chatcmpl-test", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "All done."}, "finish_reason": "stop"}]
    })
    .to_string();
    let port = spawn_one_shot_http_server(&http_response("HTTP/1.1 200 OK", &body));
    let (ctx, recorded) = make_ctx(make_agent_config(port));
    run_agent(ctx);
    let events = collect_observer_events(&recorded, std::time::Duration::from_secs(5));
    let saw_finished = events
        .iter()
        .any(|e| matches!(e, AgentObserverEvent::SessionFinished(..)));
    assert!(saw_finished, "must see SessionFinished");
    let saw_done = events
        .iter()
        .any(|e| matches!(e, AgentObserverEvent::Status(AgentStatus::Done)));
    assert!(saw_done, "must see Status(Done); events: {:?}", events);
}

#[test]
fn test_run_agent_skips_done_status_when_cancelled() {
    let body = serde_json::json!({
        "id": "chatcmpl-test", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "All done."}, "finish_reason": "stop"}]
    })
    .to_string();
    let port = spawn_one_shot_http_server(&http_response("HTTP/1.1 200 OK", &body));
    let default_browser: std::sync::Arc<dyn crate::tools::browser::BrowserAutomationExt> =
        std::sync::Arc::new(DummyBrowser);
    let recorded = Arc::new(RecordingObserver::new());
    let ctx = crate::context::AgentContextBuilder::new(
        make_agent_config(port),
        uuid::Uuid::new_v4(),
        "List files".to_string(),
    )
    .with_file_observer(std::sync::Arc::new(
        crate::tools::observer::DefaultFileObserver,
    ))
    .with_observer(recorded.clone())
    .with_system_prompts(Vec::new())
    .with_extension(Arc::new(crate::tools::browser::BrowserExt(default_browser)))
    .with_cancel_flag(std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
        true,
    )))
    .build();
    run_agent(ctx);
    let events = collect_observer_events(&recorded, std::time::Duration::from_secs(5));
    let saw_finished = events
        .iter()
        .any(|e| matches!(e, AgentObserverEvent::SessionFinished(..)));
    let saw_done = events
        .iter()
        .any(|e| matches!(e, AgentObserverEvent::Status(AgentStatus::Done)));
    assert!(saw_finished, "must see SessionFinished");
    assert!(!saw_done, "must NOT see Status(Done) when cancelled");
}

#[test]
fn test_run_agent_emits_executing_tool_message_immediately() {
    let tool_call_body = serde_json::json!({
        "id": "chatcmpl-tool", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "read_tags",
                        "arguments": "{}"
                    }
                }]
            },
            "finish_reason": "tool_calls"
        }]
    })
    .to_string();
    let final_body = serde_json::json!({
        "id": "chatcmpl-final", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "Done with tools."}, "finish_reason": "stop"}]
    })
    .to_string();

    use std::io::{Read, Write};
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for body in [tool_call_body, final_body] {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0; 8192];
                let _ = stream.read(&mut buf);
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(resp.as_bytes());
                let _ = stream.flush();
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    });
    let (ctx, recorded) = make_ctx(make_agent_config(port));
    run_agent(ctx);
    let events = collect_observer_events(&recorded, std::time::Duration::from_secs(5));
    assert!(
        events.iter().any(|e| matches!(
            e,
            AgentObserverEvent::ToolCallStarted { name, .. } if name == "read_tags"
        )),
        "must see ToolCallStarted for read_tags; events: {:?}",
        events
    );
    assert!(
        events.iter().any(|e| matches!(
            e,
            AgentObserverEvent::ToolResult { name, .. } if name == "read_tags"
        )),
        "must see ToolResult for read_tags; events: {:?}",
        events
    );
}

#[test]
fn test_run_agent_datamarks_tool_results_in_conversation_history() {
    use crate::datamark::{EXTERNAL_DATA_END, EXTERNAL_DATA_START};

    let tool_call_body = serde_json::json!({
        "id": "chatcmpl-tool", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "read_tags",
                        "arguments": "{}"
                    }
                }]
            },
            "finish_reason": "tool_calls"
        }]
    })
    .to_string();
    let final_body = serde_json::json!({
        "id": "chatcmpl-final", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "Done with tools."}, "finish_reason": "stop"}]
    })
    .to_string();

    use std::io::{Read, Write};
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for body in [tool_call_body, final_body] {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0; 8192];
                let _ = stream.read(&mut buf);
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(resp.as_bytes());
                let _ = stream.flush();
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    });
    let (ctx, recorded) = make_ctx(make_agent_config(port));
    run_agent(ctx);

    let events = collect_observer_events(&recorded, std::time::Duration::from_secs(5));
    let history = events
        .iter()
        .find_map(|e| match e {
            AgentObserverEvent::SessionFinished(history) => Some(history.clone()),
            _ => None,
        })
        .expect("agent must emit SessionFinished with history");

    // Find the role:tool entry.
    let tool_entry = history
        .iter()
        .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("tool"))
        .expect("history must contain a role:tool entry");
    let content = tool_entry
        .get("content")
        .and_then(|c| c.as_str())
        .expect("tool entry must have string content");

    assert!(
        content.contains(EXTERNAL_DATA_START),
        "role:tool content must be wrapped in the EXTERNAL_DATA envelope; got: {content}"
    );
    assert!(
        content.contains(EXTERNAL_DATA_END),
        "role:tool content must end with the EXTERNAL_DATA envelope; got: {content}"
    );
    assert!(
        content.contains("provenance=tool:read_tags"),
        "envelope must carry the tool name as provenance; got: {content}"
    );
    assert!(
        content.contains("trust=untrusted"),
        "envelope must carry the trust=untrusted marker; got: {content}"
    );
}

#[test]
fn test_run_agent_system_prompt_starts_with_security_header() {
    let body = serde_json::json!({
        "id": "chatcmpl-test", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "All done."}, "finish_reason": "stop"}]
    })
    .to_string();
    let port = spawn_one_shot_http_server(&http_response("HTTP/1.1 200 OK", &body));
    let (mut ctx, recorded) = make_ctx(make_agent_config(port));
    ctx.system_prompts = vec![format!(
        "{}\nYou are FastMD AI...",
        crate::datamark::SECURITY_HEADER
    )];
    run_agent(ctx);

    let events = collect_observer_events(&recorded, std::time::Duration::from_secs(5));
    let history = events
        .iter()
        .find_map(|e| match e {
            AgentObserverEvent::SessionFinished(history) => Some(history.clone()),
            _ => None,
        })
        .expect("agent must emit SessionFinished with history");

    let system = history
        .first()
        .expect("history must start with a system message");
    assert_eq!(
        system.get("role").and_then(|r| r.as_str()),
        Some("system"),
        "first message must be the system prompt"
    );
    let content = system
        .get("content")
        .and_then(|c| c.as_str())
        .expect("system message must have string content");
    assert!(
        content.starts_with(crate::datamark::SECURITY_HEADER),
        "system prompt must start with the security header; got first 200 chars: {:?}",
        &content[..content.len().min(200)]
    );
}

#[test]
fn test_run_agent_system_prompt_without_context_has_no_file_context() {
    let body = serde_json::json!({
        "id": "chatcmpl-test", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "All done."}, "finish_reason": "stop"}]
    })
    .to_string();
    let port = spawn_one_shot_http_server(&http_response("HTTP/1.1 200 OK", &body));

    let (ctx, recorded) = make_ctx(make_agent_config(port));
    run_agent(ctx);

    let events = collect_observer_events(&recorded, std::time::Duration::from_secs(5));
    let history = events
        .iter()
        .find_map(|e| match e {
            AgentObserverEvent::SessionFinished(history) => Some(history.clone()),
            _ => None,
        })
        .expect("agent must emit SessionFinished with history");

    let system = history
        .first()
        .expect("history must start with a system message");
    let content = system
        .get("content")
        .and_then(|c| c.as_str())
        .expect("system message must have string content");
    for marker in [
        "viewing the file",
        "directory context",
        "selected the following files",
    ] {
        assert!(
            !content.contains(marker),
            "system prompt must not contain {marker:?} when no context is handed over; got: {content}"
        );
    }
}

#[test]
fn test_run_agent_emits_debug_entries() {
    let body = serde_json::json!({
        "id": "chatcmpl-test", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "All done."}, "finish_reason": "stop"}]
    })
    .to_string();
    let port = spawn_one_shot_http_server(&http_response("HTTP/1.1 200 OK", &body));
    let (ctx, recorded) = make_ctx(make_agent_config(port));
    run_agent(ctx);

    let events = collect_observer_events(&recorded, std::time::Duration::from_secs(5));
    let debug_entries: Vec<AgentDebugEntry> = events
        .iter()
        .filter_map(|e| match e {
            AgentObserverEvent::DebugEntry(entry) => Some(entry.clone()),
            _ => None,
        })
        .collect();

    assert!(
        !debug_entries.is_empty(),
        "must emit at least one debug entry"
    );

    // First entry must be the session boundary
    let boundary = &debug_entries[0];
    assert_eq!(boundary.row_type, DebugEntryRow::SessionBoundary);
    assert_eq!(boundary.turn, 0);
    assert!(boundary.summary.contains("Session"));
    assert!(boundary.content.is_none());

    // Find outgoing entry
    let outgoing = debug_entries
        .iter()
        .find(|e| {
            matches!(e.kind, DebugEntryKind::Outgoing) && matches!(e.row_type, DebugEntryRow::Entry)
        })
        .expect("must emit an outgoing debug entry");
    assert_eq!(outgoing.turn, 1);
    assert!(outgoing.summary.contains("Outgoing"));
    assert!(outgoing.summary.contains("Turn 1"));
    let content = outgoing
        .content
        .as_ref()
        .expect("outgoing must have content");
    assert!(content.get("model").is_some());
    assert!(
        content
            .get("new_messages")
            .and_then(|m| m.as_array())
            .is_some()
    );

    // Find incoming entry
    let incoming = debug_entries
        .iter()
        .find(|e| {
            matches!(e.kind, DebugEntryKind::Incoming) && matches!(e.row_type, DebugEntryRow::Entry)
        })
        .expect("must emit an incoming debug entry");
    assert_eq!(incoming.turn, 1);
    let content = incoming
        .content
        .as_ref()
        .expect("incoming must have content");
    assert!(content.get("choices").is_some());
}

#[test]
fn test_run_agent_debug_entries_with_tool_calls() {
    let tool_call_body = serde_json::json!({
        "id": "chatcmpl-tool", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "read_tags",
                        "arguments": "{}"
                    }
                }]
            },
            "finish_reason": "tool_calls"
        }]
    })
    .to_string();
    let final_body = serde_json::json!({
        "id": "chatcmpl-final", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "Done with tools."}, "finish_reason": "stop"}]
    })
    .to_string();

    use std::io::{Read, Write};
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for body in [tool_call_body, final_body] {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0; 8192];
                let _ = stream.read(&mut buf);
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(resp.as_bytes());
                let _ = stream.flush();
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    });
    let (ctx, recorded) = make_ctx(make_agent_config(port));
    run_agent(ctx);

    let events = collect_observer_events(&recorded, std::time::Duration::from_secs(5));
    let debug_entries: Vec<AgentDebugEntry> = events
        .iter()
        .filter_map(|e| match e {
            AgentObserverEvent::DebugEntry(entry) => Some(entry.clone()),
            _ => None,
        })
        .collect();

    // Must have a session boundary
    assert!(
        debug_entries
            .iter()
            .any(|e| matches!(e.row_type, DebugEntryRow::SessionBoundary)),
        "must emit session boundary"
    );

    // Must have at least one tool results entry
    let tool_results: Vec<_> = debug_entries
        .iter()
        .filter(|e| matches!(e.kind, DebugEntryKind::ToolResults))
        .collect();
    assert!(
        !tool_results.is_empty(),
        "must emit tool results debug entries"
    );
    let tr = tool_results[0];
    assert_eq!(tr.turn, 1);
    assert!(tr.summary.contains("Tool results"));
    assert!(tr.summary.contains("1 tools"));
    let content = tr.content.as_ref().expect("tool results must have content");
    let arr = content
        .as_array()
        .expect("tool results content must be an array");
    assert_eq!(arr.len(), 1);
    assert_eq!(
        arr[0].get("name").and_then(|v| v.as_str()),
        Some("read_tags")
    );

    // Must have outgoing entries for both turns
    let outgoing: Vec<_> = debug_entries
        .iter()
        .filter(|e| {
            matches!(e.kind, DebugEntryKind::Outgoing) && matches!(e.row_type, DebugEntryRow::Entry)
        })
        .collect();
    assert_eq!(outgoing.len(), 2, "must have outgoing for both turns");

    // Turn 2 outgoing should contain only the delta (assistant + tool results from turn 1)
    let turn2_outgoing = outgoing
        .iter()
        .find(|e| e.turn == 2)
        .expect("must have turn 2 outgoing");
    let delta = turn2_outgoing
        .content
        .as_ref()
        .and_then(|c| c.get("new_messages"))
        .and_then(|m| m.as_array())
        .expect("turn 2 outgoing must have new_messages array");
    assert!(!delta.is_empty(), "turn 2 outgoing delta must not be empty");
}

#[test]
fn test_debug_outgoing_turn1_includes_full_initial_messages() {
    let body = serde_json::json!({
        "id": "chatcmpl-test", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "All done."}, "finish_reason": "stop"}]
    })
    .to_string();
    let port = spawn_one_shot_http_server(&http_response("HTTP/1.1 200 OK", &body));
    let (mut ctx, recorded) = make_ctx(make_agent_config(port));
    ctx.system_prompts = vec![format!(
        "{}\nYou are FastMD AI...",
        crate::datamark::SECURITY_HEADER
    )];
    run_agent(ctx);

    let events = collect_observer_events(&recorded, std::time::Duration::from_secs(5));
    let outgoing: Vec<AgentDebugEntry> = events
        .iter()
        .filter_map(|e| match e {
            AgentObserverEvent::DebugEntry(entry) => {
                if matches!(entry.kind, DebugEntryKind::Outgoing)
                    && matches!(entry.row_type, DebugEntryRow::Entry)
                {
                    Some(entry.clone())
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect();

    let turn1 = outgoing
        .iter()
        .find(|e| e.turn == 1)
        .expect("must have turn 1 outgoing");
    let new_messages = turn1
        .content
        .as_ref()
        .and_then(|c| c.get("new_messages"))
        .and_then(|m| m.as_array())
        .expect("turn 1 outgoing must have new_messages");
    assert!(!new_messages.is_empty());

    // The first message should be the system prompt
    let first = &new_messages[0];
    assert_eq!(
        first.get("role").and_then(|r| r.as_str()),
        Some("system"),
        "first message in turn 1 delta must be the system prompt"
    );
}

#[test]
fn test_observer_receives_lifecycle_events() {
    let body = serde_json::json!({
        "id": "chatcmpl-1", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "Hi"}, "finish_reason": "stop"}],
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
    })
    .to_string();
    let port = spawn_one_shot_http_server(&http_response("HTTP/1.1 200 OK", &body));
    let (ctx, recorded) = make_ctx(make_agent_config(port));
    run_agent(ctx);

    let observer_events = collect_observer_events(&recorded, std::time::Duration::from_secs(5));

    assert!(
        observer_events
            .iter()
            .any(|e| matches!(e, AgentObserverEvent::SessionStarted)),
        "Observer must receive SessionStarted"
    );
    assert!(
        observer_events
            .iter()
            .any(|e| matches!(e, AgentObserverEvent::SessionFinished(..))),
        "Observer must receive SessionFinished"
    );
}
