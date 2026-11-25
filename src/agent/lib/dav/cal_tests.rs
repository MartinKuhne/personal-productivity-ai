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

/// One `json_to_ical` render case: the JSON payload, the UID override, the
/// prefix the output must start with, and the fragments it must contain.
struct IcalRenderCase<'a> {
    label: &'a str,
    json: &'a str,
    uid: Option<&'a str>,
    starts_with: Option<&'a str>,
    must_contain: &'a [&'a str],
}

/// Assert one `json_to_ical` render case set.
fn assert_ical_render(cases: Vec<IcalRenderCase<'_>>) {
    for case in cases {
        let IcalRenderCase {
            label,
            json,
            uid,
            starts_with,
            must_contain,
        } = case;
        let ical = json_to_ical(json, uid);
        if let Some(prefix) = starts_with {
            assert!(
                ical.starts_with(prefix),
                "case '{label}': expected the iCalendar to start with '{prefix}', got:\n{ical}"
            );
        }
        for fragment in must_contain {
            assert!(
                ical.contains(fragment),
                "case '{label}': expected the iCalendar to contain '{fragment}', got:\n{ical}"
            );
        }
    }
}

#[test]
fn test_json_to_ical_renders_the_expected_lines_for_each_payload() {
    assert_ical_render(vec![
        IcalRenderCase {
            label: "the full payload",
            json: r#"{"summary":"Test","start":"2024-01-01T12:00:00","end":"2024-01-01T13:00:00","description":"desc","location":"loc"}"#,
            uid: None,
            starts_with: Some("BEGIN:VCALENDAR"),
            must_contain: &[
                "BEGIN:VEVENT",
                "END:VEVENT",
                "END:VCALENDAR",
                "SUMMARY:Test",
                "DESCRIPTION:desc",
                "LOCATION:loc",
            ],
        },
        IcalRenderCase {
            label: "date-only start and end",
            json: r#"{"summary":"Date Only","start":"2024-01-01","end":"2024-01-02"}"#,
            uid: None,
            starts_with: None,
            must_contain: &["DTSTART;VALUE=DATE:20240101", "DTEND;VALUE=DATE:20240102"],
        },
        IcalRenderCase {
            label: "invalid JSON falls back to the default summary",
            json: "not json at all",
            uid: None,
            starts_with: None,
            must_contain: &["SUMMARY:New Event"],
        },
        // Even an empty JSON should produce a valid structure with a default
        // summary.
        IcalRenderCase {
            label: "empty JSON still produces a valid event",
            json: "{}",
            uid: None,
            starts_with: Some("BEGIN:VCALENDAR"),
            must_contain: &[
                "BEGIN:VEVENT",
                "END:VEVENT",
                "END:VCALENDAR",
                "SUMMARY:New Event",
            ],
        },
        IcalRenderCase {
            label: "an explicit UID override is emitted",
            json: r#"{"summary":"Test"}"#,
            uid: Some("custom-uid-123"),
            starts_with: None,
            must_contain: &["UID:custom-uid-123"],
        },
        IcalRenderCase {
            label: "special chars are escaped",
            json: r#"{"summary":"Hello;World,Line1\nLine2"}"#,
            uid: None,
            starts_with: None,
            must_contain: &["Hello\\;World\\,Line1\\nLine2"],
        },
    ]);
}

// --- update_ical_string tests ---

/// One `update_ical_string` case: the original iCalendar, the JSON updates and
/// the fragments the result must and must not contain.
struct IcalUpdateCase<'a> {
    label: &'a str,
    original: &'a str,
    updates: serde_json::Value,
    must_contain: &'a [&'a str],
    must_not_contain: &'a [&'a str],
}

/// Assert one `update_ical_string` case set.
fn assert_ical_update(cases: Vec<IcalUpdateCase<'_>>) {
    for case in cases {
        let IcalUpdateCase {
            label,
            original,
            updates,
            must_contain,
            must_not_contain,
        } = case;
        let result = update_ical_string(original, &updates);
        for fragment in must_contain {
            assert!(
                result.contains(fragment),
                "case '{label}': expected the result to contain '{fragment}', got:\n{result}"
            );
        }
        for fragment in must_not_contain {
            assert!(
                !result.contains(fragment),
                "case '{label}': expected the result NOT to contain '{fragment}', got:\n{result}"
            );
        }
    }
}

