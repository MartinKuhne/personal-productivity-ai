//! CalDAV agent tools — search, retrieve, create, update, and delete calendar events across configured CalDAV servers.
//!
//! Layering:
//!
//! 1. [`DavClient`] owns the per-server connection. It wraps
//!    `fast_dav_rs::CalDavClient` plus the per-server metadata (name,
//!    base URL, username) needed by the discovery helpers. The
//!    methods are sync (use [`block_on`] under the hood) so the
//!    LLM-tool loop can call them without an async runtime.
//! 2. The `tool_*` functions in this module are the LLM-adapter
//!    layer. They iterate `config.caldav_clients`, build a
//!    [`DavClient`] per server, aggregate the per-server results,
//!    and serialize to the LLM-facing DTO from
//!    `crate::agent::tools::dtos`.
//! 3. Pure helpers (`parse_ical_data`, `json_to_ical`,
//!    `update_ical_string`) are independent of any client.
//!
//! Unit tests live in the sibling `cal_tests.rs` sidecar.

use crate::agent::tools::blocking::block_on;
use crate::agent::tools::dtos::{
    AddCalendarItemResponse, DeleteCalendarItemResponse, GetCalendarItemResponse,
    GetCalendarResponse, SearchCalendarResponse, UpdateCalendarItemResponse,
};
use crate::config::{AppConfig, CalDavClient as CalDavClientConfig};
use fast_dav_rs::CalDavClient;

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

/// A single CalDAV server connection.
///
/// Holds the [`CalDavClient`] plus the per-server metadata needed by
/// the discovery helpers (base URL for the initial PROPFIND, username
/// for the Fastmail `/dav/principals/user/{username}/` fallback).
/// Construct one per entry in `config.caldav_clients` and call the
/// `search_calendar` / `get_calendar` / `get_calendar_item` /
/// `add_calendar_item` / `update_calendar_item` /
/// `delete_calendar_item` methods to drive the wire protocol.
pub struct DavClient {
    name: String,
    base_url: String,
    username: String,
    client: CalDavClient,
}

impl DavClient {
    /// Build a `DavClient` from a name and a [`CalDavClientConfig`]
    /// entry. Returns an error if the underlying `CalDavClient` cannot
    /// be built (e.g. malformed URL).
    pub fn new(name: String, config: &CalDavClientConfig) -> Result<Self, String> {
        let client = CalDavClient::new(&config.url, Some(&config.username), Some(&config.password))
            .map_err(|e| format!("Client config error: {}", e))?;
        Ok(Self {
            name,
            base_url: config.url.clone(),
            username: config.username.clone(),
            client,
        })
    }

    /// The friendly name from the config map. Returned in every
    /// [`CalDavEventDetails::client`] field so the LLM can attribute
    /// results to a server.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Discover every calendar href this server exposes. Tries
    /// `list_calendars` first; on empty, falls through to
    /// `discover_calendar_home_set` → `discover_current_user_principal`,
    /// then a Fastmail-style `/dav/principals/user/{username}/` guess.
    async fn list_calendar_hrefs(&self) -> anyhow::Result<Vec<String>> {
        if let Ok(calendars) = self.client.list_calendars(&self.base_url).await
            && !calendars.is_empty()
        {
            return Ok(calendars.into_iter().map(|c| c.href).collect());
        }

        if let Ok(homes) = self.client.discover_calendar_home_set(&self.base_url).await
            && let Some(home) = homes.first()
            && let Ok(calendars) = self.client.list_calendars(home).await
            && !calendars.is_empty()
        {
            return Ok(calendars.into_iter().map(|c| c.href).collect());
        }

        let mut principal_opt = self
            .client
            .discover_current_user_principal()
            .await
            .ok()
            .flatten();

        // Fallback for Fastmail or other servers that use /dav/principals/user/username/
        if principal_opt.is_none() {
            let base_trimmed = self.base_url.trim_end_matches('/');
            let guess = format!("{}/dav/principals/user/{}/", base_trimmed, self.username);
            if let Ok(homes) = self.client.discover_calendar_home_set(&guess).await
                && !homes.is_empty()
            {
                principal_opt = Some(guess);
            }
        }

        let principal = principal_opt.ok_or_else(|| anyhow::anyhow!("No principal found"))?;
        let homes = self.client.discover_calendar_home_set(&principal).await?;
        let home = homes
            .first()
            .ok_or_else(|| anyhow::anyhow!("No calendar home found"))?;
        let calendars = self.client.list_calendars(home).await?;
        Ok(calendars.into_iter().map(|c| c.href).collect())
    }

