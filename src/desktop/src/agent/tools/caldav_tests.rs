//! Tests for `tools/caldav.rs`.
//!
//! Sidecar file. Extracted from `caldav.rs` so the implementation
//! module stays focused on production code.
//!
//! Originally a `#[cfg(test)] mod tests { ... }` block at the bottom of
//! `caldav.rs`. Lives in a sibling file so private item access via
//! `super::*` keeps working.

use super::*;

// --- parse_ical_data tests ---

#[test]
fn test_parse_ical_data_summary() {
    let data = "BEGIN:VEVENT\r\nSUMMARY:Test Event\r\nDTSTART:20240101T120000\r\nDTEND:20240101T130000\r\nEND:VEVENT";
    let ev = parse_ical_data("client1", "/cal/item.ics", data);
    assert_eq!(ev.client, "client1");
    assert_eq!(ev.href, "/cal/item.ics");
    assert_eq!(ev.summary, Some("Test Event".to_string()));
}

#[test]
fn test_parse_ical_data_dates() {
    let data = "BEGIN:VEVENT\r\nSUMMARY:Test\r\nDTSTART:20240101T120000\r\nDTEND:20240101T130000\r\nEND:VEVENT";
    let ev = parse_ical_data("c", "/h", data);
    assert_eq!(ev.start, Some("2024-01-01T12:00:00".to_string()));
    assert_eq!(ev.end, Some("2024-01-01T13:00:00".to_string()));
}

#[test]
fn test_parse_ical_data_utc_and_escapes() {
    let data = "BEGIN:VEVENT\r\nSUMMARY:Test UTC\r\nDTSTART:20240101T120000Z\r\nDTEND:invalid\r\nDESCRIPTION:Line 1\\nLine 2\\NLine 3\\,Comma\\;Semicolon\r\nLOCATION:Room 1\\, Building A\\; Fl 2\r\nORGANIZER;CN=Alice:mailto:alice@test.com\r\nEND:VEVENT";
    let ev = parse_ical_data("c", "/h", data);
    assert_eq!(ev.start, Some("2024-01-01T12:00:00Z".to_string()));
    assert_eq!(ev.end, Some("invalid".to_string()));
    assert_eq!(
        ev.description,
        Some("Line 1\nLine 2\nLine 3,Comma;Semicolon".to_string())
    );
    assert_eq!(ev.location, Some("Room 1, Building A; Fl 2".to_string()));
    assert_eq!(ev.organizer, Some("mailto:alice@test.com".to_string()));
}

#[test]
fn test_parse_ical_data_date_only() {
    let data = "BEGIN:VEVENT\r\nSUMMARY:All Day\r\nDTSTART;VALUE=DATE:20240101\r\nDTEND;VALUE=DATE:20240102\r\nEND:VEVENT";
    let ev = parse_ical_data("c", "/h", data);
    assert_eq!(ev.start, Some("2024-01-01".to_string()));
    assert_eq!(ev.end, Some("2024-01-02".to_string()));
}

#[test]
fn test_parse_ical_data_description_location() {
    let data = "BEGIN:VEVENT\r\nSUMMARY:Mtg\r\nDESCRIPTION:Discuss project\r\nLOCATION:Room 42\r\nORGANIZER:mailto:alice@test.com\r\nEND:VEVENT";
    let ev = parse_ical_data("c", "/h", data);
    assert_eq!(ev.description, Some("Discuss project".to_string()));
    assert_eq!(ev.location, Some("Room 42".to_string()));
    assert_eq!(ev.organizer, Some("mailto:alice@test.com".to_string()));
}

#[test]
fn test_parse_ical_data_unfolds_lines() {
    let data = "BEGIN:VEVENT\r\nSUMMARY:Very long\r\n summary line\r\nDTSTART:20240101T120000\r\nEND:VEVENT";
    let ev = parse_ical_data("c", "/h", data);
    // The code unfolds by removing the leading space and concatenating without adding a separator
    assert_eq!(ev.summary, Some("Very longsummary line".to_string()));
}

// --- json_to_ical tests ---