#[test]
fn test_update_ical_string_replaces_and_adds_the_requested_fields() {
    assert_ical_update(vec![
        IcalUpdateCase {
            label: "replaces the summary",
            original: "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:Old\r\nDTSTART:20240101T120000\r\nEND:VEVENT\r\nEND:VCALENDAR",
            updates: serde_json::json!({"summary": "New"}),
            must_contain: &["SUMMARY:New"],
            must_not_contain: &["SUMMARY:Old"],
        },
        IcalUpdateCase {
            // A missing SUMMARY gets added at the end of VEVENT.
            label: "adds fields that were missing",
            original: "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nDTSTART:20240101T120000\r\nEND:VEVENT\r\nEND:VCALENDAR",
            updates: serde_json::json!({
                "summary": "Added Summary",
                "start": "2024-05-01",
                "end": "2024-05-02",
                "description": "Added Desc",
                "location": "Added Loc"
            }),
            must_contain: &[
                "SUMMARY:Added Summary",
                "DTSTART;VALUE=DATE:20240501",
                "DTEND;VALUE=DATE:20240502",
                "DESCRIPTION:Added Desc",
                "LOCATION:Added Loc",
            ],
            must_not_contain: &[],
        },
        IcalUpdateCase {
            label: "replaces every field and drops folded continuations",
            original: "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:Old Summary\r\n folded line\r\nDTSTART:20240101T120000\r\nDTEND:20240101T130000\r\nDESCRIPTION:Old Desc\r\n LOCATION:Old Loc\r\nEND:VEVENT\r\nEND:VCALENDAR",
            updates: serde_json::json!({
                "summary": "New Summary",
                "start": "2024-02-01T10:00:00",
                "end": "2024-02-01T11:00:00",
                "description": "New Desc with; special, chars\nline2",
                "location": "New Loc"
            }),
            must_contain: &[
                "SUMMARY:New Summary",
                "DTSTART:20240201T100000",
                "DTEND:20240201T110000",
                "DESCRIPTION:New Desc with\\; special\\, chars\\nline2",
                "LOCATION:New Loc",
            ],
            must_not_contain: &["folded line"],
        },
        IcalUpdateCase {
            label: "no updates preserves the original",
            original: "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:Keep\r\nDTSTART:20240101T120000\r\nEND:VEVENT\r\nEND:VCALENDAR",
            updates: serde_json::json!({}),
            must_contain: &["SUMMARY:Keep"],
            must_not_contain: &[],
        },
    ]);
}

#[test]
fn test_update_ical_string_replaces_dtstart() {
    let original = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:Test\r\nDTSTART:20240101\r\nEND:VEVENT\r\nEND:VCALENDAR";
    let updates = serde_json::json!({"start": "20250101"});
    let result = update_ical_string(original, &updates);
    assert!(result.contains("DTSTART;VALUE=DATE:20250101") || result.contains("DTSTART:20250101"));
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

    let search_res = tool_search_calendar(&config, Some("test"), None, &cache, &uuid_gen).unwrap();
    assert!(search_res.results.is_empty());
    assert_eq!(search_res.total, 0);
    assert_eq!(search_res.hint.as_deref(), Some("Final page."));

    let get_res = tool_get_calendar(
        &config,
        Some("2024-01-01"),
        Some("2024-01-02"),
        None,
        &cache,
        &uuid_gen,
    )
    .unwrap();
    assert!(get_res.results.is_empty());
    assert_eq!(get_res.total, 0);
    assert_eq!(get_res.hint.as_deref(), Some("Final page."));

    let item_res = tool_get_calendar_item(&config, "/item.ics").unwrap();
    assert_eq!(item_res.item, None);
    assert!(item_res.errors.is_empty());
}

#[test]
fn test_calendar_tools_use_cursor_and_ignore_fresh_params() {
    let config = AgentConfig::default();
    let cache = crate::tools::registry::cache::ToolCache::new();
    let uuid_gen = crate::utils::uuid::SystemUuidGenerator;

    let items: Vec<CalDavEventDetails> = (0..35)
        .map(|i| CalDavEventDetails {
            client: "c1".to_string(),
            id: format!("event_{i}"),
            href: format!("/events/{i}.ics"),
            summary: Some(format!("Event {i}")),
            start: None,
            end: None,
            description: None,
            location: None,
            organizer: None,
        })
        .collect();

    let search_page = cache
        .calendar_search_sessions
        .create_session(items.clone(), &uuid_gen);
    let search_cursor = search_page.cursor.expect("session should have cursor");

    let search_res = tool_search_calendar(
        &config,
        Some("meeting"),
        Some(search_cursor),
        &cache,
        &uuid_gen,
    )
    .expect("tool_search_calendar should succeed with cursor and ignore keyword");
    assert_eq!(search_res.count, 3);
    assert_eq!(search_res.total, 35);

    let get_page = cache.calendar_get_sessions.create_session(items, &uuid_gen);
    let get_cursor = get_page.cursor.expect("session should have cursor");

    let get_res = tool_get_calendar(
        &config,
        Some("2024-01-01"),
        Some("2024-01-02"),
        Some(get_cursor),
        &cache,
        &uuid_gen,
    )
    .expect("tool_get_calendar should succeed with cursor and ignore date range");
    assert_eq!(get_res.count, 3);
    assert_eq!(get_res.total, 35);
}

