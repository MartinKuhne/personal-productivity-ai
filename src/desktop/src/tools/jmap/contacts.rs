//! JMAP Contact operations.
//!
//! Uses the core JMAP protocol (RFC 8620) for contact data management
//! via the `urn:ietf:params:jmap:contacts` capability.
//! Error handling follows RFC 8620 §3.6.2.

use crate::config::AppConfig;
use serde_json::Value;

use super::client::{get_account_id, get_jmap_session, jmap_call, jmap_check_errors};

pub fn tool_search_contact(
    config: &AppConfig,
    keyword: &str,
) -> Result<crate::tools::dtos::SearchContactResponse, String> {
    let mut all_results = Vec::new();
    for (name, client) in &config.jmap_clients {
        let (api_url, token, accs) = match get_jmap_session(client) {
            Ok(s) => s,
            Err(e) => {
                all_results.push(format!("Error fetching JMAP session for {}: {}", name, e));
                continue;
            }
        };
        let account_id = get_account_id(&accs, "urn:ietf:params:jmap:contacts");
        let calls = serde_json::json!([
            ["Contact/query", { "accountId": account_id, "filter": { "text": keyword } }, "0"],
            ["Contact/get", { "accountId": account_id, "#ids": { "resultOf": "0", "name": "Contact/query", "path": "/ids" } }, "1"]
        ]);
        match jmap_call(&api_url, &token, &["urn:ietf:params:jmap:contacts"], calls) {
            Ok(res) => {
                if let Some(err) = jmap_check_errors(&res) {
                    all_results.push(format!("Error from JMAP server for {}: {}", name, err));
                } else {
                    all_results.push(format!(
                        "--- Client: {} ---\n{}",
                        name,
                        serde_json::to_string_pretty(&res).unwrap_or_default()
                    ));
                }
            }
            Err(e) => all_results.push(format!("Error querying contacts for {}: {}", name, e)),
        }
    }
    if all_results.is_empty() {
        Err("No JMAP clients configured.".to_string())
    } else {
        Ok(crate::tools::dtos::SearchContactResponse {
            results: all_results.join("\n\n"),
        })
    }
}

pub fn tool_get_contact(
    config: &AppConfig,
    id: &str,
) -> Result<crate::tools::dtos::GetContactResponse, String> {
    let mut all_results = Vec::new();
    for (name, client) in &config.jmap_clients {
        let (api_url, token, accs) = match get_jmap_session(client) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let account_id = get_account_id(&accs, "urn:ietf:params:jmap:contacts");
        let calls =
            serde_json::json!([["Contact/get", { "accountId": account_id, "ids": [id] }, "0"]]);
        match jmap_call(&api_url, &token, &["urn:ietf:params:jmap:contacts"], calls) {
            Ok(res) => {
                if let Some(err) = jmap_check_errors(&res) {
                    all_results.push(format!("Error from JMAP server for {}: {}", name, err));
                } else {
                    all_results.push(format!(
                        "--- Client: {} ---\n{}",
                        name,
                        serde_json::to_string_pretty(&res).unwrap_or_default()
                    ));
                }
            }
            Err(_) => continue,
        }
    }
    if all_results.is_empty() {
        Err("Contact not found in any client or no clients configured.".to_string())
    } else {
        Ok(crate::tools::dtos::GetContactResponse {
            result: all_results.join("\n\n"),
        })
    }
}

