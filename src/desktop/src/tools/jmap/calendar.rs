// NOTE: This module is intentionally unreachable code. Fastmail does not currently support
// JMAP Calendar, so all calendar operations go through CalDAV (tools/caldav.rs) instead.
// This implementation is kept as a reference and should be restored once Fastmail enables
// JMAP Calendar support.

use crate::config::AppConfig;
use serde_json::Value;

use super::client::{get_jmap_session, jmap_call, get_account_id};

pub fn tool_search_calendar(config: &AppConfig, keyword: &str) -> Result<crate::tools::dtos::SearchCalendarResponse, String> {
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
            Ok(res) => all_results.push(format!("--- Client: {} ---\n{}", name, serde_json::to_string_pretty(&res).unwrap_or_default())),
            Err(e) => all_results.push(format!("Error querying calendar for {}: {}", name, e)),
        }
    }
    if all_results.is_empty() {
        Err("No JMAP clients configured.".to_string())
    } else {
        Ok(crate::tools::dtos::SearchCalendarResponse { results: all_results.join("\n\n") })
    }
}

pub fn tool_get_calendar(config: &AppConfig, start: &str, end: &str) -> Result<crate::tools::dtos::GetCalendarResponse, String> {
    let mut all_results = Vec::new();
    for (name, client) in &config.jmap_clients {
        let (api_url, token, accs) = match get_jmap_session(client) { Ok(s) => s, Err(_) => continue };
        let account_id = get_account_id(&accs, "urn:ietf:params:jmap:calendars");
        let calls = serde_json::json!([
            ["CalendarEvent/query", { "accountId": account_id, "filter": { "after": start, "before": end } }, "0"],
            ["CalendarEvent/get", { "accountId": account_id, "#ids": { "resultOf": "0", "name": "CalendarEvent/query", "path": "/ids" } }, "1"]
        ]);
        match jmap_call(&api_url, &token, &["urn:ietf:params:jmap:calendars"], calls) {
            Ok(res) => all_results.push(format!("--- Client: {} ---\n{}", name, serde_json::to_string_pretty(&res).unwrap_or_default())),
            Err(_) => continue,
        }
    }
    if all_results.is_empty() {
        Err("No JMAP clients configured.".to_string())
    } else {
        Ok(crate::tools::dtos::GetCalendarResponse { results: all_results.join("\n\n") })
    }
}

pub fn tool_get_calendar_item(config: &AppConfig, id: &str) -> Result<crate::tools::dtos::GetCalendarItemResponse, String> {
    let mut all_results = Vec::new();
    for (name, client) in &config.jmap_clients {
        let (api_url, token, accs) = match get_jmap_session(client) { Ok(s) => s, Err(_) => continue };
        let account_id = get_account_id(&accs, "urn:ietf:params:jmap:calendars");
        let calls = serde_json::json!([["CalendarEvent/get", { "accountId": account_id, "ids": [id] }, "0"]]);
        match jmap_call(&api_url, &token, &["urn:ietf:params:jmap:calendars"], calls) {
            Ok(res) => all_results.push(format!("--- Client: {} ---\n{}", name, serde_json::to_string_pretty(&res).unwrap_or_default())),
            Err(_) => continue,
        }
    }
    if all_results.is_empty() {
        Err("Event not found in any client or no clients configured.".to_string())
    } else {
        Ok(crate::tools::dtos::GetCalendarItemResponse { result: all_results.join("\n\n") })
    }
}

pub fn tool_add_calendar_item(config: &AppConfig, item_json: &str) -> Result<crate::tools::dtos::AddCalendarItemResponse, String> {
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
            Ok(i) => i, Err(e) => return Err(format!("Invalid JSON: {}", e)),
        };
        let calls = serde_json::json!([["CalendarEvent/set", { "accountId": account_id, "create": { "new_event_1": item } }, "0"]]);
        match jmap_call(&api_url, &token, &["urn:ietf:params:jmap:calendars"], calls) {
            Ok(res) => all_results.push(format!("--- Client: {} ---\n{}", name, serde_json::to_string_pretty(&res).unwrap_or_default())),
            Err(e) => all_results.push(format!("Error creating calendar event in {}: {}", name, e)),
        }
    }
    if all_results.is_empty() {
        Err("No JMAP clients configured.".to_string())
    } else {
        Ok(crate::tools::dtos::AddCalendarItemResponse { result: all_results.join("\n\n") })
    }
}

pub fn tool_update_calendar_item(config: &AppConfig, id: &str, update_json: &str) -> Result<crate::tools::dtos::UpdateCalendarItemResponse, String> {
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
            Ok(u) => u, Err(e) => return Err(format!("Invalid JSON: {}", e)),
        };
        let calls = serde_json::json!([["CalendarEvent/set", { "accountId": account_id, "update": { id: update } }, "0"]]);
        match jmap_call(&api_url, &token, &["urn:ietf:params:jmap:calendars"], calls) {
            Ok(res) => all_results.push(format!("--- Client: {} ---\n{}", name, serde_json::to_string_pretty(&res).unwrap_or_default())),
            Err(e) => all_results.push(format!("Error updating calendar event in {}: {}", name, e)),
        }
    }
    if all_results.is_empty() {
        Err("No JMAP clients configured.".to_string())
    } else {
        Ok(crate::tools::dtos::UpdateCalendarItemResponse { result: all_results.join("\n\n") })
    }
}

pub fn tool_delete_calendar_item(config: &AppConfig, id: &str) -> Result<crate::tools::dtos::DeleteCalendarItemResponse, String> {
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
            Ok(res) => all_results.push(format!("--- Client: {} ---\n{}", name, serde_json::to_string_pretty(&res).unwrap_or_default())),
            Err(e) => all_results.push(format!("Error deleting calendar event in {}: {}", name, e)),
        }
    }
    if all_results.is_empty() {
        Err("No JMAP clients configured.".to_string())
    } else {
        Ok(crate::tools::dtos::DeleteCalendarItemResponse { result: all_results.join("\n\n") })
    }
}