#[test]
fn test_calendar_tools_reject_missing_params_and_cursor() {
    let config = AgentConfig::default();
    let cache = crate::tools::registry::cache::ToolCache::new();
    let uuid_gen = crate::utils::uuid::SystemUuidGenerator;

    let err = tool_search_calendar(&config, None, None, &cache, &uuid_gen).unwrap_err();
    assert!(err.contains("Give `keyword`"), "unexpected error: {err}");

    let err = tool_get_calendar(&config, None, None, None, &cache, &uuid_gen).unwrap_err();
    assert!(err.contains("`start_date`"), "unexpected error: {err}");
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

    let search_res = tool_search_calendar(&config, Some("test"), None, &cache, &uuid_gen).unwrap();
    assert!(
        search_res
            .errors
            .iter()
            .any(|e| e.contains("Error on client test_client"))
    );

    let get_res = tool_get_calendar(
        &config,
        Some("2024-01-01"),
        Some("2024-01-02"),
        None,
        &cache,
        &uuid_gen,
    )
    .unwrap();
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
    let search_res = tool_search_calendar(&config, Some("Bob"), None, &cache, &uuid_gen).unwrap();
    assert!(
        search_res
            .results
            .iter()
            .any(|r| r.summary.as_deref() == Some("Meeting with Bob"))
    );

    // 2. Get calendar (date range)
    let get_res = tool_get_calendar(
        &config,
        Some("2024-01-01"),
        Some("2024-01-02"),
        None,
        &cache,
        &uuid_gen,
    )
    .unwrap();
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
        let get_res = tool_get_calendar(
            &config,
            Some("2024-01-01"),
            Some("2024-01-02"),
            None,
            &cache,
            &uuid_gen,
        )
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
fn dav_client_delete_contact_treats_204_and_404_as_success() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    register_carddav_stubs(&mock);
    let cfg = dav_client_config(mock.uri());
    let client = DavClient::new("primary".to_string(), &cfg).expect("build DavClient");

    // A 204 is a plain delete; a 404 is reported as an idempotent no-op.
    let cases = [
        ("/alice.vcf", "Deleted"),
        ("/missing.vcf", "Already absent"),
    ];
    for (href, expected) in cases {
        let out = client
            .delete_contact(href)
            .unwrap_or_else(|err| panic!("href '{href}' should succeed, got: {err}"));
        assert!(
            out.contains(expected),
            "href '{href}': expected an idempotent success message, got: {out}"
        );
    }
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

// ---------------------------------------------------------------------------
// DavClient construction and URL resolution
// ---------------------------------------------------------------------------

fn caldav_client_at(uri: String) -> DavClient {
    let cfg = dav_client_config(uri);
    DavClient::new("primary".to_string(), &cfg).expect("build DavClient")
}

fn raw_client_with_url(url: &str) -> crate::config::CalDavClient {
    crate::config::CalDavClient {
        url: url.to_string(),
        username: "user".to_string(),
        password: "password".to_string(),
    }
}

#[test]
fn dav_client_new_rejects_a_non_http_url() {
    install_rustls_provider();
    // `DavClient` is not `Debug`, so this cannot use `expect_err`.
    let err = match DavClient::new(
        "c".to_string(),
        &raw_client_with_url("ftp://example.invalid/dav"),
    ) {
        Ok(_) => panic!("a non-HTTP scheme must be rejected"),
        Err(err) => err,
    };
    assert_eq!(
        err,
        "Client config error: invalid URL 'ftp://example.invalid/dav'"
    );
}

#[test]
fn dav_client_new_accepts_both_http_schemes() {
    install_rustls_provider();
    let plain = DavClient::new(
        "a".to_string(),
        &raw_client_with_url("http://example.invalid"),
    );
    assert!(plain.is_ok(), "http:// must be accepted");
    let secure = DavClient::new(
        "b".to_string(),
        &raw_client_with_url("https://example.invalid"),
    );
    assert!(secure.is_ok(), "https:// must be accepted");
    assert_eq!(secure.unwrap().name(), "b");
}

#[test]
fn dav_client_resolve_url_passes_an_absolute_href_through() {
    // `GET {id}` with a fully-qualified href must not be joined onto the
    // base URL — the request must land on the mock server as-is.
    install_rustls_provider();
    let mock = WiremockGuard::start();
    register_caldav_stubs(&mock);
    let client = caldav_client_at(mock.uri());

    let absolute = format!("{}/item1.ics", mock.uri());
    let item = client
        .get_calendar_item(&absolute)
        .expect("an absolute href should resolve unchanged");
    assert_eq!(item.summary.as_deref(), Some("Existing Item"));
    assert_eq!(item.href, absolute);
}

#[test]
fn dav_client_resolve_url_joins_a_relative_path_onto_the_base() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    register_caldav_stubs(&mock);
    // A base URL with a path segment: the relative href must be resolved
    // against it rather than appended blindly.
    let client = caldav_client_at(format!("{}/dav", mock.uri()));
    let item = client
        .get_calendar_item("/item1.ics")
        .expect("the joined URL should still reach the mock");
    assert_eq!(item.summary.as_deref(), Some("Existing Item"));
}

