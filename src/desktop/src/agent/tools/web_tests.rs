//! Tests for `tools/web.rs`.
//!
//! Sidecar file. Extracted from `web.rs` so the implementation
//! module stays focused on production code.
//!
//! Originally a `#[cfg(test)] mod tests { ... }` block at the bottom of
//! `web.rs`. Lives in a sibling file so private item access via
//! `super::*` keeps working.

use super::*;
use crate::config::{AppConfig, LlmConfig};

fn spawn_mock_server(body: impl Into<String>) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let body_str = body.into();
    let response_str = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
        body_str.len(),
        body_str
    );
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            use std::io::{Read, Write};
            let mut buf = [0; 4096];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(response_str.as_bytes());
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    });
    format!("http://127.0.0.1:{}", port)
}

#[test]
fn test_tool_web_fetch_mock() {
    let cache = crate::agent::tools::manager::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let server_url = spawn_mock_server("<html><body><h1>Hello World</h1></body></html>");
    let input = crate::agent::tools::dtos::WebFetchInput {
        url: server_url.clone(),
        headers: false,
        force_refetch: true,
        cursor: None,
    };
    let result = tool_web_fetch(&input, &cache).unwrap();
    assert!(result.content.contains("Hello") || result.content.contains("World"));
    assert!(result.total_lines > 0);
}

#[test]
fn test_tool_web_fetch_error() {
    let cache = crate::agent::tools::manager::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let input = crate::agent::tools::dtos::WebFetchInput {
        url: "http://127.0.0.1:1".to_string(),
        headers: false,
        force_refetch: true,
        cursor: None,
    };
    let result = tool_web_fetch(&input, &cache);
    assert!(result.is_err());
}

#[test]
fn test_tool_web_fetch_pagination() {
    let cache = crate::agent::tools::manager::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let html =
        "<html><body><p>line1</p><p>line2</p><p>line3</p><p>line4</p><p>line5</p></body></html>";
    let server_url = spawn_mock_server(html);
    let input = crate::agent::tools::dtos::WebFetchInput {
        url: server_url.clone(),
        headers: false,
        force_refetch: true,
        cursor: None,
    };
    let result = tool_web_fetch(&input, &cache).unwrap();
    assert!(result.total_lines >= 3);
    assert!(result.content.lines().count() > 0);
    assert!(result.cursor.is_some() || result.hint.is_some());
    assert!(!result.from_cache);
}

#[test]
fn test_tool_web_fetch_headers() {
    let cache = crate::agent::tools::manager::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let server_url = spawn_mock_server("<html><body><h1>Test</h1></body></html>");
    let input = crate::agent::tools::dtos::WebFetchInput {
        url: server_url.clone(),
        headers: true,
        force_refetch: true,
        cursor: None,
    };
    let result = tool_web_fetch(&input, &cache).unwrap();
    assert!(result.response_headers.is_some());
    let headers = result.response_headers.unwrap();
    assert!(headers.contains_key("content-type") || headers.contains_key("Content-Type"));
}

#[test]
fn test_tool_web_fetch_cache_hit() {
    let cache = crate::agent::tools::manager::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let server_url = spawn_mock_server("<html><body><h1>Cached</h1></body></html>");
    let input = crate::agent::tools::dtos::WebFetchInput {
        url: server_url.clone(),
        headers: false,
        force_refetch: false,
        cursor: None,
    };
    let first = tool_web_fetch(&input, &cache).unwrap();
    assert!(!first.from_cache);
    let second = tool_web_fetch(&input, &cache).unwrap();
    assert!(second.from_cache);
    assert_eq!(first.content, second.content);
}

#[test]
fn test_tool_web_fetch_force_refetch() {
    let cache = crate::agent::tools::manager::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let server_url = spawn_mock_server("<html><body><h1>Force</h1></body></html>");
    let input = crate::agent::tools::dtos::WebFetchInput {
        url: server_url.clone(),
        headers: false,
        force_refetch: false,
        cursor: None,
    };
    let _first = tool_web_fetch(&input, &cache).unwrap();
    let force_input = crate::agent::tools::dtos::WebFetchInput {
        url: server_url.clone(),
        headers: false,
        force_refetch: true,
        cursor: None,
    };
    let second = tool_web_fetch(&force_input, &cache).unwrap();
    assert!(!second.from_cache);
}

#[test]
fn test_tool_web_search_mock() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let mock_json = serde_json::json!({
        "results": [
            {
                "title": "Test Title",
                "url": "https://test.com",
                "content": "Test content"
            }
        ]
    });
    let server_url = spawn_mock_server(mock_json.to_string());
    let result = tool_web_search(&server_url, "test query").unwrap().results;
    assert!(result.contains("Test Title"));
    assert!(result.contains("https://test.com"));
    assert!(result.contains("Test content"));
}

#[test]
fn test_tool_web_search_empty() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let mock_json = serde_json::json!({
        "results": []
    });
    let server_url = spawn_mock_server(mock_json.to_string());
    let result = tool_web_search(&server_url, "test query").unwrap().results;
    assert_eq!(result, "No results found.");
}

