//! Integration tests for `run_agent` — mock HTTP server simulating LLM responses, verifying tool calls, streaming, and cancellation.

use crate::agent::agent_impl::run_agent;
use crate::agent::context::AgentContext;
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
    let browser_session = std::sync::Arc::new(crate::app::browser::BrowserSession::new(
        &crate::config::AppConfig::default(),
    ));
    let ctx = AgentContext {
        config,
        tx_gui: tx,
        file_event_bus: crate::bus::core::Bus::new(),
        active_file: None,
        active_dir: None,
        selected_files: HashSet::new(),
        prompt: "Hello".to_string(),
        cancel_flag: Arc::new(AtomicBool::new(false)),
        history: None,
        current_response: String::new(),
        model_name: None,
        browser_session,
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
    let browser_session = std::sync::Arc::new(crate::app::browser::BrowserSession::new(
        &crate::config::AppConfig::default(),
    ));
    let ctx = AgentContext {
        config: make_config(port),
        tx_gui: tx,
        file_event_bus: crate::bus::core::Bus::new(),
        active_file: None,
        active_dir: None,
        selected_files: HashSet::new(),
        prompt: "Hello".to_string(),
        cancel_flag: Arc::new(AtomicBool::new(true)),
        history: None,
        current_response: String::new(),
        model_name: None,
        browser_session,
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