#[test]
fn dav_client_resolve_url_falls_back_when_the_base_cannot_be_parsed() {
    // `"http://["` passes the scheme check in `new` but is not a URL
    // `url::Url` can parse, so `resolve_url` takes its manual
    // string-concatenation fallback. The resulting request then fails at
    // the transport layer rather than panicking.
    install_rustls_provider();
    let client = DavClient::new("c".to_string(), &raw_client_with_url("http://[")).unwrap();
    let err = client
        .get_calendar_item("/item.ics")
        .expect_err("an unparseable base cannot produce a request");
    assert!(!err.is_empty(), "expected a transport error");
    assert!(
        !err.contains("Not found by href"),
        "this must be a transport failure, not an HTTP status: {err}"
    );
}

// ---------------------------------------------------------------------------
// DavClient protocol-method error statuses
// ---------------------------------------------------------------------------

fn register_status(mock: &WiremockGuard, method_name: &'static str, status: u16, body: &str) {
    mock.register(
        Mock::given(method(method_name)).respond_with(
            ResponseTemplate::new(status)
                .insert_header("content-type", "text/plain")
                .set_body_string(body),
        ),
    );
}

#[test]
fn dav_client_list_calendars_reports_a_non_2xx_status() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    register_status(&mock, "PROPFIND", 403, "forbidden");
    let client = caldav_client_at(mock.uri());

    let err = crate::tools::blocking::block_on(client.list_calendars("/"))
        .expect_err("403 must be an error");
    assert_eq!(err.to_string(), "PROPFIND calendars failed: 403 Forbidden");
}

#[test]
fn dav_client_list_calendars_returns_the_parsed_hrefs() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    register_caldav_stubs(&mock);
    let client = caldav_client_at(mock.uri());

    let hrefs = crate::tools::blocking::block_on(client.list_calendars("/")).unwrap();
    assert_eq!(hrefs, vec!["/calendars/primary/".to_string()]);
}

/// A multistatus advertising a calendar-home-set at `href`.
fn cal_home_set_body(href: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
 <d:response><d:propstat><d:prop>
  <c:calendar-home-set><d:href>{href}</d:href></c:calendar-home-set>
 </d:prop></d:propstat></d:response>
</d:multistatus>"#
    )
}

/// A multistatus advertising a current-user-principal at `href`.
fn principal_body(href: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
 <d:response><d:propstat><d:prop>
  <d:current-user-principal><d:href>{href}</d:href></d:current-user-principal>
 </d:prop></d:propstat></d:response>
</d:multistatus>"#
    )
}

/// A multistatus carrying both a calendar-home-set and a principal.
fn home_set_and_principal_body(home: &str, principal: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
 <d:response><d:propstat><d:prop>
  <c:calendar-home-set><d:href>{home}</d:href></c:calendar-home-set>
  <d:current-user-principal><d:href>{principal}</d:href></d:current-user-principal>
 </d:prop></d:propstat></d:response>
</d:multistatus>"#
    )
}

/// A well-formed multistatus with no children at all.
const EMPTY_MULTISTATUS: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
</d:multistatus>"#;

fn xml_207(body: &str) -> ResponseTemplate {
    ResponseTemplate::new(207)
        .insert_header("content-type", "application/xml")
        .set_body_string(body.to_string())
}

fn propfind_at(mock: &WiremockGuard, path: &str, body: &str) {
    mock.register(
        Mock::given(method("PROPFIND"))
            .and(wm_path(path))
            .respond_with(xml_207(body)),
    );
}

#[test]
fn dav_client_discover_calendar_home_set_returns_hrefs() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    propfind_at(&mock, "/", &cal_home_set_body("/calendars/user/"));
    let client = caldav_client_at(mock.uri());

    let homes = crate::tools::blocking::block_on(client.discover_calendar_home_set("/")).unwrap();
    assert_eq!(homes, vec!["/calendars/user/".to_string()]);
}

#[test]
fn dav_client_discover_calendar_home_set_reports_a_non_2xx_status() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    register_status(&mock, "PROPFIND", 401, "nope");
    let client = caldav_client_at(mock.uri());

    let err = crate::tools::blocking::block_on(client.discover_calendar_home_set("/"))
        .expect_err("401 must be an error");
    assert_eq!(
        err.to_string(),
        "PROPFIND calendar-home-set failed: 401 Unauthorized"
    );
}

