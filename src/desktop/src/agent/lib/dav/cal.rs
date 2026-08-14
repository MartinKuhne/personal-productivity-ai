//! CalDAV agent tools — search, retrieve, create, update, and delete calendar events across configured CalDAV servers.
//!
//! Layering:
//!
//! 1. [`DavClient`] (in `dav::client`) owns the per-server
//!    connection. It wraps both a `fast_dav_rs::CalDavClient` and
//!    a `fast_dav_rs::CardDavClient` plus the per-server metadata
//!    (name, base URL, username). The CalDAV methods
//!    (`search_calendar`, `get_calendar`, `get_calendar_item`,
//!    `add_calendar_item`, `update_calendar_item`,
//!    `delete_calendar_item`) are sync (use
//!    `agent::tools::blocking::block_on` internally) so the
//!    LLM-tool loop can call them without an async runtime.
//! 2. The `tool_*` functions in this module are the LLM-adapter
//!    layer. They iterate `config.caldav_clients`, build a
//!    [`DavClient`] per server, aggregate the per-server results,
//!    and serialize to the LLM-facing DTO from
//!    `crate::tools::dtos`.
//! 3. Pure helpers (`parse_ical_data`, `json_to_ical`,
//!    `update_ical_string`) are independent of any client and are
//!    `pub(super)` so `DavClient` can call them.
//!
//! Unit tests live in the sibling `cal_tests.rs` sidecar.

use crate::tools::dtos::{
    AddCalendarItemResponse, DeleteCalendarItemResponse, GetCalendarItemResponse,
    GetCalendarResponse, SearchCalendarResponse, UpdateCalendarItemResponse,
};
// Re-exported at `crate::lib::dav::DavClient` so the
// LLM-adapter `tool_*` wrappers below can build one per server
// without reaching into `client` directly.
use super::DavClient;

#[derive(serde::Serialize, Debug, Clone, PartialEq, Eq)]
pub struct CalDavEventDetails {
    pub client: String,
    pub id: String,
    pub href: String,
    pub summary: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub organizer: Option<String>,
}

#[derive(serde::Serialize, Debug, Default)]
pub struct CalDavResponse {
    pub results: Vec<CalDavEventDetails>,
    pub errors: Vec<String>,
}

pub(crate) fn parse_ical_data(client: &str, href: &str, data: &str) -> CalDavEventDetails {
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
            format!(
                "{}-{}-{}T{}:{}:{}",
                &d[0..4],
                &d[4..6],
                &d[6..8],
                &d[9..11],
                &d[11..13],
                &d[13..15]
            )
        } else if d.len() == 16 && d.chars().nth(8) == Some('T') && d.ends_with('Z') {
            format!(
                "{}-{}-{}T{}:{}:{}Z",
                &d[0..4],
                &d[4..6],
                &d[6..8],
                &d[9..11],
                &d[11..13],
                &d[13..15]
            )
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
            let desc = rest
                .replace("\\n", "\n")
                .replace("\\N", "\n")
                .replace("\\,", ",")
                .replace("\\;", ";");
            event.description = Some(desc.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("LOCATION:") {
            let loc = rest.replace("\\,", ",").replace("\\;", ";");
            event.location = Some(loc.trim().to_string());
        } else if line.starts_with("DTSTART:") || line.starts_with("DTSTART;") {
            if let Some(idx) = line.find(':') {
                event.start = Some(format_ical_date(&line[idx + 1..]));
            }
        } else if line.starts_with("DTEND:") || line.starts_with("DTEND;") {
            if let Some(idx) = line.find(':') {
                event.end = Some(format_ical_date(&line[idx + 1..]));
            }
        } else if (line.starts_with("ORGANIZER:") || line.starts_with("ORGANIZER;"))
            && let Some(idx) = line.find(':')
        {
            event.organizer = Some(line[idx + 1..].trim().to_string());
        }
    }

    event
}

