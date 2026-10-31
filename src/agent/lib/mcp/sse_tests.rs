//! Tests for the SSE body parser and response walker (`agent::lib::mcp::sse`).

use super::*;

#[test]
fn parses_simple_event() {
    let body = "data: hello\n\n";
    let events = parse_sse_body(body);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "hello");
}

#[test]
fn parses_multiple_events_with_names() {
    let body = "event: ping\ndata: {\"x\":1}\n\ndata: second\n\n";
    let events = parse_sse_body(body);
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].event.as_deref(), Some("ping"));
    assert_eq!(events[0].data, "{\"x\":1}");
    assert_eq!(events[1].data, "second");
}

#[test]
fn handles_crlf_line_endings() {
    let body = "data: line1\r\ndata: line2\r\n\r\n";
    let events = parse_sse_body(body);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "line1\nline2");
}

#[test]
fn ignores_comments_and_unknown_fields() {
    let body = ": this is a comment\nretry: 5000\ndata: ok\n\n";
    let events = parse_sse_body(body);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "ok");
}

#[test]
fn walk_finds_response_and_collects_notifications() {
    let body = "\
data: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\",\"params\":{\"progressToken\":\"t1\",\"progress\":50}}

data: {\"jsonrpc\":\"2.0\",\"id\":7,\"result\":{\"ok\":true}}
";
    let events = parse_sse_body(body);
    let mut collected = Vec::new();
    let walk = walk_for_response(events, 7, &mut |n| collected.push(n.clone()))
        .expect("walk should find response");
    assert_eq!(walk.response["result"]["ok"], true);
    assert_eq!(walk.notifications.len(), 1);
    assert_eq!(walk.notifications[0].method, "notifications/progress");
    assert_eq!(collected.len(), 1);
}

#[test]
fn walk_errors_on_stream_without_response() {
    let body =
        "data: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\",\"params\":{}}\n\n";
    let events = parse_sse_body(body);
    let result = walk_for_response(events, 99, &mut |_| {});
    assert!(result.is_err());
    assert!(result.unwrap_err().message.contains("ended without"));
}

#[test]
fn walk_handles_string_ids() {
    // Spec allows string ids; we should match them too.
    let body = "data: {\"jsonrpc\":\"2.0\",\"id\":\"42\",\"result\":{}}\n\n";
    let events = parse_sse_body(body);
    let walk = walk_for_response(events, 42, &mut |_| {}).expect("walk should find");
    assert_eq!(walk.response["result"], serde_json::json!({}));
}
