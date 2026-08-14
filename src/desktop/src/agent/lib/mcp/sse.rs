//! Minimal Server-Sent Events (SSE) body parser for MCP HTTP transport.
//!
//! Per the 2025-11-25 spec (basic/transports), an MCP server responding
//! to a JSON-RPC `request` over Streamable HTTP may return either
//! `Content-Type: application/json` (single response) or
//! `Content-Type: text/event-stream` (SSE stream with one or more
//! events, ending with the response that matches the request id).
//!
//! This module parses the SSE framing and walks the events to find
//! the response that matches a given request id. Server→client
//! notifications and progress events that arrive before the
//! response are surfaced to a callback so the caller can log them
//! (and, in the future, dispatch them to a progress subscriber).

use super::error::McpError;

/// One parsed SSE event: the accumulated `data:` lines (joined by
/// `\n`) plus the raw event name, if any, and the event id.
#[derive(Debug, Default, Clone)]
pub struct SseEvent {
    pub event: Option<String>,
    pub id: Option<String>,
    /// `data:` lines joined by `\n`, with the trailing newline stripped
    /// from the final line. Empty if the event had no `data:` field.
    pub data: String,
}

impl SseEvent {
    pub fn is_empty(&self) -> bool {
        self.data.is_empty() && self.event.is_none() && self.id.is_none()
    }
}

/// Parse a complete SSE body into a sequence of events. `\n\n`-delimited
/// per the WHATWG SSE spec; the trailing delimiter is optional.
pub fn parse_sse_body(body: &str) -> Vec<SseEvent> {
    let mut events = Vec::new();
    let mut current = SseEvent::default();

    for raw_line in body.split('\n') {
        // Strip a single trailing CR (the spec allows \r\n line endings).
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);

        // Blank line = event boundary.
        if line.is_empty() {
            if !current.is_empty() {
                events.push(std::mem::take(&mut current));
            }
            continue;
        }

        // Lines starting with ':' are comments; skip.
        if let Some(stripped) = line.strip_prefix(':') {
            // We don't currently surface SSE comments, but be tolerant.
            let _ = stripped;
            continue;
        }

        // Field is `name: value` or `name:value`. We only care about
        // `event`, `id`, and `data`.
        let (name, value) = match line.split_once(':') {
            Some(parts) => parts,
            None => continue, // malformed; skip
        };
        // Per spec, the value starts after the first optional space.
        let value = value.strip_prefix(' ').unwrap_or(value);

        match name {
            "event" => current.event = Some(value.to_owned()),
            "id" => current.id = Some(value.to_owned()),
            "data" => {
                if current.data.is_empty() {
                    current.data.push_str(value);
                } else {
                    current.data.push('\n');
                    current.data.push_str(value);
                }
            }
            _ => {
                // Ignore unknown fields (`retry`, etc).
            }
        }
    }

    // Flush the last event if the body didn't end with a blank line.
    if !current.is_empty() {
        events.push(current);
    }
    events
}

/// Outcome of [`walk_for_response`]: the response event (matching
/// `expected_id`) or an aggregate of notifications seen along the
/// way (for logging / future dispatch).
#[derive(Debug)]
pub struct SseWalkResult {
    pub response: serde_json::Value,
    /// Notifications the server sent before the response. Each is the
    /// already-parsed `params` object. The `method` is preserved
    /// alongside so the caller can dispatch.
    #[allow(dead_code)]
    pub notifications: Vec<SseNotification>,
    /// The most recent SSE `id:` field observed on any event in
    /// the stream (notifications or the response itself). Spec
    /// §3.3: "The server MAY assign event IDs to SSE events for
    /// resumability. Event IDs MUST be globally unique within the
    /// session or per-client." The caller is expected to cache
    /// this on the session and re-use it as the `Last-Event-ID`
    /// header on a subsequent GET if the stream disconnects.
    /// `None` if no event in the stream carried an `id:` field.
    pub last_event_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SseNotification {
    pub method: String,
    pub params: serde_json::Value,
}

/// Walk a parsed SSE event list, looking for the response to a
/// specific request id. Notifications and progress updates seen
/// before the response are returned alongside.
pub fn walk_for_response(
    events: Vec<SseEvent>,
    expected_id: u64,
    on_notification: &mut dyn FnMut(&SseNotification),
) -> Result<SseWalkResult, McpError> {
    let mut notifications = Vec::new();
    let mut last_event_id: Option<String> = None;
    for event in events {
        // Track every event id we see — even notifications and
        // the response itself — so the caller can resume from
        // the most recent one if the stream disconnects
        // mid-session.
        if let Some(id) = event.id.as_ref() {
            last_event_id = Some(id.clone());
        }
        if event.data.is_empty() {
            continue;
        }
        let value: serde_json::Value = match serde_json::from_str(&event.data) {
            Ok(v) => v,
            Err(e) => {
                return Err(McpError::transport(
                    "SSE stream",
                    format!("invalid JSON in event data: {e}; body: {}", event.data),
                ));
            }
        };

        // Per spec, a JSON-RPC response MUST echo the request id and
        // contain either `result` or `error`. A JSON-RPC notification
        // MUST NOT carry an id.
        let id = value.get("id");
        let method = value.get("method").and_then(|v| v.as_str());

        if id.is_none()
            && let Some(m) = method
        {
            let params = value
                .get("params")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let note = SseNotification {
                method: m.to_owned(),
                params,
            };
            on_notification(&note);
            notifications.push(note);
            continue;
        }

        // Match by id (string or number per spec).
        let matches = match id {
            Some(serde_json::Value::Number(n)) => n.as_u64() == Some(expected_id),
            Some(serde_json::Value::String(s)) => s.parse::<u64>().ok() == Some(expected_id),
            _ => false,
        };
        if matches {
            return Ok(SseWalkResult {
                response: value,
                notifications,
                last_event_id,
            });
        }

        // Notifications that DO carry an id are protocol errors, but
        // we'll just ignore them rather than fail the whole stream.
    }
    Err(McpError::transport(
        "SSE stream",
        format!("stream ended without a JSON-RPC response for id {expected_id}"),
    ))
}

#[cfg(test)]
mod tests {
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
}

#[cfg(test)]
#[path = "sse_proptests.rs"]
mod sse_proptests;
