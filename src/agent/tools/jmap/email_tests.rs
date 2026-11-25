use crate::config::AgentConfig;
// Tests for `jmap/email.rs`.

use serde_json::json;

use super::{convert_html_in_jmap, simplify_jmap_emails};

/// Wrap a single `Email/get` list entry in a JMAP method-response envelope.
fn email_get_response(email: serde_json::Value) -> serde_json::Value {
    json!({
        "methodResponses": [[
            "Email/get",
            {
                "list": [email]
            },
            "0"
        ]]
    })
}

/// The HTML converter only touches a body value that looks like markup, so a
/// plain-text value must survive the pass byte-for-byte.
#[test]
fn test_convert_html_plain_text_body_values_are_left_unchanged() {
    let cases: Vec<(&str, &str)> = vec![
        ("punctuation but no markup", "Hello, world!"),
        ("no angle brackets at all", "Hello World"),
    ];
    for (label, value) in cases {
        let _cache = crate::tools::registry::cache::ToolCache::new();
        let res = email_get_response(json!({
            "id": "1",
            "bodyValues": { "part1": { "value": value, "isTruncated": false } }
        }));
        let result = convert_html_in_jmap(res);
        let actual = result["methodResponses"][0][1]["list"][0]["bodyValues"]["part1"]["value"]
            .as_str()
            .unwrap();
        assert_eq!(
            actual, value,
            "case '{label}': plain text must be untouched"
        );
    }
}

#[test]
fn test_convert_html_converts_simple_html() {
    let _cache = crate::tools::registry::cache::ToolCache::new();
    let res = json!({
        "methodResponses": [[
            "Email/get",
            {
                "list": [{
                    "id": "1",
                    "bodyValues": {
                        "part1": { "value": "<p>Hello</p>", "isTruncated": false }
                    }
                }]
            },
            "0"
        ]]
    });
    let result = convert_html_in_jmap(res);
    let val = result["methodResponses"][0][1]["list"][0]["bodyValues"]["part1"]["value"]
        .as_str()
        .unwrap();
    assert!(val.starts_with("Hello"));
    assert!(!val.contains('<'));
}

#[test]
fn test_convert_html_multiple_body_parts() {
    let _cache = crate::tools::registry::cache::ToolCache::new();
    let res = json!({
        "methodResponses": [[
            "Email/get",
            {
                "list": [{
                    "id": "1",
                    "bodyValues": {
                        "part1": { "value": "<h1>Title</h1>", "isTruncated": false },
                        "part2": { "value": "Plain text", "isTruncated": false },
                        "part3": { "value": "<p>Para</p>", "isTruncated": false }
                    }
                }]
            },
            "0"
        ]]
    });
    let result = convert_html_in_jmap(res);
    let bv = &result["methodResponses"][0][1]["list"][0]["bodyValues"];
    assert!(bv["part1"]["value"].as_str().unwrap().contains("Title"));
    assert_eq!(bv["part2"]["value"].as_str().unwrap(), "Plain text");
    assert!(bv["part3"]["value"].as_str().unwrap().starts_with("Para"));
}

#[test]
fn test_convert_html_no_body_values() {
    let _cache = crate::tools::registry::cache::ToolCache::new();
    let res = json!({
        "methodResponses": [[
            "Email/get",
            {
                "list": [{ "id": "1", "subject": "test" }]
            },
            "0"
        ]]
    });
    let result = convert_html_in_jmap(res);
    assert!(
        result["methodResponses"][0][1]["list"][0]["subject"]
            .as_str()
            .is_some()
    );
}

/// A body value the converter cannot work with — no parts at all, or a
/// non-string `value` — must leave the response untouched instead of panicking.
#[test]
fn test_convert_html_unusable_body_values_leave_the_response_untouched() {
    let cases: Vec<(&str, serde_json::Value)> = vec![
        ("empty bodyValues", json!({ "id": "1", "bodyValues": {} })),
        (
            "non-string body value",
            json!({
                "id": "1",
                "bodyValues": { "part1": { "value": 42, "isTruncated": false } }
            }),
        ),
    ];
    for (label, email) in cases {
        let _cache = crate::tools::registry::cache::ToolCache::new();
        let res = email_get_response(email);
        let result = convert_html_in_jmap(res.clone());
        assert_eq!(
            result, res,
            "case '{label}': response must be returned unchanged"
        );
    }
}

/// `simplify_jmap_emails` yields an empty array whenever the response carries no
/// `Email/get` hits — whether the array is empty, another method answered, or
/// the `Email/get` list itself is empty.
#[test]
fn test_simplify_jmap_emails_returns_an_empty_array_without_email_hits() {
    let cases: Vec<(&str, serde_json::Value)> = vec![
        ("no method responses", json!({ "methodResponses": [] })),
        (
            "no Email/get method",
            json!({
                "methodResponses": [[
                    "Contact/query", { "ids": [] }, "0"
                ]]
            }),
        ),
        (
            "Email/get with an empty list",
            json!({
                "methodResponses": [[
                    "Email/get",
                    { "accountId": "a1", "list": [], "notFound": [] },
                    "0"
                ]]
            }),
        ),
    ];
    for (label, res) in cases {
        let _cache = crate::tools::registry::cache::ToolCache::new();
        let result = simplify_jmap_emails(res, None);
        assert_eq!(result, json!([]), "case '{label}'");
    }
}

/// A single `Email/get` hit becomes exactly one simplified item, and its `body`
/// is read from the referenced HTML part or, when no HTML part is referenced,
/// from the text part.
#[test]
fn test_simplify_single_email_takes_the_body_from_the_referenced_part() {
    for (label, email, expected_id, expected_subject, expected_body) in [
        (
            "html part",
            json!({
                "id": "email-1",
                "subject": "Hello",
                "receivedAt": "2026-07-19T10:00:00Z",
                "from": [{ "name": "Alice", "email": "alice@test.com" }],
                "to": [{ "name": "Bob", "email": "bob@test.com" }],
                "cc": [],
                "bcc": [],
                "htmlBody": [{ "partId": "p1" }],
                "bodyValues": {
                    "p1": { "value": "Hello Bob!", "isTruncated": false }
                }
            }),
            "email-1",
            "Hello",
            "Hello Bob!",
        ),
        (
            "text part fallback",
            json!({
                "id": "email-2",
                "subject": "No HTML",
                "receivedAt": "2026-07-19T11:00:00Z",
                "from": [{ "name": "Charlie", "email": "charlie@test.com" }],
                "to": [{ "name": "Dave", "email": "dave@test.com" }],
                "textBody": [{ "partId": "tp1" }],
                "bodyValues": {
                    "tp1": { "value": "Plain text body", "isTruncated": false }
                }
            }),
            "email-2",
            "No HTML",
            "Plain text body",
        ),
    ] {
        let _cache = crate::tools::registry::cache::ToolCache::new();
        let res = json!({
            "methodResponses": [[
                "Email/get",
                { "accountId": "a1", "list": [email], "notFound": [] },
                "0"
            ]]
        });
        let result = simplify_jmap_emails(res, None);
        let arr = result.as_array().expect("expected a JSON array");
        assert_eq!(arr.len(), 1, "case '{label}': expected one email");
        assert_eq!(arr[0]["id"], expected_id, "case '{label}': unexpected id");
        assert_eq!(
            arr[0]["subject"], expected_subject,
            "case '{label}': unexpected subject"
        );
        assert_eq!(
            arr[0]["body"], expected_body,
            "case '{label}': unexpected body"
        );
    }
}

#[test]
fn test_simplify_multiple_emails() {
    let _cache = crate::tools::registry::cache::ToolCache::new();
    let res = json!({
        "methodResponses": [[
            "Email/get",
            {
                "accountId": "a1",
                "list": [
                    {
                        "id": "e1",
                        "subject": "First",
                        "receivedAt": "2026-07-19T10:00:00Z",
                        "from": [{"email": "a@t.com"}],
                        "to": [{"email": "b@t.com"}],
                        "htmlBody": [{"partId": "p1"}],
                        "bodyValues": { "p1": { "value": "Body 1", "isTruncated": false } }
                    },
                    {
                        "id": "e2",
                        "subject": "Second",
                        "receivedAt": "2026-07-19T11:00:00Z",
                        "from": [{"email": "c@t.com"}],
                        "to": [{"email": "d@t.com"}],
                        "htmlBody": [{"partId": "p2"}],
                        "bodyValues": { "p2": { "value": "Body 2", "isTruncated": false } }
                    }
                ],
                "notFound": []
            },
            "0"
        ]]
    });
    let result = simplify_jmap_emails(res, None);
    let arr = result.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["id"], "e1");
    assert_eq!(arr[1]["id"], "e2");
}

/// `simplify_jmap_emails` appends the truncation hint exactly when the body is
/// truncated: when `max_lines` cuts the value below its line count, or when the
/// server already marked the body value as truncated. A body that fits keeps its
/// value untouched.
#[test]
fn test_simplify_jmap_emails_appends_the_hint_only_for_cut_bodies() {
    const HINT: &str = "\n... (truncated - use the get_email_by_id tool with the email id to read the full content)";
    /// One truncation case: `(label, body, is_truncated_flag, line_limit,
    /// expected rendered body, whether the hint must appear)`.
    type TruncationCase<'a> = (&'a str, &'a str, bool, Option<usize>, String, bool);
    let cases: Vec<TruncationCase<'_>> = vec![
        (
            "five lines cut to three",
            "Line 1\nLine 2\nLine 3\nLine 4\nLine 5",
            false,
            Some(3),
            format!("Line 1\nLine 2\nLine 3{HINT}"),
            true,
        ),
        (
            "four lines cut to two",
            "Line 1\nLine 2\nLine 3\nLine 4",
            false,
            Some(2),
            format!("Line 1\nLine 2{HINT}"),
            true,
        ),
        (
            "server-truncated body value",
            "Partial body here...",
            true,
            None,
            format!("Partial body here...{HINT}"),
            true,
        ),
        (
            "body below the limit",
            "Just one line",
            false,
            Some(10),
            "Just one line".to_string(),
            false,
        ),
    ];
    for (label, value, is_truncated, max_lines, expected_body, hint_expected) in cases {
        let _cache = crate::tools::registry::cache::ToolCache::new();
        let res = json!({
            "methodResponses": [[
                "Email/get",
                {
                    "accountId": "a1",
                    "list": [{
                        "id": "e1",
                        "subject": "Body truncation",
                        "receivedAt": "2026-07-19T10:00:00Z",
                        "from": [{"email": "a@t.com"}],
                        "to": [{"email": "b@t.com"}],
                        "htmlBody": [{"partId": "p1"}],
                        "bodyValues": { "p1": { "value": value, "isTruncated": is_truncated } }
                    }],
                    "notFound": []
                },
                "0"
            ]]
        });
        let result = simplify_jmap_emails(res, max_lines);
        let body = result[0]["body"]
            .as_str()
            .expect("the body must stay a string");
        assert_eq!(body, expected_body, "case '{label}': unexpected body");
        assert_eq!(
            body.contains("truncated"),
            hint_expected,
            "case '{label}': truncation hint expected: {hint_expected}"
        );
    }
}

