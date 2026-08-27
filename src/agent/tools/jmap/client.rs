//! JMAP client transport layer.
//!
//! Wraps `jmap_client::Client` and provides convenience methods for
//! account resolution, capability checks, and raw JMAP POST requests.
//! - Session resource: Section 2
//! - Request/Response objects: Sections 3.3-3.4
//! - Method-level error handling: Section 3.6.2
//!
//! See: <https://www.rfc-editor.org/rfc/rfc8620>

use std::collections::HashMap;

use crate::config::JmapClient;

/// Attempt to extract a human-readable error detail from a response body.
///
/// Handles RFC 7807 problem+json (`{"type", "title", "detail", "status"}`),
/// JMAP method errors (`{"type", "description"}`), and falls back to None
/// for unstructured bodies.
fn parse_error_detail(body: &str) -> Option<String> {
    let val: serde_json::Value = serde_json::from_str(body).ok()?;
    let obj = val.as_object()?;

    let rfc7807_detail = obj.get("detail").and_then(|v| v.as_str());
    let rfc7807_title = obj.get("title").and_then(|v| v.as_str());
    let jmap_description = obj.get("description").and_then(|v| v.as_str());
    let error_type = obj.get("type").and_then(|v| v.as_str());

    match (error_type, rfc7807_title, rfc7807_detail, jmap_description) {
        (_, _, Some(detail), _) => Some(detail.to_string()),
        (_, Some(title), _, _) => Some(title.to_string()),
        (Some(typ), _, _, Some(desc)) => Some(format!("{typ}: {desc}")),
        (Some(typ), _, _, _) => Some(typ.to_string()),
        _ => None,
    }
}

/// Opaque handle to a connected JMAP session.
///
/// Wraps `jmap_client::Client` and provides:
/// - `connect()` — establishes a session with the JMAP server
/// - `account_id()` — resolves a primary account ID for a capability
/// - `has_capability()` — checks if the session supports a capability
/// - `inner()` — exposes the underlying `jmap_client::Client` for raw calls
/// - `post()` — sends a raw JMAP POST request for operations not covered
///   by the typed crate methods (contacts, calendar, etc.)
pub struct JmapSession {
    client: jmap_client::client::Client,
    account_cache: HashMap<String, String>,
}

impl JmapSession {
    /// Connect to a JMAP server using a bearer token.
    pub fn connect(config: &JmapClient) -> Result<Self, String> {
        let client = jmap_client::client::Client::new()
            .credentials(config.token.as_str())
            .connect(&config.url)
            .map_err(|e| format!("Failed to connect to JMAP server: {}", e))?;
        Ok(Self {
            client,
            account_cache: HashMap::new(),
        })
    }

    /// Resolve the primary account ID for a capability.
    ///
    /// Falls back to the core account ID if the specific capability has
    /// no primary account. Results are cached for subsequent calls.
    pub fn account_id(&mut self, cap: &str) -> String {
        if let Some(id) = self.account_cache.get(cap) {
            return id.clone();
        }
        let id = self
            .client
            .session()
            .primary_accounts()
            .find_map(|(capability, account_id)| {
                if capability == cap {
                    Some(account_id.to_string())
                } else {
                    None
                }
            })
            .or_else(|| {
                self.client
                    .session()
                    .primary_accounts()
                    .find_map(|(capability, account_id)| {
                        if capability == "urn:ietf:params:jmap:core" {
                            Some(account_id.to_string())
                        } else {
                            None
                        }
                    })
            })
            .unwrap_or_default();
        self.account_cache.insert(cap.to_string(), id.clone());
        id
    }

    /// Check if the session supports a given capability.
    pub fn has_capability(&self, cap: &str) -> bool {
        self.client.session().has_capability(cap)
    }

    /// Access the underlying `jmap_client::Client`.
    pub fn inner(&self) -> &jmap_client::client::Client {
        &self.client
    }

    /// Send a raw JMAP POST request using the session's authentication.
    ///
    /// This is used for JMAP methods not covered by the typed crate API
    /// (e.g., Contact/query, CalendarEvent/query). Builds the request body
    /// with the `using` array and `methodCalls`, sends via `reqwest`, and
    /// returns the parsed JSON response.
    pub fn post(
        &self,
        capabilities: &[&str],
        method_calls: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        // Build the using array from capabilities
        let mut using = vec!["urn:ietf:params:jmap:core".to_string()];
        for cap in capabilities {
            using.push(cap.to_string());
        }

        // Build the request body
        let request_body = serde_json::json!({
            "using": using,
            "methodCalls": method_calls
        });

        // Send via reqwest using the session's auth headers
        let session = self.client.session();
        let headers = self.client.headers();

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

        let response = client
            .post(session.api_url())
            .headers(headers.clone())
            .body(serde_json::to_string(&request_body).map_err(|e| format!("JSON error: {}", e))?)
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        let status = response.status();
        let body = response
            .text()
            .map_err(|e| format!("Failed to read response body: {}", e))?;

        if !status.is_success() {
            let detail = parse_error_detail(&body);
            let msg = match detail {
                Some(ref d) => format!("Server returned {status}: {d}"),
                None => {
                    let snippet = if body.len() > 500 {
                        format!("{}...", &body[..500])
                    } else {
                        body.clone()
                    };
                    if snippet.is_empty() {
                        format!("Server returned {status} (empty body)")
                    } else {
                        format!("Server returned {status}: {snippet}")
                    }
                }
            };
            tracing::error!(
                name = "jmap.post.http_error",
                status = %status.as_u16(),
                body = %body,
                "JMAP POST returned non-success HTTP status"
            );
            return Err(msg);
        }

        let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
            let snippet = if body.len() > 500 {
                format!("{}...", &body[..500])
            } else {
                body.clone()
            };
            tracing::error!(
                name = "jmap.post.parse_error",
                error = %e,
                body = %snippet,
                "Failed to parse JMAP response as JSON"
            );
            format!("Failed to parse response as JSON: {e}: {snippet}")
        })?;
        Ok(json)
    }
}

/// Check if a JMAP response contains method-level errors (RFC 8620 §3.6.2).
///
/// JMAP error responses follow the format:
/// ```json
/// ["error", { "type": "unknownMethod", ... }, "call-id"]
/// ```
/// The `type` field is mandatory; `description` is optional.
/// Returns the first error found as a formatted string, or None.
///
/// **Note:** This function is kept for use in tests and legacy inline
/// error checks. Production code should prefer typed methods from
/// `jmap_client::Client` which return `Result<T, Error>` directly.
pub fn jmap_check_errors(res: &serde_json::Value) -> Option<String> {
    if let Some(method_responses) = res.get("methodResponses").and_then(|mr| mr.as_array()) {
        for resp in method_responses {
            if let Some(resp_arr) = resp.as_array()
                && resp_arr.first().and_then(|s| s.as_str()) == Some("error")
            {
                let err_obj = resp_arr.get(1);
                let err_type = err_obj
                    .and_then(|e| e.get("type"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("unknown");
                let description = err_obj
                    .and_then(|e| e.get("description"))
                    .and_then(|d| d.as_str());
                let call_id = resp_arr.get(2).and_then(|c| c.as_str()).unwrap_or("?");
                let msg = match description {
                    Some(desc) => format!("type: {}: {} (callId: {})", err_type, desc, call_id),
                    None => format!("type: {} (callId: {})", err_type, call_id),
                };
                return Some(msg);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
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
}
