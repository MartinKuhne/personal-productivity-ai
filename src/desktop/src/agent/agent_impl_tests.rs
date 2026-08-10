//! Integration tests for `run_agent` — mock HTTP server simulating LLM responses, verifying tool calls, streaming, and cancellation.

use crate::agent::agent_impl::run_agent;
use crate::agent::context::AgentContext;
use crate::bus::events::debug::{AgentDebugEntry, DebugEntryKind, DebugEntryRow};
use crate::bus::events::typed::{AgentEvent, BackgroundEvent};
use crate::config::AppConfig;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

fn make_config(port: u16) -> AppConfig {
    let mut config = AppConfig::default();
    config.models.insert(
        "test".to_string(),
        crate::config::LlmConfig {
            model: "test".to_string(),
            api_url: format!("http://127.0.0.1:{}", port),
            api_key: "valid-key".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        },
    );
    config
}

fn make_ctx(config: AppConfig) -> (AgentContext, std::sync::mpsc::Receiver<BackgroundEvent>) {
    let (tx, rx) = std::sync::mpsc::channel();
    let browser_session = std::sync::Arc::new(crate::app::session::BrowserSession::new(
        &crate::config::AppConfig::default(),
    ));
    let ctx = AgentContext {
        config,
        tx_gui: tx,
        file_event_bus: crate::bus::core::Bus::new(),
        agent_event_bus: crate::bus::core::Bus::new(),
        active_file: None,
        active_dir: None,
        selected_files: HashSet::new(),
        prompt: "Hello".to_string(),
        cancel_flag: Arc::new(AtomicBool::new(false)),
        history: None,
        current_response: String::new(),
        model_name: None,
        session_number: 1,
        session_id: uuid::Uuid::new_v4(),
        browser_session,
        pdf_backing: std::sync::Arc::new(crate::app::session::PdfBackingTracker::new()),
        tool_manager: std::sync::Arc::new(std::sync::RwLock::new(
            crate::agent::tools::manager::ToolManager::new(),
        )),
        uuid_gen: std::sync::Arc::new(crate::utils::uuid::SystemUuidGenerator),
    };
    (ctx, rx)
}

/// Spawn a one-shot HTTP server on a random localhost port and bind
/// the given `body` as the response to the first incoming request.
/// Returns the bound port. The server thread sleeps for 200 ms after
/// writing the response so the client has time to read it before the
/// thread exits.
///
/// Consolidated from 5 inline `TcpListener::bind` + `thread::spawn`
/// blocks that all did the same `accept` + `read` + `write_all` +
/// `sleep` pattern. The bodies they served were the only thing that
/// varied.
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