#[test]
fn test_simplify_cc_and_bcc_preserved() {
    let _cache = crate::tools::registry::cache::ToolCache::new();
    let res = json!({
        "methodResponses": [[
            "Email/get",
            {
                "accountId": "a1",
                "list": [{
                    "id": "e1",
                    "subject": "CC test",
                    "receivedAt": "2026-07-19T10:00:00Z",
                    "from": [{"email": "a@t.com"}],
                    "to": [{"email": "b@t.com"}],
                    "cc": [{"email": "cc@t.com"}],
                    "bcc": [{"email": "bcc@t.com"}],
                    "htmlBody": [{"partId": "p1"}],
                    "bodyValues": { "p1": { "value": "Body", "isTruncated": false } }
                }],
                "notFound": []
            },
            "0"
        ]]
    });
    let result = simplify_jmap_emails(res, None);
    assert_eq!(result[0]["cc"][0]["email"], "cc@t.com");
    assert_eq!(result[0]["bcc"][0]["email"], "bcc@t.com");
}

use super::{
    SearchEmailFilters, simplify_email_for_search, tool_get_email_by_id, tool_search_email,
    tool_send_email,
};
use crate::config::JmapClient;
use crate::tools::jmap::mock_server::{spawn_mock_server, spawn_recording_mock_server};
#[test]
fn test_tool_search_email_no_clients() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    let config = AgentConfig::default();
    let res = tool_search_email(
        &config,
        SearchEmailFilters {
            keyword: Some("test"),
            ..Default::default()
        },
        None,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
    );
    assert!(res.is_err());
}

#[test]
fn test_tool_search_email_uses_cursor_and_ignores_filters() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    let uuid = crate::utils::uuid::SystemUuidGenerator;
    let items: Vec<crate::tools::registry::cache::SearchEmailItem> = (0..35)
        .map(|i| crate::tools::registry::cache::SearchEmailItem {
            client: "test".to_string(),
            preview: serde_json::json!({ "id": format!("e{i}") }),
        })
        .collect();
    let config = AgentConfig::default();
    for filters in [
        SearchEmailFilters {
            keyword: Some("test"),
            ..Default::default()
        },
        SearchEmailFilters {
            folder: Some("INBOX"),
            ..Default::default()
        },
        SearchEmailFilters {
            is_unread: Some(true),
            ..Default::default()
        },
    ] {
        let first_page = cache.email_sessions.create_session(items.clone(), &uuid);
        let cursor = first_page.cursor.expect("session should have cursor");
        let res = tool_search_email(&config, filters, Some(cursor), &cache, &uuid)
            .expect("tool_search_email with cursor should succeed and ignore filters");
        assert_eq!(res.count, 3);
        assert_eq!(res.total, 35);
    }
}

/// A first-call search succeeds both for a plain keyword filter and for the
/// date / address / status filter combination.
#[test]
fn test_tool_search_email_succeeds_for_populated_filters() {
    let keyword_body = "{\
            \"apiUrl\": \"{API_URL}\",\
            \"primaryAccounts\": {\"urn:ietf:params:jmap:mail\": \"acc1\"},\
            \"methodResponses\": [\
                [\"Email/query\", {\"ids\": [\"e1\"]}, \"0\"],\
                [\"Email/get\", {\"list\": [{\"id\": \"e1\", \"subject\": \"Test\"}]}, \"1\"]\
            ]\
        }";
    let status_body = "{\
            \"apiUrl\": \"{API_URL}\",\
            \"primaryAccounts\": {\"urn:ietf:params:jmap:mail\": \"acc1\"},\
            \"methodResponses\": [\
                [\"Email/query\", {\"ids\": [\"e1\"]}, \"0\"],\
                [\"Email/get\", {\"list\": [{\"id\": \"e1\", \"subject\": \"Unread\"}]}, \"1\"]\
            ]\
        }";
    let cases: Vec<(&str, SearchEmailFilters<'static>, &str)> = vec![
        (
            "keyword filter",
            SearchEmailFilters {
                keyword: Some("test"),
                ..Default::default()
            },
            keyword_body,
        ),
        (
            "date, address and status filters",
            SearchEmailFilters {
                keyword: None,
                folder: None,
                start_date: Some("2026-07-01"),
                end_date: Some("2026-07-10"),
                from: Some("s@test.com"),
                to: Some("r@test.com"),
                is_unread: Some(true),
                is_flagged: Some(false),
            },
            status_body,
        ),
    ];
    for (label, filters, body) in cases {
        let cache = crate::tools::registry::cache::ToolCache::new();
        ensure_rustls_provider();
        let url = spawn_mock_server(body);
        let config = jmap_config(&url);
        let res = tool_search_email(
            &config,
            filters,
            None,
            &cache,
            &crate::utils::uuid::SystemUuidGenerator,
        );
        assert!(res.is_ok(), "case '{label}': got: {:?}", res.err());
    }
}

#[test]
fn test_tool_get_email_by_id_success() {
    let _cache = crate::tools::registry::cache::ToolCache::new();
    ensure_rustls_provider();
    let body = "{\
            \"apiUrl\": \"{API_URL}\",\
            \"primaryAccounts\": {\"urn:ietf:params:jmap:mail\": \"acc1\"},\
            \"methodResponses\": [\
                [\"Email/get\", {\"list\": [{\"id\": \"e1\", \"subject\": \"Test\"}]}, \"0\"]\
            ]\
        }";
    let url = spawn_mock_server(body);
    let config = jmap_config(&url);
    let res = tool_get_email_by_id(&config, "e1");
    assert!(res.is_ok(), "Error: {}", res.unwrap_err());
}

/// Regression: `Email/get` MUST be sent with the body-fetching arguments
/// (`fetchTextBodyValues`, `fetchHTMLBodyValues`, `maxBodyValueBytes`)
/// per RFC 8621 §6.1.2, otherwise a real JMAP server returns empty
/// `bodyValues` (default `maxBodyValueBytes` is 0).
#[test]
fn test_tool_get_email_by_id_sends_body_value_args() {
    let _cache = crate::tools::registry::cache::ToolCache::new();
    ensure_rustls_provider();
    let body = "{\
            \"apiUrl\": \"{API_URL}\",\
            \"primaryAccounts\": {\"urn:ietf:params:jmap:mail\": \"acc1\"},\
            \"methodResponses\": [\
                [\"Email/get\", {\"list\": [{\"id\": \"e1\", \"subject\": \"Test\"}]}, \"0\"]\
            ]\
        }";
    let (url, recorder) = spawn_recording_mock_server(body);
    let config = jmap_config(&url);
    let res = tool_get_email_by_id(&config, "e1");
    assert!(res.is_ok(), "Error: {}", res.unwrap_err());

    let recorded = recorder.lock().expect("mock recorder poisoned");
    let last_post = recorded
        .last()
        .expect("no POST was recorded by the mock server");
    let last_post_str = std::str::from_utf8(last_post).expect("mock recorded non-UTF8 POST body");
    assert!(
        last_post_str.contains("maxBodyValueBytes"),
        "Email/get request must include maxBodyValueBytes so the server \
             returns body content; got: {last_post_str}"
    );
    assert!(
        last_post_str.contains("\"fetchTextBodyValues\":true"),
        "Email/get request must opt in to text body values; got: {last_post_str}"
    );
    assert!(
        last_post_str.contains("\"fetchHTMLBodyValues\":true"),
        "Email/get request must opt in to HTML body values; got: {last_post_str}"
    );
}

/// Defence-in-depth: when the server returns `bodyValues`, `tool_get_email_by_id`
/// must surface the body content (not a silently empty string).
#[test]
fn test_tool_get_email_by_id_returns_body_content() {
    let _cache = crate::tools::registry::cache::ToolCache::new();
    ensure_rustls_provider();
    let body = "{\
            \"apiUrl\": \"{API_URL}\",\
            \"primaryAccounts\": {\"urn:ietf:params:jmap:mail\": \"acc1\"},\
            \"methodResponses\": [\
                [\"Email/get\", {\
                    \"list\": [{\
                        \"id\": \"e1\",\
                        \"subject\": \"Hi\",\
                        \"htmlBody\": [{\"partId\": \"p1\"}],\
                        \"bodyValues\": {\"p1\": {\"value\": \"Hello, world!\", \"isTruncated\": false}}\
                    }]\
                }, \"0\"]\
            ]\
        }";
    let url = spawn_mock_server(body);
    let config = jmap_config(&url);
    let res = tool_get_email_by_id(&config, "e1").expect("tool call should succeed");
    assert!(
        res.result.contains("Hello, world!"),
        "tool_get_email_by_id should surface body content; got: {}",
        res.result
    );
}

#[test]
fn test_tool_get_email_by_id_returns_yaml_frontmatter_and_markdown_body() {
    let _cache = crate::tools::registry::cache::ToolCache::new();
    ensure_rustls_provider();
    let body = "{\
            \"apiUrl\": \"{API_URL}\",\
            \"primaryAccounts\": {\"urn:ietf:params:jmap:mail\": \"acc1\"},\
            \"methodResponses\": [\
                [\"Email/get\", {\
                    \"list\": [{\
                        \"id\": \"e1\",\
                        \"subject\": \"Order #42\",\
                        \"receivedAt\": \"2026-07-19T10:00:00Z\",\
                        \"from\": [{\"name\": \"Store\", \"email\": \"orders@store.com\"}],\
                        \"to\": [{\"name\": \"Alice\", \"email\": \"alice@example.com\"}],\
                        \"htmlBody\": [{\"partId\": \"p1\"}],\
                        \"bodyValues\": {\"p1\": {\"value\": \"Thank you for your order!\\nYour item has shipped.\", \"isTruncated\": false}}\
                    }]\
                }, \"0\"]\
            ]\
        }";
    let url = spawn_mock_server(body);
    let config = jmap_config(&url);
    let res = tool_get_email_by_id(&config, "e1").expect("tool call should succeed");

    assert!(
        res.result.starts_with("---\n"),
        "result must start with YAML front matter delimiter, got: {}",
        res.result
    );
    assert!(res.result.contains("id: e1"));
    assert!(res.result.contains("Order #42"));
    assert!(res.result.contains("orders@store.com"));
    assert!(res.result.contains("alice@example.com"));
    assert!(
        res.result
            .contains("---\n\nThank you for your order!\nYour item has shipped."),
        "result must contain markdown body separated by closing YAML delimiter and blank line; got: {}",
        res.result
    );
    assert!(
        !res.result.contains("preview:"),
        "preview field must be omitted to save tokens"
    );
}

#[test]
fn test_tool_send_email_success() {
    let _cache = crate::tools::registry::cache::ToolCache::new();
    ensure_rustls_provider();
    let body = "{\
            \"apiUrl\": \"{API_URL}\",\
            \"primaryAccounts\": {\"urn:ietf:params:jmap:mail\": \"acc1\", \"urn:ietf:params:jmap:submission\": \"acc1\"},\
            \"methodResponses\": [\
                [\"Mailbox/query\", {\"ids\": [\"drafts-id\"]}, \"0\"],\
                [\"Identity/get\", {\"state\": \"id-state-0\", \"list\": [{\"id\": \"ident-1\", \"email\": \"sender@test.com\"}], \"notFound\": []}, \"1\"],\
                [\"Email/set\", {\"created\": {\"c0\": {\"id\": \"email-1\"}}}, \"2\"],\
                [\"EmailSubmission/set\", {\"created\": {\"c0\": {\"id\": \"sub-1\"}}}, \"3\"]\
            ]\
        }";
    let url = spawn_mock_server(body);
    let config = jmap_config(&url);
    let res = tool_send_email(&config, "to@test.com", "Subject", "Body");
    assert!(res.is_ok(), "Error: {}", res.unwrap_err());
}

