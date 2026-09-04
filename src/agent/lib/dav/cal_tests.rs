//! Tests for `agent/lib/dav/cal.rs`.

use super::*;
use crate::config::AgentConfig;

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

// --- CalDAV Tool Config & Client Tests (mock server) ---

use wiremock::matchers::{method, path as wm_path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// A wiremock server whose backing tokio runtime lives as long as
/// this guard. The runtime owns the hyper task that serves mock
/// responses; drop the guard and the server stops. Same shape as
/// the guard used in `agent/lib/trello/client_tests.rs` and
/// `agent::lib::weather/mod.rs`.
struct WiremockGuard {
    server: MockServer,
    _runtime: tokio::runtime::Runtime,
}

impl WiremockGuard {
    fn start() -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");
        let server = runtime.block_on(MockServer::start());
        Self {
            server,
            _runtime: runtime,
        }
    }

    fn uri(&self) -> String {
        self.server.uri()
    }

    fn register(&self, mock: Mock) {
        self._runtime.block_on(self.server.register(mock));
    }
}

/// XML body returned by the mock for any PROPFIND that probes for
/// calendars. Declares a single calendar at `/calendars/primary/`
/// so `DavClient::get_all_calendars` succeeds on the
/// first try and never falls through to the principal-discovery
/// branches.
const PROPFIND_BODY: &str = r#"<?xml version="1.0" encoding="utf-8"?>
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

/// XML body returned for any REPORT. Carries one event whose
/// `SUMMARY` is `Meeting with Bob` — the marker the smoke test
/// greps for.
const REPORT_BODY: &str = r#"<?xml version="1.0" encoding="utf-8"?>
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

/// Body returned for `GET /item1.ics`. The smoke test greps for
/// the literal `Existing Item` so we make the SUMMARY match.
const ITEM1_ICS_BODY: &str = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:Existing Item\r\nDTSTART:20240101T100000Z\r\nEND:VEVENT\r\nEND:VCALENDAR";