#[test]
fn dav_client_discover_current_user_principal_returns_the_href() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    propfind_at(&mock, "/", &principal_body("/principals/me/"));
    let client = caldav_client_at(mock.uri());

    let principal =
        crate::tools::blocking::block_on(client.discover_current_user_principal()).unwrap();
    assert_eq!(principal.as_deref(), Some("/principals/me/"));
}

#[test]
fn dav_client_discover_current_user_principal_is_none_without_the_property() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    propfind_at(&mock, "/", EMPTY_MULTISTATUS);
    let client = caldav_client_at(mock.uri());

    let principal =
        crate::tools::blocking::block_on(client.discover_current_user_principal()).unwrap();
    assert_eq!(principal, None);
}

#[test]
fn dav_client_discover_current_user_principal_swallows_a_bad_status() {
    // Unlike the other discovery helpers this one answers `Ok(None)` on a
    // non-2xx status so callers can keep falling through.
    install_rustls_provider();
    let mock = WiremockGuard::start();
    register_status(&mock, "PROPFIND", 500, "boom");
    let client = caldav_client_at(mock.uri());

    let principal =
        crate::tools::blocking::block_on(client.discover_current_user_principal()).unwrap();
    assert_eq!(principal, None);
}

#[test]
fn dav_client_calendar_query_timerange_reports_a_non_2xx_status() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    register_status(&mock, "REPORT", 403, "nope");
    let client = caldav_client_at(mock.uri());

    let err = crate::tools::blocking::block_on(client.calendar_query_timerange(
        "/calendars/primary/",
        "VEVENT",
        None,
        None,
    ))
    .expect_err("403 must be an error");
    assert_eq!(
        err.to_string(),
        "REPORT calendar-query failed: 403 Forbidden"
    );
}

#[test]
fn dav_client_calendar_query_timerange_keeps_items_without_calendar_data() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    mock.register(
        Mock::given(method("REPORT")).respond_with(xml_207(
            r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
 <d:response><d:href>/calendars/primary/no-data.ics</d:href>
  <d:propstat><d:prop/><d:status>HTTP/1.1 404 Not Found</d:status></d:propstat>
 </d:response>
 <d:response><d:propstat><d:prop/><d:status>HTTP/1.1 404 Not Found</d:status></d:propstat></d:response>
</d:multistatus>"#,
        )),
    );
    let client = caldav_client_at(mock.uri());

    let items = crate::tools::blocking::block_on(client.calendar_query_timerange(
        "/calendars/primary/",
        "VEVENT",
        None,
        None,
    ))
    .unwrap();
    // The response without an href is dropped; the one without
    // calendar-data survives with `None`.
    assert_eq!(items.len(), 1, "got: {items:?}");
    assert_eq!(items[0].href, "/calendars/primary/no-data.ics");
    assert_eq!(items[0].calendar_data, None);
}

// ---------------------------------------------------------------------------
// list_calendar_hrefs discovery strategies (driven through search_calendar)
// ---------------------------------------------------------------------------

#[test]
fn dav_client_search_calendar_discovers_through_the_home_set() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    // The base URL is not itself a calendar collection but advertises a
    // calendar-home-set.
    propfind_at(&mock, "/", &cal_home_set_body("/calendars/user/"));
    propfind_at(&mock, "/calendars/user/", PROPFIND_BODY);
    mock.register(Mock::given(method("REPORT")).respond_with(xml_207(REPORT_BODY)));
    let client = caldav_client_at(mock.uri());

    let events = client
        .search_calendar("Bob")
        .expect("search should succeed");
    assert_eq!(events.len(), 1, "got: {events:?}");
    assert_eq!(events[0].summary.as_deref(), Some("Meeting with Bob"));
}

#[test]
fn dav_client_search_calendar_discovers_through_the_principal() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    propfind_at(&mock, "/", &principal_body("/principals/me/"));
    propfind_at(
        &mock,
        "/principals/me/",
        &cal_home_set_body("/calendars/user/"),
    );
    propfind_at(&mock, "/calendars/user/", PROPFIND_BODY);
    mock.register(Mock::given(method("REPORT")).respond_with(xml_207(REPORT_BODY)));
    let client = caldav_client_at(mock.uri());

    let events = client
        .search_calendar("bob")
        .expect("search should succeed");
    assert_eq!(events.len(), 1, "got: {events:?}");
}