#[test]
fn test_tool_send_email_uses_drafts_mailbox() {
    let _cache = crate::tools::registry::cache::ToolCache::new();
    ensure_rustls_provider();
    let body = "{\
            \"apiUrl\": \"{API_URL}\",\
            \"primaryAccounts\": {\"urn:ietf:params:jmap:mail\": \"acc1\", \"urn:ietf:params:jmap:submission\": \"acc1\"},\
            \"methodResponses\": [\
                [\"Mailbox/query\", {\"filter\": {\"role\": \"drafts\"}, \"ids\": [\"drafts-id\"]}, \"0\"],\
                [\"Identity/get\", {\"state\": \"id-state-0\", \"list\": [{\"id\": \"ident-1\", \"email\": \"sender@test.com\"}], \"notFound\": []}, \"1\"],\
                [\"Email/set\", {\"created\": {\"c0\": {\"id\": \"email-1\"}}}, \"2\"],\
                [\"EmailSubmission/set\", {\"created\": {\"c0\": {\"id\": \"sub-1\"}}}, \"3\"]\
            ]\
        }";
    let url = spawn_mock_server(body);
    let config = jmap_config(&url);
    let res = tool_send_email(&config, "to@test.com", "Subject", "Body");
    assert!(res.is_ok(), "Error: {}", res.unwrap_err());
}

#[test]
fn test_tool_send_email_ai_agent_footer() {
    let _cache = crate::tools::registry::cache::ToolCache::new();
    use super::AI_AGENT_FOOTER;
    assert_eq!(
        AI_AGENT_FOOTER,
        "\n---\nSent by FastMD on behalf of the user"
    );
}

/// An empty filter set is rejected before any client is contacted — whether no
/// JMAP client is configured or one is.
#[test]
fn test_tool_search_email_rejects_an_empty_filter_set() {
    ensure_rustls_provider();
    let session_only_body = "{\"apiUrl\": \"{API_URL}\", \"primaryAccounts\": {\"urn:ietf:params:jmap:mail\": \"acc1\"}}";
    let without_clients = AgentConfig::default();
    let with_client = jmap_config(&spawn_mock_server(session_only_body));

    let cases: Vec<(&str, &AgentConfig)> = vec![
        ("no clients configured", &without_clients),
        ("one client configured", &with_client),
    ];
    for (label, config) in cases {
        let cache = crate::tools::registry::cache::ToolCache::new();
        let res = tool_search_email(
            config,
            SearchEmailFilters {
                ..Default::default()
            },
            None,
            &cache,
            &crate::utils::uuid::SystemUuidGenerator,
        );
        assert!(
            res.is_err(),
            "case '{label}': an empty filter set must fail"
        );
        let msg = res.unwrap_err();
        assert!(
            msg.contains("At least one filter field must be provided"),
            "case '{label}': unexpected error: {msg}"
        );
    }
}

/// A result set that fits entirely on one page (the page size is 100) omits
/// `cursor` and reports the `Final page.` hint, whether it holds one email or
/// three. There is no separate "page-beyond-total" case because the cursor model
/// makes that scenario impossible: the LLM only ever holds a valid cursor or no
/// cursor at all.
#[test]
fn test_tool_search_email_single_page_result_reports_the_final_page_hint() {
    for (label, count) in [("one email", 1usize), ("three emails", 3usize)] {
        ensure_rustls_provider();
        let ids: Vec<String> = (1..=count).map(|i| format!("e{i}")).collect();
        let list: Vec<serde_json::Value> = (1..=count)
            .map(|i| {
                json!({
                    "id": format!("e{i}"),
                    "subject": format!("Subject {i}"),
                    "receivedAt": "2026-07-19T10:00:00Z",
                    "from": [{"email": "a@t.com"}],
                    "to": [{"email": "b@t.com"}],
                    "htmlBody": [{"partId": "p1"}],
                    "bodyValues": {
                        "p1": { "value": format!("Body {i}"), "isTruncated": false }
                    }
                })
            })
            .collect();
        let body = json!({
            "apiUrl": "{API_URL}",
            "primaryAccounts": {"urn:ietf:params:jmap:mail": "acc1"},
            "methodResponses": [
                ["Email/query", { "ids": ids }, "0"],
                ["Email/get", { "list": list, "notFound": [] }, "1"]
            ]
        })
        .to_string();
        let url = spawn_mock_server(body);
        let config = jmap_config(&url);
        let res = search_first_call(
            &config,
            SearchEmailFilters {
                keyword: Some("test"),
                ..Default::default()
            },
        );
        assert!(res.is_ok(), "case '{label}': search must succeed");
        let response = res.unwrap();
        assert_eq!(response.total, count, "case '{label}': unexpected total");
        assert!(
            response.cursor.is_none(),
            "case '{label}': single-page result must omit cursor"
        );
        assert_eq!(
            response.hint.as_deref(),
            Some(super::SEARCH_EMAIL_FINAL_PAGE_HINT),
            "case '{label}': expected the final-page hint"
        );
        assert!(
            !response.results.is_empty(),
            "case '{label}': expected at least one result"
        );
        assert!(
            response.errors.is_empty(),
            "case '{label}': successful single-client search must not report errors"
        );
    }
}

#[test]
fn test_tool_search_email_cursor_unknown_returns_error() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    ensure_rustls_provider();
    let body = r#"{
                "apiUrl": "{API_URL}",
                "primaryAccounts": {"urn:ietf:params:jmap:mail": "acc1"},
                "methodResponses": [
                    ["Email/query", {"ids": ["e1"]}, "0"],
                    ["Email/get", {
                        "list": [
                            {"id": "e1", "subject": "Only", "receivedAt": "2026-07-19T10:00:00Z", "from": [{"email":"a@t.com"}], "to": [{"email":"b@t.com"}], "htmlBody": [{"partId":"p1"}], "bodyValues": {"p1": {"value":"Body","isTruncated":false}}}
                        ],
                        "notFound": []
                    }, "1"]
                ]
            }"#
        .to_string();
    let url = spawn_mock_server(body);
    let config = jmap_config(&url);
    // A bogus cursor that does not match any live cache entry
    // must return the documented "expired or unknown" error.
    // The call sends only the cursor: sending filters together
    // with a cursor is rejected by the either-or rule.
    let res = tool_search_email(
        &config,
        SearchEmailFilters::default(),
        Some("00000000-0000-0000-0000-000000000000".to_string()),
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
    );
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.contains("Cursor expired or unknown"),
        "unexpected error: {err}"
    );
}

/// Regression test for the cursor round-trip (TOOL-029, TOOL-031).
///
/// The first call must return a cursor; the second call, passing the cursor
/// back, must serve the next page from the cache without re-querying JMAP.
/// Mirrors `test_tool_web_fetch_cursor_pagination` in `web_tests.rs` as a
/// guard against the same shape of bug that broke `web_fetch`.
#[test]
fn test_tool_search_email_cursor_pagination() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    ensure_rustls_provider();
    // Force pagination: total emails = page size + 5. Using a constant offset
    // (rather than a hard-coded count) keeps the test correct if the
    // page-size constant changes.
    let total_emails = super::SEARCH_EMAIL_PAGE_SIZE + 5;
    let mut ids: Vec<String> = Vec::with_capacity(total_emails);
    let mut email_objects: Vec<String> = Vec::with_capacity(total_emails);
    for i in 1..=total_emails {
        ids.push(format!("\"e{i}\""));
        email_objects.push(format!(
            r#"{{"id": "e{i}", "subject": "Subject {i}", "receivedAt": "2026-07-19T10:00:00Z", "from": [{{"email":"a@t.com"}}], "to": [{{"email":"b@t.com"}}], "htmlBody": [{{"partId":"p{i}"}}], "bodyValues": {{"p{i}": {{"value":"Body {i}","isTruncated":false}}}}}}"#,
            i = i
        ));
    }
    let body = format!(
        r#"{{
            "apiUrl": "{{API_URL}}",
            "primaryAccounts": {{"urn:ietf:params:jmap:mail": "acc1"}},
            "methodResponses": [
                ["Email/query", {{"ids": [{}]}}, "0"],
                ["Email/get", {{"list": [{}], "notFound": []}}, "1"]
            ]
        }}"#,
        ids.join(","),
        email_objects.join(",")
    );
    let url = spawn_mock_server(body);
    let config = jmap_config(&url);
    let filters = SearchEmailFilters {
        keyword: Some("test"),
        ..Default::default()
    };
    let uuid = crate::utils::uuid::SystemUuidGenerator;

    // First call: no cursor, must return first page + cursor.
    let first = tool_search_email(&config, filters.clone(), None, &cache, &uuid)
        .expect("first call must succeed");
    assert_eq!(first.total, total_emails);
    assert!(
        first.cursor.is_some(),
        "first call must return a cursor when result set exceeds page size"
    );
    assert!(first.hint.is_none());
    let cursor = first.cursor.clone().unwrap();
    // Results are now a structured Vec<SearchEmailItem>; count them directly.
    assert_eq!(
        first.results.len(),
        super::SEARCH_EMAIL_PAGE_SIZE,
        "first page must contain exactly SEARCH_EMAIL_PAGE_SIZE items"
    );
    let subject_of = |item: &crate::tools::cache::SearchEmailItem| -> String {
        item.preview
            .get("subject")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string()
    };
    let id_of = |item: &crate::tools::cache::SearchEmailItem| -> String {
        item.preview
            .get("id")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string()
    };
    assert!(first.results.iter().any(|i| subject_of(i) == "Subject 1"));
    assert!(
        !first
            .results
            .iter()
            .any(|i| subject_of(i) == format!("Subject {total_emails}"))
    );

    // Second call: with cursor only (no filters), must return remaining items + final hint.
    let second = tool_search_email(
        &config,
        SearchEmailFilters::default(),
        Some(cursor.clone()),
        &cache,
        &uuid,
    )
    .expect("second call with cursor must succeed");
    assert_eq!(second.total, total_emails);
    assert!(
        second.cursor.is_none(),
        "second call (final page) must omit cursor"
    );
    assert_eq!(
        second.hint.as_deref(),
        Some(super::SEARCH_EMAIL_FINAL_PAGE_HINT)
    );
    assert_eq!(
        second.results.len(),
        total_emails - super::SEARCH_EMAIL_PAGE_SIZE,
        "second page must contain the remaining items"
    );
    assert!(!second.results.iter().any(|i| subject_of(i) == "Subject 1"));
    assert!(
        second
            .results
            .iter()
            .any(|i| subject_of(i) == format!("Subject {total_emails}"))
    );

    // Pages must not repeat any email id. The first page must contain e1 and
    // the last page must not; the last page must contain e{total_emails} and
    // the first must not.
    assert!(first.results.iter().any(|i| id_of(i) == "e1"));
    assert!(!second.results.iter().any(|i| id_of(i) == "e1"));
    assert!(
        !first
            .results
            .iter()
            .any(|i| id_of(i) == format!("e{total_emails}"))
    );
    assert!(
        second
            .results
            .iter()
            .any(|i| id_of(i) == format!("e{total_emails}"))
    );
}

