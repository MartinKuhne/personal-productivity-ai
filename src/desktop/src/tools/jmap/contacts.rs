use crate::config::AppConfig;
use serde_json::Value;

use super::client::{get_jmap_session, jmap_call, get_account_id};

pub fn tool_search_contact(config: &AppConfig, keyword: &str) -> Result<crate::tools::dtos::SearchContactResponse, String> {
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
            Ok(res) => all_results.push(format!("--- Client: {} ---\n{}", name, serde_json::to_string_pretty(&res).unwrap_or_default())),
            Err(e) => all_results.push(format!("Error querying contacts for {}: {}", name, e)),
        }
    }
    if all_results.is_empty() {
        Err("No JMAP clients configured.".to_string())
    } else {
        Ok(crate::tools::dtos::SearchContactResponse { results: all_results.join("\n\n") })
    }
}

pub fn tool_get_contact(config: &AppConfig, id: &str) -> Result<crate::tools::dtos::GetContactResponse, String> {
    let mut all_results = Vec::new();
    for (name, client) in &config.jmap_clients {
        let (api_url, token, accs) = match get_jmap_session(client) { 
            Ok(s) => s, 
            Err(_) => continue 
        };
        let account_id = get_account_id(&accs, "urn:ietf:params:jmap:contacts");
        let calls = serde_json::json!([["Contact/get", { "accountId": account_id, "ids": [id] }, "0"]]);
        match jmap_call(&api_url, &token, &["urn:ietf:params:jmap:contacts"], calls) {
            Ok(res) => all_results.push(format!("--- Client: {} ---\n{}", name, serde_json::to_string_pretty(&res).unwrap_or_default())),
            Err(_) => continue,
        }
    }
    if all_results.is_empty() {
        Err("Contact not found in any client or no clients configured.".to_string())
    } else {
        Ok(crate::tools::dtos::GetContactResponse { result: all_results.join("\n\n") })
    }
}

pub fn tool_add_contact(config: &AppConfig, contact_json: &str) -> Result<crate::tools::dtos::AddContactResponse, String> {
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
            Ok(i) => i, Err(e) => return Err(format!("Invalid JSON: {}", e)),
        };
        let calls = serde_json::json!([["Contact/set", { "accountId": account_id, "create": { "new_contact_1": item } }, "0"]]);
        match jmap_call(&api_url, &token, &["urn:ietf:params:jmap:contacts"], calls) {
            Ok(res) => all_results.push(format!("--- Client: {} ---\n{}", name, serde_json::to_string_pretty(&res).unwrap_or_default())),
            Err(e) => all_results.push(format!("Error creating contact in {}: {}", name, e)),
        }
    }
    if all_results.is_empty() {
        Err("No JMAP clients configured.".to_string())
    } else {
        Ok(crate::tools::dtos::AddContactResponse { result: all_results.join("\n\n") })
    }
}