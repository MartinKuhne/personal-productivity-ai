//! Tests for the JMAP transport layer (`agent::tools::jmap::client`):
//! method-response error scanning, RFC 7807 / JMAP error-detail parsing,
//! session connect failures and the mock-server happy paths.

use serde_json::json;

use super::{JmapSession, jmap_check_errors, parse_error_detail};
use crate::config::JmapClient;
use crate::tools::jmap::mock_server::spawn_mock_server;

#[test]
fn test_jmap_check_errors_no_errors() {
    let res = json!({
        "methodResponses": [
            ["Email/get", { "list": [] }, "0"]
        ]
    });
    assert_eq!(jmap_check_errors(&res), None);
}

#[test]
fn test_jmap_check_errors_with_description() {
    let res = json!({
        "methodResponses": [
            ["error", { "type": "unknownMethod", "description": "The method is unknown" }, "call-1"]
        ]
    });
    assert_eq!(
        jmap_check_errors(&res),
        Some("type: unknownMethod: The method is unknown (callId: call-1)".to_string())
    );
}

#[test]
fn test_jmap_check_errors_without_description() {
    let res = json!({
        "methodResponses": [
            ["error", { "type": "invalidArguments" }, "call-2"]
        ]
    });
    assert_eq!(
        jmap_check_errors(&res),
        Some("type: invalidArguments (callId: call-2)".to_string())
    );
}

#[test]
fn test_jmap_check_errors_missing_method_responses() {
    let res = json!({ "session": "something" });
    assert_eq!(jmap_check_errors(&res), None);
}

#[test]
fn test_jmap_check_errors_multiple_responses_one_error() {
    let res = json!({
        "methodResponses": [
            ["Email/get", { "list": [] }, "0"],
            ["error", { "type": "accountNotFound" }, "1"]
        ]
    });
    assert_eq!(
        jmap_check_errors(&res),
        Some("type: accountNotFound (callId: 1)".to_string())
    );
}

#[test]
fn test_parse_error_detail_rfc7807_detail_wins() {
    let body = r#"{"type":"about:blank","title":"Bad Thing","detail":"The specific problem","status":400}"#;
    assert_eq!(
        parse_error_detail(body),
        Some("The specific problem".to_string())
    );
}

#[test]
fn test_parse_error_detail_falls_back_to_title() {
    let body = r#"{"type":"about:blank","title":"A title","status":500}"#;
    assert_eq!(parse_error_detail(body), Some("A title".to_string()));
}

#[test]
fn test_parse_error_detail_jmap_type_and_description() {
    let body = r#"{"type":"invalidArguments","description":"arg was wrong"}"#;
    assert_eq!(
        parse_error_detail(body),
        Some("invalidArguments: arg was wrong".to_string())
    );
}

#[test]
fn test_parse_error_detail_jmap_type_only() {
    let body = r#"{"type":"accountNotFound"}"#;
    assert_eq!(
        parse_error_detail(body),
        Some("accountNotFound".to_string())
    );
}

#[test]
fn test_parse_error_detail_non_json_returns_none() {
    assert_eq!(parse_error_detail("this is not json"), None);
}

#[test]
fn test_parse_error_detail_non_object_returns_none() {
    assert_eq!(parse_error_detail(r#"["an","array"]"#), None);
    assert_eq!(parse_error_detail(r#"42"#), None);
}

#[test]
fn test_parse_error_detail_empty_object_returns_none() {
    assert_eq!(parse_error_detail(r#"{}"#), None);
}

#[test]
fn test_jmap_connect_failure_returns_error() {
    let cfg = JmapClient {
        url: "http://127.0.0.1:1".to_string(),
        token: "token".to_string(),
    };
    let result = JmapSession::connect(&cfg);
    assert!(result.is_err());
    let err = result.err().unwrap();
    assert!(err.contains("Failed to connect to JMAP server"));
}

#[test]
fn test_jmap_post_happy_path_via_mock_server() {
    let body = json!({
        "apiUrl": "{API_URL}",
        "primaryAccounts": {"urn:ietf:params:jmap:mail": "acc1"},
        "methodResponses": [
            ["Email/get", {"list": [{"id": "e1"}]}, "0"]
        ]
    })
    .to_string();
    let url = spawn_mock_server(body);
    let cfg = JmapClient {
        url,
        token: "token".to_string(),
    };
    let session = JmapSession::connect(&cfg).expect("connect should succeed");
    let response = session
        .post(
            &["urn:ietf:params:jmap:mail"],
            json!([["Email/get", {"ids": ["e1"]}, "0"]]),
        )
        .expect("post should succeed");
    let list = response
        .get("methodResponses")
        .and_then(|mr| mr.as_array())
        .and_then(|arr| arr.first())
        .and_then(|first| first.as_array())
        .and_then(|arr| arr.get(1))
        .and_then(|args| args.get("list"))
        .and_then(|l| l.as_array());
    assert_eq!(list.map(|l| l.len()), Some(1));
}

#[test]
fn test_jmap_account_id_resolves_from_mock() {
    let body = json!({
        "apiUrl": "{API_URL}",
        "primaryAccounts": {"urn:ietf:params:jmap:mail": "acc1"}
    })
    .to_string();
    let url = spawn_mock_server(body);
    let cfg = JmapClient {
        url,
        token: "token".to_string(),
    };
    let mut session = JmapSession::connect(&cfg).expect("connect should succeed");
    assert_eq!(
        session.account_id("urn:ietf:params:jmap:mail"),
        "acc1".to_string()
    );
    assert_eq!(session.account_id("urn:ietf:params:jmap:mail"), "acc1");
}
