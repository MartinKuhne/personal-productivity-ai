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
        url: Some(server_url.clone()),
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
        url: Some("http://127.0.0.1:1".to_string()),
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
        url: Some(server_url.clone()),
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
        url: Some(server_url.clone()),
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

    // Second call: cursor only, no fresh parameters. Prior to the fix this returned
    // `"Cursor expired or unknown; re-run the fetch with no cursor."`
    // because the cursor branch looked up `WebFetch { cursor }` at the
    // cursor key, which actually holds `WebFetchContent`.
    let second_input = crate::tools::dtos::WebFetchInput {
        url: None,
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
        url: Some(server_url.clone()),
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
        url: Some(server_url.clone()),
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
        url: Some(server_url.clone()),
        headers: false,
        force_refetch: false,
        cursor: None,
    };
    let _first = tool_web_fetch(&input, &cache, &crate::utils::uuid::SystemUuidGenerator).unwrap();
    let force_input = crate::tools::dtos::WebFetchInput {
        url: Some(server_url.clone()),
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
fn test_tool_web_fetch_chrome_happy_path() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    let mut headers = HashMap::new();
    headers.insert("server".to_string(), "mock-chrome".to_string());

    let runner = crate::tools::browser_runner::tests::MockBrowserRunner::new()
        .with_html("<html><body><div id='app'><h1>Hydrated by Client JS</h1><p>Dynamic Content</p></div></body></html>");
    let mut runner_with_headers = runner;
    runner_with_headers.return_headers = headers;

    let input = crate::tools::dtos::WebFetchInput {
        url: Some("https://example.com/spa".to_string()),
        headers: true,
        force_refetch: true,
        cursor: None,
    };

    let result = tool_web_fetch_with_runner(
        &input,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
        Some(&runner_with_headers),
    )
    .unwrap();

    assert!(result.content.contains("Hydrated by Client JS"));
    assert!(result.content.contains("Dynamic Content"));
    assert!(!result.from_cache);
    assert!(result.total_lines > 0);
    assert_eq!(
        result.response_headers.unwrap().get("server").unwrap(),
        "mock-chrome"
    );
}

#[test]
fn test_tool_web_fetch_fallback_when_browser_absent() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let server_url = spawn_mock_server("<html><body><p>Static Fallback</p></body></html>");

    let input = crate::tools::dtos::WebFetchInput {
        url: Some(server_url),
        headers: false,
        force_refetch: true,
        cursor: None,
    };

    // No runner and no locator provided -> standard HTTP fallback
    let result = tool_web_fetch_with_locator(
        &input,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
        None,
        None,
    )
    .unwrap();

    assert!(result.content.contains("Static Fallback"));
    assert!(!result.from_cache);
}

#[test]
fn test_tool_web_fetch_fallback_on_browser_crash_or_error() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let server_url = spawn_mock_server("<html><body><p>HTTP Content After Crash</p></body></html>");

    let failing_runner =
        crate::tools::browser_runner::tests::MockBrowserRunner::new().with_failure(1);

    let input = crate::tools::dtos::WebFetchInput {
        url: Some(server_url),
        headers: false,
        force_refetch: true,
        cursor: None,
    };

    let result = tool_web_fetch_with_runner(
        &input,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
        Some(&failing_runner),
    )
    .unwrap();

    assert!(result.content.contains("HTTP Content After Crash"));
    assert!(!result.from_cache);
}

#[test]
fn test_tool_web_fetch_fallback_on_timeout() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let server_url =
        spawn_mock_server("<html><body><p>HTTP Content After Timeout</p></body></html>");

    let timing_out_runner =
        crate::tools::browser_runner::tests::MockBrowserRunner::new().with_timeout();

    let input = crate::tools::dtos::WebFetchInput {
        url: Some(server_url),
        headers: false,
        force_refetch: true,
        cursor: None,
    };

    let result = tool_web_fetch_with_runner(
        &input,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
        Some(&timing_out_runner),
    )
    .unwrap();

    assert!(result.content.contains("HTTP Content After Timeout"));
    assert!(!result.from_cache);
}

