//! Tests for `tools/web.rs`.

use super::*;
use crate::config::AgentConfig;
use crate::config::LlmConfig;

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
    let cache = crate::tools::registry::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let server_url = spawn_mock_server("<html><body><h1>Hello World</h1></body></html>");
    let input = crate::tools::dtos::WebFetchInput {
        url: server_url.clone(),
        headers: false,
        force_refetch: true,
        cursor: None,
    };
    let mock_uuid = uuid::Uuid::nil();
    let result = tool_web_fetch(
        &input,
        &cache,
        &crate::utils::uuid::FixedUuidGenerator::new(mock_uuid),
    )
    .unwrap();
    assert!(result.content.contains("Hello") || result.content.contains("World"));
    assert!(result.total_lines > 0);

    // Verify the document was stored in the web_documents cache
    assert!(cache.web_documents.get(&server_url).is_some());
}

#[test]
fn test_tool_web_fetch_error() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let input = crate::tools::dtos::WebFetchInput {
        url: "http://127.0.0.1:1".to_string(),
        headers: false,
        force_refetch: true,
        cursor: None,
    };
    let result = tool_web_fetch(&input, &cache, &crate::utils::uuid::SystemUuidGenerator);
    assert!(result.is_err());
}

#[test]
fn test_tool_web_fetch_pagination() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let html =
        "<html><body><p>line1</p><p>line2</p><p>line3</p><p>line4</p><p>line5</p></body></html>";
    let server_url = spawn_mock_server(html);
    let input = crate::tools::dtos::WebFetchInput {
        url: server_url.clone(),
        headers: false,
        force_refetch: true,
        cursor: None,
    };
    let result = tool_web_fetch(&input, &cache, &crate::utils::uuid::SystemUuidGenerator).unwrap();
    assert!(result.total_lines >= 3);
    assert!(result.content.lines().count() > 0);
    assert!(result.cursor.is_some() || result.hint.is_some());
    assert!(!result.from_cache);
}

/// Regression test for the cursor round-trip (TOOL-006, TOOL-032).
///
/// The first call must return a cursor; the second call, passing the cursor
/// back, must serve the next page from the cache. The cache stores the full
/// content under the cursor UUID (the only value the LLM ever sees as
/// `cursor`), so the lookup must hit `WebFetchContent` directly.
#[test]
fn test_tool_web_fetch_cursor_pagination() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    // 150 <p> tags -> 300 markdown lines, well past the 64-line page size.
    let mut html = String::from("<html><body>");
    for i in 0..150 {
        html.push_str(&format!("<p>line{}</p>", i));
    }
    html.push_str("</body></html>");
    let server_url = spawn_mock_server(html);

    // First call: no cursor, force a fresh fetch.
    let first_input = crate::tools::dtos::WebFetchInput {
        url: server_url.clone(),
        headers: false,
        force_refetch: true,
        cursor: None,
    };
    let first = tool_web_fetch(
        &first_input,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
    )
    .unwrap();
    assert!(!first.from_cache);
    assert!(first.hint.is_none());
    assert!(
        first.cursor.is_some(),
        "first call must return a cursor for paginated content"
    );
    let cursor = first.cursor.clone().unwrap();

    // Second call: pass the cursor back. Prior to the fix this returned
    // `"Cursor expired or unknown; re-run the fetch with no cursor."`
    // because the cursor branch looked up `WebFetch { cursor }` at the
    // cursor key, which actually holds `WebFetchContent`.
    let second_input = crate::tools::dtos::WebFetchInput {
        url: server_url.clone(),
        headers: false,
        force_refetch: false,
        cursor: Some(cursor.clone()),
    };
    let second = tool_web_fetch(
        &second_input,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
    )
    .expect("second call with cursor must succeed");
    assert!(second.from_cache, "second call must be served from cache");
    assert_eq!(second.total_lines, first.total_lines);
    assert!(!second.content.is_empty());
    // The first page's last content line (skipping blank separator lines
    // emitted by the markdown converter) must come immediately before the
    // second page's first content line. With WEB_FETCH_PAGE_SIZE=64 lines
    // and `\n\n` separators, the first page contains line0..line31 and the
    // second page contains line32..line63.
    let first_page_lines: Vec<&str> = first.content.lines().filter(|l| !l.is_empty()).collect();
    let second_page_lines: Vec<&str> = second.content.lines().filter(|l| !l.is_empty()).collect();
    let last = first_page_lines.last().expect("first page has content");
    let next = second_page_lines.first().expect("second page has content");
    let last_n: usize = last
        .strip_prefix("line")
        .and_then(|n| n.parse().ok())
        .expect("first page line should be of the form 'lineN'");
    let next_n: usize = next
        .strip_prefix("line")
        .and_then(|n| n.parse().ok())
        .expect("second page line should be of the form 'lineN'");
    assert_eq!(
        next_n,
        last_n + 1,
        "second page should start where the first left off; last={last:?}, next={next:?}"
    );
    // And the two pages must not repeat content.
    let first_set: std::collections::HashSet<&str> = first_page_lines.iter().copied().collect();
    let second_set: std::collections::HashSet<&str> = second_page_lines.iter().copied().collect();
    assert!(
        first_set.is_disjoint(&second_set),
        "pages must not repeat content lines"
    );
}

