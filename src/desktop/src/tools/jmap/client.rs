//! JMAP client transport layer.
//!
//! Implements the core JMAP protocol as defined in RFC 8620.
//! - Session resource: Section 2
//! - Request/Response objects: Sections 3.3-3.4
//! - Method-level error handling: Section 3.6.2
//!
//! See: <https://www.rfc-editor.org/rfc/rfc8620>

use crate::config::JmapClient;
use serde_json::Value;

pub fn get_jmap_session(client: &JmapClient) -> Result<(String, String, Value), String> {
    let url = &client.url;
    let token = &client.token;

    let session_url = if url.ends_with("/api") {
        format!("{}/session", url.strip_suffix("/api").unwrap_or(url))
    } else {
        url.to_string()
    };

    let resp = match ureq::get(&session_url)
        .set("Authorization", &format!("Bearer {}", token))
        .call()
    {
        Ok(r) => {
            println!("JMAP Status Code: {}", r.status());
            r
        }
        Err(e) => {
            if let ureq::Error::Status(code, response) = e {
                let body = response.into_string().unwrap_or_default();
                return Err(format!("JMAP Error {}: {}", code, body));
            }
            return Err(e.to_string());
        }
    };

    let json: Value = resp.into_json().map_err(|e| e.to_string())?;
    let api_url = json["apiUrl"].as_str().unwrap_or(url).to_string();
    let primary_accounts = json["primaryAccounts"].clone();

    Ok((api_url, token.to_string(), primary_accounts))
}

pub fn jmap_call(
    api_url: &str,
    token: &str,
    capabilities: &[&str],
    method_calls: Value,
) -> Result<Value, String> {
    let mut using = vec!["urn:ietf:params:jmap:core"];
    using.extend(capabilities);

    let payload = serde_json::json!({
        "using": using,
        "methodCalls": method_calls
    });

    let resp = match ureq::post(api_url)
        .set("Authorization", &format!("Bearer {}", token))
        .set("Content-Type", "application/json")
        .send_json(payload)
    {
        Ok(r) => {
            println!("JMAP Status Code: {}", r.status());
            r
        }
        Err(e) => {
            if let ureq::Error::Status(code, response) = e {
                let body = response.into_string().unwrap_or_default();
                return Err(format!("JMAP Error {}: {}", code, body));
            }
            return Err(e.to_string());
        }
    };

    resp.into_json().map_err(|e| e.to_string())
}

