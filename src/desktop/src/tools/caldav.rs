use crate::config::AppConfig;
use fast_dav_rs::CalDavClient;
use std::sync::OnceLock;
use tokio::runtime::Runtime;

#[derive(serde::Serialize)]
struct CalDavEventDetails {
    client: String,
    id: String,
    href: String,
    summary: Option<String>,
    start: Option<String>,
    end: Option<String>,
    description: Option<String>,
    location: Option<String>,
    organizer: Option<String>,
}

#[derive(serde::Serialize)]
struct CalDavResponse {
    results: Vec<CalDavEventDetails>,
    errors: Vec<String>,
}

fn parse_ical_data(client: &str, href: &str, data: &str) -> CalDavEventDetails {
    let mut event = CalDavEventDetails {
        client: client.to_string(),
        id: href.to_string(),
        href: href.to_string(),
        summary: None,
        start: None,
        end: None,
        description: None,
        location: None,
        organizer: None,
    };

    fn format_ical_date(d: &str) -> String {
        let d = d.trim();
        if d.len() == 8 {
            format!("{}-{}-{}", &d[0..4], &d[4..6], &d[6..8])
        } else if d.len() == 15 && d.chars().nth(8) == Some('T') {
            format!("{}-{}-{}T{}:{}:{}", &d[0..4], &d[4..6], &d[6..8], &d[9..11], &d[11..13], &d[13..15])
        } else if d.len() == 16 && d.chars().nth(8) == Some('T') && d.ends_with('Z') {
            format!("{}-{}-{}T{}:{}:{}Z", &d[0..4], &d[4..6], &d[6..8], &d[9..11], &d[11..13], &d[13..15])
        } else {
            d.to_string()
        }
    }

    let mut unfolded = String::new();
    for line in data.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            unfolded.push_str(&line[1..]);
        } else {
            if !unfolded.is_empty() {
                unfolded.push('\n');
            }
            unfolded.push_str(line);
        }
    }

    for line in unfolded.lines() {
        if let Some(rest) = line.strip_prefix("SUMMARY:") {
            event.summary = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("DESCRIPTION:") {
            let desc = rest.replace("\\n", "\n").replace("\\N", "\n").replace("\\,", ",").replace("\\;", ";");
            event.description = Some(desc.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("LOCATION:") {
            let loc = rest.replace("\\,", ",").replace("\\;", ";");
            event.location = Some(loc.trim().to_string());
        } else if line.starts_with("DTSTART:") || line.starts_with("DTSTART;") {
            if let Some(idx) = line.find(':') {
                event.start = Some(format_ical_date(&line[idx+1..]));
            }
        } else if line.starts_with("DTEND:") || line.starts_with("DTEND;") {
            if let Some(idx) = line.find(':') {
                event.end = Some(format_ical_date(&line[idx+1..]));
            }
        } else if line.starts_with("ORGANIZER:") || line.starts_with("ORGANIZER;") {
            if let Some(idx) = line.find(':') {
                event.organizer = Some(line[idx+1..].trim().to_string());
            }
        }
    }
    
    event
}
fn block_on<F: std::future::Future>(f: F) -> F::Output {
    static RT: OnceLock<Runtime> = OnceLock::new();
    let rt = RT.get_or_init(|| Runtime::new().expect("Failed to create Tokio runtime"));
    rt.block_on(f)
}

async fn get_all_calendars(client: &CalDavClient, base_url: &str, username: &str) -> anyhow::Result<Vec<String>> {
    if let Ok(calendars) = client.list_calendars(base_url).await {
        if !calendars.is_empty() {
            return Ok(calendars.into_iter().map(|c| c.href).collect());
        }
    }
    
    if let Ok(homes) = client.discover_calendar_home_set(base_url).await {
        if let Some(home) = homes.first() {
            if let Ok(calendars) = client.list_calendars(home).await {
                if !calendars.is_empty() {
                    return Ok(calendars.into_iter().map(|c| c.href).collect());
                }
            }
        }
    }

    let mut principal_opt = client.discover_current_user_principal().await.ok().flatten();
    
    // Fallback for Fastmail or other servers that use /dav/principals/user/username/
    if principal_opt.is_none() {
        let base_trimmed = base_url.trim_end_matches('/');
        let guess = format!("{}/dav/principals/user/{}/", base_trimmed, username);
        if let Ok(homes) = client.discover_calendar_home_set(&guess).await {
            if !homes.is_empty() {
                principal_opt = Some(guess);
            }
        }
    }

    let principal = principal_opt.ok_or_else(|| anyhow::anyhow!("No principal found"))?;
    let homes = client.discover_calendar_home_set(&principal).await?;
    let home = homes.first().ok_or_else(|| anyhow::anyhow!("No calendar home found"))?;
    let calendars = client.list_calendars(home).await?;
    Ok(calendars.into_iter().map(|c| c.href).collect())
}

pub fn tool_search_calendar(config: &AppConfig, keyword: &str) -> Result<crate::tools::dtos::SearchCalendarResponse, String> {
    let mut results = Vec::new();
    let mut errors = Vec::new();
    let kw = keyword.to_lowercase();
    
    for (name, client_config) in &config.caldav_clients {
        let res = block_on(async {
            let client = match CalDavClient::new(&client_config.url, Some(&client_config.username), Some(&client_config.password)) {
                Ok(c) => c,
                Err(e) => return Err(anyhow::anyhow!("Client config error: {}", e)),
            };
            
            let cals = get_all_calendars(&client, &client_config.url, &client_config.username).await?;
            let mut matches = Vec::new();
            for cal_path in cals {
                let items = client.calendar_query_timerange(&cal_path, "VEVENT", None, None, true).await?;
                for item in items {
                    if let Some(data) = &item.calendar_data {
                        if data.to_lowercase().contains(&kw) {
                            matches.push(parse_ical_data(name, &item.href, data));
                        }
                    }
                }
            }
            anyhow::Result::<Vec<_>>::Ok(matches)
        });
        
        match res {
            Ok(mut matches) => results.append(&mut matches),
            Err(e) => errors.push(format!("Error on client {}: {}", name, e)),
        }
    }
    
    let resp = CalDavResponse { results, errors };
    Ok(crate::tools::dtos::SearchCalendarResponse {
        results: serde_json::to_string_pretty(&resp).unwrap_or_else(|_| "{}".to_string())
    })
}

pub fn tool_get_calendar(config: &AppConfig, start: &str, end: &str) -> Result<crate::tools::dtos::GetCalendarResponse, String> {
    let mut results = Vec::new();
    let mut errors = Vec::new();

    let format_caldav_date = |d: &str, is_end: bool| -> String {
        if d.len() == 10 && d.chars().nth(4) == Some('-') && d.chars().nth(7) == Some('-') {
            let clean = d.replace("-", "");
            if is_end { format!("{}T235959Z", clean) } else { format!("{}T000000Z", clean) }
        } else {
            d.to_string()
        }
    };
    
    let start_fmt = format_caldav_date(start, false);
    let end_fmt = format_caldav_date(end, true);
    
    for (name, client_config) in &config.caldav_clients {
        let res = block_on(async {
            let client = CalDavClient::new(&client_config.url, Some(&client_config.username), Some(&client_config.password))
                .map_err(|e| anyhow::anyhow!("Client config error: {}", e))?;
            let cals = get_all_calendars(&client, &client_config.url, &client_config.username).await?;
            let mut matches = Vec::new();
            for cal_path in cals {
                let items = client.calendar_query_timerange(&cal_path, "VEVENT", Some(&start_fmt), Some(&end_fmt), true).await?;
                for item in items {
                    if let Some(data) = &item.calendar_data {
                        matches.push(parse_ical_data(name, &item.href, data));
                    }
                }
            }
            anyhow::Result::<Vec<_>>::Ok(matches)
        });
        
        match res {
            Ok(mut m) => results.append(&mut m),
            Err(e) => errors.push(format!("Error on client {}: {}", name, e)),
        }
    }
    
    let resp = CalDavResponse { results, errors };
    Ok(crate::tools::dtos::GetCalendarResponse {
        results: serde_json::to_string_pretty(&resp).unwrap_or_else(|_| "{}".to_string())
    })
}

pub fn tool_get_calendar_item(config: &AppConfig, id: &str) -> Result<crate::tools::dtos::GetCalendarItemResponse, String> {
    let mut results = Vec::new();
    let mut errors = Vec::new();
    
    for (name, client_config) in &config.caldav_clients {
        let res = block_on(async {
            let client = CalDavClient::new(&client_config.url, Some(&client_config.username), Some(&client_config.password))
                .map_err(|e| anyhow::anyhow!("Client config error: {}", e))?;
            let resp = client.get(id).await?;
            if !resp.status().is_success() {
                let status = resp.status();
                let bytes = resp.into_body();
                let body = String::from_utf8_lossy(&bytes).to_string();
                return Err(anyhow::anyhow!("Not found by href: {} - {}", status, body));
            }
            let bytes = resp.into_body();
            let body = String::from_utf8_lossy(&bytes).to_string();
            anyhow::Result::<CalDavEventDetails>::Ok(parse_ical_data(name, id, &body))
        });
        
        match res {
            Ok(data) => results.push(data),
            Err(e) => errors.push(format!("Error on client {}: {}", name, e)),
        }
    }
    
    let resp = CalDavResponse { results, errors };
    Ok(crate::tools::dtos::GetCalendarItemResponse {
        result: serde_json::to_string_pretty(&resp).unwrap_or_else(|_| "{}".to_string())
    })
}

pub fn update_ical_string(original: &str, updates: &serde_json::Value) -> String {
    let mut out = String::new();
    let mut in_vevent = false;
    
    let mut has_summary = false;
    let mut has_start = false;
    let mut has_end = false;
    let mut has_desc = false;
    let mut has_loc = false;

    fn escape_ical_text(text: &str) -> String {
        text.replace("\\", "\\\\").replace(";", "\\;").replace(",", "\\,").replace("\n", "\\n").replace("\r", "")
    }

    let u_summary = updates.get("summary").and_then(|v| v.as_str()).map(escape_ical_text);
    let u_start = updates.get("start").and_then(|v| v.as_str()).map(|s| s.replace("-", "").replace(":", ""));
    let u_end = updates.get("end").and_then(|v| v.as_str()).map(|s| s.replace("-", "").replace(":", ""));
    let u_desc = updates.get("description").and_then(|v| v.as_str()).map(escape_ical_text);
    let u_loc = updates.get("location").and_then(|v| v.as_str()).map(escape_ical_text);

    let mut lines = original.lines().peekable();
    while let Some(line) = lines.next() {
        if line.starts_with("BEGIN:VEVENT") {
            in_vevent = true;
            out.push_str(&format!("{}\r\n", line));
            continue;
        }
        
        if line.starts_with("END:VEVENT") {
            if let Some(s) = &u_summary { if !has_summary { out.push_str(&format!("SUMMARY:{}\r\n", s)); } }
            if let Some(s) = &u_start { if !has_start { if s.len() == 8 { out.push_str(&format!("DTSTART;VALUE=DATE:{}\r\n", s)); } else { out.push_str(&format!("DTSTART:{}\r\n", s)); } } }
            if let Some(e) = &u_end { if !has_end { if e.len() == 8 { out.push_str(&format!("DTEND;VALUE=DATE:{}\r\n", e)); } else { out.push_str(&format!("DTEND:{}\r\n", e)); } } }
            if let Some(s) = &u_desc { if !has_desc { out.push_str(&format!("DESCRIPTION:{}\r\n", s)); } }
            if let Some(s) = &u_loc { if !has_loc { out.push_str(&format!("LOCATION:{}\r\n", s)); } }
            
            out.push_str(&format!("{}\r\n", line));
            in_vevent = false;
            continue;
        }

        if in_vevent {
            let mut replace_line = None;
            if line.starts_with("SUMMARY:") {
                has_summary = true;
                if let Some(s) = &u_summary { replace_line = Some(format!("SUMMARY:{}", s)); }
            } else if line.starts_with("DTSTART:") || line.starts_with("DTSTART;") {
                has_start = true;
                if let Some(s) = &u_start { replace_line = Some(if s.len() == 8 { format!("DTSTART;VALUE=DATE:{}", s) } else { format!("DTSTART:{}", s) }); }
            } else if line.starts_with("DTEND:") || line.starts_with("DTEND;") {
                has_end = true;
                if let Some(e) = &u_end { replace_line = Some(if e.len() == 8 { format!("DTEND;VALUE=DATE:{}", e) } else { format!("DTEND:{}", e) }); }
            } else if line.starts_with("DESCRIPTION:") {
                has_desc = true;
                if let Some(s) = &u_desc { replace_line = Some(format!("DESCRIPTION:{}", s)); }
            } else if line.starts_with("LOCATION:") {
                has_loc = true;
                if let Some(s) = &u_loc { replace_line = Some(format!("LOCATION:{}", s)); }
            }

            if let Some(repl) = replace_line {
                out.push_str(&format!("{}\r\n", repl));
                while let Some(next) = lines.peek() {
                    if next.starts_with(' ') || next.starts_with('\t') {
                        lines.next();
                    } else {
                        break;
                    }
                }
                continue;
            }
        }
        out.push_str(&format!("{}\r\n", line));
    }
    out
}

pub fn tool_add_calendar_item(config: &AppConfig, item_json: &str) -> Result<crate::tools::dtos::AddCalendarItemResponse, String> {
    let mut all_results = Vec::new();
    if let Some((name, client_config)) = config.caldav_clients.iter().next() {
        let res = block_on(async {
            let client = CalDavClient::new(&client_config.url, Some(&client_config.username), Some(&client_config.password))
                .map_err(|e| anyhow::anyhow!("Client config error: {}", e))?;
            let cals = get_all_calendars(&client, &client_config.url, &client_config.username).await?;
            let default_cal = cals.first().ok_or_else(|| anyhow::anyhow!("No calendar found to add to"))?;
            let uid = format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
            let path = format!("{}{}.ics", default_cal, uid);
            let ical_data = crate::tools::caldav::json_to_ical(item_json, Some(&uid));
            let resp = client.put(&path, ical_data.into_bytes().into()).await?;
            if !resp.status().is_success() {
                let status = resp.status();
                let body = String::from_utf8_lossy(&resp.into_body()).to_string();
                return Err(anyhow::anyhow!("Failed to PUT event: {} - {}", status, body));
            }
            anyhow::Result::<String>::Ok(format!("Created at {}", path))
        });
        match res {
            Ok(s) => all_results.push(format!("--- Client: {} ---\n{}", name, s)),
            Err(e) => all_results.push(format!("Error on client {}: {}", name, e)),
        }
    }
    if all_results.is_empty() {
        Err("No CalDAV clients configured.".to_string())
    } else {
        Ok(crate::tools::dtos::AddCalendarItemResponse { result: all_results.join("\n\n") })
    }
}

pub fn tool_update_calendar_item(config: &AppConfig, id: &str, update_json: &str) -> Result<crate::tools::dtos::UpdateCalendarItemResponse, String> {
    let mut all_results = Vec::new();
    for (name, client_config) in &config.caldav_clients {
        let res = block_on(async {
            let client = CalDavClient::new(&client_config.url, Some(&client_config.username), Some(&client_config.password))
                .map_err(|e| anyhow::anyhow!("Client config error: {}", e))?;
            
            let get_resp = client.get(id).await?;
            if !get_resp.status().is_success() {
                let status = get_resp.status();
                let body = String::from_utf8_lossy(&get_resp.into_body()).to_string();
                return Err(anyhow::anyhow!("Failed to fetch event for update: {} - {}", status, body));
            }
            let bytes = get_resp.into_body();
            let body = String::from_utf8_lossy(&bytes).to_string();
            
            let update_parsed: serde_json::Value = serde_json::from_str(update_json).unwrap_or_else(|_| serde_json::json!({}));
            let ical_data = crate::tools::caldav::update_ical_string(&body, &update_parsed);
            
            let resp = client.put(id, ical_data.into_bytes().into()).await?;
            if !resp.status().is_success() {
                let status = resp.status();
                let body = String::from_utf8_lossy(&resp.into_body()).to_string();
                return Err(anyhow::anyhow!("Failed to PUT update event: {} - {}", status, body));
            }
            anyhow::Result::<String>::Ok("Updated successfully".to_string())
        });
        match res {
            Ok(s) => all_results.push(format!("--- Client: {} ---\n{}", name, s)),
            Err(e) => all_results.push(format!("Error on client {}: {}", name, e)),
        }
    }
    if all_results.is_empty() {
        Err("No CalDAV clients configured.".to_string())
    } else {
        Ok(crate::tools::dtos::UpdateCalendarItemResponse { result: all_results.join("\n\n") })
    }
}

pub fn tool_delete_calendar_item(config: &AppConfig, id: &str) -> Result<crate::tools::dtos::DeleteCalendarItemResponse, String> {
    let mut all_results = Vec::new();
    for (name, client_config) in &config.caldav_clients {
        let res = block_on(async {
            let client = CalDavClient::new(&client_config.url, Some(&client_config.username), Some(&client_config.password))
                .map_err(|e| anyhow::anyhow!("Client config error: {}", e))?;
            let resp = client.delete(id).await?;
            if !resp.status().is_success() {
                let status = resp.status();
                let body = String::from_utf8_lossy(&resp.into_body()).to_string();
                return Err(anyhow::anyhow!("Failed to DELETE event: {} - {}", status, body));
            }
            anyhow::Result::<String>::Ok("Deleted successfully".to_string())
        });
        match res {
            Ok(s) => all_results.push(format!("--- Client: {} ---\n{}", name, s)),
            Err(e) => all_results.push(format!("Error on client {}: {}", name, e)),
        }
    }
    if all_results.is_empty() {
        Err("No CalDAV clients configured.".to_string())
    } else {
        Ok(crate::tools::dtos::DeleteCalendarItemResponse { result: all_results.join("\n\n") })
    }
}

pub fn json_to_ical(json_str: &str, uid_override: Option<&str>) -> String {
    let parsed: serde_json::Value = serde_json::from_str(json_str).unwrap_or_else(|_| serde_json::json!({}));
    
    let uid = uid_override.map(|s| s.to_string()).unwrap_or_else(|| {
        format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis())
    });
    
    fn escape_ical_text(text: &str) -> String {
        text.replace("\\", "\\\\")
            .replace(";", "\\;")
            .replace(",", "\\,")
            .replace("\n", "\\n")
            .replace("\r", "")
    }
    
    let summary = escape_ical_text(parsed.get("summary").and_then(|v| v.as_str()).unwrap_or("New Event"));
    let start = parsed.get("start").and_then(|v| v.as_str()).unwrap_or("");
    let end = parsed.get("end").and_then(|v| v.as_str()).unwrap_or("");
    let description = escape_ical_text(parsed.get("description").and_then(|v| v.as_str()).unwrap_or(""));
    let location = escape_ical_text(parsed.get("location").and_then(|v| v.as_str()).unwrap_or(""));

    let start_fmt = start.replace("-", "").replace(":", "");
    let end_fmt = end.replace("-", "").replace(":", "");
    let dtstamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();

    let mut ical = String::new();
    ical.push_str("BEGIN:VCALENDAR\r\n");
    ical.push_str("VERSION:2.0\r\n");
    ical.push_str("BEGIN:VEVENT\r\n");
    ical.push_str(&format!("UID:{}\r\n", uid));
    ical.push_str(&format!("DTSTAMP:{}\r\n", dtstamp));
    
    if !start_fmt.is_empty() {
        if start_fmt.len() == 8 {
            ical.push_str(&format!("DTSTART;VALUE=DATE:{}\r\n", start_fmt));
        } else {
            ical.push_str(&format!("DTSTART:{}\r\n", start_fmt));
        }
    }
    
    if !end_fmt.is_empty() {
        if end_fmt.len() == 8 {
            ical.push_str(&format!("DTEND;VALUE=DATE:{}\r\n", end_fmt));
        } else {
            ical.push_str(&format!("DTEND:{}\r\n", end_fmt));
        }
    }

    ical.push_str(&format!("SUMMARY:{}\r\n", summary));
    if !description.is_empty() {
        ical.push_str(&format!("DESCRIPTION:{}\r\n", description));
    }
    if !location.is_empty() {
        ical.push_str(&format!("LOCATION:{}\r\n", location));
    }
    
    ical.push_str("END:VEVENT\r\n");
    ical.push_str("END:VCALENDAR\r\n");
    ical
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_ical_data tests ---

    #[test]
    fn test_parse_ical_data_summary() {
        let data = "BEGIN:VEVENT\r\nSUMMARY:Test Event\r\nDTSTART:20240101T120000\r\nDTEND:20240101T130000\r\nEND:VEVENT";
        let ev = parse_ical_data("client1", "/cal/item.ics", data);
        assert_eq!(ev.client, "client1");
        assert_eq!(ev.href, "/cal/item.ics");
        assert_eq!(ev.summary, Some("Test Event".to_string()));
    }

    #[test]
    fn test_parse_ical_data_dates() {
        let data = "BEGIN:VEVENT\r\nSUMMARY:Test\r\nDTSTART:20240101T120000\r\nDTEND:20240101T130000\r\nEND:VEVENT";
        let ev = parse_ical_data("c", "/h", data);
        assert_eq!(ev.start, Some("2024-01-01T12:00:00".to_string()));
        assert_eq!(ev.end, Some("2024-01-01T13:00:00".to_string()));
    }

    #[test]
    fn test_parse_ical_data_utc_and_escapes() {
        let data = "BEGIN:VEVENT\r\nSUMMARY:Test UTC\r\nDTSTART:20240101T120000Z\r\nDTEND:invalid\r\nDESCRIPTION:Line 1\\nLine 2\\NLine 3\\,Comma\\;Semicolon\r\nLOCATION:Room 1\\, Building A\\; Fl 2\r\nORGANIZER;CN=Alice:mailto:alice@test.com\r\nEND:VEVENT";
        let ev = parse_ical_data("c", "/h", data);
        assert_eq!(ev.start, Some("2024-01-01T12:00:00Z".to_string()));
        assert_eq!(ev.end, Some("invalid".to_string()));
        assert_eq!(ev.description, Some("Line 1\nLine 2\nLine 3,Comma;Semicolon".to_string()));
        assert_eq!(ev.location, Some("Room 1, Building A; Fl 2".to_string()));
        assert_eq!(ev.organizer, Some("mailto:alice@test.com".to_string()));
    }

    #[test]
    fn test_parse_ical_data_date_only() {
        let data = "BEGIN:VEVENT\r\nSUMMARY:All Day\r\nDTSTART;VALUE=DATE:20240101\r\nDTEND;VALUE=DATE:20240102\r\nEND:VEVENT";
        let ev = parse_ical_data("c", "/h", data);
        assert_eq!(ev.start, Some("2024-01-01".to_string()));
        assert_eq!(ev.end, Some("2024-01-02".to_string()));
    }

    #[test]
    fn test_parse_ical_data_description_location() {
        let data = "BEGIN:VEVENT\r\nSUMMARY:Mtg\r\nDESCRIPTION:Discuss project\r\nLOCATION:Room 42\r\nORGANIZER:mailto:alice@test.com\r\nEND:VEVENT";
        let ev = parse_ical_data("c", "/h", data);
        assert_eq!(ev.description, Some("Discuss project".to_string()));
        assert_eq!(ev.location, Some("Room 42".to_string()));
        assert_eq!(ev.organizer, Some("mailto:alice@test.com".to_string()));
    }

    #[test]
    fn test_parse_ical_data_unfolds_lines() {
        let data = "BEGIN:VEVENT\r\nSUMMARY:Very long\r\n summary line\r\nDTSTART:20240101T120000\r\nEND:VEVENT";
        let ev = parse_ical_data("c", "/h", data);
        // The code unfolds by removing the leading space and concatenating without adding a separator
        assert_eq!(ev.summary, Some("Very longsummary line".to_string()));
    }

    // --- json_to_ical tests ---

    #[test]
    fn test_json_to_ical_basic() {
        let input = r#"{"summary":"Test","start":"2024-01-01T12:00:00","end":"2024-01-01T13:00:00","description":"desc","location":"loc"}"#;
        let ical = json_to_ical(input, None);
        assert!(ical.starts_with("BEGIN:VCALENDAR"));
        assert!(ical.contains("BEGIN:VEVENT"));
        assert!(ical.contains("END:VEVENT"));
        assert!(ical.contains("END:VCALENDAR"));
        assert!(ical.contains("SUMMARY:Test"));
        assert!(ical.contains("DESCRIPTION:desc"));
        assert!(ical.contains("LOCATION:loc"));
    }

    #[test]
    fn test_json_to_ical_date_only_and_invalid_json() {
        let input = r#"{"summary":"Date Only","start":"2024-01-01","end":"2024-01-02"}"#;
        let ical = json_to_ical(input, None);
        assert!(ical.contains("DTSTART;VALUE=DATE:20240101"));
        assert!(ical.contains("DTEND;VALUE=DATE:20240102"));

        let invalid = "not json at all";
        let ical_invalid = json_to_ical(invalid, None);
        assert!(ical_invalid.contains("SUMMARY:New Event"));
    }

    #[test]
    fn test_json_to_ical_minimal() {
        // Even an empty JSON should produce a valid structure
        let input = "{}";
        let ical = json_to_ical(input, None);
        assert!(ical.starts_with("BEGIN:VCALENDAR"));
        assert!(ical.contains("BEGIN:VEVENT"));
        assert!(ical.contains("END:VEVENT"));
        assert!(ical.contains("END:VCALENDAR"));
        // Should have a default summary
        assert!(ical.contains("SUMMARY:New Event"));
    }

    #[test]
    fn test_json_to_ical_with_uid() {
        let input = r#"{"summary":"Test"}"#;
        let ical = json_to_ical(input, Some("custom-uid-123"));
        assert!(ical.contains("UID:custom-uid-123"));
    }

    #[test]
    fn test_json_to_ical_escapes_special_chars() {
        let input = r#"{"summary":"Hello;World,Line1\nLine2"}"#;
        let ical = json_to_ical(input, None);
        assert!(ical.contains("Hello\\;World\\,Line1\\nLine2"));
    }

    // --- update_ical_string tests ---

    #[test]
    fn test_update_ical_string_replaces_summary() {
        let original = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:Old\r\nDTSTART:20240101T120000\r\nEND:VEVENT\r\nEND:VCALENDAR";
        let updates = serde_json::json!({"summary": "New"});
        let result = update_ical_string(original, &updates);
        assert!(result.contains("SUMMARY:New"));
        assert!(!result.contains("SUMMARY:Old"));
    }

    #[test]
    fn test_update_ical_string_adds_missing_field() {
        // Test that a missing SUMMARY gets added at the end of VEVENT
        let original = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nDTSTART:20240101T120000\r\nEND:VEVENT\r\nEND:VCALENDAR";
        let updates = serde_json::json!({
            "summary": "Added Summary",
            "start": "2024-05-01",
            "end": "2024-05-02",
            "description": "Added Desc",
            "location": "Added Loc"
        });
        let result = update_ical_string(original, &updates);
        assert!(result.contains("SUMMARY:Added Summary"));
        assert!(result.contains("DTSTART;VALUE=DATE:20240501"));
        assert!(result.contains("DTEND;VALUE=DATE:20240502"));
        assert!(result.contains("DESCRIPTION:Added Desc"));
        assert!(result.contains("LOCATION:Added Loc"));
    }

    #[test]
    fn test_update_ical_string_all_fields_with_folding() {
        let original = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:Old Summary\r\n folded line\r\nDTSTART:20240101T120000\r\nDTEND:20240101T130000\r\nDESCRIPTION:Old Desc\r\n LOCATION:Old Loc\r\nEND:VEVENT\r\nEND:VCALENDAR";
        let updates = serde_json::json!({
            "summary": "New Summary",
            "start": "2024-02-01T10:00:00",
            "end": "2024-02-01T11:00:00",
            "description": "New Desc with; special, chars\nline2",
            "location": "New Loc"
        });
        let result = update_ical_string(original, &updates);
        assert!(result.contains("SUMMARY:New Summary"));
        assert!(result.contains("DTSTART:20240201T100000"));
        assert!(result.contains("DTEND:20240201T110000"));
        assert!(result.contains("DESCRIPTION:New Desc with\\; special\\, chars\\nline2"));
        assert!(result.contains("LOCATION:New Loc"));
        assert!(!result.contains("folded line"));
    }

    #[test]
    fn test_update_ical_string_replaces_dtstart() {
        let original = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:Test\r\nDTSTART:20240101\r\nEND:VEVENT\r\nEND:VCALENDAR";
        let updates = serde_json::json!({"start": "20250101"});
        let result = update_ical_string(original, &updates);
        assert!(result.contains("DTSTART;VALUE=DATE:20250101") || result.contains("DTSTART:20250101"));
    }

    #[test]
    fn test_update_ical_string_no_updates_preserves() {
        let original = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:Keep\r\nDTSTART:20240101T120000\r\nEND:VEVENT\r\nEND:VCALENDAR";
        let updates = serde_json::json!({});
        let result = update_ical_string(original, &updates);
        assert!(result.contains("SUMMARY:Keep"));
    }

    // --- CalDAV Tool Config & Client Tests ---

    #[test]
    fn test_caldav_tools_empty_config() {
        let config = AppConfig::default();

        assert_eq!(tool_add_calendar_item(&config, "{}").unwrap_err(), "No CalDAV clients configured.");
        assert_eq!(tool_update_calendar_item(&config, "/item.ics", "{}").unwrap_err(), "No CalDAV clients configured.");
        assert_eq!(tool_delete_calendar_item(&config, "/item.ics").unwrap_err(), "No CalDAV clients configured.");

        let search_res = tool_search_calendar(&config, "test").unwrap();
        assert_eq!(search_res.results, serde_json::to_string_pretty(&CalDavResponse { results: vec![], errors: vec![] }).unwrap());

        let get_res = tool_get_calendar(&config, "2024-01-01", "2024-01-02").unwrap();
        assert_eq!(get_res.results, serde_json::to_string_pretty(&CalDavResponse { results: vec![], errors: vec![] }).unwrap());

        let item_res = tool_get_calendar_item(&config, "/item.ics").unwrap();
        assert_eq!(item_res.result, serde_json::to_string_pretty(&CalDavResponse { results: vec![], errors: vec![] }).unwrap());
    }

    #[test]
    fn test_caldav_tools_unreachable_client() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let mut config = AppConfig::default();
        config.caldav_clients.insert("test_client".to_string(), crate::config::CalDavClient {
            url: "http://127.0.0.1:1".to_string(),
            username: "user".to_string(),
            password: "password".to_string(),
        });

        let search_res = tool_search_calendar(&config, "test").unwrap();
        assert!(search_res.results.contains("Error on client test_client"));

        let get_res = tool_get_calendar(&config, "2024-01-01", "2024-01-02").unwrap();
        assert!(get_res.results.contains("Error on client test_client"));

        let item_res = tool_get_calendar_item(&config, "/item.ics").unwrap();
        assert!(item_res.result.contains("Error on client test_client"));

        let add_res = tool_add_calendar_item(&config, "{}").unwrap();
        assert!(add_res.result.contains("Error on client test_client"));

        let update_res = tool_update_calendar_item(&config, "/item.ics", "{}").unwrap();
        assert!(update_res.result.contains("Error on client test_client"));

        let delete_res = tool_delete_calendar_item(&config, "/item.ics").unwrap();
        assert!(delete_res.result.contains("Error on client test_client"));
    }

    fn spawn_mock_caldav_server() -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                while let Ok((mut socket, _)) = listener.accept().await {
                    tokio::spawn(async move {
                        use tokio::io::{AsyncReadExt, AsyncWriteExt};
                        let mut buf = [0u8; 4096];
                        let n = socket.read(&mut buf).await.unwrap_or(0);
                        let req = String::from_utf8_lossy(&buf[..n]);

                        let response = if req.starts_with("GET /item1.ics") {
                            "HTTP/1.1 200 OK\r\nContent-Type: text/calendar\r\nContent-Length: 104\r\n\r\nBEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:Existing Item\r\nDTSTART:20240101T100000Z\r\nEND:VEVENT\r\nEND:VCALENDAR".to_string()
                        } else if req.starts_with("GET /notfound") {
                            "HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\n\r\nNot Found".to_string()
                        } else if req.starts_with("PUT") {
                            "HTTP/1.1 201 Created\r\nContent-Length: 0\r\n\r\n".to_string()
                        } else if req.starts_with("DELETE /item1.ics") {
                            "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n".to_string()
                        } else if req.starts_with("DELETE /fail") {
                            "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 5\r\n\r\nError".to_string()
                        } else if req.starts_with("PROPFIND") {
                            let xml_body = r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
 <d:response>
  <d:href>/calendars/primary/</d:href>
  <d:propstat>
   <d:prop>
    <d:resourcetype><d:collection/><c:calendar/></d:resourcetype>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>
</d:multistatus>"#;
                            format!("HTTP/1.1 207 Multi-Status\r\nContent-Type: application/xml\r\nContent-Length: {}\r\n\r\n{}", xml_body.len(), xml_body)
                        } else if req.starts_with("REPORT") {
                            let xml_body = r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
 <d:response>
  <d:href>/calendars/primary/event1.ics</d:href>
  <d:propstat>
   <d:prop>
    <c:calendar-data>BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
SUMMARY:Meeting with Bob
DTSTART:20240101T100000Z
DTEND:20240101T110000Z
END:VEVENT
END:VCALENDAR</c:calendar-data>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>
</d:multistatus>"#;
                            format!("HTTP/1.1 207 Multi-Status\r\nContent-Type: application/xml\r\nContent-Length: {}\r\n\r\n{}", xml_body.len(), xml_body)
                        } else {
                            "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n".to_string()
                        };

                        let _ = socket.write_all(response.as_bytes()).await;
                    });
                }
            });
        });

        format!("http://{}", addr)
    }

    #[test]
    fn test_caldav_tools_mock_server() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let server_url = spawn_mock_caldav_server();

        let mut config = AppConfig::default();
        config.caldav_clients.insert("mock_client".to_string(), crate::config::CalDavClient {
            url: server_url,
            username: "user".to_string(),
            password: "password".to_string(),
        });

        // 1. Search calendar
        let search_res = tool_search_calendar(&config, "Bob").unwrap();
        assert!(search_res.results.contains("Meeting with Bob"));

        // 2. Get calendar (date range)
        let get_res = tool_get_calendar(&config, "2024-01-01", "2024-01-02").unwrap();
        assert!(get_res.results.contains("Meeting with Bob"));

        // 3. Get calendar item success
        let item_res = tool_get_calendar_item(&config, "/item1.ics").unwrap();
        assert!(item_res.result.contains("Existing Item"));

        // 4. Get calendar item 404
        let item_res_404 = tool_get_calendar_item(&config, "/notfound").unwrap();
        assert!(item_res_404.result.contains("Not found by href"));

        // 5. Add calendar item
        let add_res = tool_add_calendar_item(&config, r#"{"summary":"New Mtg"}"#).unwrap();
        assert!(add_res.result.contains("Created at /calendars/primary/"));

        // 6. Update calendar item success
        let update_res = tool_update_calendar_item(&config, "/item1.ics", r#"{"summary":"Updated Mtg"}"#).unwrap();
        assert!(update_res.result.contains("Updated successfully"));

        // 7. Update calendar item 404
        let update_res_404 = tool_update_calendar_item(&config, "/notfound", r#"{"summary":"Updated Mtg"}"#).unwrap();
        assert!(update_res_404.result.contains("Failed to fetch event for update"));

        // 8. Delete calendar item success
        let delete_res = tool_delete_calendar_item(&config, "/item1.ics").unwrap();
        assert!(delete_res.result.contains("Deleted successfully"));

        // 9. Delete calendar item 500 error
        let delete_res_err = tool_delete_calendar_item(&config, "/fail").unwrap();
        assert!(delete_res_err.result.contains("Failed to DELETE event"));
    }
}