#[test]
fn test_tool_search_email_multiple_clients() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    ensure_rustls_provider();
    let body = r#"{
                "apiUrl": "{API_URL}",
                "primaryAccounts": {"urn:ietf:params:jmap:mail": "acc1"},
                "methodResponses": [
                    ["Email/query", {"ids": ["e1"]}, "0"],
                    ["Email/get", {
                        "list": [
                            {"id": "e1", "subject": "Multi", "receivedAt": "2026-07-19T10:00:00Z", "from": [{"email":"a@t.com"}], "to": [{"email":"b@t.com"}], "htmlBody": [{"partId":"p1"}], "bodyValues": {"p1": {"value":"Body","isTruncated":false}}}
                        ],
                        "notFound": []
                    }, "1"]
                ]
            }"#
        .to_string();
    let url = spawn_mock_server(body);
    let mut config = AgentConfig::default();
    config.jmap_clients.insert(
        "client1".to_string(),
        JmapClient {
            url: url.clone(),
            token: "tok1".to_string(),
        },
    );
    config.jmap_clients.insert(
        "client2".to_string(),
        JmapClient {
            url,
            token: "tok2".to_string(),
        },
    );
    let res = tool_search_email(
        &config,
        SearchEmailFilters {
            keyword: Some("test"),
            ..Default::default()
        },
        None,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
    );
    assert!(res.is_ok());
    let response = res.unwrap();
    assert_eq!(response.total, 2);
    assert!(response.results.iter().any(|i| i.client == "client1"));
    assert!(response.results.iter().any(|i| i.client == "client2"));
}

#[test]
fn test_tool_search_email_logs_tracing() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    ensure_rustls_provider();
    let body = r#"{
                "apiUrl": "{API_URL}",
                "primaryAccounts": {"urn:ietf:params:jmap:mail": "acc1"},
                "methodResponses": [
                    ["Email/query", {"ids": ["e1", "e2"]}, "0"],
                    ["Email/get", {
                        "list": [
                            {"id": "e1", "subject": "First"},
                            {"id": "e2", "subject": "Second"}
                        ],
                        "notFound": []
                    }, "1"]
                ]
            }"#
    .to_string();
    let url = spawn_mock_server(body);
    let mut config = AgentConfig::default();
    config.jmap_clients.insert(
        "fastmail".to_string(),
        JmapClient {
            url,
            token: "tok".to_string(),
        },
    );
    let res = tool_search_email(
        &config,
        SearchEmailFilters {
            keyword: Some("fastmail"),
            ..Default::default()
        },
        None,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
    );
    assert!(res.is_ok());
    let response = res.unwrap();
    assert_eq!(response.total, 2);
}

#[test]
fn test_tool_search_email_returns_preview_without_body() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    ensure_rustls_provider();
    let body = r#"{
        "apiUrl": "{API_URL}",
        "primaryAccounts": {"urn:ietf:params:jmap:mail": "acc1"},
        "methodResponses": [
            ["Email/query", {"ids": ["e1"]}, "0"],
            ["Email/get", {
                "list": [{
                    "id": "e1",
                    "subject": "Preview Test",
                    "preview": "Here is the short preview snippet without bloat"
                }],
                "notFound": []
            }, "1"]
        ]
    }"#;
    let url = spawn_mock_server(body);
    let config = jmap_config(&url);
    let res = tool_search_email(
        &config,
        SearchEmailFilters {
            keyword: Some("test"),
            ..Default::default()
        },
        None,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
    )
    .expect("tool_search_email should succeed");

    assert_eq!(res.results.len(), 1);
    let email_item = &res.results[0].preview;
    assert_eq!(
        email_item["preview"],
        "Here is the short preview snippet without bloat"
    );
    assert!(
        email_item.get("body").is_none(),
        "Search results must not contain body bloat"
    );
}

#[test]
fn test_tool_search_email_sends_lightweight_get_request() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    ensure_rustls_provider();
    let body = r#"{
        "apiUrl": "{API_URL}",
        "primaryAccounts": {"urn:ietf:params:jmap:mail": "acc1"},
        "methodResponses": [
            ["Email/query", {"ids": ["e1"]}, "0"],
            ["Email/get", {
                "list": [{
                    "id": "e1",
                    "subject": "Preview Test",
                    "preview": "Snippet"
                }],
                "notFound": []
            }, "1"]
        ]
    }"#;
    let (url, recorder) = spawn_recording_mock_server(body);
    let config = jmap_config(&url);
    let res = tool_search_email(
        &config,
        SearchEmailFilters {
            keyword: Some("test"),
            ..Default::default()
        },
        None,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
    );
    assert!(res.is_ok());

    let recorded = recorder.lock().expect("mock recorder poisoned");
    let get_call = recorded
        .iter()
        .find(|bytes| {
            let s = std::str::from_utf8(bytes).unwrap_or("");
            s.contains("Email/get")
        })
        .expect("Email/get request must be made");
    let get_call_str = std::str::from_utf8(get_call).unwrap();
    assert!(
        !get_call_str.contains("maxBodyValueBytes"),
        "search Email/get request must NOT request heavy body values; got: {get_call_str}"
    );
    assert!(
        !get_call_str.contains("\"fetchTextBodyValues\":true"),
        "search Email/get request must NOT opt into text body values; got: {get_call_str}"
    );
    assert!(
        !get_call_str.contains("\"fetchHTMLBodyValues\":true"),
        "search Email/get request must NOT opt into HTML body values; got: {get_call_str}"
    );
}

/// `simplify_email_for_search` fills `preview` from the native RFC 8621
/// `preview` property and falls back to the plain-text body when that property
/// is missing; it never emits a `body` field.
#[test]
fn test_simplify_email_for_search_fills_preview_from_the_property_or_the_body() {
    let cases = [
        (
            "native preview property",
            "email-preview-1",
            json!({
                "id": "email-preview-1",
                "subject": "Hello with Preview",
                "receivedAt": "2026-07-19T10:00:00Z",
                "from": [{ "name": "Alice", "email": "alice@test.com" }],
                "to": [{ "name": "Bob", "email": "bob@test.com" }],
                "preview": "This is RFC 8621 preview content."
            }),
            "This is RFC 8621 preview content.",
        ),
        (
            "text body fallback",
            "email-fallback-1",
            json!({
                "id": "email-fallback-1",
                "subject": "Fallback Test",
                "receivedAt": "2026-07-19T10:00:00Z",
                "from": [{ "name": "Alice", "email": "alice@test.com" }],
                "to": [{ "name": "Bob", "email": "bob@test.com" }],
                "textBody": [{ "partId": "tp1" }],
                "bodyValues": {
                    "tp1": { "value": "Fallback plain text body content.", "isTruncated": false }
                }
            }),
            "Fallback plain text body content.",
        ),
    ];
    for (label, expected_id, email_json, expected_preview) in cases {
        let mut email: jmap_client::email::Email<jmap_client::Get> =
            serde_json::from_value(email_json).expect("valid Email");
        let simplified = simplify_email_for_search(&mut email);
        assert_eq!(
            simplified["id"], expected_id,
            "case '{label}': unexpected id"
        );
        assert_eq!(
            simplified["preview"], expected_preview,
            "case '{label}': unexpected preview"
        );
        assert!(
            simplified.get("body").is_none(),
            "case '{label}': a search item must not carry a body"
        );
    }
}

// ===========================================================================
// Deep coverage: markdown formatting, previews and address helpers
// ===========================================================================

use super::{
    create_email_via_set, extract_address_strings, extract_fallback_preview,
    format_email_as_markdown, get_first_identity_email, get_first_identity_id, lookup_mailbox_id,
    resolve_drafts, serialize_address_list,
};
use crate::tools::jmap::client::JmapSession;

/// Install the ring crypto provider once per process (JMAP tests talk plain
/// HTTP, but `jmap_client` builds a TLS-capable client regardless).
fn ensure_rustls_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// Deserialize a JSON fixture into a typed `Email<Get>`.
fn email_fixture(value: serde_json::Value) -> jmap_client::email::Email<jmap_client::Get> {
    serde_json::from_value(value).expect("fixture must deserialize into Email<Get>")
}

/// Parse the YAML front matter emitted by `format_email_as_markdown`.
///
/// Parsing (instead of string matching) keeps the assertions exact even when
/// the YAML emitter quotes a scalar, e.g. a subject containing ` #`.
fn parse_front_matter(markdown: &str) -> serde_json::Value {
    let rest = markdown
        .strip_prefix("---\n")
        .unwrap_or_else(|| panic!("front matter must open with '---\\n', got: {markdown}"));
    let end = rest
        .find("\n---")
        .unwrap_or_else(|| panic!("front matter must close with '---', got: {markdown}"));
    serde_norway::from_str::<serde_json::Value>(&rest[..end]).unwrap_or_else(|e| {
        panic!(
            "front matter must be valid YAML: {e}\n---\n{}",
            &rest[..end]
        )
    })
}

/// Return the markdown body that follows the YAML front matter block.
///
/// `format_email_as_markdown` emits `---\n<yaml>\n---\n\n<body>`: the closing
/// `---` ends its own line and is then followed by one blank line before the
/// content (the standard front-matter separator). Both newlines belong to the
/// separator, not to the body, so both are stripped here. An empty body is
/// emitted as `---\n<yaml>\n---\n` with no separator at all, which the
/// conditional strips simply leave the result empty.
fn body_after_front_matter(markdown: &str) -> &str {
    let rest = markdown
        .strip_prefix("---\n")
        .expect("front matter must open with '---\\n'");
    let end = rest
        .find("\n---")
        .expect("front matter must close with '---'");
    let tail = &rest[end + "\n---".len()..];
    let tail = tail.strip_prefix('\n').unwrap_or(tail);
    tail.strip_prefix('\n').unwrap_or(tail)
}

/// Build an `AgentConfig` with a single JMAP client named `test`.
fn jmap_config(url: &str) -> AgentConfig {
    let mut config = AgentConfig::default();
    insert_jmap_client(&mut config, "test", url);
    config
}

fn insert_jmap_client(config: &mut AgentConfig, name: &str, url: &str) {
    config.jmap_clients.insert(
        name.to_string(),
        JmapClient {
            url: url.to_string(),
            token: "tok".to_string(),
        },
    );
}

/// Connect a raw `JmapSession` to a mock server URL.
fn session_at(url: &str) -> JmapSession {
    JmapSession::connect(&JmapClient {
        url: url.to_string(),
        token: "tok".to_string(),
    })
    .expect("connect should succeed")
}

/// Run `tool_search_email` in first-call (no cursor) mode.
fn search_first_call(
    config: &AgentConfig,
    filters: SearchEmailFilters<'_>,
) -> Result<crate::tools::dtos::SearchEmailResponse, String> {
    let cache = crate::tools::registry::cache::ToolCache::new();
    tool_search_email(
        config,
        filters,
        None,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
    )
}