/// Build a minimal HTTP/1.1 response with the given status line and
/// body. Centralises the `Content-Length` math so individual tests
/// don't have to repeat the `format!` boilerplate.
fn http_response(status_line: &str, body: &str) -> Vec<u8> {
    format!(
        "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

#[test]
fn test_run_agent_missing_api_key() {
    let mut config = AppConfig::default();
    config.models.insert(
        "test".to_string(),
        crate::config::LlmConfig {
            model: "test".to_string(),
            api_url: "http://localhost".to_string(),
            api_key: "".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        },
    );
    let (ctx, rx) = make_ctx(config);
    run_agent(ctx);
    match rx.recv().unwrap() {
        BackgroundEvent::Agent(AgentEvent::Failed(err)) => {
            assert!(err.contains("API key not set"))
        }
        _ => panic!("Expected AgentEvent::Failed"),
    }
}

#[test]
fn test_run_agent_network_error() {
    let mut config = AppConfig::default();
    config.models.insert(
        "test".to_string(),
        crate::config::LlmConfig {
            model: "test".to_string(),
            api_url: "http://127.0.0.1:0".to_string(),
            api_key: "valid-key".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        },
    );
    let (ctx, rx) = make_ctx(config);
    run_agent(ctx);
    let mut got = false;
    while let Ok(ev) = rx.recv() {
        if let BackgroundEvent::Agent(AgentEvent::Failed(err)) = ev {
            assert!(err.contains("Network error") || err.contains("timed out"));
            got = true;
            break;
        }
    }
    assert!(got);
}

#[test]
fn test_run_agent_invalid_json_response() {
    let port = spawn_one_shot_http_server(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\n{");
    let (ctx, rx) = make_ctx(make_config(port));
    run_agent(ctx);
    let mut got = false;
    while let Ok(ev) = rx.recv() {
        if let BackgroundEvent::Agent(AgentEvent::Failed(err)) = ev {
            assert!(err.contains("Failed to parse"));
            got = true;
            break;
        }
    }
    assert!(got);
}

#[test]
fn test_run_agent_http_status_error() {
    let port = spawn_one_shot_http_server(
        b"HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\n\r\nbad request",
    );
    let (ctx, rx) = make_ctx(make_config(port));
    run_agent(ctx);
    let mut got = false;
    while let Ok(ev) = rx.recv() {
        if let BackgroundEvent::Agent(AgentEvent::Failed(err)) = ev {
            assert!(err.contains("HTTP 400 error"));
            got = true;
            break;
        }
    }
    assert!(got);
}

#[test]
fn test_run_agent_missing_choices() {
    let port = spawn_one_shot_http_server(&http_response("HTTP/1.1 200 OK", "{}"));
    let (ctx, rx) = make_ctx(make_config(port));
    run_agent(ctx);
    let mut got = false;
    while let Ok(ev) = rx.recv() {
        if let BackgroundEvent::Agent(AgentEvent::Failed(err)) = ev {
            assert!(err.contains("Invalid response schema"));
            got = true;
            break;
        }
    }
    assert!(got);
}

#[test]
fn test_run_agent_emits_done_status_on_natural_completion() {
    let body = serde_json::json!({
        "id": "chatcmpl-test", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "All done."}, "finish_reason": "stop"}]
    })
    .to_string();
    let port = spawn_one_shot_http_server(&http_response("HTTP/1.1 200 OK", &body));
    let (ctx, rx) = make_ctx(make_config(port));
    run_agent(ctx);
    let mut statuses = Vec::new();
    let mut saw_finished = false;
    while let Ok(ev) = rx.recv() {
        match ev {
            BackgroundEvent::Agent(AgentEvent::Status(s)) => statuses.push(s),
            BackgroundEvent::Agent(AgentEvent::Finished(_)) => {
                saw_finished = true;
                break;
            }
            BackgroundEvent::Agent(AgentEvent::Failed(err)) => panic!("agent failed: {}", err),
            _ => {}
        }
    }
    assert!(saw_finished);
    assert!(statuses.iter().any(|s| s == "Done"));
}

#[test]
fn test_run_agent_skips_done_status_when_cancelled() {
    let body = serde_json::json!({
        "id": "chatcmpl-test", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "All done."}, "finish_reason": "stop"}]
    })
    .to_string();
    let port = spawn_one_shot_http_server(&http_response("HTTP/1.1 200 OK", &body));
    let (tx, rx) = std::sync::mpsc::channel();
    let browser_session = std::sync::Arc::new(crate::app::session::BrowserSession::new(
        &crate::config::AppConfig::default(),
    ));
    let ctx = AgentContext {
        config: make_config(port),
        tx_gui: tx,
        file_event_bus: crate::bus::core::Bus::new(),
        agent_event_bus: crate::bus::core::Bus::new(),
        active_file: None,
        active_dir: None,
        selected_files: HashSet::new(),
        prompt: "Hello".to_string(),
        cancel_flag: Arc::new(AtomicBool::new(true)),
        history: None,
        current_response: String::new(),
        model_name: None,
        session_number: 1,
        session_id: uuid::Uuid::new_v4(),
        browser_session,
        pdf_backing: std::sync::Arc::new(crate::app::session::PdfBackingTracker::new()),
        tool_manager: std::sync::Arc::new(std::sync::RwLock::new(
            crate::agent::tools::manager::ToolManager::new(),
        )),
        uuid_gen: std::sync::Arc::new(crate::utils::uuid::SystemUuidGenerator),
    };
    run_agent(ctx);
    let mut saw_done = false;
    let mut saw_finished = false;
    while let Ok(ev) = rx.recv() {
        match ev {
            BackgroundEvent::Agent(AgentEvent::Status(s)) if s == "Done" => saw_done = true,
            BackgroundEvent::Agent(AgentEvent::Finished(_)) => saw_finished = true,
            _ => {}
        }
    }
    assert!(saw_finished);
    assert!(!saw_done);
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

    // Tool-call flow needs a 2-request server (tool_calls response, then
    // a final assistant response). We use a dedicated helper rather than
    // `spawn_one_shot_http_server` because the latter is single-request.
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
    let (ctx, rx) = make_ctx(make_config(port));
    run_agent(ctx);
    let mut responses = Vec::new();
    while let Ok(ev) = rx.recv() {
        match ev {
            BackgroundEvent::Agent(AgentEvent::Response(resp)) => responses.push(resp),
            BackgroundEvent::Agent(AgentEvent::Finished(_)) => break,
            BackgroundEvent::Agent(AgentEvent::Failed(err)) => panic!("agent failed: {}", err),
            _ => {}
        }
    }
    assert!(
        responses
            .iter()
            .any(|r| r.contains("Executing tool `read_tags`"))
    );
    assert!(responses.iter().any(|r| r.contains("Result (`read_tags`)")));
}

