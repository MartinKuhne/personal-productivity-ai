//! Integration tests for `run_agent` — mock HTTP server simulating LLM responses, verifying tool calls, streaming, and cancellation.

use crate::agent::agent_impl::run_agent;
use crate::agent::context::AgentContext;
use crate::agent::events::AgentEvent as SeamAgentEvent;
use crate::bus::core::BusReader;
use crate::bus::events::debug::{AgentDebugEntry, DebugEntryKind, DebugEntryRow};
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

fn make_ctx(config: AppConfig) -> (AgentContext, BusReader<SeamAgentEvent>) {
    let browser_session = std::sync::Arc::new(crate::app::session::BrowserSession::new(
        &crate::config::AppConfig::default(),
    ));
    let agent_event_bus = crate::bus::core::Bus::new();
    let bus_reader = agent_event_bus.subscribe();
    let ctx = AgentContext {
        config,
        file_event_bus: crate::bus::core::Bus::new(),
        agent_event_bus,
        active_file: None,
        active_dir: None,
        selected_files: HashSet::new(),
        prompt: "Hello".to_string(),
        cancel_flag: Arc::new(AtomicBool::new(false)),
        history: None,
        model_name: None,
        session_id: uuid::Uuid::new_v4(),
        browser_session,
        pdf_backing: std::sync::Arc::new(crate::app::session::PdfBackingTracker::new()),
        cache: std::sync::Arc::new(crate::agent::tools::manager::cache::ToolCache::new()),
        tool_manager: std::sync::Arc::new(std::sync::RwLock::new(
            crate::agent::tools::manager::ToolManager::new(),
        )),
        uuid_gen: std::sync::Arc::new(crate::utils::uuid::SystemUuidGenerator),
    };
    (ctx, bus_reader)
}