/// A JMAP stub that answers every `POST` with a fixed status and body.
///
/// `spawn_mock_server` can only produce HTTP 200, so the transport-level error
/// handling of `JmapSession::post` needs a stub that can fail at the HTTP layer.
fn spawn_fixed_response_mock_server(status: u16, post_body: &str) -> String {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind stub listener");
    let port = listener.local_addr().expect("stub address").port();
    let api_url = format!("http://127.0.0.1:{port}");

    let session_str = json!({
        "capabilities": {
            "urn:ietf:params:jmap:core": {
                "maxSizeUpload": 100_000_000,
                "maxConcurrentUpload": 5,
                "maxSizeRequest": 100_000_000,
                "maxConcurrentRequests": 10,
                "maxCallsInRequest": 16,
                "maxObjectsInGet": 1000,
                "maxObjectsInSet": 100,
                "collationAlgorithms": ["i;ascii-casemap"]
            },
            "urn:ietf:params:jmap:mail": {},
            "urn:ietf:params:jmap:submission": {}
        },
        "accounts": {
            "acc1": {
                "name": "Stub Account",
                "isPersonal": true,
                "isReadOnly": false,
                "accountCapabilities": {
                    "urn:ietf:params:jmap:core": {},
                    "urn:ietf:params:jmap:mail": {
                        "maxMailboxesPerEmail": 100,
                        "maxMailboxDepth": 10,
                        "maxSizeMailboxName": 256,
                        "maxSizeAttachmentsPerEmail": 100_000_000,
                        "emailQuerySortOptions": ["receivedAt"],
                        "mayCreateTopLevelMailbox": true
                    },
                    "urn:ietf:params:jmap:submission": {
                        "maxDelayedSend": 0,
                        "submissionExtensions": {}
                    }
                }
            }
        },
        "username": "stub",
        "apiUrl": &api_url,
        "downloadUrl": format!("{api_url}/download/{{accountId}}/{{blobId}}/{{name}}"),
        "uploadUrl": format!("{api_url}/upload/{{accountId}}"),
        "eventSourceUrl": format!("{api_url}/eventsource"),
        "state": "stub-state-0",
        "primaryAccounts": {
            "urn:ietf:params:jmap:core": "acc1",
            "urn:ietf:params:jmap:mail": "acc1"
        }
    })
    .to_string();

    let post_str = format!(
        "HTTP/1.1 {status} Stubbed\r\n\
         Content-Type: application/json\r\n\
         Connection: close\r\n\
         Content-Length: {}\r\n\
         \r\n\
         {post_body}",
        post_body.len()
    );
    let session_response = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: application/json; charset=utf-8\r\n\
         Connection: close\r\n\
         Content-Length: {}\r\n\
         \r\n\
         {session_str}",
        session_str.len()
    );

    thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut buf = [0u8; 65536];
            let _ = stream.read(&mut buf);
            let request_line: String = buf
                .iter()
                .take_while(|b| **b != b'\r' && **b != b'\n')
                .map(|b| *b as char)
                .collect();
            let reply = if request_line.starts_with("POST") {
                &post_str
            } else {
                &session_response
            };
            let _ = stream.write_all(reply.as_bytes());
            let _ = stream.flush();
        }
    });
    api_url
}

/// A loopback URL whose port is guaranteed to be closed (instant refusal).
fn closed_loopback_url() -> String {
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind probe port");
    let port = listener.local_addr().expect("probe address").port();
    drop(listener);
    format!("http://127.0.0.1:{port}")
}

/// Session resource without any configured method responses, so every JMAP
/// method call comes back as an `unknownMethod` error.
const SESSION_ONLY: &str =
    r#"{"apiUrl": "{API_URL}", "primaryAccounts": {"urn:ietf:params:jmap:mail": "acc1"}}"#;

/// `Email/query` + `Email/get` that always yield a single hit.
const SEARCH_ONE_HIT: &str = r#"{
    "apiUrl": "{API_URL}",
    "primaryAccounts": {"urn:ietf:params:jmap:mail": "acc1"},
    "methodResponses": [
        ["Email/query", {"ids": ["e1"]}, "0"],
        ["Email/get", {"list": [{"id": "e1", "subject": "Hit"}]}, "1"]
    ]
}"#;

/// `Mailbox/query` + `Mailbox/get` for a mailbox named `INBOX`.
const MAILBOX_INBOX: &str = r#"{
    "apiUrl": "{API_URL}",
    "primaryAccounts": {"urn:ietf:params:jmap:mail": "acc1"},
    "methodResponses": [
        ["Mailbox/query", {"ids": ["m1"]}, "0"],
        ["Mailbox/get", {"list": [{"id": "m1", "name": "INBOX"}]}, "1"]
    ]
}"#;

/// A JMAP session body carrying the given `methodResponses` array literal.
fn send_body(method_responses: &str) -> String {
    format!(
        r#"{{"apiUrl": "{{API_URL}}", "primaryAccounts": {{"urn:ietf:params:jmap:mail": "acc1", "urn:ietf:params:jmap:submission": "acc1"}}, "methodResponses": {method_responses}}}"#
    )
}

const IDENTITY_OK: &str = r#"[{"id": "ident-1", "email": "sender@test.com"}]"#;

// ---------------------------------------------------------------------------
// format_email_as_markdown
// ---------------------------------------------------------------------------

#[test]
fn format_email_markdown_renders_html_body_and_full_front_matter() {
    let mut email = email_fixture(json!({
        "id": "e-html",
        "subject": "Order #42",
        "receivedAt": "2026-07-19T10:00:00Z",
        "from": [
            { "name": "Store", "email": "orders@store.com" },
            { "email": "noreply@store.com" }
        ],
        "to": [{ "name": "   ", "email": "alice@example.com" }],
        "cc": [{ "name": "Carbon", "email": "cc@example.com" }],
        "bcc": [{ "email": "bcc@example.com" }],
        "htmlBody": [{ "partId": "h1" }],
        "bodyValues": { "h1": { "value": "<p>Hello</p>", "isTruncated": false } }
    }));

    let md = format_email_as_markdown(&mut email, None);
    let fm = parse_front_matter(&md);
    assert_eq!(fm["id"], json!("e-html"));
    assert_eq!(fm["subject"], json!("Order #42"));
    let date = fm.get("date").and_then(|v| v.as_str()).unwrap_or_default();
    assert!(
        date.starts_with("2026-07-19T10:00:00"),
        "unexpected date: {date:?}"
    );
    assert_eq!(
        fm["from"],
        json!(["Store <orders@store.com>", "noreply@store.com"])
    );
    assert_eq!(fm["to"], json!(["alice@example.com"]));
    assert_eq!(fm["cc"], json!(["Carbon <cc@example.com>"]));
    assert_eq!(fm["bcc"], json!(["bcc@example.com"]));
    assert_eq!(body_after_front_matter(&md).trim(), "Hello");
}

#[test]
fn format_email_markdown_without_optional_fields_emits_only_the_id() {
    let mut email = email_fixture(json!({ "id": "e-bare" }));
    let md = format_email_as_markdown(&mut email, None);
    let fm = parse_front_matter(&md);
    assert_eq!(fm, json!({ "id": "e-bare" }));
    assert!(md.ends_with("---\n"), "got: {md:?}");
    assert_eq!(body_after_front_matter(&md), "");
}

#[test]
fn format_email_markdown_ignores_html_parts_without_a_usable_reference() {
    for (label, html_body) in [
        ("empty htmlBody", json!([])),
        ("part without partId", json!([{}])),
        ("unknown partId", json!([{ "partId": "zz" }])),
    ] {
        let mut email = email_fixture(json!({
            "id": "e-fallback",
            "htmlBody": html_body,
            "textBody": [{ "partId": "t1" }],
            "bodyValues": { "t1": { "value": "Text wins", "isTruncated": false } }
        }));
        let md = format_email_as_markdown(&mut email, None);
        assert_eq!(body_after_front_matter(&md), "Text wins", "case {label}");
    }
}

/// The markdown body is the email's body value with leading blank lines
/// stripped, plus the `... (truncated)` marker when the line limit cuts the
/// value or when the server already truncated it. A value that does not look
/// like markup is passed through unconverted.
#[test]
fn format_email_markdown_renders_the_body_value_within_the_line_limit() {
    for (label, id, value, is_truncated, max_lines, expected) in [
        (
            "beyond the line limit",
            "e-lines",
            "l1\nl2\nl3\nl4",
            false,
            Some(2),
            "l1\nl2\n... (truncated)",
        ),
        (
            "exactly at the line limit",
            "e-fit",
            "l1\nl2\nl3",
            false,
            Some(3),
            "l1\nl2\nl3",
        ),
        (
            "server-truncated body value",
            "e-trunc",
            "Only line",
            true,
            None,
            "Only line\n... (truncated)",
        ),
        (
            "leading blank lines",
            "e-blank",
            "\n\nIndented body",
            false,
            None,
            "Indented body",
        ),
        (
            "lone angle bracket is not markup",
            "e-plain",
            "Total: 1 < 2",
            false,
            None,
            "Total: 1 < 2",
        ),
    ] {
        let mut email = email_fixture(json!({
            "id": id,
            "textBody": [{ "partId": "t1" }],
            "bodyValues": { "t1": { "value": value, "isTruncated": is_truncated } }
        }));
        let md = format_email_as_markdown(&mut email, max_lines);
        assert_eq!(
            body_after_front_matter(&md),
            expected,
            "case '{label}': unexpected body"
        );
    }
}

// ---------------------------------------------------------------------------
// extract_fallback_preview
// ---------------------------------------------------------------------------

#[test]
fn extract_fallback_preview_prefers_the_text_body() {
    let email = email_fixture(json!({
        "id": "p-text",
        "textBody": [{ "partId": "t1" }],
        "htmlBody": [{ "partId": "h1" }],
        "bodyValues": {
            "t1": { "value": "Plain first", "isTruncated": false },
            "h1": { "value": "<p>Html second</p>", "isTruncated": false }
        }
    }));
    assert_eq!(extract_fallback_preview(&email), "Plain first");
}

#[test]
fn extract_fallback_preview_converts_the_html_body_when_no_text_part_exists() {
    let email = email_fixture(json!({
        "id": "p-html",
        "htmlBody": [{ "partId": "h1" }],
        "bodyValues": { "h1": { "value": "<p>Html preview</p>", "isTruncated": false } }
    }));
    let preview = extract_fallback_preview(&email);
    assert_eq!(preview.trim(), "Html preview");
    assert!(!preview.contains('<'), "got: {preview:?}");
}

#[test]
fn extract_fallback_preview_caps_the_snippet_at_256_characters() {
    let email = email_fixture(json!({
        "id": "p-long",
        "textBody": [{ "partId": "t1" }],
        "bodyValues": { "t1": { "value": "a".repeat(300), "isTruncated": false } }
    }));
    let preview = extract_fallback_preview(&email);
    assert_eq!(preview.chars().count(), 256);
    assert_eq!(preview, "a".repeat(256));
}

#[test]
fn extract_fallback_preview_is_empty_without_any_usable_body_part() {
    for (label, fixture) in [
        ("no bodies at all", json!({ "id": "p-none" })),
        (
            "html part without a body value",
            json!({
                "id": "p-missing",
                "htmlBody": [{ "partId": "zz" }],
                "bodyValues": {}
            }),
        ),
        (
            "empty body part lists",
            json!({
                "id": "p-empty-lists",
                "textBody": [],
                "htmlBody": []
            }),
        ),
    ] {
        let email = email_fixture(fixture);
        assert_eq!(extract_fallback_preview(&email), "", "case {label}");
    }
}

// ---------------------------------------------------------------------------
// extract_address_strings / serialize_address_list
// ---------------------------------------------------------------------------

#[test]
fn extract_address_strings_formats_named_and_bare_addresses() {
    let addrs: Vec<jmap_client::email::EmailAddress<jmap_client::Get>> =
        serde_json::from_value(json!([
            { "name": "Alice", "email": "alice@test.com" },
            { "email": "bob@test.com" },
            { "name": "   ", "email": "carol@test.com" }
        ]))
        .expect("valid address list");

    assert_eq!(
        extract_address_strings(Some(&addrs[..])).expect("non-empty list"),
        vec![
            "Alice <alice@test.com>".to_string(),
            "bob@test.com".to_string(),
            "carol@test.com".to_string(),
        ]
    );
}

#[test]
fn extract_address_strings_maps_missing_and_empty_lists_to_none() {
    assert!(extract_address_strings(None).is_none());
    let empty: Vec<jmap_client::email::EmailAddress<jmap_client::Get>> = Vec::new();
    assert!(extract_address_strings(Some(&empty[..])).is_none());
}