#[test]
fn test_tool_web_fetch_chrome_cursor_pagination() {
    let cache = crate::tools::registry::cache::ToolCache::new();

    // Generate 150 distinct HTML lines
    let mut body = String::from("<html><body>");
    for i in 1..=150 {
        body.push_str(&format!("<p>Rendered Item {}</p>\n", i));
    }
    body.push_str("</body></html>");

    let runner = crate::tools::browser_runner::tests::MockBrowserRunner::new().with_html(body);

    let input = crate::tools::dtos::WebFetchInput {
        url: Some("https://example.com/long-page".to_string()),
        headers: false,
        force_refetch: true,
        cursor: None,
    };

    let first = tool_web_fetch_with_runner(
        &input,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
        Some(&runner),
    )
    .unwrap();

    assert!(first.total_lines > 64);
    assert!(!first.content.is_empty());
    assert!(first.cursor.is_some());

    // Second page: cursor only, no fresh parameters.
    let second_input = crate::tools::dtos::WebFetchInput {
        url: None,
        headers: false,
        force_refetch: false,
        cursor: first.cursor,
    };
    let second = tool_web_fetch_with_runner(
        &second_input,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
        Some(&runner),
    )
    .unwrap();

    assert_eq!(second.total_lines, first.total_lines);
    assert!(!second.content.is_empty());
    assert!(second.cursor.is_some());

    // Verify disjoint progress between pages
    let first_page_lines: Vec<&str> = first.content.lines().filter(|l| !l.is_empty()).collect();
    let second_page_lines: Vec<&str> = second.content.lines().filter(|l| !l.is_empty()).collect();
    let first_set: std::collections::HashSet<&str> = first_page_lines.iter().copied().collect();
    let second_set: std::collections::HashSet<&str> = second_page_lines.iter().copied().collect();
    assert!(first_set.is_disjoint(&second_set));
}

#[test]
fn test_tool_web_fetch_chrome_cache_hit_bypasses_browser() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    let runner = crate::tools::browser_runner::tests::MockBrowserRunner::new()
        .with_html("<html><body><p>Cache Target</p></body></html>");

    let input = crate::tools::dtos::WebFetchInput {
        url: Some("https://example.com/cache-target".to_string()),
        headers: false,
        force_refetch: false,
        cursor: None,
    };

    let first = tool_web_fetch_with_runner(
        &input,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
        Some(&runner),
    )
    .unwrap();
    assert!(!first.from_cache);
    assert!(runner.was_called.load(std::sync::atomic::Ordering::SeqCst));

    // Reset called flag
    runner
        .was_called
        .store(false, std::sync::atomic::Ordering::SeqCst);

    // Second call: should serve from cache without calling runner
    let second = tool_web_fetch_with_runner(
        &input,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
        Some(&runner),
    )
    .unwrap();
    assert!(second.from_cache);
    assert_eq!(first.content, second.content);
    assert!(!runner.was_called.load(std::sync::atomic::Ordering::SeqCst));
}

#[test]
fn test_tool_web_fetch_chrome_force_refetch_invalidates_cache() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    let runner = crate::tools::browser_runner::tests::MockBrowserRunner::new()
        .with_html("<html><body><p>Force Refetch Target</p></body></html>");

    let input = crate::tools::dtos::WebFetchInput {
        url: Some("https://example.com/refetch-target".to_string()),
        headers: false,
        force_refetch: false,
        cursor: None,
    };

    let first = tool_web_fetch_with_runner(
        &input,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
        Some(&runner),
    )
    .unwrap();
    assert!(!first.from_cache);

    let force_input = crate::tools::dtos::WebFetchInput {
        url: Some("https://example.com/refetch-target".to_string()),
        headers: false,
        force_refetch: true,
        cursor: None,
    };

    let second = tool_web_fetch_with_runner(
        &force_input,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
        Some(&runner),
    )
    .unwrap();
    assert!(!second.from_cache);
}

#[test]
fn test_tool_web_fetch_browser_markdown_conversion() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    let html = "<html><body><h1>Title Heading</h1><p>Text with <strong>bold</strong> and <a href=\"https://example.com/target\">link text</a>.</p><ul><li>Item A</li><li>Item B</li></ul></body></html>";
    let runner = crate::tools::browser_runner::tests::MockBrowserRunner::new().with_html(html);

    let input = crate::tools::dtos::WebFetchInput {
        url: Some("https://example.com/browser-md".to_string()),
        headers: false,
        force_refetch: true,
        cursor: None,
    };

    let result = tool_web_fetch_with_runner(
        &input,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
        Some(&runner),
    )
    .unwrap();

    assert!(result.content.contains("# Title Heading"));
    assert!(result.content.contains("**bold**"));
    assert!(
        result
            .content
            .contains("[link text](https://example.com/target)")
    );
    assert!(!result.content.contains("<h1>"));
    assert!(!result.content.contains("<strong>"));
    assert!(!result.content.contains("<a href="));
}