/// Register every stub the smoke test, keep-alive test, and
/// single-client-reuse test need against `mock`. Centralised so
/// the three tests register a consistent DAV service.
fn register_caldav_stubs(mock: &WiremockGuard) {
    // PROPFIND: any path → 207 with the calendar list.
    mock.register(
        Mock::given(method("PROPFIND")).respond_with(
            ResponseTemplate::new(207)
                .insert_header("content-type", "application/xml")
                .set_body_string(PROPFIND_BODY),
        ),
    );
    // REPORT: any path → 207 with one calendar event.
    mock.register(
        Mock::given(method("REPORT")).respond_with(
            ResponseTemplate::new(207)
                .insert_header("content-type", "application/xml")
                .set_body_string(REPORT_BODY),
        ),
    );
    // GET /item1.ics → 200 with the existing iCal body.
    mock.register(
        Mock::given(method("GET"))
            .and(wm_path("/item1.ics"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/calendar")
                    .set_body_string(ITEM1_ICS_BODY),
            ),
    );
    // GET /notfound → 404.
    mock.register(
        Mock::given(method("GET"))
            .and(wm_path("/notfound"))
            .respond_with(
                ResponseTemplate::new(404)
                    .insert_header("content-type", "text/plain")
                    .set_body_string("Not Found"),
            ),
    );
    // PUT (any path) → 201 Created.
    mock.register(
        Mock::given(method("PUT")).respond_with(ResponseTemplate::new(201).set_body_string("")),
    );
    // DELETE /item1.ics → 204.
    mock.register(
        Mock::given(method("DELETE"))
            .and(wm_path("/item1.ics"))
            .respond_with(ResponseTemplate::new(204).set_body_string("")),
    );
    // DELETE /fail → 500 Internal Server Error.
    mock.register(
        Mock::given(method("DELETE"))
            .and(wm_path("/fail"))
            .respond_with(
                ResponseTemplate::new(500)
                    .insert_header("content-type", "text/plain")
                    .set_body_string("Error"),
            ),
    );
}

#[test]
fn test_caldav_tools_empty_config() {
    let config = AgentConfig::default();

    assert_eq!(
        tool_add_calendar_item(&config, None, "{}").unwrap_err(),
        "No CalDAV clients configured."
    );
    assert_eq!(
        tool_update_calendar_item(&config, "/item.ics", None, "{}").unwrap_err(),
        "No CalDAV clients configured."
    );
    assert_eq!(
        tool_delete_calendar_item(&config, "/item.ics", None).unwrap_err(),
        "No CalDAV clients configured."
    );

    let cache = crate::tools::registry::cache::ToolCache::new();
    let uuid_gen = crate::utils::uuid::SystemUuidGenerator;

    let search_res = tool_search_calendar(&config, "test", None, &cache, &uuid_gen).unwrap();
    assert!(search_res.results.is_empty());
    assert_eq!(search_res.total, 0);
    assert_eq!(search_res.hint.as_deref(), Some("Final page."));

    let get_res =
        tool_get_calendar(&config, "2024-01-01", "2024-01-02", None, &cache, &uuid_gen).unwrap();
    assert!(get_res.results.is_empty());
    assert_eq!(get_res.total, 0);
    assert_eq!(get_res.hint.as_deref(), Some("Final page."));

    let item_res = tool_get_calendar_item(&config, "/item.ics").unwrap();
    assert_eq!(item_res.item, None);
    assert!(item_res.errors.is_empty());
}

#[test]
#[ignore = "tries to connect to a dead TCP port (127.0.0.1:1); the OS-level connect \
                timeout makes this test take ~37s, dominating the suite. Re-enable on demand \
                with `cargo nextest run --run-ignored all -- tools::caldav::tests::test_caldav_tools_unreachable_client`."]
fn test_caldav_tools_unreachable_client() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mut config = AgentConfig::default();
    config.caldav_clients.insert(
        "test_client".to_string(),
        crate::config::CalDavClient {
            url: "http://127.0.0.1:1".to_string(),
            username: "user".to_string(),
            password: "password".to_string(),
        },
    );
    let cache = crate::tools::registry::cache::ToolCache::new();
    let uuid_gen = crate::utils::uuid::SystemUuidGenerator;

    let search_res = tool_search_calendar(&config, "test", None, &cache, &uuid_gen).unwrap();
    assert!(
        search_res
            .errors
            .iter()
            .any(|e| e.contains("Error on client test_client"))
    );

    let get_res =
        tool_get_calendar(&config, "2024-01-01", "2024-01-02", None, &cache, &uuid_gen).unwrap();
    assert!(
        get_res
            .errors
            .iter()
            .any(|e| e.contains("Error on client test_client"))
    );

    let item_res = tool_get_calendar_item(&config, "/item.ics").unwrap();
    assert!(
        item_res
            .errors
            .iter()
            .any(|e| e.contains("Error on client test_client"))
    );

    let add_res = tool_add_calendar_item(&config, None, "{}").unwrap();
    assert!(add_res.result.contains("Error on client test_client"));

    let update_res = tool_update_calendar_item(&config, "/item.ics", None, "{}").unwrap();
    assert!(update_res.result.contains("Error on client test_client"));

    let delete_res = tool_delete_calendar_item(&config, "/item.ics", None).unwrap();
    assert!(delete_res.result.contains("Error on client test_client"));
}

/// Build a [`AgentConfig`] with a single CalDAV client pointing at
/// `server_uri`. Keeps the three mock tests below focused on the
/// behaviour they exercise.
fn dav_config_for(server_uri: String) -> AgentConfig {
    let mut config = AgentConfig::default();
    config.caldav_clients.insert(
        "mock_client".to_string(),
        crate::config::CalDavClient {
            url: server_uri,
            username: "user".to_string(),
            password: "password".to_string(),
        },
    );
    config
}