#[test]
fn test_json_to_ical_basic() {
    let input = r#"{"summary":"Test","start":"2024-01-01T12:00:00","end":"2024-01-01T13:00:00","description":"desc","location":"loc"}"#;
    let ical = json_to_ical(input, None);
    assert!(ical.starts_with("BEGIN:VCALENDAR"));
    assert!(ical.contains("BEGIN:VEVENT"));
    assert!(ical.contains("END:VEVENT"));
    assert!(ical.contains("END:VCALENDAR"));
    assert!(ical.contains("SUMMARY:Test"));
    assert!(ical.contains("DESCRIPTION:desc"));
    assert!(ical.contains("LOCATION:loc"));
}

#[test]
fn test_json_to_ical_date_only_and_invalid_json() {
    let input = r#"{"summary":"Date Only","start":"2024-01-01","end":"2024-01-02"}"#;
    let ical = json_to_ical(input, None);
    assert!(ical.contains("DTSTART;VALUE=DATE:20240101"));
    assert!(ical.contains("DTEND;VALUE=DATE:20240102"));

    let invalid = "not json at all";
    let ical_invalid = json_to_ical(invalid, None);
    assert!(ical_invalid.contains("SUMMARY:New Event"));
}

#[test]
fn test_json_to_ical_minimal() {
    // Even an empty JSON should produce a valid structure
    let input = "{}";
    let ical = json_to_ical(input, None);
    assert!(ical.starts_with("BEGIN:VCALENDAR"));
    assert!(ical.contains("BEGIN:VEVENT"));
    assert!(ical.contains("END:VEVENT"));
    assert!(ical.contains("END:VCALENDAR"));
    // Should have a default summary
    assert!(ical.contains("SUMMARY:New Event"));
}

#[test]
fn test_json_to_ical_with_uid() {
    let input = r#"{"summary":"Test"}"#;
    let ical = json_to_ical(input, Some("custom-uid-123"));
    assert!(ical.contains("UID:custom-uid-123"));
}

#[test]
fn test_json_to_ical_escapes_special_chars() {
    let input = r#"{"summary":"Hello;World,Line1\nLine2"}"#;
    let ical = json_to_ical(input, None);
    assert!(ical.contains("Hello\\;World\\,Line1\\nLine2"));
}

// --- update_ical_string tests ---

#[test]
fn test_update_ical_string_replaces_summary() {
    let original = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:Old\r\nDTSTART:20240101T120000\r\nEND:VEVENT\r\nEND:VCALENDAR";
    let updates = serde_json::json!({"summary": "New"});
    let result = update_ical_string(original, &updates);
    assert!(result.contains("SUMMARY:New"));
    assert!(!result.contains("SUMMARY:Old"));
}

#[test]
fn test_update_ical_string_adds_missing_field() {
    // Test that a missing SUMMARY gets added at the end of VEVENT
    let original =
        "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nDTSTART:20240101T120000\r\nEND:VEVENT\r\nEND:VCALENDAR";
    let updates = serde_json::json!({
        "summary": "Added Summary",
        "start": "2024-05-01",
        "end": "2024-05-02",
        "description": "Added Desc",
        "location": "Added Loc"
    });
    let result = update_ical_string(original, &updates);
    assert!(result.contains("SUMMARY:Added Summary"));
    assert!(result.contains("DTSTART;VALUE=DATE:20240501"));
    assert!(result.contains("DTEND;VALUE=DATE:20240502"));
    assert!(result.contains("DESCRIPTION:Added Desc"));
    assert!(result.contains("LOCATION:Added Loc"));
}

#[test]
fn test_update_ical_string_all_fields_with_folding() {
    let original = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:Old Summary\r\n folded line\r\nDTSTART:20240101T120000\r\nDTEND:20240101T130000\r\nDESCRIPTION:Old Desc\r\n LOCATION:Old Loc\r\nEND:VEVENT\r\nEND:VCALENDAR";
    let updates = serde_json::json!({
        "summary": "New Summary",
        "start": "2024-02-01T10:00:00",
        "end": "2024-02-01T11:00:00",
        "description": "New Desc with; special, chars\nline2",
        "location": "New Loc"
    });
    let result = update_ical_string(original, &updates);
    assert!(result.contains("SUMMARY:New Summary"));
    assert!(result.contains("DTSTART:20240201T100000"));
    assert!(result.contains("DTEND:20240201T110000"));
    assert!(result.contains("DESCRIPTION:New Desc with\\; special\\, chars\\nline2"));
    assert!(result.contains("LOCATION:New Loc"));
    assert!(!result.contains("folded line"));
}