/// R1 (Spotlighting) regression: every `role:tool` message that
/// joins the conversation history must be wrapped in the
/// `EXTERNAL_DATA` datamark envelope. The user-facing chat-panel
/// response (the `Result (...)` line emitted above) is NOT wrapped
/// — that string goes to the UI, not to the LLM. Only the
/// `messages` array that gets sent on the next LLM call is
/// wrapped.
///
/// This test sends a tool-call → final assistant flow, then
/// inspects the `Finished(messages)` payload that `run_agent`
/// emits and asserts that the `role:tool` entry is wrapped.
#[test]
fn test_run_agent_datamarks_tool_results_in_conversation_history() {
    use crate::agent::datamark::{EXTERNAL_DATA_END, EXTERNAL_DATA_START};

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
    let (ctx, rx) = make_ctx(make_config(port));
    run_agent(ctx);

    // Drain the event stream, capturing the Finished history.
    let mut history: Option<Vec<serde_json::Value>> = None;
    while let Ok(ev) = rx.recv() {
        if let BackgroundEvent::Agent(AgentEvent::Finished(messages)) = ev {
            history = Some(messages);
            break;
        }
    }
    let history = history.expect("agent must emit Finished with history");

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

/// R1 (Spotlighting) regression: the system prompt sent on the
/// first LLM call must begin with the security header. We assert
/// this by checking that the *first* message in the history is
/// the system prompt and that it contains the canonical header
/// text. If a future edit accidentally reorders the prompt so
/// the header lands below the role definition, the LLM no longer
/// follows it under adversarial pressure and the test fires.
#[test]
fn test_run_agent_system_prompt_starts_with_security_header() {
    let body = serde_json::json!({
        "id": "chatcmpl-test", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "All done."}, "finish_reason": "stop"}]
    })
    .to_string();
    let port = spawn_one_shot_http_server(&http_response("HTTP/1.1 200 OK", &body));
    let (ctx, rx) = make_ctx(make_config(port));
    run_agent(ctx);

    let mut history: Option<Vec<serde_json::Value>> = None;
    while let Ok(ev) = rx.recv() {
        if let BackgroundEvent::Agent(AgentEvent::Finished(messages)) = ev {
            history = Some(messages);
            break;
        }
    }
    let history = history.expect("agent must emit Finished with history");

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
        content.starts_with(crate::agent::datamark::SECURITY_HEADER),
        "system prompt must start with the security header; got first 200 chars: {:?}",
        &content[..content.len().min(200)]
    );
}

