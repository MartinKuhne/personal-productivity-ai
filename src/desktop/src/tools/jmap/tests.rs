//! Unit tests for the JMAP client's `jmap_check_errors` function.

use super::client::jmap_check_errors;
use serde_json::json;

// -- jmap_check_errors tests --

#[test]
fn test_check_no_method_responses() {
    let res = json!({});
    assert_eq!(jmap_check_errors(&res), None);
}

#[test]
fn test_check_empty_method_responses() {
    let res = json!({ "methodResponses": [] });
    assert_eq!(jmap_check_errors(&res), None);
}

#[test]
fn test_check_no_error() {
    let res = json!({
        "methodResponses": [
            ["Contact/query", { "ids": ["1", "2"] }, "0"]
        ]
    });
    assert_eq!(jmap_check_errors(&res), None);
}

#[test]
fn test_check_multiple_no_errors() {
    let res = json!({
        "methodResponses": [
            ["Contact/query", { "ids": [] }, "0"],
            ["Contact/get", { "list": [] }, "1"]
        ]
    });
    assert_eq!(jmap_check_errors(&res), None);
}

#[test]
fn test_check_error_type_only() {
    let res = json!({
        "methodResponses": [
            ["error", { "type": "unknownMethod" }, "c1"]
        ]
    });
    assert_eq!(
        jmap_check_errors(&res),
        Some("type: unknownMethod (callId: c1)".to_string())
    );
}

#[test]
fn test_check_error_with_description() {
    let res = json!({
        "methodResponses": [
            ["error", { "type": "unknownMethod", "description": "Method not recognized by server" }, "c1"]
        ]
    });
    assert_eq!(
        jmap_check_errors(&res),
        Some("type: unknownMethod: Method not recognized by server (callId: c1)".to_string())
    );
}

#[test]
fn test_check_error_type_forbidden() {
    let res = json!({
        "methodResponses": [
            ["error", { "type": "forbidden" }, "c1"]
        ]
    });
    assert_eq!(
        jmap_check_errors(&res),
        Some("type: forbidden (callId: c1)".to_string())
    );
}

#[test]
fn test_check_error_type_request_too_large() {
    let res = json!({
        "methodResponses": [
            ["error", { "type": "requestTooLarge" }, "c1"]
        ]
    });
    assert_eq!(
        jmap_check_errors(&res),
        Some("type: requestTooLarge (callId: c1)".to_string())
    );
}

#[test]
fn test_check_error_no_call_id() {
    let res = json!({
        "methodResponses": [
            ["error", { "type": "unknownMethod" }]
        ]
    });
    assert_eq!(
        jmap_check_errors(&res),
        Some("type: unknownMethod (callId: ?)".to_string())
    );
}

#[test]
fn test_check_error_first_of_multiple() {
    let res = json!({
        "methodResponses": [
            ["error", { "type": "unknownMethod" }, "c1"],
            ["Contact/query", { "ids": [] }, "0"]
        ]
    });
    assert_eq!(
        jmap_check_errors(&res),
        Some("type: unknownMethod (callId: c1)".to_string())
    );
}

#[test]
fn test_check_error_after_valid_response() {
    let res = json!({
        "methodResponses": [
            ["Email/get", { "list": [] }, "0"],
            ["error", { "type": "requestTooLarge" }, "1"]
        ]
    });
    assert_eq!(
        jmap_check_errors(&res),
        Some("type: requestTooLarge (callId: 1)".to_string())
    );
}

#[test]
fn test_check_error_missing_type_field() {
    let res = json!({
        "methodResponses": [
            ["error", { "description": "something broke" }, "c1"]
        ]
    });
    let msg = jmap_check_errors(&res).unwrap();
    assert!(msg.contains("type: unknown"));
    assert!(msg.contains("something broke"));
    assert!(msg.contains("callId: c1"));
}

// -- Malformed / edge cases --

#[test]
fn test_check_method_responses_not_array() {
    let res = json!({ "methodResponses": "not_an_array" });
    assert_eq!(jmap_check_errors(&res), None);
}

#[test]
fn test_check_response_item_not_array() {
    let res = json!({
        "methodResponses": [
            "not_an_array"
        ]
    });
    assert_eq!(jmap_check_errors(&res), None);
}

#[test]
fn test_check_response_first_element_not_string() {
    let res = json!({
        "methodResponses": [
            [42, { "type": "unknownMethod" }, "c1"]
        ]
    });
    assert_eq!(jmap_check_errors(&res), None);
}

#[test]
fn test_check_response_empty_inner_array() {
    let res = json!({
        "methodResponses": [
            []
        ]
    });
    assert_eq!(jmap_check_errors(&res), None);
}

#[test]
fn test_check_response_with_extra_fields_in_error_obj() {
    let res = json!({
        "methodResponses": [
            ["error", { "type": "invalidArguments", "description": "bad params", "details": { "field": "accountId" } }, "c1"]
        ]
    });
    assert_eq!(
        jmap_check_errors(&res),
        Some("type: invalidArguments: bad params (callId: c1)".to_string())
    );
}

// -- Email inline error pattern tests --

/// Helper that simulates the inline error check pattern used in
/// tool_search_email and tool_get_email_by_id.
fn email_inline_check(res: &serde_json::Value, name: &str) -> Option<String> {
    if let Some(method_responses) = res.get("methodResponses").and_then(|mr| mr.as_array()) {
        for resp in method_responses {
            if let Some(resp_arr) = resp.as_array() {
                if resp_arr.get(0).and_then(|s| s.as_str()) == Some("error") {
                    return Some(format!(
                        "Error from JMAP server for {}: {}",
                        name,
                        serde_json::to_string_pretty(resp_arr).unwrap_or_default()
                    ));
                }
            }
        }
    }
    None
}

#[test]
fn test_email_inline_no_error() {
    let res = json!({
        "methodResponses": [
            ["Email/get", { "list": [] }, "0"]
        ]
    });
    assert_eq!(email_inline_check(&res, "test"), None);
}

#[test]
fn test_email_inline_detects_error() {
    let res = json!({
        "methodResponses": [
            ["Email/get", { "list": [] }, "0"],
            ["error", { "type": "requestTooLarge" }, "1"]
        ]
    });
    let msg = email_inline_check(&res, "MyClient").unwrap();
    assert!(msg.contains("Error from JMAP server for MyClient"));
    assert!(msg.contains("requestTooLarge"));
}

#[test]
fn test_email_inline_detects_unknown_method() {
    let res = json!({
        "methodResponses": [
            ["error", { "type": "unknownMethod" }, "0"]
        ]
    });
    let msg = email_inline_check(&res, "test").unwrap();
    assert!(msg.contains("unknownMethod"));
}