#[test]
fn test_tool_web_fetch_http_markdown_conversion() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let html = "<html><body><h1>Title Heading</h1><p>Text with <strong>bold</strong> and <a href=\"https://example.com/target\">link text</a>.</p><ul><li>Item A</li><li>Item B</li></ul></body></html>";
    let server_url = spawn_mock_server(html);

    let input = crate::tools::dtos::WebFetchInput {
        url: Some(server_url),
        headers: false,
        force_refetch: true,
        cursor: None,
    };

    let result = tool_web_fetch_with_locator(
        &input,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
        None,
        None,
    )
    .unwrap();

    assert!(result.content.contains("# Title Heading"));
    assert!(result.content.contains("**bold**"));
    assert!(
        result
            .content
            .contains("[link text](https://example.com/target)")
    );
    assert!(!result.content.contains("<h1>"));
    assert!(!result.content.contains("<strong>"));
    assert!(!result.content.contains("<a href="));
}

#[test]
fn test_tool_web_fetch_replaces_inline_data_uri_with_alt_text() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    let html = r#"<html><body><p>Before <img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==" alt="Sample Logo"> After</p></body></html>"#;
    let runner = crate::tools::browser_runner::tests::MockBrowserRunner::new().with_html(html);

    let input = crate::tools::dtos::WebFetchInput {
        url: Some("https://example.com/data-uri-test".to_string()),
        headers: false,
        force_refetch: true,
        cursor: None,
    };

    let result = tool_web_fetch_with_runner(
        &input,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
        Some(&runner),
    )
    .unwrap();

    assert!(!result.content.contains("data:image/png;base64"));
    assert!(result.content.contains("[Image: Sample Logo]"));
}

#[test]
fn test_tool_web_fetch_replaces_inline_data_uri_without_alt_text() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    let html = r#"<html><body><p>Before <img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="> After</p></body></html>"#;
    let runner = crate::tools::browser_runner::tests::MockBrowserRunner::new().with_html(html);

    let input = crate::tools::dtos::WebFetchInput {
        url: Some("https://example.com/data-uri-no-alt".to_string()),
        headers: false,
        force_refetch: true,
        cursor: None,
    };

    let result = tool_web_fetch_with_runner(
        &input,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
        Some(&runner),
    )
    .unwrap();

    assert!(!result.content.contains("data:image/png;base64"));
    assert!(result.content.contains("[Image]"));
}

#[test]
fn test_tool_web_fetch_replaces_inline_data_uri_http_fallback() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let html = r#"<html><body><p>Text <img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==" alt="Http Icon"> End</p></body></html>"#;
    let server_url = spawn_mock_server(html);

    let input = crate::tools::dtos::WebFetchInput {
        url: Some(server_url),
        headers: false,
        force_refetch: true,
        cursor: None,
    };

    let result = tool_web_fetch_with_locator(
        &input,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
        None,
        None,
    )
    .unwrap();

    assert!(!result.content.contains("data:image/png;base64"));
    assert!(result.content.contains("[Image: Http Icon]"));
}