/// AGENT-026 regression: when no file, directory, or selected-files
/// context is handed to the agent (all tabs closed), the system
/// prompt sent to the LLM must not mention any file or directory
/// context. This guards the whole chain: the context computed at
/// prompt start is the only thing the LLM sees.
#[test]
fn test_run_agent_system_prompt_without_context_has_no_file_context() {
    let body = serde_json::json!({
        "id": "chatcmpl-test", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "All done."}, "finish_reason": "stop"}]
    })
    .to_string();
    let port = spawn_one_shot_http_server(&http_response("HTTP/1.1 200 OK", &body));

    // make_ctx already passes active_file=None, active_dir=None,
    // selected_files empty — the state the interactive entry points
    // hand over when all tabs are closed.
    let (ctx, rx) = make_ctx(make_config(port));
    run_agent(ctx);

    let mut history: Option<Vec<serde_json::Value>> = None;
    while let Ok(ev) = rx.recv() {
        if let BackgroundEvent::Agent(AgentEvent::Finished(messages)) = ev {
            history = Some(messages);
            break;
        }
    }
    let history = history.expect("agent must emit Finished with history");

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

/// Verify that the agent emits debug entries during a simple
/// (no tool-call) run: session boundary, outgoing, incoming.
#[test]
fn test_run_agent_emits_debug_entries() {
    let body = serde_json::json!({
        "id": "chatcmpl-test", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "All done."}, "finish_reason": "stop"}]
    })
    .to_string();
    let port = spawn_one_shot_http_server(&http_response("HTTP/1.1 200 OK", &body));
    let (ctx, rx) = make_ctx(make_config(port));
    run_agent(ctx);

    let mut debug_entries: Vec<AgentDebugEntry> = Vec::new();
    while let Ok(ev) = rx.recv() {
        match &ev {
            BackgroundEvent::Agent(AgentEvent::DebugEntry(e)) => debug_entries.push(e.clone()),
            BackgroundEvent::Agent(AgentEvent::Finished(_)) => break,
            _ => {}
        }
    }

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

/// Verify that debug entries are emitted during a tool-call run,
/// including ToolResults entries.
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
    let (ctx, rx) = make_ctx(make_config(port));
    run_agent(ctx);

    let mut debug_entries: Vec<AgentDebugEntry> = Vec::new();
    while let Ok(ev) = rx.recv() {
        match &ev {
            BackgroundEvent::Agent(AgentEvent::DebugEntry(e)) => debug_entries.push(e.clone()),
            BackgroundEvent::Agent(AgentEvent::Finished(_)) => break,
            _ => {}
        }
    }

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

/// Verify that the outgoing debug entry for turn 1 contains the full
/// initial messages (since there is no previous turn to diff against).
#[test]
fn test_debug_outgoing_turn1_includes_full_initial_messages() {
    let body = serde_json::json!({
        "id": "chatcmpl-test", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "All done."}, "finish_reason": "stop"}]
    })
    .to_string();
    let port = spawn_one_shot_http_server(&http_response("HTTP/1.1 200 OK", &body));
    let (ctx, rx) = make_ctx(make_config(port));
    run_agent(ctx);

    let mut outgoing: Vec<AgentDebugEntry> = Vec::new();
    while let Ok(ev) = rx.recv() {
        match &ev {
            BackgroundEvent::Agent(AgentEvent::DebugEntry(e)) => {
                if matches!(e.kind, DebugEntryKind::Outgoing)
                    && matches!(e.row_type, DebugEntryRow::Entry)
                {
                    outgoing.push(e.clone());
                }
            }
            BackgroundEvent::Agent(AgentEvent::Finished(_)) => break,
            _ => {}
        }
    }

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

