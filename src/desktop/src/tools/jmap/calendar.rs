// JMAP Calendar operations — currently unreachable (Fastmail lacks JMAP Calendar support).
// Uses the core JMAP protocol (RFC 8620) via `urn:ietf:params:jmap:calendars`.
// Error handling follows RFC 8620 §3.6.2.
// See: <https://www.rfc-editor.org/rfc/rfc8620>

use crate::config::AppConfig;
use serde_json::Value;

use super::client::{get_account_id, get_jmap_session, jmap_call, jmap_check_errors};

pub fn tool_search_calendar(
    config: &AppConfig,
    keyword: &str,
) -> Result<crate::tools::dtos::SearchCalendarResponse, String> {
    let mut all_results = Vec::new();
    for (name, client) in &config.jmap_clients {
        let (api_url, token, accs) = match get_jmap_session(client) {
            Ok(s) => s,
            Err(e) => {
                all_results.push(format!("Error fetching JMAP session for {}: {}", name, e));
                continue;
            }
        };
        let account_id = get_account_id(&accs, "urn:ietf:params:jmap:calendars");
        let calls = serde_json::json!([
            ["CalendarEvent/query", { "accountId": account_id, "filter": { "text": keyword } }, "0"],
            ["CalendarEvent/get", { "accountId": account_id, "#ids": { "resultOf": "0", "name": "CalendarEvent/query", "path": "/ids" } }, "1"]
        ]);
        match jmap_call(&api_url, &token, &["urn:ietf:params:jmap:calendars"], calls) {
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
            Err(e) => all_results.push(format!("Error querying calendar for {}: {}", name, e)),
        }
    }
    if all_results.is_empty() {
        Err("No JMAP clients configured.".to_string())
    } else {
        Ok(crate::tools::dtos::SearchCalendarResponse {
            results: all_results.join("\n\n"),
        })
    }
}