#[test]
fn test_caldav_tools_mock_server() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock = WiremockGuard::start();
    register_caldav_stubs(&mock);
    let config = dav_config_for(mock.uri());
    let cache = crate::tools::registry::cache::ToolCache::new();
    let uuid_gen = crate::utils::uuid::SystemUuidGenerator;

    // 1. Search calendar
    let search_res = tool_search_calendar(&config, "Bob", None, &cache, &uuid_gen).unwrap();
    assert!(
        search_res
            .results
            .iter()
            .any(|r| r.summary.as_deref() == Some("Meeting with Bob"))
    );

    // 2. Get calendar (date range)
    let get_res =
        tool_get_calendar(&config, "2024-01-01", "2024-01-02", None, &cache, &uuid_gen).unwrap();
    assert!(
        get_res
            .results
            .iter()
            .any(|r| r.summary.as_deref() == Some("Meeting with Bob"))
    );

    // 3. Get calendar item success
    let item_res = tool_get_calendar_item(&config, "/item1.ics").unwrap();
    assert_eq!(
        item_res.item.as_ref().and_then(|it| it.summary.as_deref()),
        Some("Existing Item")
    );

    // 4. Get calendar item 404
    let item_res_404 = tool_get_calendar_item(&config, "/notfound").unwrap();
    assert!(item_res_404.item.is_none());
    assert!(
        item_res_404
            .errors
            .iter()
            .any(|e| e.contains("Not found by href"))
    );

    // 5. Add calendar item
    let add_res = tool_add_calendar_item(&config, None, r#"{"summary":"New Mtg"}"#).unwrap();
    assert!(add_res.result.contains("Created at /calendars/primary/"));

    // 6. Update calendar item success
    let update_res =
        tool_update_calendar_item(&config, "/item1.ics", None, r#"{"summary":"Updated Mtg"}"#)
            .unwrap();
    assert!(update_res.result.contains("Updated successfully"));

    // 7. Update calendar item 404
    let update_res_404 =
        tool_update_calendar_item(&config, "/notfound", None, r#"{"summary":"Updated Mtg"}"#)
            .unwrap();
    assert!(
        update_res_404
            .result
            .contains("Failed to fetch event for update")
    );

    // 8. Delete calendar item success
    let delete_res = tool_delete_calendar_item(&config, "/item1.ics", None).unwrap();
    assert!(delete_res.result.contains("Deleted successfully"));

    // 9. Delete calendar item 500 error
    let delete_res_err = tool_delete_calendar_item(&config, "/fail", None).unwrap();
    assert!(delete_res_err.result.contains("Failed to DELETE event"));
}

/// Regression: `DavClient` uses reqwest connection pooling, so
/// `tool_get_calendar` reuses the same TCP connection for its internal
/// PROPFIND + REPORT sequence. A mock server that drops the connection
/// after one request turns the second request into a connection-closed
/// error and silently returns an empty `results` payload. Loop the call
/// to make the race observable without flakiness.
///
/// wiremock is built on hyper and keeps HTTP/1.1 connections alive by
/// default, so the test exercises the production path: a kept-alive
/// socket carries the PROPFIND + REPORT pair for each call.
#[test]
fn test_caldav_tools_mock_server_keep_alive() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock = WiremockGuard::start();
    register_caldav_stubs(&mock);
    let config = dav_config_for(mock.uri());

    let cache = crate::tools::registry::cache::ToolCache::new();
    let uuid_gen = crate::utils::uuid::SystemUuidGenerator;

    for _ in 0..16 {
        let get_res =
            tool_get_calendar(&config, "2024-01-01", "2024-01-02", None, &cache, &uuid_gen)
                .unwrap();
        assert!(
            get_res
                .results
                .iter()
                .any(|r| r.summary.as_deref() == Some("Meeting with Bob")),
            "expected REPORT response on reused connection, got: {:?}",
            get_res.results
        );
    }
}

#[test]
fn test_caldav_tools_targeted_client_routing() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock_personal = WiremockGuard::start();
    let mock_work = WiremockGuard::start();
    register_caldav_stubs(&mock_personal);
    register_caldav_stubs(&mock_work);

    let mut config = AgentConfig::default();
    config.caldav_clients.insert(
        "personal".to_string(),
        crate::config::CalDavClient {
            url: mock_personal.uri(),
            username: "user".to_string(),
            password: "password".to_string(),
        },
    );
    config.caldav_clients.insert(
        "work".to_string(),
        crate::config::CalDavClient {
            url: mock_work.uri(),
            username: "user".to_string(),
            password: "password".to_string(),
        },
    );

    // 1. Add targeted to work
    let add_res =
        tool_add_calendar_item(&config, Some("work"), r#"{"summary":"Work Project"}"#).unwrap();
    assert!(add_res.result.contains("--- Client: work ---"));
    assert!(!add_res.result.contains("--- Client: personal ---"));

    // 2. Add targeted to invalid client
    let add_err =
        tool_add_calendar_item(&config, Some("nonexistent"), r#"{"summary":"X"}"#).unwrap_err();
    assert!(add_err.contains("CalDAV client 'nonexistent' not found"));

    // 3. Update targeted to personal
    let update_res = tool_update_calendar_item(
        &config,
        "/item1.ics",
        Some("personal"),
        r#"{"summary":"Personal Event"}"#,
    )
    .unwrap();
    assert!(update_res.result.contains("--- Client: personal ---"));
    assert!(!update_res.result.contains("--- Client: work ---"));

    // 4. Update targeted to invalid client
    let update_err = tool_update_calendar_item(
        &config,
        "/item1.ics",
        Some("nonexistent"),
        r#"{"summary":"X"}"#,
    )
    .unwrap_err();
    assert!(update_err.contains("CalDAV client 'nonexistent' not found"));

    // 5. Delete targeted to work
    let delete_res = tool_delete_calendar_item(&config, "/item1.ics", Some("work")).unwrap();
    assert!(delete_res.result.contains("--- Client: work ---"));
    assert!(!delete_res.result.contains("--- Client: personal ---"));

    // 6. Delete targeted to invalid client
    let delete_err =
        tool_delete_calendar_item(&config, "/item1.ics", Some("nonexistent")).unwrap_err();
    assert!(delete_err.contains("CalDAV client 'nonexistent' not found"));
}

#[test]
fn test_caldav_tools_multi_server_404_suppression() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock_found = WiremockGuard::start();
    let mock_missing = WiremockGuard::start();

    // mock_found has calendar discovery, GET /item1.ics (200), PUT (201), DELETE /item1.ics (204)
    register_caldav_stubs(&mock_found);

    // mock_missing has calendar discovery, but GET /item1.ics returns 404, DELETE /item1.ics returns 404
    mock_missing.register(
        Mock::given(method("PROPFIND")).respond_with(
            ResponseTemplate::new(207)
                .insert_header("content-type", "application/xml")
                .set_body_string(PROPFIND_BODY),
        ),
    );
    mock_missing.register(
        Mock::given(method("GET"))
            .and(wm_path("/item1.ics"))
            .respond_with(ResponseTemplate::new(404).set_body_string("Not Found")),
    );
    mock_missing.register(
        Mock::given(method("DELETE"))
            .and(wm_path("/item1.ics"))
            .respond_with(ResponseTemplate::new(404).set_body_string("Not Found")),
    );

    let mut config = AgentConfig::default();
    config.caldav_clients.insert(
        "found_client".to_string(),
        crate::config::CalDavClient {
            url: mock_found.uri(),
            username: "user".to_string(),
            password: "password".to_string(),
        },
    );
    config.caldav_clients.insert(
        "missing_client".to_string(),
        crate::config::CalDavClient {
            url: mock_missing.uri(),
            username: "user".to_string(),
            password: "password".to_string(),
        },
    );

    // 1. Get calendar item: found_client succeeds, missing_client returns 404.
    // 404 error from missing_client should be suppressed!
    let get_res = tool_get_calendar_item(&config, "/item1.ics").unwrap();
    assert_eq!(
        get_res.item.as_ref().and_then(|i| i.summary.as_deref()),
        Some("Existing Item")
    );
    assert!(
        get_res.errors.is_empty(),
        "404 error should be suppressed, got: {:?}",
        get_res.errors
    );

    // 2. Update calendar item: found_client updates, missing_client returns 404.
    // Result should contain found_client success, and missing_client 404 is suppressed!
    let update_res =
        tool_update_calendar_item(&config, "/item1.ics", None, r#"{"summary":"Updated"}"#).unwrap();
    assert!(update_res.result.contains("--- Client: found_client ---"));
    assert!(update_res.result.contains("Updated successfully"));
    assert!(
        !update_res.result.contains("missing_client"),
        "404 from missing_client should be suppressed, got: {}",
        update_res.result
    );

    // 3. Delete calendar item: found_client deletes, missing_client returns 404.
    // Result should contain found_client success, and missing_client 404 is suppressed!
    let delete_res = tool_delete_calendar_item(&config, "/item1.ics", None).unwrap();
    assert!(delete_res.result.contains("--- Client: found_client ---"));
    assert!(delete_res.result.contains("Deleted successfully"));
    assert!(
        !delete_res.result.contains("missing_client"),
        "404 from missing_client should be suppressed, got: {}",
        delete_res.result
    );

    // 4. Update when BOTH fail with 404: error is NOT suppressed because nothing succeeded.
    let update_all_fail =
        tool_update_calendar_item(&config, "/notfound", None, r#"{"summary":"Updated"}"#).unwrap();
    assert!(update_all_fail.result.contains("found_client"));
    assert!(update_all_fail.result.contains("missing_client"));
}

/// Regression: many sequential requests through one `DavClient` —
/// the most aggressive form of the keep-alive race. A mock that drops
/// the connection after each response will see every other request
/// fail with `connection closed before message completed`.
#[test]
fn test_caldav_tools_mock_server_single_client_reuse() {
    use crate::tools::blocking::block_on;

    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock = WiremockGuard::start();
    register_caldav_stubs(&mock);

    block_on(async {
        let cfg = dav_client_config(mock.uri());
        let client = DavClient::new("primary".to_string(), &cfg).unwrap();
        for _ in 0..32 {
            let items = client
                .calendar_query_timerange(
                    "/calendars/primary/",
                    "VEVENT",
                    Some("20240101T000000Z"),
                    Some("20240102T000000Z"),
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

// ---------------------------------------------------------------------------
// DavClient direct tests — exercise the new struct API without going
// through the `tool_*` LLM-adapter wrappers.
// ---------------------------------------------------------------------------

fn dav_client_config(uri: String) -> crate::config::CalDavClient {
    crate::config::CalDavClient {
        url: uri,
        username: "user".to_string(),
        password: "password".to_string(),
    }
}

/// All DavClient tests must call this. Initialises the rustls crypto stack the
/// first time it runs in a process. Under `cargo test` the earlier
/// `test_caldav_tools_*` tests install the provider before this
/// one runs; under `cargo nextest` each test runs in its own
/// process so the init must be repeated.
fn install_rustls_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

#[test]
fn dav_client_new_and_name() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    let cfg = dav_client_config(mock.uri());
    let client = DavClient::new("primary".to_string(), &cfg).expect("build DavClient");
    assert_eq!(client.name(), "primary");
}

#[test]
fn dav_client_search_calendar_returns_typed_results() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock = WiremockGuard::start();
    register_caldav_stubs(&mock);
    let cfg = dav_client_config(mock.uri());
    let client = DavClient::new("primary".to_string(), &cfg).expect("build DavClient");

    let events = client
        .search_calendar("Bob")
        .expect("search_calendar should succeed");
    assert!(
        events
            .iter()
            .any(|e| e.summary.as_deref() == Some("Meeting with Bob")),
        "expected to find 'Meeting with Bob' in {events:?}"
    );
    // Every result must carry the client name so the LLM can attribute
    // it to a server.
    assert!(events.iter().all(|e| e.client == "primary"));
}

#[test]
fn dav_client_get_calendar_returns_typed_results() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock = WiremockGuard::start();
    register_caldav_stubs(&mock);
    let cfg = dav_client_config(mock.uri());
    let client = DavClient::new("primary".to_string(), &cfg).expect("build DavClient");

    let events = client
        .get_calendar("2024-01-01", "2024-01-02")
        .expect("get_calendar should succeed");
    assert!(!events.is_empty(), "expected at least one event");
    assert!(
        events[0].summary.as_deref() == Some("Meeting with Bob"),
        "got: {events:?}"
    );
}

#[test]
fn dav_client_get_calendar_item_404_includes_status() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock = WiremockGuard::start();
    register_caldav_stubs(&mock);
    let cfg = dav_client_config(mock.uri());
    let client = DavClient::new("primary".to_string(), &cfg).expect("build DavClient");

    let err = client
        .get_calendar_item("/notfound")
        .expect_err("expected 404");
    assert!(
        err.contains("Not found by href"),
        "error should mention the lookup failure, got: {err}"
    );
    assert!(
        err.contains("404"),
        "error should include the status line, got: {err}"
    );
}

#[test]
fn dav_client_add_calendar_item_returns_created_path() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock = WiremockGuard::start();
    register_caldav_stubs(&mock);
    let cfg = dav_client_config(mock.uri());
    let client = DavClient::new("primary".to_string(), &cfg).expect("build DavClient");

    let out = client
        .add_calendar_item(r#"{"summary":"New Mtg"}"#)
        .expect("add_calendar_item should succeed");
    assert!(
        out.starts_with("Created at /calendars/primary/"),
        "got: {out}"
    );
    assert!(out.ends_with(".ics"), "expected .ics suffix, got: {out}");
}

#[test]
fn dav_client_delete_calendar_item_500_carries_error_text() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock = WiremockGuard::start();
    register_caldav_stubs(&mock);
    let cfg = dav_client_config(mock.uri());
    let client = DavClient::new("primary".to_string(), &cfg).expect("build DavClient");

    let err = client
        .delete_calendar_item("/fail")
        .expect_err("expected 500");
    assert!(
        err.contains("Failed to DELETE event"),
        "error should mention the DELETE failure, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// Unified `DavClient` CardDAV tests — exercise the new card surface
// on the same struct that handles CalDAV.
// ---------------------------------------------------------------------------

/// vCard body returned for `GET /alice.vcf`. The smoke test greps
/// for the literal `Alice Example` so we make the FN match.
const CARD_ALICE_VCF: &str = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Alice Example\r\nUID:alice-1\r\nEMAIL:alice@example.com\r\nEND:VCARD";

/// Register every stub the card-side DavClient tests need against
/// `mock`. Centralised so the four tests below share a consistent
/// CardDAV service.
fn register_carddav_stubs(mock: &WiremockGuard) {
    // PROPFIND: any path → 207 with one addressbook.
    mock.register(
        Mock::given(method("PROPFIND")).respond_with(
            ResponseTemplate::new(207)
                .insert_header("content-type", "application/xml")
                .set_body_string(
                    r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:carddav">
 <d:response>
  <d:href>/addressbooks/primary/</d:href>
  <d:propstat>
   <d:prop>
    <d:resourcetype><d:collection/><c:addressbook/></d:resourcetype>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>
</d:multistatus>"#,
                ),
        ),
    );
    // GET /alice.vcf → 200 with the vCard body.
    mock.register(
        Mock::given(method("GET"))
            .and(wm_path("/alice.vcf"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/vcard")
                    .set_body_string(CARD_ALICE_VCF),
            ),
    );
    // DELETE /alice.vcf → 204.
    mock.register(
        Mock::given(method("DELETE"))
            .and(wm_path("/alice.vcf"))
            .respond_with(ResponseTemplate::new(204).set_body_string("")),
    );
    // DELETE /missing.vcf → 404 (treated as success in
    // `delete_contact`).
    mock.register(
        Mock::given(method("DELETE"))
            .and(wm_path("/missing.vcf"))
            .respond_with(
                ResponseTemplate::new(404)
                    .insert_header("content-type", "text/plain")
                    .set_body_string("Not Found"),
            ),
    );
}

#[test]
fn dav_client_get_contact_parses_vcard() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    register_carddav_stubs(&mock);
    let cfg = dav_client_config(mock.uri());
    let client = DavClient::new("primary".to_string(), &cfg).expect("build DavClient");

    let contact = client
        .get_contact("/alice.vcf")
        .expect("get_contact should succeed");
    assert_eq!(contact.fn_name.as_deref(), Some("Alice Example"));
    assert_eq!(contact.email.as_deref(), Some("alice@example.com"));
    assert_eq!(contact.client, "primary");
}

#[test]
fn dav_client_delete_contact_204_returns_status() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    register_carddav_stubs(&mock);
    let cfg = dav_client_config(mock.uri());
    let client = DavClient::new("primary".to_string(), &cfg).expect("build DavClient");

    let out = client
        .delete_contact("/alice.vcf")
        .expect("delete_contact should succeed");
    assert!(
        out.contains("Deleted"),
        "expected success message, got: {out}"
    );
}

#[test]
fn dav_client_delete_contact_404_treated_as_success() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    register_carddav_stubs(&mock);
    let cfg = dav_client_config(mock.uri());
    let client = DavClient::new("primary".to_string(), &cfg).expect("build DavClient");

    let out = client
        .delete_contact("/missing.vcf")
        .expect("delete_contact should treat 404 as success");
    assert!(
        out.contains("Already absent"),
        "expected idempotent 404 message, got: {out}"
    );
}

#[test]
fn dav_client_get_contact_404_includes_status() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    register_carddav_stubs(&mock);
    let cfg = dav_client_config(mock.uri());
    let client = DavClient::new("primary".to_string(), &cfg).expect("build DavClient");

    let err = client
        .get_contact("/no-such-contact.vcf")
        .expect_err("expected 404");
    assert!(
        err.contains("Not found by href"),
        "error should mention the lookup failure, got: {err}"
    );
}

#[test]
fn test_calendar_cursor_sessions() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    let uuid_gen = crate::utils::uuid::SystemUuidGenerator;
    let events: Vec<CalDavEventDetails> = (1..=70)
        .map(|i| CalDavEventDetails {
            client: "c".to_string(),
            id: format!("/cal/{}.ics", i),
            href: format!("/cal/{}.ics", i),
            summary: Some(format!("Event {}", i)),
            start: None,
            end: None,
            description: None,
            location: None,
            organizer: None,
        })
        .collect();

    let page1 = cache
        .calendar_search_sessions
        .create_session(events, &uuid_gen);
    assert_eq!(page1.total, 70);
    assert_eq!(page1.items.len(), 32);
    assert!(page1.cursor.is_some());
    assert!(page1.hint.is_none());
    let cursor1 = page1.cursor.unwrap();

    let page2 = cache.calendar_search_sessions.next_page(&cursor1).unwrap();
    assert_eq!(page2.total, 70);
    assert_eq!(page2.items.len(), 32);
    assert!(page2.cursor.is_some());
    assert!(page2.hint.is_none());
    let cursor2 = page2.cursor.unwrap();

    let page3 = cache.calendar_search_sessions.next_page(&cursor2).unwrap();
    assert_eq!(page3.total, 70);
    assert_eq!(page3.items.len(), 6);
    assert!(page3.cursor.is_none());
    assert_eq!(page3.hint.as_deref(), Some("Final page."));
}

// ---------------------------------------------------------------------------
// Additional DavClient error-branch coverage (P1-1): PUT failures,
// update-GET failures, and the CardDAV add/update failure paths.
// ---------------------------------------------------------------------------

/// Register a CalDAV service where the calendar discovery succeeds but the
/// PUT endpoint returns 500. Used to drive the `add_calendar_item` /
/// `update_calendar_item` non-2xx branches.
fn register_caldav_put_failure(mock: &WiremockGuard) {
    mock.register(
        Mock::given(method("PROPFIND")).respond_with(
            ResponseTemplate::new(207)
                .insert_header("content-type", "application/xml")
                .set_body_string(PROPFIND_BODY),
        ),
    );
    mock.register(
        Mock::given(method("PUT")).respond_with(
            ResponseTemplate::new(500)
                .insert_header("content-type", "text/plain")
                .set_body_string("put failed"),
        ),
    );
}

#[test]
fn dav_client_add_calendar_item_put_failure() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock = WiremockGuard::start();
    register_caldav_put_failure(&mock);
    let cfg = dav_client_config(mock.uri());
    let client = DavClient::new("primary".to_string(), &cfg).expect("build DavClient");

    let err = client
        .add_calendar_item(r#"{"summary":"New Mtg"}"#)
        .expect_err("expected PUT failure");
    assert!(
        err.contains("Failed to PUT event"),
        "error should mention the PUT failure, got: {err}"
    );
}

#[test]
fn dav_client_update_calendar_item_put_failure() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock = WiremockGuard::start();
    // Successful GET + failing PUT.
    mock.register(
        Mock::given(method("PROPFIND")).respond_with(
            ResponseTemplate::new(207)
                .insert_header("content-type", "application/xml")
                .set_body_string(PROPFIND_BODY),
        ),
    );
    mock.register(
        Mock::given(method("GET"))
            .and(wm_path("/item1.ics"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/calendar")
                    .set_body_string(ITEM1_ICS_BODY),
            ),
    );
    mock.register(
        Mock::given(method("PUT")).respond_with(
            ResponseTemplate::new(500)
                .insert_header("content-type", "text/plain")
                .set_body_string("put failed"),
        ),
    );
    let cfg = client_config(mock.uri());
    let client = DavClient::new("primary".to_string(), &cfg).expect("build DavClient");

    let err = client
        .update_calendar_item("/item1.ics", r#"{"summary":"Updated"}"#)
        .expect_err("expected PUT failure");
    assert!(
        err.contains("Failed to PUT update event"),
        "error should mention the update PUT failure, got: {err}"
    );
}

/// Register a CardDAV service with one addressbook and a PUT endpoint that
/// returns 500. Used to drive `add_contact` / `update_contact` PUT failures.
fn register_carddav_put_failure(mock: &WiremockGuard) {
    mock.register(
        Mock::given(method("PROPFIND")).respond_with(
            ResponseTemplate::new(207)
                .insert_header("content-type", "application/xml")
                .set_body_string(
                    r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:carddav">
 <d:response>
  <d:href>/addressbooks/primary/</d:href>
  <d:propstat>
   <d:prop>
    <d:resourcetype><d:collection/><c:addressbook/></d:resourcetype>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>
</d:multistatus>"#,
                ),
        ),
    );
    mock.register(
        Mock::given(method("PUT")).respond_with(
            ResponseTemplate::new(500)
                .insert_header("content-type", "text/plain")
                .set_body_string("put failed"),
        ),
    );
}

fn client_config(uri: String) -> crate::config::CalDavClient {
    dav_client_config(uri)
}

#[test]
fn dav_client_add_contact_put_failure() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock = WiremockGuard::start();
    register_carddav_put_failure(&mock);
    let cfg = client_config(mock.uri());
    let client = DavClient::new("primary".to_string(), &cfg).expect("build DavClient");

    let err = client
        .add_contact(r#"{"fn":"Bob"}"#)
        .expect_err("expected PUT failure");
    assert!(
        err.contains("Failed to PUT contact"),
        "error should mention the add PUT failure, got: {err}"
    );
}

#[test]
fn dav_client_update_contact_put_failure() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock = WiremockGuard::start();
    // Addressbook PROPFIND + successful GET + failing PUT.
    mock.register(
        Mock::given(method("PROPFIND")).respond_with(
            ResponseTemplate::new(207)
                .insert_header("content-type", "application/xml")
                .set_body_string(
                    r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:carddav">
 <d:response>
  <d:href>/addressbooks/primary/</d:href>
  <d:propstat>
   <d:prop>
    <d:resourcetype><d:collection/><c:addressbook/></d:resourcetype>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>
</d:multistatus>"#,
                ),
        ),
    );
    mock.register(
        Mock::given(method("GET"))
            .and(wm_path("/alice.vcf"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/vcard")
                    .set_body_string(CARD_ALICE_VCF),
            ),
    );
    mock.register(
        Mock::given(method("PUT")).respond_with(
            ResponseTemplate::new(500)
                .insert_header("content-type", "text/plain")
                .set_body_string("put failed"),
        ),
    );
    let cfg = client_config(mock.uri());
    let client = DavClient::new("primary".to_string(), &cfg).expect("build DavClient");

    let err = client
        .update_contact("/alice.vcf", r#"{"email":"bob@example.com"}"#)
        .expect_err("expected PUT failure");
    assert!(
        err.contains("Failed to PUT updated contact"),
        "error should mention the update PUT failure, got: {err}"
    );
}