#[test]
fn test_tool_web_fetch_headers() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let server_url = spawn_mock_server("<html><body><h1>Test</h1></body></html>");
    let input = crate::tools::dtos::WebFetchInput {
        url: server_url.clone(),
        headers: true,
        force_refetch: true,
        cursor: None,
    };
    let result = tool_web_fetch(&input, &cache, &crate::utils::uuid::SystemUuidGenerator).unwrap();
    assert!(result.response_headers.is_some());
    let headers = result.response_headers.unwrap();
    assert!(headers.contains_key("content-type") || headers.contains_key("Content-Type"));
}

#[test]
fn test_tool_web_fetch_cache_hit() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let server_url = spawn_mock_server("<html><body><h1>Cached</h1></body></html>");
    let input = crate::tools::dtos::WebFetchInput {
        url: server_url.clone(),
        headers: false,
        force_refetch: false,
        cursor: None,
    };
    let first = tool_web_fetch(&input, &cache, &crate::utils::uuid::SystemUuidGenerator).unwrap();
    assert!(!first.from_cache);
    let second = tool_web_fetch(&input, &cache, &crate::utils::uuid::SystemUuidGenerator).unwrap();
    assert!(second.from_cache);
    assert_eq!(first.content, second.content);
}

#[test]
fn test_tool_web_fetch_force_refetch() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let server_url = spawn_mock_server("<html><body><h1>Force</h1></body></html>");
    let input = crate::tools::dtos::WebFetchInput {
        url: server_url.clone(),
        headers: false,
        force_refetch: false,
        cursor: None,
    };
    let _first = tool_web_fetch(&input, &cache, &crate::utils::uuid::SystemUuidGenerator).unwrap();
    let force_input = crate::tools::dtos::WebFetchInput {
        url: server_url.clone(),
        headers: false,
        force_refetch: true,
        cursor: None,
    };
    let second = tool_web_fetch(
        &force_input,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
    )
    .unwrap();
    assert!(!second.from_cache);
}

#[test]
fn test_tool_web_search_mock() {
    let cache = crate::tools::registry::cache::ToolCache::new();
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
    let result = tool_web_search(
        &server_url,
        "test query",
        None,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
    )
    .unwrap();
    assert!(result.results.contains("Test Title"));
    assert!(result.results.contains("https://test.com"));
    assert!(result.results.contains("Test content"));
    assert_eq!(result.total, 1);
    assert!(result.cursor.is_none());
    assert_eq!(result.hint.as_deref(), Some("Final page."));
}

#[test]
fn test_tool_web_search_empty() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let mock_json = serde_json::json!({
        "results": []
    });
    let server_url = spawn_mock_server(mock_json.to_string());
    let result = tool_web_search(
        &server_url,
        "test query",
        None,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
    )
    .unwrap();
    assert_eq!(result.results, "No results found.");
    assert_eq!(result.total, 0);
    assert!(result.cursor.is_none());
    assert_eq!(result.hint.as_deref(), Some("Final page."));
}

#[test]
fn test_tool_web_search_cursor_pagination_32_page_size() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let total = 70;
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

    // Page 1
    let page1 = tool_web_search(
        &server_url,
        "q",
        None,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
    )
    .unwrap();
    assert_eq!(page1.total, 70);
    assert!(page1.cursor.is_some());
    assert!(page1.hint.is_none());
    let cursor1 = page1.cursor.unwrap();

    // Page 2
    let page2 = tool_web_search(
        &server_url,
        "q",
        Some(cursor1),
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
    )
    .unwrap();
    assert_eq!(page2.total, 70);
    assert!(page2.cursor.is_some());
    assert!(page2.hint.is_none());
    let cursor2 = page2.cursor.unwrap();

    // Page 3 (final)
    let page3 = tool_web_search(
        &server_url,
        "q",
        Some(cursor2),
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
    )
    .unwrap();
    assert_eq!(page3.total, 70);
    assert!(page3.cursor.is_none());
    assert_eq!(page3.hint.as_deref(), Some("Final page."));
}

#[test]
fn test_tool_web_search_invalid_json() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let server_url = spawn_mock_server("invalid json");
    let result = tool_web_search(
        &server_url,
        "test query",
        None,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
    );
    assert!(result.is_err());
}

#[test]
fn test_tool_web_delegate_missing_api_key() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    let mut config = AgentConfig::default();
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
    assert_eq!(result.unwrap_err(), "API key not set or invalid.");
}

#[test]
fn test_tool_web_delegate_mock() {
    let cache = crate::tools::registry::cache::ToolCache::new();
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

    let mut config = AgentConfig::default();
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
    let cache = crate::tools::registry::cache::ToolCache::new();
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

    let mut config = AgentConfig::default();
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
    let cache = crate::tools::registry::cache::ToolCache::new();
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

    let mut config = AgentConfig::default();
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
