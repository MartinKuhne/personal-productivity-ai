//! JMAP Email and EmailSubmission operations (RFC 8621).
//!
//! Uses `urn:ietf:params:jmap:mail` and `urn:ietf:params:jmap:submission` capabilities.
//! Core protocol handling per RFC 8620; error handling per RFC 8620 §3.6.2.
//! See: <https://www.rfc-editor.org/rfc/rfc8620>
//! See: <https://www.rfc-editor.org/rfc/rfc8621>

use crate::config::AppConfig;
use fast_h2m::convert;

use super::client::{get_account_id, get_jmap_session, jmap_call, jmap_check_errors};

fn convert_html_in_jmap(mut res: serde_json::Value) -> serde_json::Value {
    fn process(val: &mut serde_json::Value) {
        match val {
            serde_json::Value::Object(map) => {
                if let Some(body_values) = map.get_mut("bodyValues") {
                    if let serde_json::Value::Object(parts) = body_values {
                        for (_, part_obj) in parts.iter_mut() {
                            if let serde_json::Value::Object(part_map) = part_obj {
                                if let Some(serde_json::Value::String(val_str)) =
                                    part_map.get_mut("value")
                                {
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
            }
            serde_json::Value::Array(arr) => {
                for v in arr.iter_mut() {
                    process(v);
                }
            }
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

                                let id =
                                    email.get("id").unwrap_or(&serde_json::Value::Null).clone();
                                simplified.insert("id".to_string(), id);

                                let subject = email
                                    .get("subject")
                                    .unwrap_or(&serde_json::Value::Null)
                                    .clone();
                                simplified.insert("subject".to_string(), subject);

                                let date = email
                                    .get("receivedAt")
                                    .unwrap_or(&serde_json::Value::Null)
                                    .clone();
                                simplified.insert("date".to_string(), date);

                                let from = email
                                    .get("from")
                                    .unwrap_or(&serde_json::Value::Null)
                                    .clone();
                                simplified.insert("from".to_string(), from);

                                let to =
                                    email.get("to").unwrap_or(&serde_json::Value::Null).clone();
                                simplified.insert("to".to_string(), to);

                                let cc =
                                    email.get("cc").unwrap_or(&serde_json::Value::Null).clone();
                                simplified.insert("cc".to_string(), cc);

                                let bcc =
                                    email.get("bcc").unwrap_or(&serde_json::Value::Null).clone();
                                simplified.insert("bcc".to_string(), bcc);

                                let mut body_str = String::new();
                                let mut is_truncated = false;
                                if let Some(body_values) =
                                    email.get("bodyValues").and_then(|bv| bv.as_object())
                                {
                                    let mut found_html = false;
                                    if let Some(html_bodies) =
                                        email.get("htmlBody").and_then(|h| h.as_array())
                                    {
                                        if let Some(first_html) =
                                            html_bodies.first().and_then(|h| h.as_object())
                                        {
                                            if let Some(part_id) =
                                                first_html.get("partId").and_then(|p| p.as_str())
                                            {
                                                if let Some(part_val) = body_values
                                                    .get(part_id)
                                                    .and_then(|v| v.as_object())
                                                {
                                                    if let Some(val) = part_val
                                                        .get("value")
                                                        .and_then(|v| v.as_str())
                                                    {
                                                        body_str = val.to_string();
                                                        is_truncated = part_val
                                                            .get("isTruncated")
                                                            .and_then(|t| t.as_bool())
                                                            .unwrap_or(false);
                                                        found_html = true;
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    if !found_html {
                                        if let Some(text_bodies) =
                                            email.get("textBody").and_then(|t| t.as_array())
                                        {
                                            if let Some(first_text) =
                                                text_bodies.first().and_then(|t| t.as_object())
                                            {
                                                if let Some(part_id) = first_text
                                                    .get("partId")
                                                    .and_then(|p| p.as_str())
                                                {
                                                    if let Some(part_val) = body_values
                                                        .get(part_id)
                                                        .and_then(|v| v.as_object())
                                                    {
                                                        if let Some(val) = part_val
                                                            .get("value")
                                                            .and_then(|v| v.as_str())
                                                        {
                                                            body_str = val.to_string();
                                                            is_truncated = part_val
                                                                .get("isTruncated")
                                                                .and_then(|t| t.as_bool())
                                                                .unwrap_or(false);
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
                                        body_str = body_str
                                            .lines()
                                            .take(limit)
                                            .collect::<Vec<_>>()
                                            .join("\n");
                                        is_truncated = true;
                                    }
                                }

                                if is_truncated {
                                    body_str.push_str("\n... (truncated - use the get_email_by_id tool with the email id to read the full content)");
                                }

                                simplified.insert(
                                    "body".to_string(),
                                    serde_json::Value::String(body_str),
                                );
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

/// Look up a mailbox ID by folder name (case-insensitive) via Mailbox/get.
fn lookup_mailbox_id(
    api_url: &str,
    token: &str,
    account_id: &str,
    folder_name: &str,
) -> Result<String, String> {
    let calls = serde_json::json!([
        ["Mailbox/get", { "accountId": account_id, "ids": null }, "0"]
    ]);
    let res = jmap_call(api_url, token, &["urn:ietf:params:jmap:mail"], calls)?;
    if let Some(method_responses) = res.get("methodResponses").and_then(|mr| mr.as_array()) {
        for resp in method_responses {
            if let Some(resp_arr) = resp.as_array() {
                if resp_arr.get(0).and_then(|n| n.as_str()) == Some("Mailbox/get") {
                    if let Some(args) = resp_arr.get(1).and_then(|a| a.as_object()) {
                        if let Some(list) = args.get("list").and_then(|l| l.as_array()) {
                            let lower_name = folder_name.to_lowercase();
                            for mailbox in list {
                                if let Some(name) = mailbox.get("name").and_then(|n| n.as_str()) {
                                    if name.to_lowercase() == lower_name {
                                        if let Some(id) = mailbox.get("id").and_then(|i| i.as_str())
                                        {
                                            return Ok(id.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Err(format!("Mailbox not found with name: {}", folder_name))
}

pub fn tool_search_email(
    config: &AppConfig,
    keyword: Option<&str>,
    folder: Option<&str>,
    start_date: Option<&str>,
    end_date: Option<&str>,
    sender: Option<&str>,
    recipient: Option<&str>,
) -> Result<crate::tools::dtos::SearchEmailResponse, String> {
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
        let mailbox_id = match folder {
            Some(f) => match lookup_mailbox_id(&api_url, &token, &account_id, f) {
                Ok(id) => Some(id),
                Err(e) => {
                    all_results.push(format!("Error for {}: {}", name, e));
                    continue;
                }
            },
            None => None,
        };
        let mut filter = match keyword {
            Some(k) if !k.is_empty() => serde_json::json!({ "text": k }),
            _ => serde_json::json!({}),
        };
        if let Some(ref mbox_id) = mailbox_id {
            filter["inMailbox"] = serde_json::Value::String(mbox_id.clone());
        }
        if let Some(s) = start_date {
            if !s.is_empty() {
                filter["after"] = serde_json::Value::String(format_jmap_date(s, false));
            }
        }
        if let Some(e) = end_date {
            if !e.is_empty() {
                filter["before"] = serde_json::Value::String(format_jmap_date(e, true));
            }
        }
        if let Some(s) = sender {
            if !s.is_empty() {
                filter["from"] = serde_json::Value::String(s.to_string());
            }
        }
        if let Some(r) = recipient {
            if !r.is_empty() {
                filter["to"] = serde_json::Value::String(r.to_string());
            }
        }
        if filter.as_object().map_or(true, |o| o.is_empty()) {
            all_results.push(format!("Error for {}: At least one filter field must be provided (keyword, folder, start_date, end_date, from, to)", name));
            continue;
        }
        let calls = serde_json::json!([
            ["Email/query", { "accountId": account_id, "filter": filter }, "0"],
            ["Email/get", { "accountId": account_id, "#ids": { "resultOf": "0", "name": "Email/query", "path": "/ids" }, "properties": ["id", "subject", "from", "receivedAt", "bodyValues", "textBody", "htmlBody"], "fetchTextBodyValues": true, "fetchHTMLBodyValues": true, "maxBodyValueBytes": 1000 }, "1"]
        ]);
        match jmap_call(&api_url, &token, &["urn:ietf:params:jmap:mail"], calls) {
            Ok(res) => {
                let mut is_error = false;
                if let Some(method_responses) =
                    res.get("methodResponses").and_then(|mr| mr.as_array())
                {
                    for resp in method_responses {
                        if let Some(resp_arr) = resp.as_array() {
                            if resp_arr.get(0).and_then(|s| s.as_str()) == Some("error") {
                                all_results.push(format!(
                                    "Error from JMAP server for {}: {}",
                                    name,
                                    serde_json::to_string_pretty(resp_arr).unwrap_or_default()
                                ));
                                is_error = true;
                            }
                        }
                    }
                }
                if !is_error {
                    let clean_res = convert_html_in_jmap(res);
                    let simplified = simplify_jmap_emails(clean_res, Some(10));
                    all_results.push(format!(
                        "--- Client: {} ---\n{}",
                        name,
                        serde_json::to_string_pretty(&simplified).unwrap_or_default()
                    ))
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
        Ok(crate::tools::dtos::SearchEmailResponse {
            results: all_results.join("\n\n"),
        })
    }
}

pub fn tool_get_email_by_id(
    config: &AppConfig,
    id: &str,
) -> Result<crate::tools::dtos::GetEmailByIdResponse, String> {
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
                if let Some(method_responses) =
                    res.get("methodResponses").and_then(|mr| mr.as_array())
                {
                    for resp in method_responses {
                        if let Some(resp_arr) = resp.as_array() {
                            if resp_arr.get(0).and_then(|s| s.as_str()) == Some("error") {
                                return Err(format!(
                                    "Error from JMAP server for {}: {}",
                                    name,
                                    serde_json::to_string_pretty(resp_arr).unwrap_or_default()
                                ));
                            }
                        }
                    }
                }
                let clean_res = convert_html_in_jmap(res);
                let simplified = simplify_jmap_emails(clean_res, None);
                return Ok(crate::tools::dtos::GetEmailByIdResponse {
                    result: serde_json::to_string_pretty(&simplified).unwrap_or_default(),
                });
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
    if let Some(s) = start_date {
        if !s.is_empty() {
            conditions.push(serde_json::json!({ "after": format_jmap_date(s, false) }));
        }
    }
    if let Some(e) = end_date {
        if !e.is_empty() {
            conditions.push(serde_json::json!({ "before": format_jmap_date(e, true) }));
        }
    }
    if let Some(s) = sender {
        if !s.is_empty() {
            conditions.push(serde_json::json!({ "from": s }));
        }
    }
    if let Some(r) = recipient {
        if !r.is_empty() {
            conditions.push(serde_json::json!({ "to": r }));
        }
    }
    if let Some(u) = is_unread {
        if u {
            conditions.push(serde_json::json!({ "notKeyword": "$seen" }));
        } else {
            conditions.push(serde_json::json!({ "hasKeyword": "$seen" }));
        }
    }
    if let Some(f) = is_flagged {
        if f {
            conditions.push(serde_json::json!({ "hasKeyword": "$flagged" }));
        } else {
            conditions.push(serde_json::json!({ "notKeyword": "$flagged" }));
        }
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
                if let Some(method_responses) =
                    res.get("methodResponses").and_then(|mr| mr.as_array())
                {
                    for resp in method_responses {
                        if let Some(resp_arr) = resp.as_array() {
                            if resp_arr.get(0).and_then(|s| s.as_str()) == Some("error") {
                                all_results.push(format!(
                                    "Error from JMAP server for {}: {}",
                                    name,
                                    serde_json::to_string_pretty(resp_arr).unwrap_or_default()
                                ));
                                is_error = true;
                            }
                        }
                    }
                }
                if !is_error {
                    let clean_res = convert_html_in_jmap(res);
                    let simplified = simplify_jmap_emails(clean_res, Some(10));
                    all_results.push(format!(
                        "--- Client: {} ---\n{}",
                        name,
                        serde_json::to_string_pretty(&simplified).unwrap_or_default()
                    ))
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
        Ok(crate::tools::dtos::GetEmailResponse {
            results: all_results.join("\n\n"),
        })
    }
}

pub fn tool_send_email(
    config: &AppConfig,
    to: &str,
    subject: &str,
    body: &str,
) -> Result<crate::tools::dtos::SendEmailResponse, String> {
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
        match jmap_call(
            &api_url,
            &token,
            &[
                "urn:ietf:params:jmap:mail",
                "urn:ietf:params:jmap:submission",
            ],
            calls,
        ) {
            Ok(res) => {
                if let Some(err) = jmap_check_errors(&res) {
                    tracing::error!(name = "tool.email.send.jmap_error", client = %name, error = %err, "JMAP server returned an error while sending email.");
                    all_results.push(format!("Error from JMAP server for {}: {}", name, err));
                } else {
                    all_results.push(format!(
                        "--- Client: {} ---\n{}",
                        name,
                        serde_json::to_string_pretty(&res).unwrap_or_default()
                    ));
                }
            }
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
        Ok(crate::tools::dtos::SendEmailResponse {
            result: all_results.join("\n\n"),
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{convert_html_in_jmap, simplify_jmap_emails};

    // -- convert_html_in_jmap --

    #[test]
    fn test_convert_html_plain_text_unchanged() {
        let res = json!({
            "methodResponses": [[
                "Email/get",
                {
                    "list": [{
                        "id": "1",
                        "bodyValues": {
                            "part1": { "value": "Hello, world!", "isTruncated": false }
                        }
                    }]
                },
                "0"
            ]]
        });
        let result = convert_html_in_jmap(res);
        let val = result["methodResponses"][0][1]["list"][0]["bodyValues"]["part1"]["value"]
            .as_str()
            .unwrap();
        assert_eq!(val, "Hello, world!");
    }

    #[test]
    fn test_convert_html_converts_simple_html() {
        let res = json!({
            "methodResponses": [[
                "Email/get",
                {
                    "list": [{
                        "id": "1",
                        "bodyValues": {
                            "part1": { "value": "<p>Hello</p>", "isTruncated": false }
                        }
                    }]
                },
                "0"
            ]]
        });
        let result = convert_html_in_jmap(res);
        let val = result["methodResponses"][0][1]["list"][0]["bodyValues"]["part1"]["value"]
            .as_str()
            .unwrap();
        assert!(val.starts_with("Hello"));
        assert!(!val.contains('<'));
    }

    #[test]
    fn test_convert_html_multiple_body_parts() {
        let res = json!({
            "methodResponses": [[
                "Email/get",
                {
                    "list": [{
                        "id": "1",
                        "bodyValues": {
                            "part1": { "value": "<h1>Title</h1>", "isTruncated": false },
                            "part2": { "value": "Plain text", "isTruncated": false },
                            "part3": { "value": "<p>Para</p>", "isTruncated": false }
                        }
                    }]
                },
                "0"
            ]]
        });
        let result = convert_html_in_jmap(res);
        let bv = &result["methodResponses"][0][1]["list"][0]["bodyValues"];
        assert!(bv["part1"]["value"].as_str().unwrap().contains("Title"));
        assert_eq!(bv["part2"]["value"].as_str().unwrap(), "Plain text");
        assert!(bv["part3"]["value"].as_str().unwrap().starts_with("Para"));
    }

    #[test]
    fn test_convert_html_no_body_values() {
        let res = json!({
            "methodResponses": [[
                "Email/get",
                {
                    "list": [{ "id": "1", "subject": "test" }]
                },
                "0"
            ]]
        });
        let result = convert_html_in_jmap(res);
        assert!(result["methodResponses"][0][1]["list"][0]["subject"]
            .as_str()
            .is_some());
    }

    #[test]
    fn test_convert_html_empty_body_values() {
        let res = json!({
            "methodResponses": [[
                "Email/get",
                {
                    "list": [{ "id": "1", "bodyValues": {} }]
                },
                "0"
            ]]
        });
        convert_html_in_jmap(res); // should not panic
    }

    #[test]
    fn test_convert_html_value_missing_angle_brackets_not_converted() {
        let res = json!({
            "methodResponses": [[
                "Email/get",
                {
                    "list": [{
                        "id": "1",
                        "bodyValues": {
                            "part1": { "value": "Hello World", "isTruncated": false }
                        }
                    }]
                },
                "0"
            ]]
        });
        let result = convert_html_in_jmap(res);
        let val = result["methodResponses"][0][1]["list"][0]["bodyValues"]["part1"]["value"]
            .as_str()
            .unwrap();
        assert_eq!(val, "Hello World");
    }

    #[test]
    fn test_convert_html_non_string_value_not_converted() {
        let res = json!({
            "methodResponses": [[
                "Email/get",
                {
                    "list": [{
                        "id": "1",
                        "bodyValues": {
                            "part1": { "value": 42, "isTruncated": false }
                        }
                    }]
                },
                "0"
            ]]
        });
        convert_html_in_jmap(res); // should not panic
    }

    // -- simplify_jmap_emails --

    #[test]
    fn test_simplify_empty_method_responses() {
        let res = json!({ "methodResponses": [] });
        let result = simplify_jmap_emails(res, None);
        assert_eq!(result, json!([]));
    }

    #[test]
    fn test_simplify_no_email_get_method() {
        let res = json!({
            "methodResponses": [[
                "Contact/query", { "ids": [] }, "0"
            ]]
        });
        let result = simplify_jmap_emails(res, None);
        assert_eq!(result, json!([]));
    }

    #[test]
    fn test_simplify_email_get_empty_list() {
        let res = json!({
            "methodResponses": [[
                "Email/get",
                { "accountId": "a1", "list": [], "notFound": [] },
                "0"
            ]]
        });
        let result = simplify_jmap_emails(res, None);
        assert_eq!(result, json!([]));
    }

    #[test]
    fn test_simplify_single_email_html_body() {
        let res = json!({
            "methodResponses": [[
                "Email/get",
                {
                    "accountId": "a1",
                    "list": [{
                        "id": "email-1",
                        "subject": "Hello",
                        "receivedAt": "2026-07-19T10:00:00Z",
                        "from": [{ "name": "Alice", "email": "alice@test.com" }],
                        "to": [{ "name": "Bob", "email": "bob@test.com" }],
                        "cc": [],
                        "bcc": [],
                        "htmlBody": [{ "partId": "p1" }],
                        "bodyValues": {
                            "p1": { "value": "Hello Bob!", "isTruncated": false }
                        }
                    }],
                    "notFound": []
                },
                "0"
            ]]
        });
        let result = simplify_jmap_emails(res, None);
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["id"], "email-1");
        assert_eq!(arr[0]["subject"], "Hello");
        assert_eq!(arr[0]["body"], "Hello Bob!");
    }

    #[test]
    fn test_simplify_email_text_body_fallback() {
        let res = json!({
            "methodResponses": [[
                "Email/get",
                {
                    "accountId": "a1",
                    "list": [{
                        "id": "email-2",
                        "subject": "No HTML",
                        "receivedAt": "2026-07-19T11:00:00Z",
                        "from": [{ "name": "Charlie", "email": "charlie@test.com" }],
                        "to": [{ "name": "Dave", "email": "dave@test.com" }],
                        "textBody": [{ "partId": "tp1" }],
                        "bodyValues": {
                            "tp1": { "value": "Plain text body", "isTruncated": false }
                        }
                    }],
                    "notFound": []
                },
                "0"
            ]]
        });
        let result = simplify_jmap_emails(res, None);
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["body"], "Plain text body");
    }

    #[test]
    fn test_simplify_multiple_emails() {
        let res = json!({
            "methodResponses": [[
                "Email/get",
                {
                    "accountId": "a1",
                    "list": [
                        {
                            "id": "e1",
                            "subject": "First",
                            "receivedAt": "2026-07-19T10:00:00Z",
                            "from": [{ "email": "a@t.com" }],
                            "to": [{ "email": "b@t.com" }],
                            "htmlBody": [{ "partId": "p1" }],
                            "bodyValues": { "p1": { "value": "Body 1", "isTruncated": false } }
                        },
                        {
                            "id": "e2",
                            "subject": "Second",
                            "receivedAt": "2026-07-19T11:00:00Z",
                            "from": [{ "email": "c@t.com" }],
                            "to": [{ "email": "d@t.com" }],
                            "htmlBody": [{ "partId": "p2" }],
                            "bodyValues": { "p2": { "value": "Body 2", "isTruncated": false } }
                        }
                    ],
                    "notFound": []
                },
                "0"
            ]]
        });
        let result = simplify_jmap_emails(res, None);
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["id"], "e1");
        assert_eq!(arr[1]["id"], "e2");
    }

    #[test]
    fn test_simplify_truncates_body_to_max_lines() {
        let res = json!({
            "methodResponses": [[
                "Email/get",
                {
                    "accountId": "a1",
                    "list": [{
                        "id": "e1",
                        "subject": "Long body",
                        "receivedAt": "2026-07-19T10:00:00Z",
                        "from": [{ "email": "a@t.com" }],
                        "to": [{ "email": "b@t.com" }],
                        "htmlBody": [{ "partId": "p1" }],
                        "bodyValues": { "p1": { "value": "Line 1\nLine 2\nLine 3\nLine 4\nLine 5", "isTruncated": false } }
                    }],
                    "notFound": []
                },
                "0"
            ]]
        });
        let result = simplify_jmap_emails(res, Some(3));
        let body = result[0]["body"].as_str().unwrap();
        assert!(body.starts_with("Line 1\nLine 2\nLine 3"));
        assert!(body.contains("truncated"));
    }

    #[test]
    fn test_simplify_truncated_body_appends_hint() {
        let res = json!({
            "methodResponses": [[
                "Email/get",
                {
                    "accountId": "a1",
                    "list": [{
                        "id": "e1",
                        "subject": "Truncated",
                        "receivedAt": "2026-07-19T10:00:00Z",
                        "from": [{ "email": "a@t.com" }],
                        "to": [{ "email": "b@t.com" }],
                        "htmlBody": [{ "partId": "p1" }],
                        "bodyValues": { "p1": { "value": "Line 1\nLine 2\nLine 3\nLine 4", "isTruncated": false } }
                    }],
                    "notFound": []
                },
                "0"
            ]]
        });
        let result = simplify_jmap_emails(res, Some(2));
        let body = result[0]["body"].as_str().unwrap();
        assert!(body.contains("truncated"));
    }

    #[test]
    fn test_simplify_body_not_truncated_if_under_limit() {
        let res = json!({
            "methodResponses": [[
                "Email/get",
                {
                    "accountId": "a1",
                    "list": [{
                        "id": "e1",
                        "subject": "Short",
                        "receivedAt": "2026-07-19T10:00:00Z",
                        "from": [{ "email": "a@t.com" }],
                        "to": [{ "email": "b@t.com" }],
                        "htmlBody": [{ "partId": "p1" }],
                        "bodyValues": { "p1": { "value": "Just one line", "isTruncated": false } }
                    }],
                    "notFound": []
                },
                "0"
            ]]
        });
        let result = simplify_jmap_emails(res, Some(10));
        let body = result[0]["body"].as_str().unwrap();
        assert!(!body.contains("truncated"));
        assert_eq!(body, "Just one line");
    }

    #[test]
    fn test_simplify_handles_missing_optional_fields() {
        let res = json!({
            "methodResponses": [[
                "Email/get",
                {
                    "accountId": "a1",
                    "list": [{
                        "id": "e1"
                    }],
                    "notFound": []
                },
                "0"
            ]]
        });
        let result = simplify_jmap_emails(res, None);
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["id"], "e1");
        assert_eq!(arr[0]["subject"], serde_json::Value::Null);
    }

    #[test]
    fn test_simplify_handles_server_truncated_body() {
        let res = json!({
            "methodResponses": [[
                "Email/get",
                {
                    "accountId": "a1",
                    "list": [{
                        "id": "e1",
                        "subject": "Server truncated",
                        "receivedAt": "2026-07-19T10:00:00Z",
                        "from": [{ "email": "a@t.com" }],
                        "to": [{ "email": "b@t.com" }],
                        "htmlBody": [{ "partId": "p1" }],
                        "bodyValues": { "p1": { "value": "Partial body here...", "isTruncated": true } }
                    }],
                    "notFound": []
                },
                "0"
            ]]
        });
        let result = simplify_jmap_emails(res, None);
        let body = result[0]["body"].as_str().unwrap();
        assert!(body.contains("truncated"));
    }

    #[test]
    fn test_simplify_cc_and_bcc_preserved() {
        let res = json!({
            "methodResponses": [[
                "Email/get",
                {
                    "accountId": "a1",
                    "list": [{
                        "id": "e1",
                        "subject": "CC test",
                        "receivedAt": "2026-07-19T10:00:00Z",
                        "from": [{ "email": "a@t.com" }],
                        "to": [{ "email": "b@t.com" }],
                        "cc": [{ "email": "cc@t.com" }],
                        "bcc": [{ "email": "bcc@t.com" }],
                        "htmlBody": [{ "partId": "p1" }],
                        "bodyValues": { "p1": { "value": "Body", "isTruncated": false } }
                    }],
                    "notFound": []
                },
                "0"
            ]]
        });
        let result = simplify_jmap_emails(res, None);
        assert_eq!(result[0]["cc"][0]["email"], "cc@t.com");
        assert_eq!(result[0]["bcc"][0]["email"], "bcc@t.com");
    }
}