pub fn tool_get_calendar(
    config: &AppConfig,
    start: &str,
    end: &str,
) -> Result<crate::tools::dtos::GetCalendarResponse, String> {
    let mut all_results = Vec::new();
    for (name, client) in &config.jmap_clients {
        let (api_url, token, accs) = match get_jmap_session(client) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let account_id = get_account_id(&accs, "urn:ietf:params:jmap:calendars");
        let calls = serde_json::json!([
            ["CalendarEvent/query", { "accountId": account_id, "filter": { "after": start, "before": end } }, "0"],
            ["CalendarEvent/get", { "accountId": account_id, "#ids": { "resultOf": "0", "name": "CalendarEvent/query", "path": "/ids" } }, "1"]
        ]);
        match jmap_call(&api_url, &token, &["urn:ietf:params:jmap:calendars"], calls) {
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
        Err("No JMAP clients configured.".to_string())
    } else {
        Ok(crate::tools::dtos::GetCalendarResponse {
            results: all_results.join("\n\n"),
        })
    }
}

pub fn tool_get_calendar_item(
    config: &AppConfig,
    id: &str,
) -> Result<crate::tools::dtos::GetCalendarItemResponse, String> {
    let mut all_results = Vec::new();
    for (name, client) in &config.jmap_clients {
        let (api_url, token, accs) = match get_jmap_session(client) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let account_id = get_account_id(&accs, "urn:ietf:params:jmap:calendars");
        let calls = serde_json::json!([["CalendarEvent/get", { "accountId": account_id, "ids": [id] }, "0"]]);
        match jmap_call(&api_url, &token, &["urn:ietf:params:jmap:calendars"], calls) {
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
        Err("Event not found in any client or no clients configured.".to_string())
    } else {
        Ok(crate::tools::dtos::GetCalendarItemResponse {
            result: all_results.join("\n\n"),
        })
    }
}

pub fn tool_add_calendar_item(
    config: &AppConfig,
    item_json: &str,
) -> Result<crate::tools::dtos::AddCalendarItemResponse, String> {
    let mut all_results = Vec::new();
    for (name, client) in &config.jmap_clients {
        let (api_url, token, accs) = match get_jmap_session(client) {
            Ok(s) => s,
            Err(e) => {
                all_results.push(format!("Error fetching JMAP session for {}: {}", name, e));
                continue;
            }
        };
        let account_id = get_account_id(&accs, "urn:ietf:params:jmap:calendars");
        let item: Value = match serde_json::from_str(item_json) {
            Ok(i) => i,
            Err(e) => return Err(format!("Invalid JSON: {}", e)),
        };
        let calls = serde_json::json!([["CalendarEvent/set", { "accountId": account_id, "create": { "new_event_1": item } }, "0"]]);
        match jmap_call(&api_url, &token, &["urn:ietf:params:jmap:calendars"], calls) {
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
            Err(e) => all_results.push(format!("Error creating calendar event in {}: {}", name, e)),
        }
    }
    if all_results.is_empty() {
        Err("No JMAP clients configured.".to_string())
    } else {
        Ok(crate::tools::dtos::AddCalendarItemResponse {
            result: all_results.join("\n\n"),
        })
    }
}

pub fn tool_update_calendar_item(
    config: &AppConfig,
    id: &str,
    update_json: &str,
) -> Result<crate::tools::dtos::UpdateCalendarItemResponse, String> {
    let mut all_results = Vec::new();
    for (name, client) in &config.jmap_clients {
        let (api_url, token, accs) = match get_jmap_session(client) {
            Ok(s) => s,
            Err(e) => {
                all_results.push(format!("Error fetching JMAP session for {}: {}", name, e));
                continue;
            }
        };
        let account_id = get_account_id(&accs, "urn:ietf:params:jmap:calendars");
        let update: Value = match serde_json::from_str(update_json) {
            Ok(u) => u,
            Err(e) => return Err(format!("Invalid JSON: {}", e)),
        };
        let calls = serde_json::json!([["CalendarEvent/set", { "accountId": account_id, "update": { id: update } }, "0"]]);
        match jmap_call(&api_url, &token, &["urn:ietf:params:jmap:calendars"], calls) {
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
            Err(e) => all_results.push(format!("Error updating calendar event in {}: {}", name, e)),
        }
    }
    if all_results.is_empty() {
        Err("No JMAP clients configured.".to_string())
    } else {
        Ok(crate::tools::dtos::UpdateCalendarItemResponse {
            result: all_results.join("\n\n"),
        })
    }
}

pub fn tool_delete_calendar_item(
    config: &AppConfig,
    id: &str,
) -> Result<crate::tools::dtos::DeleteCalendarItemResponse, String> {
    let mut all_results = Vec::new();
    for (name, client) in &config.jmap_clients {
        let (api_url, token, accs) = match get_jmap_session(client) {
            Ok(s) => s,
            Err(e) => {
                all_results.push(format!("Error fetching JMAP session for {}: {}", name, e));
                continue;
            }
        };
        let account_id = get_account_id(&accs, "urn:ietf:params:jmap:calendars");
        let calls = serde_json::json!([["CalendarEvent/set", { "accountId": account_id, "destroy": [id] }, "0"]]);
        match jmap_call(&api_url, &token, &["urn:ietf:params:jmap:calendars"], calls) {
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
            Err(e) => all_results.push(format!("Error deleting calendar event in {}: {}", name, e)),
        }
    }
    if all_results.is_empty() {
        Err("No JMAP clients configured.".to_string())
    } else {
        Ok(crate::tools::dtos::DeleteCalendarItemResponse {
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
    fn test_calendar_operations_success() {
        rustls::crypto::ring::default_provider().install_default().ok();
        let json_resp = serde_json::json!({
            "apiUrl": "{API_URL}",
            "primaryAccounts": {
                "urn:ietf:params:jmap:calendars": "acc-1"
            },
            "methodResponses": [
                ["CalendarEvent/get", { "list": [{"id": "ev-1", "title": "Meeting"}] }, "1"],
                ["CalendarEvent/set", { "created": {"new_event_1": {"id": "new-id"}}, "updated": {"ev-1": null}, "destroyed": ["ev-2"] }, "0"]
            ]
        });
        let url = spawn_mock_server(serde_json::to_string(&json_resp).unwrap());
        let config = mock_config(&url);

        let res_search = tool_search_calendar(&config, "meet");
        assert!(res_search.is_ok());

        let res_get = tool_get_calendar(&config, "2023-01-01T00:00:00Z", "2023-12-31T23:59:59Z");
        assert!(res_get.is_ok());

        let res_item = tool_get_calendar_item(&config, "ev-1");
        assert!(res_item.is_ok());

        let res_add = tool_add_calendar_item(&config, r#"{"title": "New"}"#);
        assert!(res_add.is_ok());

        let res_upd = tool_update_calendar_item(&config, "ev-1", r#"{"title": "Updated"}"#);
        assert!(res_upd.is_ok());

        let res_del = tool_delete_calendar_item(&config, "ev-2");
        assert!(res_del.is_ok());
    }

    #[test]
    fn test_calendar_operations_errors() {
        rustls::crypto::ring::default_provider().install_default().ok();
        let json_resp = serde_json::json!({
            "apiUrl": "{API_URL}",
            "primaryAccounts": {
                "urn:ietf:params:jmap:calendars": "acc-1"
            },
            "methodResponses": [
                ["error", { "type": "serverError", "description": "mock error" }, "0"]
            ]
        });
        let url = spawn_mock_server(serde_json::to_string(&json_resp).unwrap());
        let config = mock_config(&url);

        let res_search = tool_search_calendar(&config, "meet");
        assert!(res_search.is_ok());
        assert!(res_search.unwrap().results.contains("Error from JMAP server"));

        let res_add_err = tool_add_calendar_item(&config, "{invalid json}");
        assert!(res_add_err.is_err());
        assert!(res_add_err.unwrap_err().contains("Invalid JSON"));

        let res_upd_err = tool_update_calendar_item(&config, "ev-1", "{invalid json}");
        assert!(res_upd_err.is_err());
        assert!(res_upd_err.unwrap_err().contains("Invalid JSON"));
    }

    #[test]
    fn test_calendar_operations_no_clients() {
        let config = AppConfig::default();
        assert!(tool_search_calendar(&config, "meet").is_err());
        assert!(tool_get_calendar(&config, "start", "end").is_err());
        assert!(tool_get_calendar_item(&config, "id").is_err());
        assert!(tool_add_calendar_item(&config, "{}").is_err());
        assert!(tool_update_calendar_item(&config, "id", "{}").is_err());
        assert!(tool_delete_calendar_item(&config, "id").is_err());
    }

    #[test]
    fn test_calendar_operations_session_error() {
        rustls::crypto::ring::default_provider().install_default().ok();
        // Respond with 401 Unauthorized for session
        let url = spawn_mock_server("HTTP/1.1 401 Unauthorized\r\nContent-Length: 5\r\n\r\nerror");
        let config = mock_config(&url);
        
        let res_search = tool_search_calendar(&config, "meet");
        // tool_search_calendar skips failed sessions and continues, but if all fail it returns an error
        assert!(res_search.unwrap().results.contains("Error fetching JMAP session"));

        let res_add = tool_add_calendar_item(&config, "{}");
        assert!(res_add.unwrap().result.contains("Error fetching JMAP session"));
    }
}