#[test]
fn test_update_ical_string_replaces_dtstart() {
    let original = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:Test\r\nDTSTART:20240101\r\nEND:VEVENT\r\nEND:VCALENDAR";
    let updates = serde_json::json!({"start": "20250101"});
    let result = update_ical_string(original, &updates);
    assert!(result.contains("DTSTART;VALUE=DATE:20250101") || result.contains("DTSTART:20250101"));
}

#[test]
fn test_update_ical_string_no_updates_preserves() {
    let original = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:Keep\r\nDTSTART:20240101T120000\r\nEND:VEVENT\r\nEND:VCALENDAR";
    let updates = serde_json::json!({});
    let result = update_ical_string(original, &updates);
    assert!(result.contains("SUMMARY:Keep"));
}

// --- CalDAV Tool Config & Client Tests ---

#[test]
fn test_caldav_tools_empty_config() {
    let config = AppConfig::default();

    assert_eq!(
        tool_add_calendar_item(&config, "{}").unwrap_err(),
        "No CalDAV clients configured."
    );
    assert_eq!(
        tool_update_calendar_item(&config, "/item.ics", "{}").unwrap_err(),
        "No CalDAV clients configured."
    );
    assert_eq!(
        tool_delete_calendar_item(&config, "/item.ics").unwrap_err(),
        "No CalDAV clients configured."
    );

    let search_res = tool_search_calendar(&config, "test").unwrap();
    assert_eq!(
        search_res.results,
        serde_json::to_string_pretty(&CalDavResponse {
            results: vec![],
            errors: vec![]
        })
        .unwrap()
    );

    let get_res = tool_get_calendar(&config, "2024-01-01", "2024-01-02").unwrap();
    assert_eq!(
        get_res.results,
        serde_json::to_string_pretty(&CalDavResponse {
            results: vec![],
            errors: vec![]
        })
        .unwrap()
    );

    let item_res = tool_get_calendar_item(&config, "/item.ics").unwrap();
    assert_eq!(
        item_res.result,
        serde_json::to_string_pretty(&CalDavResponse {
            results: vec![],
            errors: vec![]
        })
        .unwrap()
    );
}

#[test]
#[ignore = "tries to connect to a dead TCP port (127.0.0.1:1); the OS-level connect \
                timeout makes this test take ~37s, dominating the suite. Re-enable on demand \
                with `cargo nextest run --run-ignored all -- tools::caldav::tests::test_caldav_tools_unreachable_client`."]
fn test_caldav_tools_unreachable_client() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mut config = AppConfig::default();
    config.caldav_clients.insert(
        "test_client".to_string(),
        crate::config::CalDavClient {
            url: "http://127.0.0.1:1".to_string(),
            username: "user".to_string(),
            password: "password".to_string(),
        },
    );

    let search_res = tool_search_calendar(&config, "test").unwrap();
    assert!(search_res.results.contains("Error on client test_client"));

    let get_res = tool_get_calendar(&config, "2024-01-01", "2024-01-02").unwrap();
    assert!(get_res.results.contains("Error on client test_client"));

    let item_res = tool_get_calendar_item(&config, "/item.ics").unwrap();
    assert!(item_res.result.contains("Error on client test_client"));

    let add_res = tool_add_calendar_item(&config, "{}").unwrap();
    assert!(add_res.result.contains("Error on client test_client"));

    let update_res = tool_update_calendar_item(&config, "/item.ics", "{}").unwrap();
    assert!(update_res.result.contains("Error on client test_client"));

    let delete_res = tool_delete_calendar_item(&config, "/item.ics").unwrap();
    assert!(delete_res.result.contains("Error on client test_client"));
}

fn spawn_mock_caldav_server() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            while let Ok((socket, _)) = listener.accept().await {
                tokio::spawn(handle_mock_connection(socket));
            }
        });
    });

    format!("http://{}", addr)
}

