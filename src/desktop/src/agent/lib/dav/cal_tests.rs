//! Tests for `agent/lib/dav/cal.rs`.

use super::*;
use crate::config::AgentConfig;
use fast_dav_rs::CalDavClient;

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
/// so `fast_dav_rs::CalDavClient::list_calendars` succeeds on the
/// first try and never falls through to the principal-discovery
/// branches of `get_all_calendars`.
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
    let mut config = AgentConfig::default();
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
    use crate::tools::blocking::block_on;

    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock = WiremockGuard::start();
    register_caldav_stubs(&mock);

    block_on(async {
        let client = CalDavClient::new(&mock.uri(), Some("user"), Some("password")).unwrap();
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

/// All DavClient tests must call this. `CalDavClient::new` (which
/// `DavClient::new` wraps) initialises the rustls crypto stack the
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