    /// Search every calendar on this server for events whose
    /// `calendar-data` contains `keyword` (case-insensitive).
    pub fn search_calendar(&self, keyword: &str) -> Result<Vec<CalDavEventDetails>, String> {
        let kw = keyword.to_lowercase();
        let name = self.name.clone();
        block_on(async {
            let cals = self.list_calendar_hrefs().await?;
            let mut matches = Vec::new();
            for cal_path in cals {
                let items = self
                    .client
                    .calendar_query_timerange(&cal_path, "VEVENT", None, None, true)
                    .await?;
                for item in items {
                    if let Some(data) = &item.calendar_data
                        && data.to_lowercase().contains(&kw)
                    {
                        matches.push(parse_ical_data(&name, &item.href, data));
                    }
                }
            }
            anyhow::Result::<Vec<_>>::Ok(matches)
        })
        .map_err(|e| e.to_string())
    }

    /// Return every VEVENT in every calendar on this server whose
    /// `DTSTART` falls between `start` and `end` (inclusive). Dates
    /// in `YYYY-MM-DD` form are widened to full-day boundaries in UTC.
    pub fn get_calendar(&self, start: &str, end: &str) -> Result<Vec<CalDavEventDetails>, String> {
        let format_caldav_date = |d: &str, is_end: bool| -> String {
            if d.len() == 10 && d.chars().nth(4) == Some('-') && d.chars().nth(7) == Some('-') {
                let clean = d.replace("-", "");
                if is_end {
                    format!("{}T235959Z", clean)
                } else {
                    format!("{}T000000Z", clean)
                }
            } else {
                d.to_string()
            }
        };

        let start_fmt = format_caldav_date(start, false);
        let end_fmt = format_caldav_date(end, true);
        let name = self.name.clone();
        block_on(async {
            let cals = self.list_calendar_hrefs().await?;
            let mut matches = Vec::new();
            for cal_path in cals {
                let items = self
                    .client
                    .calendar_query_timerange(
                        &cal_path,
                        "VEVENT",
                        Some(&start_fmt),
                        Some(&end_fmt),
                        true,
                    )
                    .await?;
                for item in items {
                    if let Some(data) = &item.calendar_data {
                        matches.push(parse_ical_data(&name, &item.href, data));
                    }
                }
            }
            anyhow::Result::<Vec<_>>::Ok(matches)
        })
        .map_err(|e| e.to_string())
    }

    /// Fetch a single calendar item by its href (`id`). The wire
    /// request is a plain `GET {id}`. Returns an error if the server
    /// responds non-2xx — the error string includes the status line
    /// and the body so the operator can diagnose 404 vs auth failure.
    pub fn get_calendar_item(&self, id: &str) -> Result<CalDavEventDetails, String> {
        let name = self.name.clone();
        block_on(async {
            let resp = self.client.get(id).await?;
            if !resp.status().is_success() {
                let status = resp.status();
                let bytes = resp.into_body();
                let body = String::from_utf8_lossy(&bytes).to_string();
                return Err(anyhow::anyhow!("Not found by href: {} - {}", status, body));
            }
            let bytes = resp.into_body();
            let body = String::from_utf8_lossy(&bytes).to_string();
            anyhow::Result::<CalDavEventDetails>::Ok(parse_ical_data(&name, id, &body))
        })
        .map_err(|e| e.to_string())
    }

    /// Create a new VEVENT on the first calendar this server exposes
    /// (no "default calendar" concept in CalDAV; the first discovery
    /// hit is what every other tool does). `item_json` is the same
    /// LLM-facing JSON the [`json_to_ical`] helper accepts.
    pub fn add_calendar_item(&self, item_json: &str) -> Result<String, String> {
        block_on(async {
            let cals = self.list_calendar_hrefs().await?;
            let default_cal = cals
                .first()
                .ok_or_else(|| anyhow::anyhow!("No calendar found to add to"))?;
            let uid = format!(
                "{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
            );
            let path = format!("{}{}.ics", default_cal, uid);
            let ical_data = json_to_ical(item_json, Some(&uid));
            let resp = self
                .client
                .put(&path, ical_data.into_bytes().into())
                .await?;
            if !resp.status().is_success() {
                let status = resp.status();
                let body = String::from_utf8_lossy(&resp.into_body()).to_string();
                return Err(anyhow::anyhow!(
                    "Failed to PUT event: {} - {}",
                    status,
                    body
                ));
            }
            anyhow::Result::<String>::Ok(format!("Created at {}", path))
        })
        .map_err(|e| e.to_string())
    }

