use crate::agent::agent_impl::run_agent;
use crate::agent::context::AgentContext;
use crate::config::AppConfig;
use crate::messages::BackgroundMessage;
use std::collections::HashSet;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

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

fn make_ctx(config: AppConfig) -> (AgentContext, std::sync::mpsc::Receiver<BackgroundMessage>) {
    let (tx, rx) = std::sync::mpsc::channel();
    let ctx = AgentContext::new(
        config,
        tx,
        crate::file_events::Bus::new(),
        None,
        None,
        HashSet::new(),
        "Hello".to_string(),
        Arc::new(AtomicBool::new(false)),
        None,
        String::new(),
    );
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
        BackgroundMessage::AgentFailed(err) => assert!(err.contains("API key not set")),
        _ => panic!("Expected AgentFailed"),
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
    while let Ok(msg) = rx.recv() {
        if let BackgroundMessage::AgentFailed(err) = msg {
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
    while let Ok(msg) = rx.recv() {
        if let BackgroundMessage::AgentFailed(err) = msg {
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
    while let Ok(msg) = rx.recv() {
        if let BackgroundMessage::AgentFailed(err) = msg {
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
    while let Ok(msg) = rx.recv() {
        if let BackgroundMessage::AgentFailed(err) = msg {
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
            let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    });
    let (ctx, rx) = make_ctx(make_config(port));
    run_agent(ctx);
    let mut statuses = Vec::new();
    let mut saw_finished = false;
    while let Ok(msg) = rx.recv() {
        match msg {
            BackgroundMessage::AgentStatus(s) => statuses.push(s),
            BackgroundMessage::AgentFinished(_) => {
                saw_finished = true;
                break;
            }
            BackgroundMessage::AgentFailed(err) => panic!("agent failed: {}", err),
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
            let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    });
    let (tx, rx) = std::sync::mpsc::channel();
    let ctx = AgentContext::new(
        make_config(port),
        tx,
        crate::file_events::Bus::new(),
        None,
        None,
        HashSet::new(),
        "Hello".to_string(),
        Arc::new(AtomicBool::new(true)),
        None,
        String::new(),
    );
    run_agent(ctx);
    let mut saw_done = false;
    let mut saw_finished = false;
    while let Ok(msg) = rx.recv() {
        match msg {
            BackgroundMessage::AgentStatus(s) if s == "Done" => saw_done = true,
            BackgroundMessage::AgentFinished(_) => saw_finished = true,
            _ => {}
        }
    }
    assert!(saw_finished);
    assert!(!saw_done);
}