#[test]
fn serialize_address_list_emits_a_json_array_or_none() {
    assert!(serialize_address_list(None).is_none());
    let empty: Vec<jmap_client::email::EmailAddress<jmap_client::Get>> = Vec::new();
    assert!(serialize_address_list(Some(&empty[..])).is_none());

    let addrs: Vec<jmap_client::email::EmailAddress<jmap_client::Get>> =
        serde_json::from_value(json!([{ "name": "Alice", "email": "alice@test.com" }]))
            .expect("valid address list");
    assert_eq!(
        serialize_address_list(Some(&addrs[..])),
        Some(json!(["Alice <alice@test.com>"]))
    );
}

// ---------------------------------------------------------------------------
// simplify_email_for_search
// ---------------------------------------------------------------------------

#[test]
fn simplify_email_for_search_minimal_email_keeps_only_id_subject_and_preview() {
    let mut email = email_fixture(json!({ "id": "bare" }));
    let value = simplify_email_for_search(&mut email);
    assert_eq!(value["id"], "bare");
    assert!(value["subject"].is_null(), "got: {value}");
    assert_eq!(value["preview"], "");
    for field in ["date", "from", "to", "cc", "bcc"] {
        assert!(
            value.get(field).is_none(),
            "case {field}: must be absent, got: {value}"
        );
    }
    assert_eq!(value.as_object().map(|m| m.len()), Some(3));
}

#[test]
fn simplify_email_for_search_keeps_cc_and_bcc_when_present() {
    let mut email = email_fixture(json!({
        "id": "full",
        "subject": "Hi",
        "receivedAt": "2026-07-19T10:00:00Z",
        "from": [{ "name": "Alice", "email": "alice@test.com" }],
        "to": [{ "email": "bob@test.com" }],
        "cc": [{ "email": "cc@test.com" }],
        "bcc": [{ "email": "bcc@test.com" }],
        "preview": "Native preview"
    }));
    let value = simplify_email_for_search(&mut email);
    assert_eq!(value["subject"], "Hi");
    assert_eq!(value["preview"], "Native preview");
    assert_eq!(value["cc"], json!(["cc@test.com"]));
    assert_eq!(value["bcc"], json!(["bcc@test.com"]));
    let date = value["date"].as_str().unwrap_or_default();
    assert!(
        date.starts_with("2026-07-19T10:00:00"),
        "unexpected date: {date:?}"
    );
}

#[test]
fn simplify_email_for_search_drops_empty_address_lists() {
    let mut email = email_fixture(json!({
        "id": "empty-addrs",
        "from": [],
        "to": [],
        "cc": [],
        "bcc": []
    }));
    let value = simplify_email_for_search(&mut email);
    for field in ["from", "to", "cc", "bcc"] {
        assert!(
            value.get(field).is_none(),
            "case {field}: empty lists must be omitted, got: {value}"
        );
    }
}

// ---------------------------------------------------------------------------
// simplify_jmap_emails — malformed and fallback paths
// ---------------------------------------------------------------------------

#[test]
fn simplify_jmap_emails_ignores_malformed_method_responses() {
    for (label, res) in [
        ("no methodResponses", json!({ "sessionState": "s" })),
        (
            "methodResponses is not an array",
            json!({ "methodResponses": "nope" }),
        ),
        (
            "response tuple is not an array",
            json!({ "methodResponses": ["nope"] }),
        ),
        (
            "wrong method name",
            json!({ "methodResponses": [["Mailbox/get", { "list": [] }, "0"]] }),
        ),
        (
            "arguments are not an object",
            json!({ "methodResponses": [["Email/get", "nope", "0"]] }),
        ),
        (
            "list is not an array",
            json!({ "methodResponses": [["Email/get", { "list": "nope" }, "0"]] }),
        ),
    ] {
        let out = simplify_jmap_emails(res, None);
        assert_eq!(out, json!([]), "case {label}");
    }
}

/// Every field an `Email/get` list entry does not carry is still emitted as
/// `null`, and a missing body defaults to the empty string.
#[test]
fn simplify_jmap_emails_defaults_missing_fields_to_null() {
    for (label, email, expected_id) in [
        ("no fields at all", json!({}), serde_json::Value::Null),
        ("only an id", json!({ "id": "e1" }), json!("e1")),
    ] {
        let res = email_get_response(email);
        let out = simplify_jmap_emails(res, None);
        assert_eq!(
            out.as_array().map(Vec::len),
            Some(1),
            "case '{label}': expected one email"
        );
        assert_eq!(out[0]["id"], expected_id, "case '{label}': unexpected id");
        for field in ["subject", "date", "from", "to", "cc", "bcc"] {
            assert!(
                out[0][field].is_null(),
                "case '{label}': field {field} must default to null, got {}",
                out[0][field]
            );
        }
        assert_eq!(
            out[0]["body"], "",
            "case '{label}': a missing body must be empty"
        );
    }
}

#[test]
fn simplify_jmap_emails_falls_back_to_text_when_html_part_is_unusable() {
    for (label, html_body) in [
        ("empty htmlBody", json!([])),
        ("part without partId", json!([{}])),
        ("unknown partId", json!([{ "partId": "zz" }])),
        ("non-string partId", json!([{ "partId": 7 }])),
        ("html entry is not an object", json!(["nope"])),
    ] {
        let res = json!({
            "methodResponses": [[ "Email/get", {
                "list": [{
                    "id": "e1",
                    "htmlBody": html_body,
                    "textBody": [{ "partId": "t1" }],
                    "bodyValues": { "t1": { "value": "Text wins", "isTruncated": false } }
                }]
            }, "0"]]
        });
        let out = simplify_jmap_emails(res, None);
        assert_eq!(out[0]["body"], "Text wins", "case {label}");
    }
}

/// An email whose body value cannot be trusted is still emitted: a non-bool
/// `isTruncated` flag is read as `false`, and a body-part reference without a
/// matching `bodyValues` entry produces an empty body.
#[test]
fn simplify_jmap_emails_falls_back_when_the_body_value_is_unusable() {
    for (label, res, expected_body) in [
        (
            "non-bool isTruncated is read as false",
            json!({
                "methodResponses": [[ "Email/get", {
                    "list": [{
                        "id": "e1",
                        "textBody": [{ "partId": "t1" }],
                        "bodyValues": { "t1": { "value": "Body", "isTruncated": "yes" } }
                    }]
                }, "0"]]
            }),
            "Body",
        ),
        (
            "body part without bodyValues",
            json!({
                "methodResponses": [[ "Email/get", {
                    "list": [{ "id": "e1", "textBody": [{ "partId": "t1" }] }]
                }, "0"]]
            }),
            "",
        ),
    ] {
        let out = simplify_jmap_emails(res, None);
        assert_eq!(
            out[0]["body"], expected_body,
            "case '{label}': unexpected body"
        );
    }
}

// ---------------------------------------------------------------------------
// convert_html_in_jmap — recursion and defensive paths
// ---------------------------------------------------------------------------

#[test]
fn convert_html_in_jmap_recurses_through_nested_arrays() {
    let res = json!({ "items": [[{
        "bodyValues": { "p1": { "value": "<p>Deep</p>", "isTruncated": false } }
    }]] });
    let out = convert_html_in_jmap(res);
    let val = out["items"][0][0]["bodyValues"]["p1"]["value"]
        .as_str()
        .expect("value must stay a string");
    assert_eq!(val.trim(), "Deep");
}

#[test]
fn convert_html_in_jmap_tolerates_non_object_shapes() {
    for (label, res) in [
        ("top-level string", json!("plain")),
        ("top-level number", json!(42)),
        ("top-level array", json!([1, 2, 3])),
        (
            "bodyValues is not an object",
            json!({ "bodyValues": "nope" }),
        ),
        (
            "body part is not an object",
            json!({ "bodyValues": { "p1": "nope" } }),
        ),
        (
            "body part has no value key",
            json!({ "bodyValues": { "p1": { "isTruncated": false } } }),
        ),
        (
            "body value is not a string",
            json!({ "bodyValues": { "p1": { "value": 42 } } }),
        ),
    ] {
        let out = convert_html_in_jmap(res.clone());
        assert_eq!(out, res, "case {label}");
    }
}

// ---------------------------------------------------------------------------
// lookup_mailbox_id
// ---------------------------------------------------------------------------

#[test]
fn lookup_mailbox_id_matches_the_name_case_insensitively() {
    ensure_rustls_provider();
    let url = spawn_mock_server(MAILBOX_INBOX);
    let session = session_at(&url);
    for name in ["INBOX", "inbox", "InBoX"] {
        assert_eq!(
            lookup_mailbox_id(&session, name).expect("mailbox must be found"),
            "m1",
            "case {name}"
        );
    }
}

/// A queried mailbox that does not carry the requested name — because it is
/// named differently or has no `name` at all — makes the lookup report
/// "not found" instead of guessing an id.
#[test]
fn lookup_mailbox_id_errors_when_no_mailbox_matches() {
    ensure_rustls_provider();
    for (label, mailbox, reason) in [
        (
            "another name",
            json!({"id": "m1", "name": "Archive"}),
            "no match",
        ),
        ("no name field", json!({"id": "m1"}), "unnamed mailbox"),
    ] {
        let url = spawn_mock_server(
            json!({
                "apiUrl": "{API_URL}",
                "primaryAccounts": {"urn:ietf:params:jmap:mail": "acc1"},
                "methodResponses": [
                    ["Mailbox/query", {"ids": ["m1"]}, "0"],
                    ["Mailbox/get", {"list": [mailbox]}, "1"]
                ]
            })
            .to_string(),
        );
        let session = session_at(&url);
        assert_eq!(
            lookup_mailbox_id(&session, "INBOX").expect_err(reason),
            "Mailbox not found with name: INBOX",
            "case '{label}': expected the not-found error"
        );
    }
}

#[test]
fn lookup_mailbox_id_reports_a_query_failure() {
    ensure_rustls_provider();
    let url = spawn_mock_server(SESSION_ONLY);
    let session = session_at(&url);
    let err = lookup_mailbox_id(&session, "INBOX").expect_err("Mailbox/query fails");
    assert!(err.starts_with("Failed to query mailboxes: "), "got: {err}");
}

#[test]
fn lookup_mailbox_id_reports_a_get_failure() {
    ensure_rustls_provider();
    let url = spawn_mock_server(
        r#"{
            "apiUrl": "{API_URL}",
            "primaryAccounts": {"urn:ietf:params:jmap:mail": "acc1"},
            "methodResponses": [["Mailbox/query", {"ids": ["m1"]}, "0"]]
        }"#,
    );
    let session = session_at(&url);
    let err = lookup_mailbox_id(&session, "INBOX").expect_err("Mailbox/get fails");
    assert!(err.starts_with("Failed to get mailbox: "), "got: {err}");
}

// ---------------------------------------------------------------------------
// resolve_drafts
// ---------------------------------------------------------------------------

#[test]
fn resolve_drafts_returns_the_first_queried_id() {
    ensure_rustls_provider();
    let url = spawn_mock_server(
        r#"{
            "apiUrl": "{API_URL}",
            "primaryAccounts": {"urn:ietf:params:jmap:mail": "acc1"},
            "methodResponses": [["Mailbox/query", {"ids": ["d1", "d2"]}, "0"]]
        }"#,
    );
    let session = session_at(&url);
    assert_eq!(
        resolve_drafts(&session, "acc1").expect("drafts found"),
        "d1"
    );
}

#[test]
fn resolve_drafts_errors_when_the_query_returns_no_ids() {
    ensure_rustls_provider();
    let url = spawn_mock_server(
        r#"{
            "apiUrl": "{API_URL}",
            "primaryAccounts": {"urn:ietf:params:jmap:mail": "acc1"},
            "methodResponses": [["Mailbox/query", {"ids": []}, "0"]]
        }"#,
    );
    let session = session_at(&url);
    assert_eq!(
        resolve_drafts(&session, "acc1").expect_err("no drafts"),
        "Drafts mailbox not found"
    );
}

