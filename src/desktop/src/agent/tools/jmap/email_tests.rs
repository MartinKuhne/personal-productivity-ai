//! Tests for `jmap/email.rs`.
//!
//! Sidecar file. Extracted from `email.rs` so the implementation
//! module stays focused on production code.
//!
//! Originally a `#[cfg(test)] mod tests { ... }` block at the bottom of
//! `email.rs`. Lives in a sibling file so private item access via
//! `super::*` keeps working.

use serde_json::json;

use super::{convert_html_in_jmap, simplify_jmap_emails};

#[test]
fn test_convert_html_plain_text_unchanged() {
    let _cache = crate::agent::tools::manager::cache::ToolCache::new();
    let res = json!({
        "methodResponses": [[
            "Email/get",
            {
                "list": [{
                    "id": "1",
                    "bodyValues": {
                        "part1": { "value": "Hello, world!", "isTruncated": false }
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
    assert_eq!(val, "Hello, world!");
}

#[test]
fn test_convert_html_converts_simple_html() {
    let _cache = crate::agent::tools::manager::cache::ToolCache::new();
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
    let _cache = crate::agent::tools::manager::cache::ToolCache::new();
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
    let _cache = crate::agent::tools::manager::cache::ToolCache::new();
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

#[test]
fn test_convert_html_empty_body_values() {
    let _cache = crate::agent::tools::manager::cache::ToolCache::new();
    let res = json!({
        "methodResponses": [[
            "Email/get",
            {
                "list": [{ "id": "1", "bodyValues": {} }]
            },
            "0"
        ]]
    });
    convert_html_in_jmap(res);
}

#[test]
fn test_convert_html_value_missing_angle_brackets_not_converted() {
    let _cache = crate::agent::tools::manager::cache::ToolCache::new();
    let res = json!({
        "methodResponses": [[
            "Email/get",
            {
                "list": [{
                    "id": "1",
                    "bodyValues": {
                        "part1": { "value": "Hello World", "isTruncated": false }
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
    assert_eq!(val, "Hello World");
}

#[test]
fn test_convert_html_non_string_value_not_converted() {
    let _cache = crate::agent::tools::manager::cache::ToolCache::new();
    let res = json!({
        "methodResponses": [[
            "Email/get",
            {
                "list": [{
                    "id": "1",
                    "bodyValues": {
                        "part1": { "value": 42, "isTruncated": false }
                    }
                }]
            },
            "0"
        ]]
    });
    convert_html_in_jmap(res);
}

#[test]
fn test_simplify_empty_method_responses() {
    let _cache = crate::agent::tools::manager::cache::ToolCache::new();
    let res = json!({ "methodResponses": [] });
    let result = simplify_jmap_emails(res, None);
    assert_eq!(result, json!([]));
}

#[test]
fn test_simplify_no_email_get_method() {
    let _cache = crate::agent::tools::manager::cache::ToolCache::new();
    let res = json!({
        "methodResponses": [[
            "Contact/query", { "ids": [] }, "0"
        ]]
    });
    let result = simplify_jmap_emails(res, None);
    assert_eq!(result, json!([]));
}

#[test]
fn test_simplify_email_get_empty_list() {
    let _cache = crate::agent::tools::manager::cache::ToolCache::new();
    let res = json!({
        "methodResponses": [[
            "Email/get",
            { "accountId": "a1", "list": [], "notFound": [] },
            "0"
        ]]
    });
    let result = simplify_jmap_emails(res, None);
    assert_eq!(result, json!([]));
}

#[test]
fn test_simplify_single_email_html_body() {
    let _cache = crate::agent::tools::manager::cache::ToolCache::new();
    let res = json!({
        "methodResponses": [[
            "Email/get",
            {
                "accountId": "a1",
                "list": [{
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
                }],
                "notFound": []
            },
            "0"
        ]]
    });
    let result = simplify_jmap_emails(res, None);
    let arr = result.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], "email-1");
    assert_eq!(arr[0]["subject"], "Hello");
    assert_eq!(arr[0]["body"], "Hello Bob!");
}

#[test]
fn test_simplify_email_text_body_fallback() {
    let _cache = crate::agent::tools::manager::cache::ToolCache::new();
    let res = json!({
        "methodResponses": [[
            "Email/get",
            {
                "accountId": "a1",
                "list": [{
                    "id": "email-2",
                    "subject": "No HTML",
                    "receivedAt": "2026-07-19T11:00:00Z",
                    "from": [{ "name": "Charlie", "email": "charlie@test.com" }],
                    "to": [{ "name": "Dave", "email": "dave@test.com" }],
                    "textBody": [{ "partId": "tp1" }],
                    "bodyValues": {
                        "tp1": { "value": "Plain text body", "isTruncated": false }
                    }
                }],
                "notFound": []
            },
            "0"
        ]]
    });
    let result = simplify_jmap_emails(res, None);
    let arr = result.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["body"], "Plain text body");
}

#[test]
fn test_simplify_multiple_emails() {
    let _cache = crate::agent::tools::manager::cache::ToolCache::new();
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

#[test]
fn test_simplify_truncates_body_to_max_lines() {
    let _cache = crate::agent::tools::manager::cache::ToolCache::new();
    let res = json!({
        "methodResponses": [[
            "Email/get",
            {
                "accountId": "a1",
                "list": [{
                    "id": "e1",
                    "subject": "Long body",
                    "receivedAt": "2026-07-19T10:00:00Z",
                    "from": [{"email": "a@t.com"}],
                    "to": [{"email": "b@t.com"}],
                    "htmlBody": [{"partId": "p1"}],
                    "bodyValues": { "p1": { "value": "Line 1\nLine 2\nLine 3\nLine 4\nLine 5", "isTruncated": false } }
                }],
                "notFound": []
            },
            "0"
        ]]
    });
    let result = simplify_jmap_emails(res, Some(3));
    let body = result[0]["body"].as_str().unwrap();
    assert!(body.starts_with("Line 1\nLine 2\nLine 3"));
    assert!(body.contains("truncated"));
}

#[test]
fn test_simplify_truncated_body_appends_hint() {
    let _cache = crate::agent::tools::manager::cache::ToolCache::new();
    let res = json!({
        "methodResponses": [[
            "Email/get",
            {
                "accountId": "a1",
                "list": [{
                    "id": "e1",
                    "subject": "Truncated",
                    "receivedAt": "2026-07-19T10:00:00Z",
                    "from": [{"email": "a@t.com"}],
                    "to": [{"email": "b@t.com"}],
                    "htmlBody": [{"partId": "p1"}],
                    "bodyValues": { "p1": { "value": "Line 1\nLine 2\nLine 3\nLine 4", "isTruncated": false } }
                }],
                "notFound": []
            },
            "0"
        ]]
    });
    let result = simplify_jmap_emails(res, Some(2));
    let body = result[0]["body"].as_str().unwrap();
    assert!(body.contains("truncated"));
}

#[test]
fn test_simplify_body_not_truncated_if_under_limit() {
    let _cache = crate::agent::tools::manager::cache::ToolCache::new();
    let res = json!({
        "methodResponses": [[
            "Email/get",
            {
                "accountId": "a1",
                "list": [{
                    "id": "e1",
                    "subject": "Short",
                    "receivedAt": "2026-07-19T10:00:00Z",
                    "from": [{"email": "a@t.com"}],
                    "to": [{"email": "b@t.com"}],
                    "htmlBody": [{"partId": "p1"}],
                    "bodyValues": { "p1": { "value": "Just one line", "isTruncated": false } }
                }],
                "notFound": []
            },
            "0"
        ]]
    });
    let result = simplify_jmap_emails(res, Some(10));
    let body = result[0]["body"].as_str().unwrap();
    assert!(!body.contains("truncated"));
    assert_eq!(body, "Just one line");
}

#[test]
fn test_simplify_handles_missing_optional_fields() {
    let _cache = crate::agent::tools::manager::cache::ToolCache::new();
    let res = json!({
        "methodResponses": [[
            "Email/get",
            {
                "accountId": "a1",
                "list": [{
                    "id": "e1"
                }],
                "notFound": []
            },
            "0"
        ]]
    });
    let result = simplify_jmap_emails(res, None);
    let arr = result.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], "e1");
    assert_eq!(arr[0]["subject"], serde_json::Value::Null);
}

#[test]
fn test_simplify_handles_server_truncated_body() {
    let _cache = crate::agent::tools::manager::cache::ToolCache::new();
    let res = json!({
        "methodResponses": [[
            "Email/get",
            {
                "accountId": "a1",
                "list": [{
                    "id": "e1",
                    "subject": "Server truncated",
                    "receivedAt": "2026-07-19T10:00:00Z",
                    "from": [{"email": "a@t.com"}],
                    "to": [{"email": "b@t.com"}],
                    "htmlBody": [{"partId": "p1"}],
                    "bodyValues": { "p1": { "value": "Partial body here...", "isTruncated": true } }
                }],
                "notFound": []
            },
            "0"
        ]]
    });
    let result = simplify_jmap_emails(res, None);
    let body = result[0]["body"].as_str().unwrap();
    assert!(body.contains("truncated"));
}

#[test]
fn test_simplify_cc_and_bcc_preserved() {
    let _cache = crate::agent::tools::manager::cache::ToolCache::new();
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

use super::{SearchEmailFilters, tool_get_email_by_id, tool_search_email, tool_send_email};
use crate::agent::tools::jmap::mock_server::{spawn_mock_server, spawn_recording_mock_server};
use crate::config::{AppConfig, JmapClient};
#[test]
fn test_tool_search_email_no_clients() {
    let cache = crate::agent::tools::manager::cache::ToolCache::new();
    let config = AppConfig::default();
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
fn test_tool_search_email_success() {
    let cache = crate::agent::tools::manager::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let body = "{\
            \"apiUrl\": \"{API_URL}\",\
            \"primaryAccounts\": {\"urn:ietf:params:jmap:mail\": \"acc1\"},\
            \"methodResponses\": [\
                [\"Email/query\", {\"ids\": [\"e1\"]}, \"0\"],\
                [\"Email/get\", {\"list\": [{\"id\": \"e1\", \"subject\": \"Test\"}]}, \"1\"]\
            ]\
        }";
    let url = spawn_mock_server(body);
    let mut config = AppConfig::default();
    config.jmap_clients.insert(
        "test".to_string(),
        JmapClient {
            url,
            token: "tok".to_string(),
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
}

#[test]
fn test_tool_get_email_by_id_success() {
    let _cache = crate::agent::tools::manager::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let body = "{\
            \"apiUrl\": \"{API_URL}\",\
            \"primaryAccounts\": {\"urn:ietf:params:jmap:mail\": \"acc1\"},\
            \"methodResponses\": [\
                [\"Email/get\", {\"list\": [{\"id\": \"e1\", \"subject\": \"Test\"}]}, \"0\"]\
            ]\
        }";
    let url = spawn_mock_server(body);
    let mut config = AppConfig::default();
    config.jmap_clients.insert(
        "test".to_string(),
        JmapClient {
            url,
            token: "tok".to_string(),
        },
    );
    let res = tool_get_email_by_id(&config, "e1");
    assert!(res.is_ok(), "Error: {}", res.unwrap_err());
}

/// Regression: `Email/get` MUST be sent with the body-fetching arguments
/// (`fetchTextBodyValues`, `fetchHTMLBodyValues`, `maxBodyValueBytes`)
/// per RFC 8621 §6.1.2, otherwise a real JMAP server returns empty
/// `bodyValues` (default `maxBodyValueBytes` is 0).
#[test]
fn test_tool_get_email_by_id_sends_body_value_args() {
    let _cache = crate::agent::tools::manager::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let body = "{\
            \"apiUrl\": \"{API_URL}\",\
            \"primaryAccounts\": {\"urn:ietf:params:jmap:mail\": \"acc1\"},\
            \"methodResponses\": [\
                [\"Email/get\", {\"list\": [{\"id\": \"e1\", \"subject\": \"Test\"}]}, \"0\"]\
            ]\
        }";
    let (url, recorder) = spawn_recording_mock_server(body);
    let mut config = AppConfig::default();
    config.jmap_clients.insert(
        "test".to_string(),
        JmapClient {
            url,
            token: "tok".to_string(),
        },
    );
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
    let _cache = crate::agent::tools::manager::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
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
    let mut config = AppConfig::default();
    config.jmap_clients.insert(
        "test".to_string(),
        JmapClient {
            url,
            token: "tok".to_string(),
        },
    );
    let res = tool_get_email_by_id(&config, "e1").expect("tool call should succeed");
    assert!(
        res.result.contains("Hello, world!"),
        "tool_get_email_by_id should surface body content; got: {}",
        res.result
    );
}

#[test]
fn test_tool_search_email_with_status_filters_success() {
    let cache = crate::agent::tools::manager::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let body = "{\
            \"apiUrl\": \"{API_URL}\",\
            \"primaryAccounts\": {\"urn:ietf:params:jmap:mail\": \"acc1\"},\
            \"methodResponses\": [\
                [\"Email/query\", {\"ids\": [\"e1\"]}, \"0\"],\
                [\"Email/get\", {\"list\": [{\"id\": \"e1\", \"subject\": \"Unread\"}]}, \"1\"]\
            ]\
        }";
    let url = spawn_mock_server(body);
    let mut config = AppConfig::default();
    config.jmap_clients.insert(
        "test".to_string(),
        JmapClient {
            url,
            token: "tok".to_string(),
        },
    );
    let res = tool_search_email(
        &config,
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
        None,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
    );
    assert!(res.is_ok());
}

#[test]
fn test_tool_send_email_success() {
    let _cache = crate::agent::tools::manager::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let body = "{\
            \"apiUrl\": \"{API_URL}\",\
            \"primaryAccounts\": {\"urn:ietf:params:jmap:mail\": \"acc1\", \"urn:ietf:params:jmap:submission\": \"acc1\"},\
            \"methodResponses\": [\
                [\"Mailbox/query\", {\"ids\": [\"inbox-id\"]}, \"0\"],\
                [\"Identity/get\", {\"state\": \"id-state-0\", \"list\": [{\"id\": \"ident-1\", \"email\": \"sender@test.com\"}], \"notFound\": []}, \"1\"],\
                [\"Email/set\", {\"created\": {\"c0\": {\"id\": \"email-1\"}}}, \"2\"],\
                [\"EmailSubmission/set\", {\"created\": {\"c0\": {\"id\": \"sub-1\"}}}, \"3\"]\
            ]\
        }";
    let url = spawn_mock_server(body);
    let mut config = AppConfig::default();
    config.jmap_clients.insert(
        "test".to_string(),
        JmapClient {
            url,
            token: "tok".to_string(),
        },
    );
    let res = tool_send_email(&config, "to@test.com", "Subject", "Body");
    assert!(res.is_ok(), "Error: {}", res.unwrap_err());
}

#[test]
fn test_tool_send_email_ai_agent_footer() {
    let _cache = crate::agent::tools::manager::cache::ToolCache::new();
    use super::AI_AGENT_FOOTER;
    assert_eq!(
        AI_AGENT_FOOTER,
        "\n---\nSent by FastMD on behalf of the user"
    );
}

#[test]
fn test_tool_search_email_empty_filters_errors() {
    let cache = crate::agent::tools::manager::cache::ToolCache::new();
    let config = AppConfig::default();
    let res = tool_search_email(
        &config,
        SearchEmailFilters {
            ..Default::default()
        },
        None,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
    );
    assert!(res.is_err());
    let msg = res.unwrap_err();
    assert!(msg.contains("At least one filter field must be provided"));
}

#[test]
fn test_tool_search_email_empty_filters_with_client_errors() {
    let cache = crate::agent::tools::manager::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let mut config = AppConfig::default();
    let body = "{\"apiUrl\": \"{API_URL}\", \"primaryAccounts\": {\"urn:ietf:params:jmap:mail\": \"acc1\"}}";
    let url = spawn_mock_server(body);
    config.jmap_clients.insert(
        "test".to_string(),
        JmapClient {
            url,
            token: "tok".to_string(),
        },
    );
    let res = tool_search_email(
        &config,
        SearchEmailFilters {
            ..Default::default()
        },
        None,
        &cache,
        &crate::utils::uuid::SystemUuidGenerator,
    );
    assert!(res.is_err());
    assert!(
        res.unwrap_err()
            .contains("At least one filter field must be provided")
    );
}

#[test]
fn test_tool_search_email_first_call_small_set_returns_final_page_hint() {
    let cache = crate::agent::tools::manager::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let body = r#"{
                "apiUrl": "{API_URL}",
                "primaryAccounts": {"urn:ietf:params:jmap:mail": "acc1"},
                "methodResponses": [
                    ["Email/query", {"ids": ["e1","e2","e3"]}, "0"],
                    ["Email/get", {
                        "list": [
                            {"id": "e1", "subject": "First", "receivedAt": "2026-07-19T10:00:00Z", "from": [{"email":"a@t.com"}], "to": [{"email":"b@t.com"}], "htmlBody": [{"partId":"p1"}], "bodyValues": {"p1": {"value":"Body 1","isTruncated":false}}},
                            {"id": "e2", "subject": "Second", "receivedAt": "2026-07-19T11:00:00Z", "from": [{"email":"c@t.com"}], "to": [{"email":"d@t.com"}], "htmlBody": [{"partId":"p2"}], "bodyValues": {"p2": {"value":"Body 2","isTruncated":false}}},
                            {"id": "e3", "subject": "Third", "receivedAt": "2026-07-19T12:00:00Z", "from": [{"email":"e@t.com"}], "to": [{"email":"f@t.com"}], "htmlBody": [{"partId":"p3"}], "bodyValues": {"p3": {"value":"Body 3","isTruncated":false}}}
                        ],
                        "notFound": []
                    }, "1"]
                ]
            }"#
        .to_string();
    let url = spawn_mock_server(body);
    let mut config = AppConfig::default();
    config.jmap_clients.insert(
        "test".to_string(),
        JmapClient {
            url,
            token: "tok".to_string(),
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
    assert_eq!(response.total, 3);
    // In cursor mode, a result set that fits entirely on one
    // page emits the `Final page.` hint and omits `cursor`. The
    // 3-item result set is smaller than the page size (100),
    // so this is the final page.
    assert!(
        response.cursor.is_none(),
        "single-page result must omit cursor"
    );
    assert_eq!(
        response.hint.as_deref(),
        Some(super::SEARCH_EMAIL_FINAL_PAGE_HINT)
    );
    assert!(!response.results.is_empty());
}

#[test]
fn test_tool_search_email_cursor_unknown_returns_error() {
    let cache = crate::agent::tools::manager::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
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
    let mut config = AppConfig::default();
    config.jmap_clients.insert(
        "test".to_string(),
        JmapClient {
            url,
            token: "tok".to_string(),
        },
    );
    // A bogus cursor that does not match any live cache entry
    // must return the documented "expired or unknown" error.
    let res = tool_search_email(
        &config,
        SearchEmailFilters {
            keyword: Some("test"),
            ..Default::default()
        },
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
    let cache = crate::agent::tools::manager::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
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
    let mut config = AppConfig::default();
    config.jmap_clients.insert(
        "test".to_string(),
        JmapClient {
            url,
            token: "tok".to_string(),
        },
    );
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
    // Count items on the first page by counting unique `"id": "e` markers in
    // the rendered JSON. The mock server enriches each email with `blobId`
    // and `threadId` that contain the same `e{n}` token, so the marker must
    // match the JSON key prefix exactly to count emails.
    let first_id_count = first.results.matches(r#""id": "e"#).count();
    assert_eq!(
        first_id_count,
        super::SEARCH_EMAIL_PAGE_SIZE,
        "first page must contain exactly SEARCH_EMAIL_PAGE_SIZE items"
    );
    assert!(first.results.contains("Subject 1"));
    assert!(!first.results.contains(&format!("Subject {total_emails}")));

    // Second call: with cursor, must return remaining items + final hint.
    let second = tool_search_email(&config, filters, Some(cursor.clone()), &cache, &uuid)
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
    let second_id_count = second.results.matches(r#""id": "e"#).count();
    assert_eq!(
        second_id_count,
        total_emails - super::SEARCH_EMAIL_PAGE_SIZE,
        "second page must contain the remaining items"
    );
    assert!(!second.results.contains("Subject 1"));
    assert!(second.results.contains(&format!("Subject {total_emails}")));

    // Pages must not repeat any content lines (i.e., no email id may appear
    // on both pages). The first page must contain e1 and the last page must
    // not; the last page must contain e{total_emails} and the first must not.
    assert!(first.results.contains(r#""id": "e1""#));
    assert!(!second.results.contains(r#""id": "e1""#));
    assert!(
        !first
            .results
            .contains(&format!(r#""id": "e{total_emails}""#))
    );
    assert!(
        second
            .results
            .contains(&format!(r#""id": "e{total_emails}""#))
    );
}

#[test]
fn test_tool_search_email_pagination_hint_on_final_page() {
    let cache = crate::agent::tools::manager::cache::ToolCache::new();
    // When the first call returns a result set that fits entirely
    // on one page (here, total = 1, page size = 100), the
    // response omits `cursor` and sets `hint` to
    // `SEARCH_EMAIL_FINAL_PAGE_HINT`. There is no separate
    // "page-beyond-total" test because the cursor model makes
    // that scenario impossible: the LLM only ever holds a
    // valid cursor or no cursor at all.
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
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
    let mut config = AppConfig::default();
    config.jmap_clients.insert(
        "test".to_string(),
        JmapClient {
            url,
            token: "tok".to_string(),
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
    assert_eq!(response.total, 1);
    assert!(
        response.cursor.is_none(),
        "single-page result must omit cursor"
    );
    assert_eq!(
        response.hint.as_deref(),
        Some(super::SEARCH_EMAIL_FINAL_PAGE_HINT)
    );
}

#[test]
fn test_tool_search_email_multiple_clients() {
    let cache = crate::agent::tools::manager::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
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
    let mut config = AppConfig::default();
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
    assert!(response.results.contains("client1"));
    assert!(response.results.contains("client2"));
}

#[test]
fn test_tool_search_email_logs_tracing() {
    let cache = crate::agent::tools::manager::cache::ToolCache::new();
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
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
    let mut config = AppConfig::default();
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