/// `DavClient` lives in `crate::lib::dav::client` and is
/// re-exported from `crate::lib::dav` as
/// [`crate::lib::dav::DavClient`]. The LLM-adapter
/// `tool_*` wrappers below build one per entry in
/// `config.caldav_clients` and delegate.
pub fn update_ical_string(original: &str, updates: &serde_json::Value) -> String {
    let mut out = String::new();
    let mut in_vevent = false;

    let mut has_summary = false;
    let mut has_start = false;
    let mut has_end = false;
    let mut has_desc = false;
    let mut has_loc = false;

    fn escape_ical_text(text: &str) -> String {
        text.replace("\\", "\\\\")
            .replace(";", "\\;")
            .replace(",", "\\,")
            .replace("\n", "\\n")
            .replace("\r", "")
    }

    let u_summary = updates
        .get("summary")
        .and_then(|v| v.as_str())
        .map(escape_ical_text);
    let u_start = updates
        .get("start")
        .and_then(|v| v.as_str())
        .map(|s| s.replace("-", "").replace(":", ""));
    let u_end = updates
        .get("end")
        .and_then(|v| v.as_str())
        .map(|s| s.replace("-", "").replace(":", ""));
    let u_desc = updates
        .get("description")
        .and_then(|v| v.as_str())
        .map(escape_ical_text);
    let u_loc = updates
        .get("location")
        .and_then(|v| v.as_str())
        .map(escape_ical_text);

    let mut lines = original.lines().peekable();
    while let Some(line) = lines.next() {
        if line.starts_with("BEGIN:VEVENT") {
            in_vevent = true;
            out.push_str(&format!("{}\r\n", line));
            continue;
        }

        if line.starts_with("END:VEVENT") {
            if let Some(s) = &u_summary
                && !has_summary
            {
                out.push_str(&format!("SUMMARY:{}\r\n", s));
            }
            if let Some(s) = &u_start
                && !has_start
            {
                if s.len() == 8 {
                    out.push_str(&format!("DTSTART;VALUE=DATE:{}\r\n", s));
                } else {
                    out.push_str(&format!("DTSTART:{}\r\n", s));
                }
            }
            if let Some(e) = &u_end
                && !has_end
            {
                if e.len() == 8 {
                    out.push_str(&format!("DTEND;VALUE=DATE:{}\r\n", e));
                } else {
                    out.push_str(&format!("DTEND:{}\r\n", e));
                }
            }
            if let Some(s) = &u_desc
                && !has_desc
            {
                out.push_str(&format!("DESCRIPTION:{}\r\n", s));
            }
            if let Some(s) = &u_loc
                && !has_loc
            {
                out.push_str(&format!("LOCATION:{}\r\n", s));
            }

            out.push_str(&format!("{}\r\n", line));
            in_vevent = false;
            continue;
        }

        if in_vevent {
            let mut replace_line = None;
            if line.starts_with("SUMMARY:") {
                has_summary = true;
                if let Some(s) = &u_summary {
                    replace_line = Some(format!("SUMMARY:{}", s));
                }
            } else if line.starts_with("DTSTART:") || line.starts_with("DTSTART;") {
                has_start = true;
                if let Some(s) = &u_start {
                    replace_line = Some(if s.len() == 8 {
                        format!("DTSTART;VALUE=DATE:{}", s)
                    } else {
                        format!("DTSTART:{}", s)
                    });
                }
            } else if line.starts_with("DTEND:") || line.starts_with("DTEND;") {
                has_end = true;
                if let Some(e) = &u_end {
                    replace_line = Some(if e.len() == 8 {
                        format!("DTEND;VALUE=DATE:{}", e)
                    } else {
                        format!("DTEND:{}", e)
                    });
                }
            } else if line.starts_with("DESCRIPTION:") {
                has_desc = true;
                if let Some(s) = &u_desc {
                    replace_line = Some(format!("DESCRIPTION:{}", s));
                }
            } else if line.starts_with("LOCATION:") {
                has_loc = true;
                if let Some(s) = &u_loc {
                    replace_line = Some(format!("LOCATION:{}", s));
                }
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

// ---------------------------------------------------------------------------
// LLM-adapter layer — the `tool_*` functions. Each one iterates the
// configured CalDAV clients, delegates to a [`DavClient`] method, and
// serialises the aggregated results to the LLM-facing DTO from
// `crate::tools::dtos`.
// ---------------------------------------------------------------------------

/// Iterate every configured CalDAV client, invoke `f` against each
/// one, and split the per-server outcomes into a `results` vec and
/// an `errors` vec. Errors are recorded as `"Error on client {name}: {e}"`
/// — the same string the previous inline-loop code produced — so the
/// existing test assertions keep working.
fn for_each_client<T, F>(config: &crate::config::AgentConfig, mut f: F) -> (Vec<T>, Vec<String>)
where
    F: FnMut(&str, &DavClient) -> Result<T, String>,
{
    let mut results = Vec::new();
    let mut errors = Vec::new();
    for (name, cc) in config.caldav_clients() {
        match DavClient::new(name.clone(), cc).and_then(|c| f(name, &c)) {
            Ok(item) => results.push(item),
            Err(e) => errors.push(format!("Error on client {}: {}", name, e)),
        }
    }
    (results, errors)
}

/// Like [`for_each_client`] but for methods that return a `Vec` per
/// server (search, get). The per-server `Vec`s are flattened into the
/// aggregate `results` vec.
fn for_each_client_vec<T, F>(config: &crate::config::AgentConfig, mut f: F) -> (Vec<T>, Vec<String>)
where
    F: FnMut(&str, &DavClient) -> Result<Vec<T>, String>,
{
    let mut results = Vec::new();
    let mut errors = Vec::new();
    for (name, cc) in config.caldav_clients() {
        match DavClient::new(name.clone(), cc).and_then(|c| f(name, &c)) {
            Ok(mut v) => results.append(&mut v),
            Err(e) => errors.push(format!("Error on client {}: {}", name, e)),
        }
    }
    (results, errors)
}

/// Serialize a [`CalDavResponse`] to a pretty JSON string. Falls back
/// to `"{}"` if the JSON encoder chokes (it shouldn't — the type
/// fields are all `String` / `Option<String>` — but the inline
/// fallback matches the previous behaviour so the LLM never sees an
/// empty error).
fn serialize_response(resp: &CalDavResponse) -> String {
    serde_json::to_string_pretty(resp).unwrap_or_else(|_| "{}".to_string())
}

pub fn tool_search_calendar(
    config: &crate::config::AgentConfig,
    keyword: &str,
) -> Result<SearchCalendarResponse, String> {
    let (results, errors) = for_each_client_vec(config, |_, c| c.search_calendar(keyword));
    Ok(SearchCalendarResponse {
        results: serialize_response(&CalDavResponse { results, errors }),
    })
}

pub fn tool_get_calendar(
    config: &crate::config::AgentConfig,
    start: &str,
    end: &str,
) -> Result<GetCalendarResponse, String> {
    let (results, errors) = for_each_client_vec(config, |_, c| c.get_calendar(start, end));
    Ok(GetCalendarResponse {
        results: serialize_response(&CalDavResponse { results, errors }),
    })
}

pub fn tool_get_calendar_item(
    config: &crate::config::AgentConfig,
    id: &str,
) -> Result<GetCalendarItemResponse, String> {
    let (results, errors) = for_each_client(config, |_, c| c.get_calendar_item(id));
    Ok(GetCalendarItemResponse {
        result: serialize_response(&CalDavResponse { results, errors }),
    })
}

pub fn tool_add_calendar_item(
    config: &crate::config::AgentConfig,
    item_json: &str,
) -> Result<AddCalendarItemResponse, String> {
    // `add_calendar_item` is special: it acts on the *first* configured
    // CalDAV client (no "default calendar" concept in CalDAV). The
    // per-server output is a single status string, so the aggregation
    // shape doesn't fit `for_each_client` cleanly.
    let mut all_results = Vec::new();
    if let Some((name, cc)) = config.caldav_clients().iter().next() {
        match DavClient::new(name.clone(), cc).and_then(|c| c.add_calendar_item(item_json)) {
            Ok(s) => all_results.push(format!("--- Client: {} ---\n{}", name, s)),
            Err(e) => all_results.push(format!("Error on client {}: {}", name, e)),
        }
    }
    if all_results.is_empty() {
        Err("No CalDAV clients configured.".to_string())
    } else {
        Ok(AddCalendarItemResponse {
            result: all_results.join("\n\n"),
        })
    }
}

pub fn tool_update_calendar_item(
    config: &crate::config::AgentConfig,
    id: &str,
    update_json: &str,
) -> Result<UpdateCalendarItemResponse, String> {
    let mut all_results = Vec::new();
    for (name, cc) in config.caldav_clients() {
        match DavClient::new(name.clone(), cc).and_then(|c| c.update_calendar_item(id, update_json))
        {
            Ok(s) => all_results.push(format!("--- Client: {} ---\n{}", name, s)),
            Err(e) => all_results.push(format!("Error on client {}: {}", name, e)),
        }
    }
    if all_results.is_empty() {
        Err("No CalDAV clients configured.".to_string())
    } else {
        Ok(UpdateCalendarItemResponse {
            result: all_results.join("\n\n"),
        })
    }
}

pub fn tool_delete_calendar_item(
    config: &crate::config::AgentConfig,
    id: &str,
) -> Result<DeleteCalendarItemResponse, String> {
    let mut all_results = Vec::new();
    for (name, cc) in config.caldav_clients() {
        match DavClient::new(name.clone(), cc).and_then(|c| c.delete_calendar_item(id)) {
            Ok(()) => all_results.push(format!("--- Client: {} ---\nDeleted successfully", name)),
            Err(e) => all_results.push(format!("Error on client {}: {}", name, e)),
        }
    }
    if all_results.is_empty() {
        Err("No CalDAV clients configured.".to_string())
    } else {
        Ok(DeleteCalendarItemResponse {
            result: all_results.join("\n\n"),
        })
    }
}

pub fn json_to_ical(json_str: &str, uid_override: Option<&str>) -> String {
    let parsed: serde_json::Value =
        serde_json::from_str(json_str).unwrap_or_else(|_| serde_json::json!({}));

    let uid = uid_override.map(|s| s.to_string()).unwrap_or_else(|| {
        format!(
            "{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        )
    });

    fn escape_ical_text(text: &str) -> String {
        text.replace("\\", "\\\\")
            .replace(";", "\\;")
            .replace(",", "\\,")
            .replace("\n", "\\n")
            .replace("\r", "")
    }

    let summary = escape_ical_text(
        parsed
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("New Event"),
    );
    let start = parsed.get("start").and_then(|v| v.as_str()).unwrap_or("");
    let end = parsed.get("end").and_then(|v| v.as_str()).unwrap_or("");
    let description = escape_ical_text(
        parsed
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or(""),
    );
    let location = escape_ical_text(
        parsed
            .get("location")
            .and_then(|v| v.as_str())
            .unwrap_or(""),
    );

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

// ---------------------------------------------------------------------------
// Tests live in the sibling `caldav_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "cal_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "cal_proptests.rs"]
mod cal_proptests;