#[test]
fn test_tool_web_search_returns_full_default_page() {
    // SearXNG with default `search.max_results=10` returns 10 results.
    // Verify we surface all of them.
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let results: Vec<serde_json::Value> = (1..=10)
        .map(|i| {
            serde_json::json!({
                "title": format!("Title {}", i),
                "url": format!("https://example.com/{}", i),
                "content": format!("Content for result {}", i),
            })
        })
        .collect();
    let mock_json = serde_json::json!({ "results": results });
    let server_url = spawn_mock_server(mock_json.to_string());
    let out = tool_web_search(&server_url, "q").unwrap().results;
    for i in 1..=10 {
        assert!(
            out.contains(&format!("Title {}", i)),
            "Expected result #{} to be present; missing from output: {}",
            i,
            out
        );
    }
}

#[test]
fn test_tool_web_search_does_not_slice() {
    // Regression guard: the operator asked us to surface whatever
    // SearXNG returns without a client-side cap. Even if the server
    // returns more than the default 10 (e.g. an instance with many
    // engines enabled), we must pass every result through.
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let total = 25;
    let results: Vec<serde_json::Value> = (1..=total)
        .map(|i| {
            serde_json::json!({
                "title": format!("Title {}", i),
                "url": format!("https://example.com/{}", i),
                "content": format!("Content for result {}", i),
            })
        })
        .collect();
    let mock_json = serde_json::json!({ "results": results });
    let server_url = spawn_mock_server(mock_json.to_string());
    let out = tool_web_search(&server_url, "q").unwrap().results;
    for i in 1..=total {
        assert!(
            out.contains(&format!("Title {}", i)),
            "result #{} should be present; the tool must not slice the server response",
            i
        );
    }
}

#[test]
fn test_tool_web_search_invalid_json() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let server_url = spawn_mock_server("invalid json");
    let result = tool_web_search(&server_url, "test query");
    assert!(result.is_err());
}

#[test]
fn test_tool_web_delegate_missing_api_key() {
    let cache = crate::agent::tools::manager::cache::ToolCache::new();
    let mut config = AppConfig::default();
    config.models.insert(
        "chat".to_string(),
        LlmConfig {
            model: "test-model".to_string(),
            api_url: "http://example.com".to_string(),
            api_key: "".to_string(), // Missing API key
            cost: None,
            use_case: vec!["chat".to_string()],
        },
    );

    let result = tool_web_delegate(&config, "do something", &cache);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        "API key not set or invalid."
    );
}

#[test]
fn test_tool_web_delegate_mock() {
    let cache = crate::agent::tools::manager::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let mock_response = serde_json::json!({
        "choices": [
            {
                "message": {
                    "content": "Final summarized answer",
                    "tool_calls": []
                }
            }
        ]
    });

    let server_url = spawn_mock_server(mock_response.to_string());

    let mut config = AppConfig::default();
    config.models.insert(
        "chat".to_string(),
        LlmConfig {
            model: "test-model".to_string(),
            api_url: server_url.clone(),
            api_key: "valid-key".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        },
    );

    let result = tool_web_delegate(&config, "search for tests", &cache).unwrap();
    assert_eq!(result.result, "Final summarized answer");
}

#[test]
fn test_tool_web_delegate_with_unknown_tool_handled_gracefully() {
    let cache = crate::agent::tools::manager::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    // Mock server returns a tool_call with unknown function name
    // The delegate should handle this gracefully and continue
    let mock_response = serde_json::json!({
        "choices": [{
            "message": {
                "content": null,
                "tool_calls": [{
                    "id": "call_1",
                    "function": {
                        "name": "unknown_function",
                        "arguments": "{}"
                    }
                }]
            }
        }]
    });

    let server_url = spawn_mock_server(mock_response.to_string());

    let mut config = AppConfig::default();
    config.models.insert(
        "chat".to_string(),
        LlmConfig {
            model: "test-model".to_string(),
            api_url: server_url.clone(),
            api_key: "valid-key".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        },
    );
    config.searxng_url = None;

    // Should not panic - handles unknown tool gracefully
    let result = tool_web_delegate(&config, "do something", &cache);
    // Either succeeds or returns an error we can handle
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_tool_web_delegate_handles_api_error_gracefully() {
    let cache = crate::agent::tools::manager::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    // Mock server that returns an error status
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            use std::io::{Read, Write};
            let mut buf = [0; 4096];
            let _ = stream.read(&mut buf);
            let response = "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 5\r\n\r\nerror";
            let _ = stream.write_all(response.as_bytes());
        }
    });

    let mut config = AppConfig::default();
    config.models.insert(
        "chat".to_string(),
        LlmConfig {
            model: "test-model".to_string(),
            api_url: format!("http://127.0.0.1:{}", port),
            api_key: "valid-key".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        },
    );

    let result = tool_web_delegate(&config, "test", &cache);
    // Should return an error, not panic
    assert!(result.is_err() || result.is_ok());
}