#[test]
fn resolve_drafts_reports_a_query_error() {
    ensure_rustls_provider();
    let url = spawn_mock_server(SESSION_ONLY);
    let session = session_at(&url);
    let err = resolve_drafts(&session, "acc1").expect_err("Mailbox/query fails");
    assert!(err.starts_with("Mailbox/query error: "), "got: {err}");
}

// ---------------------------------------------------------------------------
// get_first_identity_email / get_first_identity_id
// ---------------------------------------------------------------------------

#[test]
fn get_first_identity_reads_the_first_identity() {
    ensure_rustls_provider();
    let url = spawn_mock_server(send_body(&format!(
        r#"[["Identity/get", {{"state": "s", "list": {IDENTITY_OK}, "notFound": []}}, "0"]]"#
    )));
    let session = session_at(&url);
    assert_eq!(
        get_first_identity_email(&session).expect("identity email"),
        "sender@test.com"
    );
    assert_eq!(
        get_first_identity_id(&session).expect("identity id"),
        "ident-1"
    );
}

#[test]
fn get_first_identity_email_errors_without_an_email_address() {
    ensure_rustls_provider();
    for (label, list) in [
        ("empty list", json!([])),
        ("identity without email", json!([{ "id": "ident-1" }])),
    ] {
        let url = spawn_mock_server(send_body(&format!(
            r#"[["Identity/get", {{"state": "s", "list": {list}, "notFound": []}}, "0"]]"#
        )));
        let session = session_at(&url);
        assert_eq!(
            get_first_identity_email(&session).expect_err("no email"),
            "No identity with an email address found",
            "case {label}"
        );
    }
}

#[test]
fn get_first_identity_id_errors_without_an_id() {
    ensure_rustls_provider();
    for (label, list) in [
        ("empty list", json!([])),
        ("identity without id", json!([{ "email": "x@y.z" }])),
    ] {
        let url = spawn_mock_server(send_body(&format!(
            r#"[["Identity/get", {{"state": "s", "list": {list}, "notFound": []}}, "0"]]"#
        )));
        let session = session_at(&url);
        assert_eq!(
            get_first_identity_id(&session).expect_err("no id"),
            "No identity found",
            "case {label}"
        );
    }
}

#[test]
fn get_first_identity_helpers_report_a_request_failure() {
    ensure_rustls_provider();
    let url = spawn_mock_server(SESSION_ONLY);
    let session = session_at(&url);
    let email_err = get_first_identity_email(&session).expect_err("Identity/get fails");
    assert!(
        email_err.starts_with("Identity/get request failed: "),
        "got: {email_err}"
    );
    let id_err = get_first_identity_id(&session).expect_err("Identity/get fails");
    assert!(
        id_err.starts_with("Identity/get request failed: "),
        "got: {id_err}"
    );
}

// ---------------------------------------------------------------------------
// create_email_via_set
// ---------------------------------------------------------------------------

#[test]
fn create_email_via_set_returns_the_created_id() {
    ensure_rustls_provider();
    let url = spawn_mock_server(send_body(
        r#"[["Email/set", {"created": {"c0": {"id": "email-1"}}}, "0"]]"#,
    ));
    let session = session_at(&url);
    assert_eq!(
        create_email_via_set(
            &session,
            "drafts-id",
            "from@test.com",
            "to@test.com",
            "Subject",
            "Body",
        )
        .expect("email created"),
        "email-1"
    );
}

#[test]
fn create_email_via_set_reports_a_not_created_error() {
    ensure_rustls_provider();
    // RFC 8620 §5.3: `notCreated` maps the creation-id to a SetError whose
    // `type` is one of the defined set-error types (`invalidProperties` here).
    let url = spawn_mock_server(send_body(
        r#"[["Email/set", {"notCreated": {"c0": {"type": "invalidProperties", "description": "nope", "properties": ["to"]}}}, "0"]]"#,
    ));
    let session = session_at(&url);
    let err = create_email_via_set(
        &session,
        "drafts-id",
        "from@test.com",
        "to@test.com",
        "Subject",
        "Body",
    )
    .expect_err("creation must fail");
    // The `notCreated` entry is surfaced verbatim (crate `SetError` display:
    // type, description and offending properties) through the creation path,
    // not the transport-failure path and not the generic "Id c0 not found".
    assert_eq!(
        err, "Email/set creation failed: Set failed: invalidProperties: nope (properties: to)",
        "the notCreated SetError must be reported as a creation failure"
    );
}

#[test]
fn create_email_via_set_errors_when_the_created_object_has_no_id() {
    ensure_rustls_provider();
    let url = spawn_mock_server(send_body(
        r#"[["Email/set", {"created": {"c0": {}}}, "0"]]"#,
    ));
    let session = session_at(&url);
    assert_eq!(
        create_email_via_set(
            &session,
            "drafts-id",
            "from@test.com",
            "to@test.com",
            "Subject",
            "Body",
        )
        .expect_err("created object lacks an id"),
        "Email/set response missing created email ID"
    );
}

#[test]
fn create_email_via_set_reports_a_request_failure() {
    ensure_rustls_provider();
    let url = spawn_mock_server(SESSION_ONLY);
    let session = session_at(&url);
    let err = create_email_via_set(
        &session,
        "drafts-id",
        "from@test.com",
        "to@test.com",
        "Subject",
        "Body",
    )
    .expect_err("Email/set is not served");
    assert!(err.starts_with("Email/set request failed: "), "got: {err}");
}

// ---------------------------------------------------------------------------
// tool_send_email — per-stage failure reporting
// ---------------------------------------------------------------------------

#[test]
fn tool_send_email_without_clients_errors() {
    let config = AgentConfig::default();
    assert_eq!(
        tool_send_email(&config, "to@test.com", "S", "B").expect_err("no clients"),
        "No JMAP clients configured."
    );
}

#[test]
fn tool_send_email_reports_a_session_failure() {
    ensure_rustls_provider();
    let config = jmap_config(&closed_loopback_url());
    let err = tool_send_email(&config, "to@test.com", "S", "B")
        .expect_err("the session cannot be established");
    assert!(
        err.starts_with("Error fetching JMAP session for test: "),
        "got: {err}"
    );
}

#[test]
fn tool_send_email_reports_a_drafts_failure() {
    ensure_rustls_provider();
    let url = spawn_mock_server(send_body(r#"[["Mailbox/query", {"ids": []}, "0"]]"#));
    let config = jmap_config(&url);
    assert_eq!(
        tool_send_email(&config, "to@test.com", "S", "B").expect_err("no drafts mailbox"),
        "Error resolving drafts for test: Drafts mailbox not found"
    );
}

#[test]
fn tool_send_email_reports_an_identity_failure() {
    ensure_rustls_provider();
    let url = spawn_mock_server(send_body(
        r#"[["Mailbox/query", {"ids": ["drafts-id"]}, "0"],
           ["Identity/get", {"state": "s", "list": [], "notFound": []}, "1"]]"#,
    ));
    let config = jmap_config(&url);
    assert_eq!(
        tool_send_email(&config, "to@test.com", "S", "B").expect_err("no identity"),
        "Error retrieving identity for test: No identity with an email address found"
    );
}

#[test]
fn tool_send_email_reports_a_create_failure() {
    ensure_rustls_provider();
    // RFC 8620 §5.3 set-error type (`invalidProperties`) — the same shape the
    // `create_email_via_set` test pins, reached through the tool wrapper.
    let url = spawn_mock_server(send_body(
        r#"[["Mailbox/query", {"ids": ["drafts-id"]}, "0"],
           ["Identity/get", {"state": "s", "list": [{"id": "ident-1", "email": "sender@test.com"}], "notFound": []}, "1"],
           ["Email/set", {"notCreated": {"c0": {"type": "invalidProperties", "description": "nope", "properties": ["to"]}}}, "2"]]"#,
    ));
    let config = jmap_config(&url);
    let err = tool_send_email(&config, "to@test.com", "S", "B").expect_err("Email/set must fail");
    assert_eq!(
        err,
        "Error creating email for test: Email/set creation failed: Set failed: invalidProperties: nope (properties: to)",
        "the notCreated SetError must be reported per client as a creation failure"
    );
}

#[test]
fn tool_send_email_reports_an_identity_id_failure() {
    ensure_rustls_provider();
    let url = spawn_mock_server(send_body(
        r#"[["Mailbox/query", {"ids": ["drafts-id"]}, "0"],
           ["Identity/get", {"state": "s", "list": [{"email": "sender@test.com"}], "notFound": []}, "1"],
           ["Email/set", {"created": {"c0": {"id": "email-1"}}}, "2"]]"#,
    ));
    let config = jmap_config(&url);
    assert_eq!(
        tool_send_email(&config, "to@test.com", "S", "B").expect_err("the identity has no id"),
        "Error retrieving identity ID for test: No identity found"
    );
}

#[test]
fn tool_send_email_reports_a_submission_failure_in_the_result() {
    ensure_rustls_provider();
    let url = spawn_mock_server(send_body(
        r#"[["Mailbox/query", {"ids": ["drafts-id"]}, "0"],
           ["Identity/get", {"state": "s", "list": [{"id": "ident-1", "email": "sender@test.com"}], "notFound": []}, "1"],
           ["Email/set", {"created": {"c0": {"id": "email-1"}}}, "2"],
           ["EmailSubmission/set", {"notCreated": {"c0": {"type": "senderInvalid", "description": "nope"}}}, "3"]]"#,
    ));
    let config = jmap_config(&url);
    let res = tool_send_email(&config, "to@test.com", "S", "B")
        .expect("a submission error is reported through the result body");
    assert!(
        res.result.starts_with("Error submitting email for test: "),
        "got: {}",
        res.result
    );
}

#[test]
fn tool_send_email_success_names_the_client_and_email_id() {
    ensure_rustls_provider();
    let url = spawn_mock_server(send_body(
        r#"[["Mailbox/query", {"ids": ["drafts-id"]}, "0"],
           ["Identity/get", {"state": "s", "list": [{"id": "ident-1", "email": "sender@test.com"}], "notFound": []}, "1"],
           ["Email/set", {"created": {"c0": {"id": "email-1"}}}, "2"],
           ["EmailSubmission/set", {"created": {"c0": {"id": "sub-1"}}}, "3"]]"#,
    ));
    let config = jmap_config(&url);
    let res = tool_send_email(&config, "to@test.com", "S", "B").expect("send should succeed");
    assert_eq!(
        res.result,
        "--- Client: test ---\nEmail sent successfully with ID: email-1"
    );
}

// ---------------------------------------------------------------------------
// tool_get_email_by_id — failure paths
// ---------------------------------------------------------------------------

const NOT_FOUND_ERROR: &str = "Email not found in any client or no clients configured.";

/// `tool_get_email_by_id` reports the same not-found error whichever reason keeps
/// the email from arriving: an empty `Email/get` list, a server that does not
/// serve the call, or a server that cannot be reached at all.
#[test]
fn tool_get_email_by_id_reports_not_found_for_every_failure_mode() {
    ensure_rustls_provider();
    let empty_list_url = spawn_mock_server(
        r#"{
            "apiUrl": "{API_URL}",
            "primaryAccounts": {"urn:ietf:params:jmap:mail": "acc1"},
            "methodResponses": [["Email/get", {"list": []}, "0"]]
        }"#,
    );
    let unserved_url = spawn_mock_server(SESSION_ONLY);
    let dead_url = closed_loopback_url();

    for (label, url, reason) in [
        ("no email in the response", &empty_list_url, "empty list"),
        (
            "the server does not serve Email/get",
            &unserved_url,
            "Email/get is not served",
        ),
        (
            "the session cannot connect",
            &dead_url,
            "connection refused",
        ),
    ] {
        let config = jmap_config(url);
        assert_eq!(
            tool_get_email_by_id(&config, "e1").expect_err(reason),
            NOT_FOUND_ERROR,
            "case '{label}': expected the not-found error"
        );
    }
}