/// Collect bus events until `SessionFinished` is seen or `timeout` elapses.
/// After `SessionFinished`, drains any remaining buffered events.
fn collect_bus_events(
    reader: &mut BusReader<SeamAgentEvent>,
    timeout: std::time::Duration,
) -> Vec<SeamAgentEvent> {
    let mut events = Vec::new();
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        while let Ok(ev) = reader.try_recv() {
            let is_finished = matches!(ev, SeamAgentEvent::SessionFinished { .. });
            events.push(ev);
            if is_finished {
                while let Ok(ev) = reader.try_recv() {
                    events.push(ev);
                }
                return events;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    events
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
    let (ctx, mut bus_reader) = make_ctx(config);
    run_agent(ctx);
    let events = collect_bus_events(&mut bus_reader, std::time::Duration::from_secs(2));
    assert!(
        events.iter().any(|e| matches!(
            e,
            SeamAgentEvent::Failed { error, .. } if error.contains("API key not set")
        )),
        "expected Failed with API key message; got: {:?}",
        events
    );
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
    let (ctx, mut bus_reader) = make_ctx(config);
    run_agent(ctx);
    let events = collect_bus_events(&mut bus_reader, std::time::Duration::from_secs(30));
    assert!(
        events.iter().any(|e| matches!(
            e,
            SeamAgentEvent::Failed { error, .. } if error.contains("Network error") || error.contains("timed out")
        )),
        "expected Failed with network error; got: {:?}",
        events
    );
}

#[test]
fn test_run_agent_invalid_json_response() {
    let port = spawn_one_shot_http_server(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\n{");
    let (ctx, mut bus_reader) = make_ctx(make_config(port));
    run_agent(ctx);
    let events = collect_bus_events(&mut bus_reader, std::time::Duration::from_secs(5));
    assert!(
        events.iter().any(|e| matches!(
            e,
            SeamAgentEvent::Failed { error, .. } if error.contains("Failed to parse")
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
    let (ctx, mut bus_reader) = make_ctx(make_config(port));
    run_agent(ctx);
    let events = collect_bus_events(&mut bus_reader, std::time::Duration::from_secs(5));
    assert!(
        events.iter().any(|e| matches!(
            e,
            SeamAgentEvent::Failed { error, .. } if error.contains("HTTP 400 error")
        )),
        "expected Failed with HTTP 400 error; got: {:?}",
        events
    );
}

#[test]
fn test_run_agent_missing_choices() {
    let port = spawn_one_shot_http_server(&http_response("HTTP/1.1 200 OK", "{}"));
    let (ctx, mut bus_reader) = make_ctx(make_config(port));
    run_agent(ctx);
    let events = collect_bus_events(&mut bus_reader, std::time::Duration::from_secs(5));
    assert!(
        events.iter().any(|e| matches!(
            e,
            SeamAgentEvent::Failed { error, .. } if error.contains("Invalid response schema")
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
    let (ctx, mut bus_reader) = make_ctx(make_config(port));
    run_agent(ctx);
    let events = collect_bus_events(&mut bus_reader, std::time::Duration::from_secs(5));
    let saw_finished = events
        .iter()
        .any(|e| matches!(e, SeamAgentEvent::SessionFinished { .. }));
    assert!(saw_finished, "must see SessionFinished");
    let saw_done = events.iter().any(|e| {
        matches!(
            e,
            SeamAgentEvent::Status {
                status: crate::agent::events::AgentStatus::Done,
                ..
            }
        )
    });
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
    let browser_session = std::sync::Arc::new(crate::app::session::BrowserSession::new(
        &crate::config::AppConfig::default(),
    ));
    let agent_event_bus = crate::bus::core::Bus::new();
    let mut bus_reader = agent_event_bus.subscribe();
    let ctx = AgentContext {
        config: make_config(port),
        file_event_bus: crate::bus::core::Bus::new(),
        agent_event_bus,
        active_file: None,
        active_dir: None,
        selected_files: HashSet::new(),
        prompt: "Hello".to_string(),
        cancel_flag: Arc::new(AtomicBool::new(true)),
        history: None,
        model_name: None,
        session_id: uuid::Uuid::new_v4(),
        browser_session,
        pdf_backing: std::sync::Arc::new(crate::app::session::PdfBackingTracker::new()),
        cache: std::sync::Arc::new(crate::agent::tools::manager::cache::ToolCache::new()),
        tool_manager: std::sync::Arc::new(std::sync::RwLock::new(
            crate::agent::tools::manager::ToolManager::new(),
        )),
        uuid_gen: std::sync::Arc::new(crate::utils::uuid::SystemUuidGenerator),
    };
    run_agent(ctx);
    let events = collect_bus_events(&mut bus_reader, std::time::Duration::from_secs(5));
    let saw_finished = events
        .iter()
        .any(|e| matches!(e, SeamAgentEvent::SessionFinished { .. }));
    let saw_done = events.iter().any(|e| {
        matches!(
            e,
            SeamAgentEvent::Status {
                status: crate::agent::events::AgentStatus::Done,
                ..
            }
        )
    });
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
    let (ctx, mut bus_reader) = make_ctx(make_config(port));
    run_agent(ctx);
    let events = collect_bus_events(&mut bus_reader, std::time::Duration::from_secs(5));
    assert!(
        events.iter().any(|e| matches!(
            e,
            SeamAgentEvent::ToolCallStarted { name, .. } if name == "read_tags"
        )),
        "must see ToolCallStarted for read_tags; events: {:?}",
        events
    );
    assert!(
        events.iter().any(|e| matches!(
            e,
            SeamAgentEvent::ToolResult { name, .. } if name == "read_tags"
        )),
        "must see ToolResult for read_tags; events: {:?}",
        events
    );
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
    let (ctx, mut bus_reader) = make_ctx(make_config(port));
    run_agent(ctx);

    // Drain the event stream, capturing the SessionFinished history.
    let events = collect_bus_events(&mut bus_reader, std::time::Duration::from_secs(5));
    let history = events
        .iter()
        .find_map(|e| match e {
            SeamAgentEvent::SessionFinished { history, .. } => Some(history.clone()),
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
    let (ctx, mut bus_reader) = make_ctx(make_config(port));
    run_agent(ctx);

    let events = collect_bus_events(&mut bus_reader, std::time::Duration::from_secs(5));
    let history = events
        .iter()
        .find_map(|e| match e {
            SeamAgentEvent::SessionFinished { history, .. } => Some(history.clone()),
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
    let (ctx, mut bus_reader) = make_ctx(make_config(port));
    run_agent(ctx);

    let events = collect_bus_events(&mut bus_reader, std::time::Duration::from_secs(5));
    let history = events
        .iter()
        .find_map(|e| match e {
            SeamAgentEvent::SessionFinished { history, .. } => Some(history.clone()),
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
    let (ctx, mut bus_reader) = make_ctx(make_config(port));
    run_agent(ctx);

    let events = collect_bus_events(&mut bus_reader, std::time::Duration::from_secs(5));
    let debug_entries: Vec<AgentDebugEntry> = events
        .iter()
        .filter_map(|e| match e {
            SeamAgentEvent::DebugEntry { entry, .. } => Some(entry.clone()),
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
    let (ctx, mut bus_reader) = make_ctx(make_config(port));
    run_agent(ctx);

    let events = collect_bus_events(&mut bus_reader, std::time::Duration::from_secs(5));
    let debug_entries: Vec<AgentDebugEntry> = events
        .iter()
        .filter_map(|e| match e {
            SeamAgentEvent::DebugEntry { entry, .. } => Some(entry.clone()),
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
    let (ctx, mut bus_reader) = make_ctx(make_config(port));
    run_agent(ctx);

    let events = collect_bus_events(&mut bus_reader, std::time::Duration::from_secs(5));
    let outgoing: Vec<AgentDebugEntry> = events
        .iter()
        .filter_map(|e| match e {
            SeamAgentEvent::DebugEntry { entry, .. } => {
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

/// T008: Bus lifecycle test — verifies that `Bus<AgentEvent>` receives
/// the expected lifecycle events (SessionStarted, Status, SessionFinished)
/// and every event carries the correct session_id. Quickstart scenario 2.
#[test]
fn test_dual_publish_bus_receives_same_lifecycle_events() {
    let body = serde_json::json!({
        "id": "chatcmpl-1", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "Hi"}, "finish_reason": "stop"}],
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
    })
    .to_string();
    let port = spawn_one_shot_http_server(&http_response("HTTP/1.1 200 OK", &body));
    let (ctx, mut bus_reader) = make_ctx(make_config(port));
    let session_id = ctx.session_id;
    run_agent(ctx);

    let bus_events = collect_bus_events(&mut bus_reader, std::time::Duration::from_secs(5));

    // Assert SessionStarted is present and carries the correct session_id
    assert!(
        bus_events
            .iter()
            .any(|e| matches!(e, SeamAgentEvent::SessionStarted { session_id: sid } if *sid == session_id)),
        "Bus must receive SessionStarted with correct session_id"
    );
    // Assert SessionFinished is present and carries the correct session_id
    assert!(
        bus_events
            .iter()
            .any(|e| matches!(e, SeamAgentEvent::SessionFinished { session_id: sid, .. } if *sid == session_id)),
        "Bus must receive SessionFinished with correct session_id"
    );
    // Every bus event carries the same session_id
    for event in &bus_events {
        assert_eq!(
            event.session_id(),
            session_id,
            "every AgentEvent must carry the correct session_id"
        );
    }
}