/// Serve a single HTTP/1.1 keep-alive connection for the mock DAV
/// server. Reads a complete request (headers + `Content-Length` body),
/// dispatches on the request line, writes the response, and loops
/// until the client half-closes the connection.
///
/// `fast-dav-rs` is built on `hyper_util::client::legacy::Client`, which
/// pools HTTP/1.1 connections. A single `CalDavClient` therefore reuses
/// the same TCP socket across the `PROPFIND` + `REPORT` sequence inside
/// `tool_get_calendar`. The previous one-shot handler closed the socket
/// after the first response, which intermittently turned the second
/// request into a "connection closed before message completed" error
/// and made the test flake.
async fn handle_mock_connection(mut socket: tokio::net::TcpStream) {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

    let (read_half, mut write_half) = socket.split();
    let mut reader = BufReader::new(read_half);
    let mut header_buf: Vec<u8> = Vec::with_capacity(512);

    loop {
        header_buf.clear();

        // Read header lines until we hit the empty CRLF that terminates
        // the header block. EOF here means the client closed the
        // connection cleanly — exit the loop.
        loop {
            let mut line = Vec::new();
            let n = match reader.read_until(b'\n', &mut line).await {
                Ok(n) => n,
                Err(_) => return,
            };
            if n == 0 {
                return;
            }
            let is_blank = line == b"\r\n" || line == b"\n";
            header_buf.extend_from_slice(&line);
            if is_blank {
                break;
            }
        }

        // Pull Content-Length so we can drain the request body before
        // dispatching. PROPFIND/REPORT carry XML bodies; the client
        // expects the server to consume them before responding.
        let header_str = String::from_utf8_lossy(&header_buf);
        let content_length: usize = header_str
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                if name.trim().eq_ignore_ascii_case("content-length") {
                    value.trim().parse().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0);

        if content_length > 0 {
            let mut body = vec![0u8; content_length];
            if reader.read_exact(&mut body).await.is_err() {
                return;
            }
        }

        let response = mock_dav_response(&header_str);
        if write_half.write_all(response.as_bytes()).await.is_err() {
            return;
        }
    }
}

/// Build the canned response for a parsed mock DAV request. Dispatches
/// purely on the request line — the request body is not inspected.
fn mock_dav_response(req: &str) -> String {
    if req.starts_with("GET /item1.ics") {
        "HTTP/1.1 200 OK\r\nContent-Type: text/calendar\r\nContent-Length: 104\r\nConnection: keep-alive\r\n\r\nBEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:Existing Item\r\nDTSTART:20240101T100000Z\r\nEND:VEVENT\r\nEND:VCALENDAR".to_string()
    } else if req.starts_with("GET /notfound") {
        "HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\nConnection: keep-alive\r\n\r\nNot Found"
            .to_string()
    } else if req.starts_with("PUT") {
        "HTTP/1.1 201 Created\r\nContent-Length: 0\r\nConnection: keep-alive\r\n\r\n".to_string()
    } else if req.starts_with("DELETE /item1.ics") {
        "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: keep-alive\r\n\r\n".to_string()
    } else if req.starts_with("DELETE /fail") {
        "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 5\r\nConnection: close\r\n\r\nError"
            .to_string()
    } else if req.starts_with("PROPFIND") {
        let xml_body = r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
 <d:response>
  <d:href>/calendars/primary/</d:href>
  <d:propstat>
   <d:prop>
    <d:resourcetype><d:collection/><c:calendar/></d:resourcetype>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>
</d:multistatus>"#;
        format!(
            "HTTP/1.1 207 Multi-Status\r\nContent-Type: application/xml\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n{}",
            xml_body.len(),
            xml_body
        )
    } else if req.starts_with("REPORT") {
        let xml_body = r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
 <d:response>
  <d:href>/calendars/primary/event1.ics</d:href>
  <d:propstat>
   <d:prop>
    <c:calendar-data>BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
SUMMARY:Meeting with Bob
DTSTART:20240101T100000Z
DTEND:20240101T110000Z
END:VEVENT
END:VCALENDAR</c:calendar-data>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>
</d:multistatus>"#;
        format!(
            "HTTP/1.1 207 Multi-Status\r\nContent-Type: application/xml\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n{}",
            xml_body.len(),
            xml_body
        )
    } else {
        "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
    }
}