/// T008: Shadow-assertion test — verifies that `Bus<AgentEvent>` receives
/// the same lifecycle events (SessionStarted, Status, Failed, SessionFinished)
/// as the old `tx_gui` mpsc path. Quickstart scenario 2 precursor.
#[test]
fn test_dual_publish_bus_receives_same_lifecycle_events() {
    let body = serde_json::json!({
        "id": "chatcmpl-1", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "Hi"}, "finish_reason": "stop"}],
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
    })
    .to_string();
    let port = spawn_one_shot_http_server(&http_response("HTTP/1.1 200 OK", &body));
    let (ctx, rx) = make_ctx(make_config(port));
    // Subscribe to the new bus BEFORE spawning the agent so we don't miss events.
    let bus_reader = ctx.agent_event_bus.subscribe();
    let session_id = ctx.session_id;
    run_agent(ctx);

    // Drain the old mpsc path
    let mut legacy_failed = false;
    let mut legacy_finished = false;
    while let Ok(ev) = rx.recv() {
        match ev {
            BackgroundEvent::Agent(AgentEvent::Failed(_)) => legacy_failed = true,
            BackgroundEvent::Agent(AgentEvent::Finished(_)) => legacy_finished = true,
            _ => {}
        }
    }

    // Drain the new bus path
    let mut bus_events: Vec<crate::agent::events::AgentEvent> = Vec::new();
    while let Ok(ev) = bus_reader.try_recv() {
        bus_events.push(ev);
    }

    // Assert SessionStarted is first and carries the correct session_id
    assert!(
        bus_events
            .iter()
            .any(|e| matches!(e, crate::agent::events::AgentEvent::SessionStarted { session_id: sid } if *sid == session_id)),
        "Bus must receive SessionStarted with correct session_id"
    );
    // Assert SessionFinished is last and carries the correct session_id
    assert!(
        bus_events
            .iter()
            .any(|e| matches!(e, crate::agent::events::AgentEvent::SessionFinished { session_id: sid, .. } if *sid == session_id)),
        "Bus must receive SessionFinished with correct session_id"
    );
    // Every bus event carries the same session_id
    for event in &bus_events {
        let sid = match event {
            crate::agent::events::AgentEvent::SessionStarted { session_id } => *session_id,
            crate::agent::events::AgentEvent::SessionFinished { session_id, .. } => *session_id,
            crate::agent::events::AgentEvent::Status { session_id, .. } => *session_id,
            crate::agent::events::AgentEvent::Thinking { session_id, .. } => *session_id,
            crate::agent::events::AgentEvent::ContentDelta { session_id, .. } => *session_id,
            crate::agent::events::AgentEvent::ToolCallStarted { session_id, .. } => *session_id,
            crate::agent::events::AgentEvent::ToolResult { session_id, .. } => *session_id,
            crate::agent::events::AgentEvent::ToolSideEffect { session_id, .. } => *session_id,
            crate::agent::events::AgentEvent::DebugEntry { session_id, .. } => *session_id,
            crate::agent::events::AgentEvent::TokenUsage { session_id, .. } => *session_id,
            crate::agent::events::AgentEvent::Failed { session_id, .. } => *session_id,
        };
        assert_eq!(
            sid, session_id,
            "every AgentEvent must carry the correct session_id"
        );
    }
    // If the legacy path saw Failed, the bus must also see Failed
    if legacy_failed {
        assert!(
            bus_events
                .iter()
                .any(|e| matches!(e, crate::agent::events::AgentEvent::Failed { .. })),
            "Bus must receive Failed if legacy path did"
        );
    }
    // If the legacy path saw Finished, the bus must also see SessionFinished
    if legacy_finished {
        assert!(
            bus_events
                .iter()
                .any(|e| matches!(e, crate::agent::events::AgentEvent::SessionFinished { .. })),
            "Bus must receive SessionFinished if legacy path saw Finished"
        );
    }
}