/// Check if a JMAP response contains method-level errors (RFC 8620 §3.6.2).
///
/// JMAP error responses follow the format:
/// ```json
/// ["error", { "type": "unknownMethod", ... }, "call-id"]
/// ```
/// The `type` field is mandatory; `description` is optional.
/// Returns the first error found as a formatted string, or None.
pub fn jmap_check_errors(res: &Value) -> Option<String> {
    if let Some(method_responses) = res.get("methodResponses").and_then(|mr| mr.as_array()) {
        for resp in method_responses {
            if let Some(resp_arr) = resp.as_array() {
                if resp_arr.get(0).and_then(|s| s.as_str()) == Some("error") {
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
    }
    None
}

pub fn get_account_id(primary_accounts: &Value, cap: &str) -> String {
    primary_accounts[cap]
        .as_str()
        .or_else(|| primary_accounts["urn:ietf:params:jmap:core"].as_str())
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::get_account_id;

    #[test]
    fn test_get_account_id_exact_match() {
        let accs = json!({
            "urn:ietf:params:jmap:mail": "account-mail-123",
            "urn:ietf:params:jmap:contacts": "account-contacts-456"
        });
        assert_eq!(
            get_account_id(&accs, "urn:ietf:params:jmap:mail"),
            "account-mail-123"
        );
        assert_eq!(
            get_account_id(&accs, "urn:ietf:params:jmap:contacts"),
            "account-contacts-456"
        );
    }

    #[test]
    fn test_get_account_id_fallback_to_core() {
        let accs = json!({
            "urn:ietf:params:jmap:core": "fallback-account"
        });
        assert_eq!(
            get_account_id(&accs, "urn:ietf:params:jmap:mail"),
            "fallback-account"
        );
    }

    #[test]
    fn test_get_account_id_nothing_matches() {
        let accs = json!({
            "other:capability": "some-id"
        });
        assert_eq!(get_account_id(&accs, "urn:ietf:params:jmap:mail"), "");
    }

    #[test]
    fn test_get_account_id_empty_object() {
        let accs = json!({});
        assert_eq!(get_account_id(&accs, "urn:ietf:params:jmap:mail"), "");
    }

    #[test]
    fn test_get_account_id_cap_matches_is_preferred() {
        let accs = json!({
            "urn:ietf:params:jmap:mail": "mail-account",
            "urn:ietf:params:jmap:core": "core-account"
        });
        assert_eq!(
            get_account_id(&accs, "urn:ietf:params:jmap:mail"),
            "mail-account"
        );
    }

    #[test]
    fn test_get_account_id_null_value_in_primary_accounts() {
        let accs = json!({
            "urn:ietf:params:jmap:mail": null
        });
        assert_eq!(get_account_id(&accs, "urn:ietf:params:jmap:mail"), "");
    }

    use super::jmap_check_errors;

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
        assert_eq!(jmap_check_errors(&res), Some("type: unknownMethod: The method is unknown (callId: call-1)".to_string()));
    }

    #[test]
    fn test_jmap_check_errors_without_description() {
        let res = json!({
            "methodResponses": [
                ["error", { "type": "invalidArguments" }, "call-2"]
            ]
        });
        assert_eq!(jmap_check_errors(&res), Some("type: invalidArguments (callId: call-2)".to_string()));
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
        assert_eq!(jmap_check_errors(&res), Some("type: accountNotFound (callId: 1)".to_string()));
    }

    use std::net::TcpListener;
    use std::thread;
    use std::io::{Read, Write};
    use crate::config::JmapClient;
    use super::{get_jmap_session, jmap_call};

    fn spawn_mock_server(response: impl Into<String>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let response_str = response.into();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0; 4096];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(response_str.as_bytes());
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        });
        format!("http://127.0.0.1:{}", port)
    }

    #[test]
    fn test_get_jmap_session_success() {
        rustls::crypto::ring::default_provider().install_default().ok();
        let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"apiUrl\": \"/api\", \"primaryAccounts\": {\"core\": \"acc1\"}}";
        let url = spawn_mock_server(response);
        let client = JmapClient {
            url: url.clone(),
            token: "tok".to_string(),
        };
        let res = get_jmap_session(&client);
        assert!(res.is_ok());
        let (api_url, token, accs) = res.unwrap();
        assert_eq!(api_url, "/api");
        assert_eq!(token, "tok");
        assert_eq!(accs["core"], "acc1");
    }

    #[test]
    fn test_get_jmap_session_error_status() {
        rustls::crypto::ring::default_provider().install_default().ok();
        let response = "HTTP/1.1 401 Unauthorized\r\nContent-Length: 5\r\n\r\nerror";
        let url = spawn_mock_server(response);
        let client = JmapClient {
            url: url.clone(),
            token: "tok".to_string(),
        };
        let res = get_jmap_session(&client);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("JMAP Error 401"));
    }

    #[test]
    fn test_jmap_call_success() {
        rustls::crypto::ring::default_provider().install_default().ok();
        let body = "{\"methodResponses\": []}";
        let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}", body.len(), body);
        let url = spawn_mock_server(response);
        let res = jmap_call(&url, "token", &["cap1"], json!([]));
        assert!(res.is_ok(), "Error: {}", res.unwrap_err());
    }

    #[test]
    fn test_jmap_call_error_status() {
        rustls::crypto::ring::default_provider().install_default().ok();
        let response = "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 4\r\n\r\nfail";
        let url = spawn_mock_server(response);
        let res = jmap_call(&url, "token", &[], json!([]));
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("JMAP Error 500"));
    }
}