pub fn tool_add_contact(
    config: &AppConfig,
    contact_json: &str,
) -> Result<crate::tools::dtos::AddContactResponse, String> {
    let mut all_results = Vec::new();
    if let Some((name, client)) = config.jmap_clients.iter().next() {
        let (api_url, token, accs) = match get_jmap_session(client) {
            Ok(s) => s,
            Err(e) => {
                all_results.push(format!("Error fetching JMAP session for {}: {}", name, e));
                return Err(all_results.join("\n\n"));
            }
        };
        let account_id = get_account_id(&accs, "urn:ietf:params:jmap:contacts");
        let item: Value = match serde_json::from_str(contact_json) {
            Ok(i) => i,
            Err(e) => return Err(format!("Invalid JSON: {}", e)),
        };
        let calls = serde_json::json!([["Contact/set", { "accountId": account_id, "create": { "new_contact_1": item } }, "0"]]);
        match jmap_call(&api_url, &token, &["urn:ietf:params:jmap:contacts"], calls) {
            Ok(res) => {
                if let Some(err) = jmap_check_errors(&res) {
                    all_results.push(format!("Error from JMAP server for {}: {}", name, err));
                } else {
                    all_results.push(format!(
                        "--- Client: {} ---\n{}",
                        name,
                        serde_json::to_string_pretty(&res).unwrap_or_default()
                    ));
                }
            }
            Err(e) => all_results.push(format!("Error creating contact in {}: {}", name, e)),
        }
    }
    if all_results.is_empty() {
        Err("No JMAP clients configured.".to_string())
    } else {
        Ok(crate::tools::dtos::AddContactResponse {
            result: all_results.join("\n\n"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, JmapClient};
    use std::collections::HashMap;

    fn spawn_mock_server(body: impl Into<String>) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let api_url = format!("http://127.0.0.1:{}", port);
        let body_str = body.into().replace("{API_URL}", &api_url);
        let response_str = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}", body_str.len(), body_str);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                if let Ok(mut stream) = stream {
                    use std::io::{Read, Write};
                    let mut buf = [0; 4096];
                    let _ = stream.read(&mut buf);
                    let _ = stream.write_all(response_str.as_bytes());
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
            }
        });
        format!("http://127.0.0.1:{}", port)
    }

    fn mock_config(api_url: &str) -> AppConfig {
        let mut clients = HashMap::new();
        clients.insert("test_jmap".to_string(), JmapClient {
            url: api_url.to_string(),
            token: "test_token".to_string(),
        });
        AppConfig {
            jmap_clients: clients,
            ..AppConfig::default()
        }
    }

    #[test]
    fn test_contact_operations_success() {
        rustls::crypto::ring::default_provider().install_default().ok();
        let json_resp = serde_json::json!({
            "apiUrl": "{API_URL}",
            "primaryAccounts": {
                "urn:ietf:params:jmap:contacts": "acc-1"
            },
            "methodResponses": [
                ["Contact/get", { "list": [{"id": "c-1", "firstName": "Alice"}] }, "1"],
                ["Contact/set", { "created": {"new_contact_1": {"id": "new-id"}} }, "0"]
            ]
        });
        let url = spawn_mock_server(serde_json::to_string(&json_resp).unwrap());
        let config = mock_config(&url);

        let res_search = tool_search_contact(&config, "alice");
        assert!(res_search.is_ok());

        let res_item = tool_get_contact(&config, "c-1");
        assert!(res_item.is_ok());

        let res_add = tool_add_contact(&config, r#"{"firstName": "New"}"#);
        assert!(res_add.is_ok());
    }

    #[test]
    fn test_contact_operations_errors() {
        rustls::crypto::ring::default_provider().install_default().ok();
        let json_resp = serde_json::json!({
            "apiUrl": "{API_URL}",
            "primaryAccounts": {
                "urn:ietf:params:jmap:contacts": "acc-1"
            },
            "methodResponses": [
                ["error", { "type": "serverError", "description": "mock error" }, "0"]
            ]
        });
        let url = spawn_mock_server(serde_json::to_string(&json_resp).unwrap());
        let config = mock_config(&url);

        let res_search = tool_search_contact(&config, "alice");
        assert!(res_search.is_ok());
        assert!(res_search.unwrap().results.contains("Error from JMAP server"));

        let res_add_err = tool_add_contact(&config, "{invalid json}");
        assert!(res_add_err.is_err());
        assert!(res_add_err.unwrap_err().contains("Invalid JSON"));
    }

    #[test]
    fn test_contact_operations_no_clients() {
        let config = AppConfig::default();
        assert!(tool_search_contact(&config, "alice").is_err());
        assert!(tool_get_contact(&config, "id").is_err());
        assert!(tool_add_contact(&config, "{}").is_err());
    }

    #[test]
    fn test_contact_operations_session_error() {
        rustls::crypto::ring::default_provider().install_default().ok();
        let url = spawn_mock_server("HTTP/1.1 401 Unauthorized\r\nContent-Length: 5\r\n\r\nerror");
        let config = mock_config(&url);
        
        let res_search = tool_search_contact(&config, "alice");
        assert!(res_search.unwrap().results.contains("Error fetching JMAP session"));

        let res_add = tool_add_contact(&config, "{}");
        assert!(res_add.is_err());
        assert!(res_add.unwrap_err().contains("Error fetching JMAP session"));
    }
}
