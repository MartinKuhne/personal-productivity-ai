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
}
