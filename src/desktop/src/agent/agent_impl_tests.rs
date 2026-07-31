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

fn make_ctx(
    config: AppConfig,
) -> (AgentContext, std::sync::mpsc::Receiver<BackgroundEvent>) {
    let (tx, rx) = std::sync::mpsc::channel();
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
    };
    (ctx, rx)
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
    use std::io::{Read, Write};
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0; 2048];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\n{");
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    });
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
    use std::io::{Read, Write};
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0; 2048];
            let _ = stream.read(&mut buf);
            let _ = stream
                .write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\n\r\nbad request");
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    });
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
    use std::io::{Read, Write};
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0; 2048];
            let _ = stream.read(&mut buf);
            let body = "{}";
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    });
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
    use std::io::{Read, Write};
    use std::net::TcpListener;
    let body = serde_json::json!({
        "id": "chatcmpl-test", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "All done."}, "finish_reason": "stop"}]
    }).to_string();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
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
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    });
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
    use std::io::{Read, Write};
    use std::net::TcpListener;
    let body = serde_json::json!({
        "id": "chatcmpl-test", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "All done."}, "finish_reason": "stop"}]
    }).to_string();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
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
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    });
    let (tx, rx) = std::sync::mpsc::channel();
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
    use std::io::{Read, Write};
    use std::net::TcpListener;
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
    }).to_string();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0; 8192];
            let _ = stream.read(&mut buf);
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                tool_call_body.len(),
                tool_call_body
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0; 8192];
            let _ = stream.read(&mut buf);
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                final_body.len(),
                final_body
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
            std::thread::sleep(std::time::Duration::from_millis(100));
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