// ---------------------------------------------------------------------------
// tool_search_email — per-client error collection
// ---------------------------------------------------------------------------

#[test]
fn tool_search_email_collects_client_errors_alongside_results() {
    ensure_rustls_provider();
    let good = spawn_mock_server(SEARCH_ONE_HIT);
    let dead = closed_loopback_url();
    let mut config = AgentConfig::default();
    insert_jmap_client(&mut config, "good", &good);
    insert_jmap_client(&mut config, "dead", &dead);

    let res = search_first_call(
        &config,
        SearchEmailFilters {
            keyword: Some("hit"),
            ..Default::default()
        },
    )
    .expect("one client succeeded");
    assert_eq!(res.count, 1);
    assert_eq!(res.results[0].client, "good");
    assert_eq!(res.results[0].preview["subject"], "Hit");
    assert_eq!(res.errors.len(), 1, "got: {:?}", res.errors);
    assert!(
        res.errors[0].starts_with("Error fetching JMAP session for dead: "),
        "got: {:?}",
        res.errors
    );
}

#[test]
fn tool_search_email_reports_a_failed_mailbox_lookup_per_client() {
    ensure_rustls_provider();
    // The "good" client needs the whole chain — mailbox lookup *and* the
    // Email/query + Email/get legs — otherwise it cannot contribute a hit.
    let good = spawn_mock_server(
        r#"{
            "apiUrl": "{API_URL}",
            "primaryAccounts": {"urn:ietf:params:jmap:mail": "acc1"},
            "methodResponses": [
                ["Mailbox/query", {"ids": ["m1"]}, "0"],
                ["Mailbox/get", {"list": [{"id": "m1", "name": "INBOX"}]}, "1"],
                ["Email/query", {"ids": ["e1"]}, "2"],
                ["Email/get", {"list": [{"id": "e1", "subject": "Hit"}]}, "3"]
            ]
        }"#,
    );
    let other = spawn_mock_server(
        r#"{
            "apiUrl": "{API_URL}",
            "primaryAccounts": {"urn:ietf:params:jmap:mail": "acc1"},
            "methodResponses": [
                ["Mailbox/query", {"ids": ["m9"]}, "0"],
                ["Mailbox/get", {"list": [{"id": "m9", "name": "Archive"}]}, "1"],
                ["Email/query", {"ids": ["e1"]}, "2"],
                ["Email/get", {"list": [{"id": "e1", "subject": "Hit"}]}, "3"]
            ]
        }"#,
    );
    let mut config = AgentConfig::default();
    insert_jmap_client(&mut config, "good", &good);
    insert_jmap_client(&mut config, "other", &other);

    let res = search_first_call(
        &config,
        SearchEmailFilters {
            folder: Some("inbox"),
            ..Default::default()
        },
    )
    .expect("the good client still yields a hit");
    assert_eq!(res.count, 1);
    assert_eq!(
        res.errors,
        vec!["Error for other: Mailbox not found with name: inbox"]
    );
}

#[test]
fn tool_search_email_reports_a_failed_query_per_client() {
    ensure_rustls_provider();
    let good = spawn_mock_server(SEARCH_ONE_HIT);
    let broken = spawn_mock_server(SESSION_ONLY);
    let mut config = AgentConfig::default();
    insert_jmap_client(&mut config, "good", &good);
    insert_jmap_client(&mut config, "broken", &broken);

    let res = search_first_call(
        &config,
        SearchEmailFilters {
            keyword: Some("hit"),
            ..Default::default()
        },
    )
    .expect("the good client still yields a hit");
    assert_eq!(res.count, 1);
    assert_eq!(res.errors.len(), 1, "got: {:?}", res.errors);
    assert!(
        res.errors[0].starts_with("Error querying email for broken: "),
        "got: {:?}",
        res.errors
    );
}

#[test]
fn tool_search_email_skips_emails_that_cannot_be_fetched() {
    ensure_rustls_provider();
    for (label, body) in [
        (
            "Email/get is not served",
            r#"{
                "apiUrl": "{API_URL}",
                "primaryAccounts": {"urn:ietf:params:jmap:mail": "acc1"},
                "methodResponses": [["Email/query", {"ids": ["e1", "e2"]}, "0"]]
            }"#,
        ),
        (
            "Email/get returns an empty list",
            r#"{
                "apiUrl": "{API_URL}",
                "primaryAccounts": {"urn:ietf:params:jmap:mail": "acc1"},
                "methodResponses": [
                    ["Email/query", {"ids": ["e1"]}, "0"],
                    ["Email/get", {"list": []}, "1"]
                ]
            }"#,
        ),
        (
            "Email/query returns no ids",
            r#"{
                "apiUrl": "{API_URL}",
                "primaryAccounts": {"urn:ietf:params:jmap:mail": "acc1"},
                "methodResponses": [["Email/query", {"ids": []}, "0"]]
            }"#,
        ),
    ] {
        let config = jmap_config(&spawn_mock_server(body));
        let res = search_first_call(
            &config,
            SearchEmailFilters {
                keyword: Some("hit"),
                ..Default::default()
            },
        )
        .expect("the search itself succeeds");
        assert!(
            res.results.is_empty(),
            "case {label}: got {:?}",
            res.results
        );
        assert_eq!(
            res.hint.as_deref(),
            Some("No matching emails found."),
            "case {label}"
        );
    }
}

// ---------------------------------------------------------------------------
// JmapSession transport behaviour (client.rs)
// ---------------------------------------------------------------------------

/// `JmapSession::post` turns an HTTP error status into a readable message: the
/// RFC 7807 `detail` when the body is structured, otherwise the raw body,
/// `(empty body)` when the body is missing, and a snippet capped at 500 bytes
/// (cut on a `char` boundary) for long bodies.
#[test]
fn jmap_post_reports_http_errors_with_the_server_message() {
    ensure_rustls_provider();
    let long_body = "x".repeat(600);
    /// One HTTP-error case: `(label, status, body, expected message fragments,
    /// request payload, expected error text)`.
    type PostErrorCase<'a> = (
        &'a str,
        u16,
        String,
        &'a [&'a str],
        serde_json::Value,
        String,
    );
    let cases: Vec<PostErrorCase<'_>> = vec![
        (
            "RFC 7807 detail",
            500,
            r#"{"type":"about:blank","title":"Boom","detail":"The mailbox exploded","status":500}"#
                .to_string(),
            &["urn:ietf:params:jmap:mail"],
            json!([["Email/get", { "ids": ["e1"] }, "0"]]),
            "Server returned 500 Internal Server Error: The mailbox exploded".to_string(),
        ),
        (
            "unstructured body",
            503,
            r#"{"description":"no type here"}"#.to_string(),
            &[],
            json!([]),
            "Server returned 503 Service Unavailable: {\"description\":\"no type here\"}"
                .to_string(),
        ),
        (
            "empty body",
            500,
            String::new(),
            &[],
            json!([]),
            "Server returned 500 Internal Server Error (empty body)".to_string(),
        ),
        (
            "body longer than the snippet limit",
            500,
            long_body,
            &[],
            json!([]),
            format!(
                "Server returned 500 Internal Server Error: {}...",
                "x".repeat(500)
            ),
        ),
    ];
    for (label, status, body, capabilities, method_calls, expected) in cases {
        let url = spawn_fixed_response_mock_server(status, &body);
        let session = session_at(&url);
        let err = session
            .post(capabilities, method_calls)
            .expect_err("an error status must be reported as an error");
        assert_eq!(err, expected, "case '{label}': unexpected error message");
    }
}

#[test]
fn jmap_post_reports_a_malformed_success_body() {
    ensure_rustls_provider();
    let url = spawn_fixed_response_mock_server(200, "this is not json");
    let session = session_at(&url);
    let err = session
        .post(&[], json!([]))
        .expect_err("an unparseable body must be an error");
    assert!(
        err.starts_with("Failed to parse response as JSON: "),
        "got: {err}"
    );
    assert!(err.ends_with(": this is not json"), "got: {err}");
}

/// Contract: an over-long error body is clipped at the snippet limit on a
/// `char` boundary, never at a raw byte index. (Was: `&body[..500]` panicked
/// when byte 500 fell inside a multi-byte UTF-8 character.)
#[test]
fn jmap_post_clips_a_multibyte_error_body_on_a_char_boundary() {
    ensure_rustls_provider();
    // 200 x 3 bytes = 600 bytes. Byte index 500 sits inside the 167th character
    // (bytes 498..501), so the last boundary at or before the limit is byte 498,
    // i.e. the snippet keeps 166 characters.
    let body = "世".repeat(200);
    let url = spawn_fixed_response_mock_server(500, &body);
    let session = session_at(&url);
    let err = session
        .post(&[], json!([]))
        .expect_err("an error status must be reported as an error");
    assert_eq!(
        err,
        format!(
            "Server returned 500 Internal Server Error: {}...",
            "世".repeat(166)
        ),
        "the snippet must stop on a char boundary below the byte limit"
    );
}

/// The malformed-success-body message uses the same clipping helper, so a
/// multi-byte body must not panic there either.
#[test]
fn jmap_post_clips_a_multibyte_malformed_body_on_a_char_boundary() {
    ensure_rustls_provider();
    let body = "世".repeat(200);
    let url = spawn_fixed_response_mock_server(200, &body);
    let session = session_at(&url);
    let err = session
        .post(&[], json!([]))
        .expect_err("a body that is not JSON must be reported as an error");
    let snippet = "世".repeat(166);
    assert!(
        err.starts_with("Failed to parse response as JSON: "),
        "got: {err}"
    );
    assert!(
        err.ends_with(&format!(": {snippet}...")),
        "the echoed snippet must stop on a char boundary, got: {err}"
    );
}

#[test]
fn jmap_session_reports_capabilities() {
    ensure_rustls_provider();
    let url = spawn_mock_server(SESSION_ONLY);
    let session = session_at(&url);
    assert!(session.has_capability("urn:ietf:params:jmap:mail"));
    assert!(!session.has_capability("urn:example:not-a-capability"));
}

#[test]
fn jmap_account_id_falls_back_to_the_core_account() {
    ensure_rustls_provider();
    let url = spawn_mock_server(
        r#"{"apiUrl": "{API_URL}", "primaryAccounts": {"urn:ietf:params:jmap:core": "acc1"}}"#,
    );
    let mut session = session_at(&url);
    assert_eq!(
        session.account_id("urn:ietf:params:jmap:mail"),
        "acc1".to_string()
    );
}

#[test]
fn jmap_account_id_is_empty_when_no_primary_account_matches() {
    ensure_rustls_provider();
    let url = spawn_mock_server(
        r#"{"apiUrl": "{API_URL}", "primaryAccounts": {"urn:ietf:params:jmap:vacationresponse": "acc1"}}"#,
    );
    let mut session = session_at(&url);
    assert_eq!(
        session.account_id("urn:ietf:params:jmap:mail"),
        String::new(),
        "neither the capability nor the core account exists"
    );
    assert_eq!(
        session.account_id("urn:ietf:params:jmap:vacationresponse"),
        "acc1".to_string()
    );
}