#[test]
fn dav_client_search_calendar_falls_back_to_the_fastmail_guess() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    // Nothing discoverable on the base URL; the guessed
    // /dav/principals/user/{username}/ path carries the home set.
    propfind_at(&mock, "/", EMPTY_MULTISTATUS);
    propfind_at(
        &mock,
        "/dav/principals/user/user/",
        &cal_home_set_body("/calendars/user/"),
    );
    propfind_at(&mock, "/calendars/user/", PROPFIND_BODY);
    mock.register(Mock::given(method("REPORT")).respond_with(xml_207(REPORT_BODY)));
    let client = caldav_client_at(mock.uri());

    let events = client
        .search_calendar("MEETING WITH BOB")
        .expect("the keyword match is case-insensitive");
    assert_eq!(events.len(), 1, "got: {events:?}");
}

#[test]
fn dav_client_search_calendar_errors_when_no_principal_is_found() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    propfind_at(&mock, "/", EMPTY_MULTISTATUS);
    propfind_at(&mock, "/dav/principals/user/user/", EMPTY_MULTISTATUS);
    let client = caldav_client_at(mock.uri());

    let err = client.search_calendar("Bob").expect_err("no principal");
    assert_eq!(err, "No principal found");
}

#[test]
fn dav_client_search_calendar_errors_when_no_home_is_found() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    propfind_at(&mock, "/", &principal_body("/principals/me/"));
    propfind_at(&mock, "/principals/me/", EMPTY_MULTISTATUS);
    let client = caldav_client_at(mock.uri());

    let err = client.search_calendar("Bob").expect_err("no home set");
    assert_eq!(err, "No calendar home found");
}

#[test]
fn dav_client_search_calendar_returns_no_matches_when_the_home_has_no_calendars() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    // Discovery succeeds end to end, but the home set is empty, so the
    // REPORT leg is never reached.
    propfind_at(
        &mock,
        "/",
        &home_set_and_principal_body("/calendars/user/", "/principals/me/"),
    );
    propfind_at(&mock, "/calendars/user/", EMPTY_MULTISTATUS);
    propfind_at(
        &mock,
        "/principals/me/",
        &cal_home_set_body("/calendars/user/"),
    );
    let client = caldav_client_at(mock.uri());

    let events = client.search_calendar("Bob").unwrap();
    assert!(events.is_empty(), "got: {events:?}");
}

#[test]
fn dav_client_search_calendar_skips_items_without_calendar_data() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    register_caldav_stubs(&mock);
    // A REPORT whose single item carries no calendar-data: nothing can
    // match the keyword, so the result set stays empty.
    // `register_caldav_stubs` already mounts a catch-all REPORT stub, and
    // wiremock breaks priority ties by insertion order (first mounted wins),
    // so this stub needs a higher priority to answer for this path at all.
    mock.register(
        Mock::given(method("REPORT"))
            .and(wm_path("/calendars/primary/"))
            .respond_with(xml_207(
                r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
 <d:response><d:href>/calendars/primary/blank.ics</d:href>
  <d:propstat><d:prop/><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
 </d:response>
</d:multistatus>"#,
            ))
            .with_priority(1),
    );
    let client = caldav_client_at(mock.uri());

    let events = client.search_calendar("Bob").unwrap();
    assert!(events.is_empty(), "got: {events:?}");
}

#[test]
fn dav_client_get_calendar_accepts_preformatted_dates() {
    // Dates that are not `YYYY-MM-DD` are passed to the REPORT untouched.
    install_rustls_provider();
    let mock = WiremockGuard::start();
    register_caldav_stubs(&mock);
    let client = caldav_client_at(mock.uri());

    let events = client
        .get_calendar("20240101T000000Z", "20240102T000000Z")
        .expect("get_calendar should succeed");
    assert_eq!(events.len(), 1, "got: {events:?}");
}

#[test]
fn dav_client_get_calendar_propagates_a_report_failure() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    propfind_at(&mock, "/", PROPFIND_BODY);
    register_status(&mock, "REPORT", 500, "boom");
    let client = caldav_client_at(mock.uri());

    let err = client
        .get_calendar("2024-01-01", "2024-01-02")
        .expect_err("the REPORT failure must surface");
    assert!(
        err.contains("REPORT calendar-query failed: 500"),
        "got: {err}"
    );
}

#[test]
fn dav_client_add_calendar_item_errors_when_no_calendar_exists() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    // Discovery succeeds all the way to a home set, but the home set
    // contains no calendar collections at all.
    propfind_at(
        &mock,
        "/",
        &home_set_and_principal_body("/calendars/user/", "/principals/me/"),
    );
    propfind_at(&mock, "/calendars/user/", EMPTY_MULTISTATUS);
    propfind_at(
        &mock,
        "/principals/me/",
        &cal_home_set_body("/calendars/user/"),
    );
    let client = caldav_client_at(mock.uri());

    let err = client
        .add_calendar_item(r#"{"summary":"X"}"#)
        .expect_err("there is nothing to add to");
    assert_eq!(err, "No calendar found to add to");
}

