use crate::config::AppConfig;
use fast_h2m::convert;

use super::client::{get_jmap_session, jmap_call, get_account_id};

fn convert_html_in_jmap(mut res: serde_json::Value) -> serde_json::Value {
    fn process(val: &mut serde_json::Value) {
        match val {
            serde_json::Value::Object(map) => {
                if let Some(body_values) = map.get_mut("bodyValues") {
                    if let serde_json::Value::Object(parts) = body_values {
                        for (_, part_obj) in parts.iter_mut() {
                            if let serde_json::Value::Object(part_map) = part_obj {
                                if let Some(serde_json::Value::String(val_str)) = part_map.get_mut("value") {
                                    if val_str.contains("<") && val_str.contains(">") {
                                        if let Ok(conv) = convert(val_str, None) {
                                            if let Some(md) = conv.content {
                                                *val_str = md;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                for (_, v) in map.iter_mut() {
                    process(v);
                }
            },
            serde_json::Value::Array(arr) => {
                for v in arr.iter_mut() {
                    process(v);
                }
            },
            _ => {}
        }
    }
    process(&mut res);
    res
}

fn simplify_jmap_emails(res: serde_json::Value, max_lines: Option<usize>) -> serde_json::Value {
    let mut simplified_emails = Vec::new();

    if let Some(method_responses) = res.get("methodResponses").and_then(|mr| mr.as_array()) {
        for resp in method_responses {
            if let Some(resp_arr) = resp.as_array() {
                if resp_arr.get(0).and_then(|n| n.as_str()) == Some("Email/get") {
                    if let Some(args) = resp_arr.get(1).and_then(|a| a.as_object()) {
                        if let Some(list) = args.get("list").and_then(|l| l.as_array()) {
                            for email in list {
                                let mut simplified = serde_json::Map::new();
                        
                        let id = email.get("id").unwrap_or(&serde_json::Value::Null).clone();
                        simplified.insert("id".to_string(), id);
                        
                        let subject = email.get("subject").unwrap_or(&serde_json::Value::Null).clone();
                        simplified.insert("subject".to_string(), subject);
                        
                        let date = email.get("receivedAt").unwrap_or(&serde_json::Value::Null).clone();
                        simplified.insert("date".to_string(), date);
                        
                        let from = email.get("from").unwrap_or(&serde_json::Value::Null).clone();
                        simplified.insert("from".to_string(), from);
                        
                        let to = email.get("to").unwrap_or(&serde_json::Value::Null).clone();
                        simplified.insert("to".to_string(), to);
                        
                        let cc = email.get("cc").unwrap_or(&serde_json::Value::Null).clone();
                        simplified.insert("cc".to_string(), cc);
                        
                        let bcc = email.get("bcc").unwrap_or(&serde_json::Value::Null).clone();
                        simplified.insert("bcc".to_string(), bcc);

                        let mut body_str = String::new();
                        let mut is_truncated = false;
                        if let Some(body_values) = email.get("bodyValues").and_then(|bv| bv.as_object()) {
                            let mut found_html = false;
                            if let Some(html_bodies) = email.get("htmlBody").and_then(|h| h.as_array()) {
                                if let Some(first_html) = html_bodies.first().and_then(|h| h.as_object()) {
                                    if let Some(part_id) = first_html.get("partId").and_then(|p| p.as_str()) {
                                        if let Some(part_val) = body_values.get(part_id).and_then(|v| v.as_object()) {
                                            if let Some(val) = part_val.get("value").and_then(|v| v.as_str()) {
                                                body_str = val.to_string();
                                                is_truncated = part_val.get("isTruncated").and_then(|t| t.as_bool()).unwrap_or(false);
                                                found_html = true;
                                            }
                                        }
                                    }
                                }
                            }
                            
                            if !found_html {
                                if let Some(text_bodies) = email.get("textBody").and_then(|t| t.as_array()) {
                                    if let Some(first_text) = text_bodies.first().and_then(|t| t.as_object()) {
                                        if let Some(part_id) = first_text.get("partId").and_then(|p| p.as_str()) {
                                            if let Some(part_val) = body_values.get(part_id).and_then(|v| v.as_object()) {
                                                if let Some(val) = part_val.get("value").and_then(|v| v.as_str()) {
                                                    body_str = val.to_string();
                                                    is_truncated = part_val.get("isTruncated").and_then(|t| t.as_bool()).unwrap_or(false);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        
                        if let Some(limit) = max_lines {
                            let total_lines = body_str.lines().count();
                            if total_lines > limit {
                                body_str = body_str.lines().take(limit).collect::<Vec<_>>().join("\n");
                                is_truncated = true;
                            }
                        }
                        
                        if is_truncated {
                            body_str.push_str("\n... (truncated - use the get_email_by_id tool with the email id to read the full content)");
                        }
                        
                        simplified.insert("body".to_string(), serde_json::Value::String(body_str));
                        simplified_emails.push(serde_json::Value::Object(simplified));
                            }
                        }
                    }
                }
            }
        }
    }
    
    serde_json::Value::Array(simplified_emails)
}

pub fn tool_search_email(config: &AppConfig, keyword: &str) -> Result<crate::tools::dtos::SearchEmailResponse, String> {
    let mut all_results = Vec::new();
    for (name, client) in &config.jmap_clients {
        let (api_url, token, accs) = match get_jmap_session(client) { 
            Ok(s) => s, 
            Err(e) => {
                tracing::error!(name = "tool.email.search.session_failed", client = %name, error = %e, "Failed to fetch JMAP session. Operator should check email account credentials.");
                all_results.push(format!("Error fetching JMAP session for {}: {}", name, e));
                continue;
            }
        };
        let account_id = get_account_id(&accs, "urn:ietf:params:jmap:mail");
        let calls = serde_json::json!([
            ["Email/query", { "accountId": account_id, "filter": { "text": keyword } }, "0"],
            ["Email/get", { "accountId": account_id, "#ids": { "resultOf": "0", "name": "Email/query", "path": "/ids" }, "properties": ["id", "subject", "from", "receivedAt", "bodyValues", "textBody", "htmlBody"], "fetchTextBodyValues": true, "fetchHTMLBodyValues": true, "maxBodyValueBytes": 1000 }, "1"]
        ]);
        match jmap_call(&api_url, &token, &["urn:ietf:params:jmap:mail"], calls) {
            Ok(res) => {
                let mut is_error = false;
                if let Some(method_responses) = res.get("methodResponses").and_then(|mr| mr.as_array()) {
                    for resp in method_responses {
                        if let Some(resp_arr) = resp.as_array() {
                            if resp_arr.get(0).and_then(|s| s.as_str()) == Some("error") {
                                all_results.push(format!("Error from JMAP server for {}: {}", name, serde_json::to_string_pretty(resp_arr).unwrap_or_default()));
                                is_error = true;
                            }
                        }
                    }
                }
                if !is_error {
                    let clean_res = convert_html_in_jmap(res);
                    let simplified = simplify_jmap_emails(clean_res, Some(10));
                    all_results.push(format!("--- Client: {} ---\n{}", name, serde_json::to_string_pretty(&simplified).unwrap_or_default()))
                }
            }
            Err(e) => {
                tracing::error!(name = "tool.email.search.api_failed", client = %name, error = %e, "Failed to query emails via JMAP. Operator should verify JMAP server status.");
                all_results.push(format!("Error querying email for {}: {}", name, e));
            }
        }
    }
    if all_results.is_empty() {
        tracing::warn!(name = "tool.email.search.no_clients", "No JMAP clients configured. Operator should configure at least one email account in settings.");
        Err("No JMAP clients configured.".to_string())
    } else {
        Ok(crate::tools::dtos::SearchEmailResponse { results: all_results.join("\n\n") })
    }
}

pub fn tool_get_email_by_id(config: &AppConfig, id: &str) -> Result<crate::tools::dtos::GetEmailByIdResponse, String> {
    for (name, client) in &config.jmap_clients {
        let (api_url, token, accs) = match get_jmap_session(client) { 
            Ok(s) => s, 
            Err(e) => {
                tracing::error!(name = "tool.email.get_by_id.session_failed", client = %name, error = %e, "Failed to fetch JMAP session. Operator should check email account credentials.");
                continue;
            }
        };
        let account_id = get_account_id(&accs, "urn:ietf:params:jmap:mail");
        let calls = serde_json::json!([["Email/get", { "accountId": account_id, "ids": [id], "properties": ["mailboxIds", "subject", "from", "to", "cc", "bcc", "receivedAt", "bodyValues", "textBody", "htmlBody"], "fetchTextBodyValues": true, "fetchHTMLBodyValues": true }, "0"]]);
        match jmap_call(&api_url, &token, &["urn:ietf:params:jmap:mail"], calls) {
            Ok(res) => {
                if let Some(method_responses) = res.get("methodResponses").and_then(|mr| mr.as_array()) {
                    for resp in method_responses {
                        if let Some(resp_arr) = resp.as_array() {
                            if resp_arr.get(0).and_then(|s| s.as_str()) == Some("error") {
                                return Err(format!("Error from JMAP server for {}: {}", name, serde_json::to_string_pretty(resp_arr).unwrap_or_default()));
                            }
                        }
                    }
                }
                let clean_res = convert_html_in_jmap(res);
                let simplified = simplify_jmap_emails(clean_res, None);
                return Ok(crate::tools::dtos::GetEmailByIdResponse { result: serde_json::to_string_pretty(&simplified).unwrap_or_default() });
            }
            Err(e) => {
                tracing::error!(name = "tool.email.get_by_id.api_failed", client = %name, error = %e, "Failed to query email by ID via JMAP. Operator should verify JMAP server status.");
                continue;
            }
        }
    }
    tracing::warn!(name = "tool.email.get_by_id.not_found", id = %id, "Email not found in any client or no clients configured. Operator should verify the email ID.");
    Err("Email not found in any client or no clients configured.".to_string())
}

pub fn tool_get_email(
    config: &AppConfig,
    start_date: Option<&str>,
    end_date: Option<&str>,
    sender: Option<&str>,
    recipient: Option<&str>,
    is_unread: Option<bool>,
    is_flagged: Option<bool>,
) -> Result<crate::tools::dtos::GetEmailResponse, String> {
    let mut all_results = Vec::new();

    let format_jmap_date = |d: &str, is_end: bool| -> String {
        if d.len() == 10 && d.chars().nth(4) == Some('-') && d.chars().nth(7) == Some('-') {
            if is_end {
                format!("{}T23:59:59Z", d)
            } else {
                format!("{}T00:00:00Z", d)
            }
        } else {
            d.to_string()
        }
    };

    let mut conditions = Vec::new();
    if let Some(s) = start_date { if !s.is_empty() { conditions.push(serde_json::json!({ "after": format_jmap_date(s, false) })); } }
    if let Some(e) = end_date { if !e.is_empty() { conditions.push(serde_json::json!({ "before": format_jmap_date(e, true) })); } }
    if let Some(s) = sender { if !s.is_empty() { conditions.push(serde_json::json!({ "from": s })); } }
    if let Some(r) = recipient { if !r.is_empty() { conditions.push(serde_json::json!({ "to": r })); } }
    if let Some(u) = is_unread { 
        if u { conditions.push(serde_json::json!({ "notKeyword": "$seen" })); } 
        else { conditions.push(serde_json::json!({ "hasKeyword": "$seen" })); }
    }
    if let Some(f) = is_flagged { 
        if f { conditions.push(serde_json::json!({ "hasKeyword": "$flagged" })); } 
        else { conditions.push(serde_json::json!({ "notKeyword": "$flagged" })); }
    }

    let filter_obj = if conditions.is_empty() {
        serde_json::json!({})
    } else if conditions.len() == 1 {
        conditions[0].clone()
    } else {
        serde_json::json!({
            "operator": "AND",
            "conditions": conditions
        })
    };

    for (name, client) in &config.jmap_clients {
        let (api_url, token, accs) = match get_jmap_session(client) { 
            Ok(s) => s, 
            Err(e) => {
                tracing::error!(name = "tool.email.get.session_failed", client = %name, error = %e, "Failed to fetch JMAP session. Operator should check email account credentials.");
                all_results.push(format!("Error fetching JMAP session for {}: {}", name, e));
                continue;
            }
        };
        let account_id = get_account_id(&accs, "urn:ietf:params:jmap:mail");
        let calls = serde_json::json!([
            ["Email/query", { "accountId": account_id, "filter": filter_obj }, "0"],
            ["Email/get", { "accountId": account_id, "#ids": { "resultOf": "0", "name": "Email/query", "path": "/ids" }, "properties": ["id", "subject", "from", "receivedAt", "bodyValues", "textBody", "htmlBody"], "fetchTextBodyValues": true, "fetchHTMLBodyValues": true, "maxBodyValueBytes": 1000 }, "1"]
        ]);
        match jmap_call(&api_url, &token, &["urn:ietf:params:jmap:mail"], calls) {
            Ok(res) => {
                let mut is_error = false;
                if let Some(method_responses) = res.get("methodResponses").and_then(|mr| mr.as_array()) {
                    for resp in method_responses {
                        if let Some(resp_arr) = resp.as_array() {
                            if resp_arr.get(0).and_then(|s| s.as_str()) == Some("error") {
                                all_results.push(format!("Error from JMAP server for {}: {}", name, serde_json::to_string_pretty(resp_arr).unwrap_or_default()));
                                is_error = true;
                            }
                        }
                    }
                }
                if !is_error {
                    let clean_res = convert_html_in_jmap(res);
                    let simplified = simplify_jmap_emails(clean_res, Some(10));
                    all_results.push(format!("--- Client: {} ---\n{}", name, serde_json::to_string_pretty(&simplified).unwrap_or_default()))
                }
            }
            Err(e) => {
                tracing::error!(name = "tool.email.get.api_failed", client = %name, error = %e, "Failed to query emails via JMAP. Operator should verify JMAP server status.");
                all_results.push(format!("Error querying emails for {}: {}", name, e));
            }
        }
    }
    if all_results.is_empty() {
        tracing::warn!(name = "tool.email.get.no_clients", "No JMAP clients configured. Operator should configure at least one email account in settings.");
        Err("No JMAP clients configured.".to_string())
    } else {
        Ok(crate::tools::dtos::GetEmailResponse { results: all_results.join("\n\n") })
    }
}

pub fn tool_send_email(config: &AppConfig, to: &str, subject: &str, body: &str) -> Result<crate::tools::dtos::SendEmailResponse, String> {
    let mut all_results = Vec::new();
    if let Some((name, client)) = config.jmap_clients.iter().next() {
        let (api_url, token, accs) = match get_jmap_session(client) { 
            Ok(s) => s, 
            Err(e) => {
                tracing::error!(name = "tool.email.send.session_failed", client = %name, error = %e, "Failed to fetch JMAP session. Operator should check email account credentials.");
                all_results.push(format!("Error fetching JMAP session for {}: {}", name, e));
                return Err(all_results.join("\n\n"));
            }
        };
        let account_id = get_account_id(&accs, "urn:ietf:params:jmap:mail");
        let submission_id = get_account_id(&accs, "urn:ietf:params:jmap:submission");
        let item = serde_json::json!({
            "subject": subject,
            "to": [{ "email": to }],
            "textBody": [{ "partId": "body" }],
            "bodyValues": { "body": { "value": body, "isEncodingProblem": false, "isTruncated": false } }
        });
        let calls = serde_json::json!([
            ["Email/set", { "accountId": account_id, "create": { "draft_1": item } }, "0"],
            ["EmailSubmission/set", { "accountId": submission_id, "create": { "sub_1": { "emailId": "#draft_1" } }, "onSuccessDestroyEmail": ["#draft_1"] }, "1"]
        ]);
        match jmap_call(&api_url, &token, &["urn:ietf:params:jmap:mail", "urn:ietf:params:jmap:submission"], calls) {
            Ok(res) => all_results.push(format!("--- Client: {} ---\n{}", name, serde_json::to_string_pretty(&res).unwrap_or_default())),
            Err(e) => {
                tracing::error!(name = "tool.email.send.api_failed", client = %name, error = %e, "Failed to send email via JMAP. Operator should verify JMAP server status.");
                all_results.push(format!("Error sending email via {}: {}", name, e));
            }
        }
    }
    if all_results.is_empty() {
        tracing::warn!(name = "tool.email.send.no_clients", "No JMAP clients configured. Operator should configure at least one email account in settings.");
        Err("No JMAP clients configured.".to_string())
    } else {
        Ok(crate::tools::dtos::SendEmailResponse { result: all_results.join("\n\n") })
    }
}