#[test]
fn test_caldav_tools_mock_server() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let server_url = spawn_mock_caldav_server();

    let mut config = AppConfig::default();
    config.caldav_clients.insert(
        "mock_client".to_string(),
        crate::config::CalDavClient {
            url: server_url,
            username: "user".to_string(),
            password: "password".to_string(),
        },
    );

    // 1. Search calendar
    let search_res = tool_search_calendar(&config, "Bob").unwrap();
    assert!(search_res.results.contains("Meeting with Bob"));

    // 2. Get calendar (date range)
    let get_res = tool_get_calendar(&config, "2024-01-01", "2024-01-02").unwrap();
    assert!(get_res.results.contains("Meeting with Bob"));

    // 3. Get calendar item success
    let item_res = tool_get_calendar_item(&config, "/item1.ics").unwrap();
    assert!(item_res.result.contains("Existing Item"));

    // 4. Get calendar item 404
    let item_res_404 = tool_get_calendar_item(&config, "/notfound").unwrap();
    assert!(item_res_404.result.contains("Not found by href"));

    // 5. Add calendar item
    let add_res = tool_add_calendar_item(&config, r#"{"summary":"New Mtg"}"#).unwrap();
    assert!(add_res.result.contains("Created at /calendars/primary/"));

    // 6. Update calendar item success
    let update_res =
        tool_update_calendar_item(&config, "/item1.ics", r#"{"summary":"Updated Mtg"}"#).unwrap();
    assert!(update_res.result.contains("Updated successfully"));

    // 7. Update calendar item 404
    let update_res_404 =
        tool_update_calendar_item(&config, "/notfound", r#"{"summary":"Updated Mtg"}"#).unwrap();
    assert!(
        update_res_404
            .result
            .contains("Failed to fetch event for update")
    );

    // 8. Delete calendar item success
    let delete_res = tool_delete_calendar_item(&config, "/item1.ics").unwrap();
    assert!(delete_res.result.contains("Deleted successfully"));

    // 9. Delete calendar item 500 error
    let delete_res_err = tool_delete_calendar_item(&config, "/fail").unwrap();
    assert!(delete_res_err.result.contains("Failed to DELETE event"));
}

/// Regression: `fast-dav-rs` uses hyper-util connection pooling, so
/// `tool_get_calendar` reuses the same TCP connection for its internal
/// PROPFIND + REPORT sequence. A mock server that drops the connection
/// after one request turns the second request into a connection-closed
/// error and silently returns an empty `results` payload. Loop the call
/// to make the race observable without flakiness.
#[test]
fn test_caldav_tools_mock_server_keep_alive() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let server_url = spawn_mock_caldav_server();

    let mut config = AppConfig::default();
    config.caldav_clients.insert(
        "mock_client".to_string(),
        crate::config::CalDavClient {
            url: server_url,
            username: "user".to_string(),
            password: "password".to_string(),
        },
    );

    for _ in 0..16 {
        let get_res = tool_get_calendar(&config, "2024-01-01", "2024-01-02").unwrap();
        assert!(
            get_res.results.contains("Meeting with Bob"),
            "expected REPORT response on reused connection, got: {}",
            get_res.results
        );
    }
}

/// Regression: many sequential requests through one `CalDavClient` —
/// the most aggressive form of the keep-alive race. A mock that drops
/// the connection after each response will see every other request
/// fail with `connection closed before message completed`.
#[test]
fn test_caldav_tools_mock_server_single_client_reuse() {
    use crate::agent::tools::blocking::block_on;

    let _ = rustls::crypto::ring::default_provider().install_default();
    let server_url = spawn_mock_caldav_server();

    block_on(async {
        let client = CalDavClient::new(&server_url, Some("user"), Some("password")).unwrap();
        for _ in 0..32 {
            let items = client
                .calendar_query_timerange(
                    "/calendars/primary/",
                    "VEVENT",
                    Some("20240101T000000Z"),
                    Some("20240102T000000Z"),
                    true,
                )
                .await
                .expect("REPORT should succeed on a kept-alive connection");
            assert!(
                !items.is_empty(),
                "expected at least one calendar item on reused connection"
            );
            assert!(
                items[0]
                    .calendar_data
                    .as_deref()
                    .unwrap_or("")
                    .contains("Meeting with Bob")
            );
        }
    });
}