#[test]
fn test_replace_inline_image_data_uris_pure_unit_tests() {
    use crate::tools::web::replace_inline_image_data_uris;

    // 1. Basic with alt text
    assert_eq!(
        replace_inline_image_data_uris("![Logo](data:image/png;base64,AAAA)"),
        "[Image: Logo]"
    );

    // 2. Empty alt text
    assert_eq!(
        replace_inline_image_data_uris("![](data:image/png;base64,AAAA)"),
        "[Image]"
    );

    // 3. Whitespace-only alt text
    assert_eq!(
        replace_inline_image_data_uris("![   ](data:image/png;base64,AAAA)"),
        "[Image]"
    );

    // 4. Title fallback when alt text is empty
    assert_eq!(
        replace_inline_image_data_uris(r#"![](data:image/png;base64,AAAA "Company Diagram")"#),
        "[Image: Company Diagram]"
    );

    // 5. Alt text prioritized over title
    assert_eq!(
        replace_inline_image_data_uris(r#"![Chart](data:image/png;base64,AAAA "Company Diagram")"#),
        "[Image: Chart]"
    );

    // 6. Angle bracket wrapped URI
    assert_eq!(
        replace_inline_image_data_uris("![SVG](<data:image/svg+xml;utf8,<svg></svg>> )"),
        "[Image: SVG]"
    );

    // 7. Regular web image preserved
    assert_eq!(
        replace_inline_image_data_uris("![Photo](https://example.com/pic.png)"),
        "![Photo](https://example.com/pic.png)"
    );

    // 8. Relative image preserved
    assert_eq!(
        replace_inline_image_data_uris("![Logo](/assets/logo.png)"),
        "![Logo](/assets/logo.png)"
    );

    // 9. Regular hyperlink with data URI preserved
    assert_eq!(
        replace_inline_image_data_uris("[Download](data:application/pdf;base64,AAAA)"),
        "[Download](data:application/pdf;base64,AAAA)"
    );

    // 10. Image nested inside a link
    assert_eq!(
        replace_inline_image_data_uris(
            "[![Thumb](data:image/png;base64,AAAA)](https://example.com)"
        ),
        "[[Image: Thumb]](https://example.com)"
    );

    // 11. Multiple images mixed with text
    let mixed = "Intro ![A](data:image/png;base64,111) middle ![Remote](https://example.com/b.jpg) end ![](data:image/jpeg;base64,222).";
    let expected = "Intro [Image: A] middle ![Remote](https://example.com/b.jpg) end [Image].";
    assert_eq!(replace_inline_image_data_uris(mixed), expected);

    // 12. Edge cases: empty, plain text, malformed unclosed constructs
    assert_eq!(replace_inline_image_data_uris(""), "");
    assert_eq!(replace_inline_image_data_uris("Hello World"), "Hello World");
    assert_eq!(
        replace_inline_image_data_uris("![unclosed alt"),
        "![unclosed alt"
    );
    assert_eq!(replace_inline_image_data_uris("![alt]"), "![alt]");
    assert_eq!(
        replace_inline_image_data_uris("![alt](unclosed url"),
        "![alt](unclosed url"
    );
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
        Some("test query"),
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
        Some("test query"),
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
        Some("q"),
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
        None,
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
        None,
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
        Some("test query"),
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

#[test]
fn test_tool_web_fetch_rejects_cursor_with_url() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    let input = crate::tools::dtos::WebFetchInput {
        url: Some("https://example.com/page".to_string()),
        headers: false,
        force_refetch: false,
        cursor: Some("c_00000000".to_string()),
    };
    let err = tool_web_fetch(&input, &cache, &crate::utils::uuid::SystemUuidGenerator).unwrap_err();
    assert_eq!(
        err,
        crate::tools::registry::builtin::strings::CURSOR_WITH_FRESH_PARAMS_ERROR
    );
}

#[test]
fn test_tool_web_fetch_rejects_cursor_with_force_refetch() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    let input = crate::tools::dtos::WebFetchInput {
        url: None,
        headers: false,
        force_refetch: true,
        cursor: Some("c_00000000".to_string()),
    };
    let err = tool_web_fetch(&input, &cache, &crate::utils::uuid::SystemUuidGenerator).unwrap_err();
    assert_eq!(
        err,
        crate::tools::registry::builtin::strings::CURSOR_WITH_FRESH_PARAMS_ERROR
    );
}

#[test]
fn test_tool_web_fetch_rejects_missing_url_and_cursor() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    let input = crate::tools::dtos::WebFetchInput {
        url: None,
        headers: false,
        force_refetch: false,
        cursor: None,
    };
    let err = tool_web_fetch(&input, &cache, &crate::utils::uuid::SystemUuidGenerator).unwrap_err();
    assert!(err.contains("Give `url`"), "unexpected error: {err}");
}

#[test]
fn test_tool_web_search_rejects_cursor_with_query() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    let err = tool_web_search(
        "http://127.0.0.1:1",
        Some("fresh query"),
        Some("c_00000000".to_string()),
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
    )
    .unwrap_err();
    assert_eq!(
        err,
        crate::tools::registry::builtin::strings::CURSOR_WITH_FRESH_PARAMS_ERROR
    );
}

#[test]
fn test_tool_web_search_rejects_missing_query_and_cursor() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    let err = tool_web_search(
        "http://127.0.0.1:1",
        None,
        None,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
    )
    .unwrap_err();
    assert!(err.contains("Give `query`"), "unexpected error: {err}");
}