#[test]
fn dav_client_update_calendar_item_treats_invalid_json_as_an_empty_update() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    register_caldav_stubs(&mock);
    let client = caldav_client_at(mock.uri());

    let out = client
        .update_calendar_item("/item1.ics", "definitely not json")
        .expect("the PUT should still go through");
    assert_eq!(out, "Updated successfully");
}

#[test]
fn dav_client_update_calendar_item_reports_a_failed_get() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    register_caldav_stubs(&mock);
    let client = caldav_client_at(mock.uri());

    let err = client
        .update_calendar_item("/notfound", r#"{"summary":"X"}"#)
        .expect_err("the GET must fail first");
    assert!(
        err.contains("Failed to fetch event for update: 404"),
        "got: {err}"
    );
}

#[test]
fn dav_client_delete_calendar_item_succeeds_on_2xx() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    register_caldav_stubs(&mock);
    let client = caldav_client_at(mock.uri());

    assert_eq!(client.delete_calendar_item("/item1.ics"), Ok(()));
}

// ---------------------------------------------------------------------------
// CardDAV protocol methods on DavClient
// ---------------------------------------------------------------------------

const ADDRESSBOOK_PROPFIND_BODY: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:carddav">
 <d:response>
  <d:href>/addressbooks/primary/</d:href>
  <d:propstat><d:prop>
   <d:resourcetype><d:collection/><c:addressbook/></d:resourcetype>
  </d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
 </d:response>
</d:multistatus>"#;

const ADDRESSBOOK_REPORT_BODY: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:carddav">
 <d:response>
  <d:href>/addressbooks/primary/alice.vcf</d:href>
  <d:propstat><d:prop>
   <c:address-data>BEGIN:VCARD
VERSION:3.0
FN:Alice Example
UID:alice-1
END:VCARD</c:address-data>
  </d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
 </d:response>
 <d:response>
  <d:href>/addressbooks/primary/no-data.vcf</d:href>
  <d:propstat><d:prop/><d:status>HTTP/1.1 404 Not Found</d:status></d:propstat>
 </d:response>
</d:multistatus>"#;

fn addressbook_home_set_body(href: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:carddav">
 <d:response><d:propstat><d:prop>
  <c:addressbook-home-set><d:href>{href}</d:href></c:addressbook-home-set>
 </d:prop></d:propstat></d:response>
</d:multistatus>"#
    )
}

#[test]
fn dav_client_list_addressbooks_returns_hrefs() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    propfind_at(&mock, "/", ADDRESSBOOK_PROPFIND_BODY);
    let client = caldav_client_at(mock.uri());

    let books = crate::tools::blocking::block_on(client.list_addressbooks("/")).unwrap();
    assert_eq!(books, vec!["/addressbooks/primary/".to_string()]);
}

#[test]
fn dav_client_list_addressbooks_reports_a_non_2xx_status() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    register_status(&mock, "PROPFIND", 403, "nope");
    let client = caldav_client_at(mock.uri());

    let err = crate::tools::blocking::block_on(client.list_addressbooks("/"))
        .expect_err("403 must be an error");
    assert_eq!(
        err.to_string(),
        "PROPFIND addressbooks failed: 403 Forbidden"
    );
}

#[test]
fn dav_client_discover_addressbook_home_set_returns_hrefs() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    propfind_at(
        &mock,
        "/",
        &addressbook_home_set_body("/addressbooks/user/"),
    );
    let client = caldav_client_at(mock.uri());

    let homes =
        crate::tools::blocking::block_on(client.discover_addressbook_home_set("/")).unwrap();
    assert_eq!(homes, vec!["/addressbooks/user/".to_string()]);
}

#[test]
fn dav_client_discover_addressbook_home_set_reports_a_non_2xx_status() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    register_status(&mock, "PROPFIND", 401, "nope");
    let client = caldav_client_at(mock.uri());

    let err = crate::tools::blocking::block_on(client.discover_addressbook_home_set("/"))
        .expect_err("401 must be an error");
    assert_eq!(
        err.to_string(),
        "PROPFIND addressbook-home-set failed: 401 Unauthorized"
    );
}

#[test]
fn dav_client_fetch_contacts_from_book_uses_the_sync_report() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    mock.register(Mock::given(method("REPORT")).respond_with(xml_207(ADDRESSBOOK_REPORT_BODY)));
    let client = caldav_client_at(mock.uri());

    let contacts =
        crate::tools::blocking::block_on(client.fetch_contacts_from_book("/addressbooks/primary/"))
            .unwrap();
    // The second response has no address-data and must be dropped.
    assert_eq!(contacts.len(), 1, "got: {contacts:?}");
    assert_eq!(contacts[0].0, "/addressbooks/primary/alice.vcf");
    assert!(contacts[0].1.contains("FN:Alice Example"));
}