    /// Update an existing VEVENT. GETs the current iCal body, merges
    /// `update_json` into it via [`update_ical_string`], and PUTs the
    /// result back. Returns an error if the GET 404s.
    pub fn update_calendar_item(&self, id: &str, update_json: &str) -> Result<String, String> {
        block_on(async {
            let get_resp = self.client.get(id).await?;
            if !get_resp.status().is_success() {
                let status = get_resp.status();
                let body = String::from_utf8_lossy(&get_resp.into_body()).to_string();
                return Err(anyhow::anyhow!(
                    "Failed to fetch event for update: {} - {}",
                    status,
                    body
                ));
            }
            let bytes = get_resp.into_body();
            let body = String::from_utf8_lossy(&bytes).to_string();

            let update_parsed: serde_json::Value =
                serde_json::from_str(update_json).unwrap_or_else(|_| serde_json::json!({}));
            let ical_data = update_ical_string(&body, &update_parsed);

            let resp = self.client.put(id, ical_data.into_bytes().into()).await?;
            if !resp.status().is_success() {
                let status = resp.status();
                let body = String::from_utf8_lossy(&resp.into_body()).to_string();
                return Err(anyhow::anyhow!(
                    "Failed to PUT update event: {} - {}",
                    status,
                    body
                ));
            }
            anyhow::Result::<String>::Ok("Updated successfully".to_string())
        })
        .map_err(|e| e.to_string())
    }

    /// DELETE a calendar item by href. Returns the error string from
    /// the server if the response is non-2xx.
    pub fn delete_calendar_item(&self, id: &str) -> Result<(), String> {
        block_on(async {
            let resp = self.client.delete(id).await?;
            if !resp.status().is_success() {
                let status = resp.status();
                let body = String::from_utf8_lossy(&resp.into_body()).to_string();
                return Err(anyhow::anyhow!(
                    "Failed to DELETE event: {} - {}",
                    status,
                    body
                ));
            }
            anyhow::Result::<()>::Ok(())
        })
        .map_err(|e| e.to_string())
    }
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
// `crate::agent::tools::dtos`.
// ---------------------------------------------------------------------------

/// Iterate every configured CalDAV client, invoke `f` against each
/// one, and split the per-server outcomes into a `results` vec and
/// an `errors` vec. Errors are recorded as `"Error on client {name}: {e}"`
/// — the same string the previous inline-loop code produced — so the
/// existing test assertions keep working.
fn for_each_client<T, F>(config: &AppConfig, mut f: F) -> (Vec<T>, Vec<String>)
where
    F: FnMut(&str, &DavClient) -> Result<T, String>,
{
    let mut results = Vec::new();
    let mut errors = Vec::new();
    for (name, cc) in &config.caldav_clients {
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
fn for_each_client_vec<T, F>(config: &AppConfig, mut f: F) -> (Vec<T>, Vec<String>)
where
    F: FnMut(&str, &DavClient) -> Result<Vec<T>, String>,
{
    let mut results = Vec::new();
    let mut errors = Vec::new();
    for (name, cc) in &config.caldav_clients {
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
    config: &AppConfig,
    keyword: &str,
) -> Result<SearchCalendarResponse, String> {
    let (results, errors) = for_each_client_vec(config, |_, c| c.search_calendar(keyword));
    Ok(SearchCalendarResponse {
        results: serialize_response(&CalDavResponse { results, errors }),
    })
}

pub fn tool_get_calendar(
    config: &AppConfig,
    start: &str,
    end: &str,
) -> Result<GetCalendarResponse, String> {
    let (results, errors) = for_each_client_vec(config, |_, c| c.get_calendar(start, end));
    Ok(GetCalendarResponse {
        results: serialize_response(&CalDavResponse { results, errors }),
    })
}

pub fn tool_get_calendar_item(
    config: &AppConfig,
    id: &str,
) -> Result<GetCalendarItemResponse, String> {
    let (results, errors) = for_each_client(config, |_, c| c.get_calendar_item(id));
    Ok(GetCalendarItemResponse {
        result: serialize_response(&CalDavResponse { results, errors }),
    })
}

pub fn tool_add_calendar_item(
    config: &AppConfig,
    item_json: &str,
) -> Result<AddCalendarItemResponse, String> {
    // `add_calendar_item` is special: it acts on the *first* configured
    // CalDAV client (no "default calendar" concept in CalDAV). The
    // per-server output is a single status string, so the aggregation
    // shape doesn't fit `for_each_client` cleanly.
    let mut all_results = Vec::new();
    if let Some((name, cc)) = config.caldav_clients.iter().next() {
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
    config: &AppConfig,
    id: &str,
    update_json: &str,
) -> Result<UpdateCalendarItemResponse, String> {
    let mut all_results = Vec::new();
    for (name, cc) in &config.caldav_clients {
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
    config: &AppConfig,
    id: &str,
) -> Result<DeleteCalendarItemResponse, String> {
    let mut all_results = Vec::new();
    for (name, cc) in &config.caldav_clients {
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