#[test]
fn dav_client_fetch_contacts_from_book_falls_back_to_addressbook_query() {
    use wiremock::matchers::body_string_contains;

    install_rustls_provider();
    let mock = WiremockGuard::start();
    // The sync-collection REPORT is rejected; the plain
    // addressbook-query REPORT is the fallback and must succeed.
    mock.register(
        Mock::given(method("REPORT"))
            .and(body_string_contains("sync-collection"))
            .respond_with(
                ResponseTemplate::new(403)
                    .insert_header("content-type", "text/plain")
                    .set_body_string("sync unsupported"),
            ),
    );
    mock.register(
        Mock::given(method("REPORT"))
            .and(body_string_contains("addressbook-query"))
            .respond_with(xml_207(ADDRESSBOOK_REPORT_BODY)),
    );
    let client = caldav_client_at(mock.uri());

    let contacts =
        crate::tools::blocking::block_on(client.fetch_contacts_from_book("/addressbooks/primary/"))
            .unwrap();
    assert_eq!(contacts.len(), 1, "got: {contacts:?}");
    assert_eq!(contacts[0].0, "/addressbooks/primary/alice.vcf");
}

#[test]
fn dav_client_fetch_contacts_from_book_errors_when_both_reports_fail() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    register_status(&mock, "REPORT", 403, "nope");
    let client = caldav_client_at(mock.uri());

    let err =
        crate::tools::blocking::block_on(client.fetch_contacts_from_book("/addressbooks/primary/"))
            .expect_err("both REPORTs fail");
    assert_eq!(
        err.to_string(),
        "REPORT addressbook-query failed: 403 Forbidden"
    );
}

#[test]
fn dav_client_search_contact_matches_case_insensitively() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    propfind_at(&mock, "/", ADDRESSBOOK_PROPFIND_BODY);
    mock.register(Mock::given(method("REPORT")).respond_with(xml_207(ADDRESSBOOK_REPORT_BODY)));
    let client = caldav_client_at(mock.uri());

    let hits = client.search_contact("ALICE EXAMPLE").unwrap();
    assert_eq!(hits.len(), 1, "got: {hits:?}");
    assert_eq!(hits[0].fn_name.as_deref(), Some("Alice Example"));
    assert_eq!(hits[0].client, "primary");

    let misses = client.search_contact("nobody").unwrap();
    assert!(misses.is_empty(), "got: {misses:?}");
}

#[test]
fn dav_client_add_contact_returns_the_created_path() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    propfind_at(&mock, "/", ADDRESSBOOK_PROPFIND_BODY);
    mock.register(
        Mock::given(method("PUT")).respond_with(
            ResponseTemplate::new(201)
                .insert_header("ETag", "\"new\"")
                .set_body_string(""),
        ),
    );
    let client = caldav_client_at(mock.uri());

    let out = client
        .add_contact(r#"{"name":"Bob","email":"bob@example.com"}"#)
        .expect("add_contact should succeed");
    assert!(
        out.starts_with("Created at /addressbooks/primary/"),
        "got: {out}"
    );
    assert!(out.ends_with(".vcf"), "got: {out}");
}

#[test]
fn dav_client_update_contact_without_an_etag_puts_unconditionally() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    mock.register(
        Mock::given(method("GET")).respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/vcard")
                .set_body_string(CARD_ALICE_VCF),
        ),
    );
    mock.register(
        Mock::given(method("PUT")).respond_with(ResponseTemplate::new(200).set_body_string("")),
    );
    let client = caldav_client_at(mock.uri());

    let out = client
        .update_contact("/alice.vcf", r#"{"email":"new@example.com"}"#)
        .expect("the unconditional PUT should succeed");
    assert_eq!(out, "Updated (status 200 OK)");
}

#[test]
fn dav_client_delete_contact_reports_the_status_on_success() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    mock.register(
        Mock::given(method("DELETE")).respond_with(ResponseTemplate::new(200).set_body_string("")),
    );
    let client = caldav_client_at(mock.uri());

    let out = client.delete_contact("/alice.vcf").unwrap();
    assert_eq!(out, "Deleted (status 200 OK)");
}

#[test]
fn dav_client_delete_contact_reports_the_body_on_failure() {
    install_rustls_provider();
    let mock = WiremockGuard::start();
    mock.register(
        Mock::given(method("DELETE")).respond_with(
            ResponseTemplate::new(403)
                .insert_header("content-type", "text/plain")
                .set_body_string("forbidden body"),
        ),
    );
    let client = caldav_client_at(mock.uri());

    let err = client.delete_contact("/alice.vcf").unwrap_err();
    assert_eq!(
        err,
        "Failed to DELETE contact: 403 Forbidden - forbidden body"
    );
}
