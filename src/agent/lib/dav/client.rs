//! Unified DAV client.
//!
//! [`DavClient`] is the per-server entry point for everything the
//! `dav` integration exposes. It owns both a
//! `fast_dav_rs::CalDavClient` and a `fast_dav_rs::CardDavClient`
//! against the same DAV server — RFC 6762/6763 say a DAV endpoint
//! usually serves both CalDAV and CardDAV, so the LLM-tool layer
//! can iterate `config.caldav_clients` once and hand each entry to
//! a single `DavClient` to drive either protocol.
//!
//! Layering:
//!
//! 1. [`DavClient`] — this module. Owns the connection state, has
//!    protocol methods (`search_calendar`, `get_calendar`,
//!    `search_contact`, `get_contact`, …) that return typed
//!    values or a `String` error.
//! 2. Protocol helpers in `dav::cal` (iCal parsing, JSON
//!    serialisation) and `dav::card` (vCard parsing, addressbook
//!    discovery). These are `pub(super)` so `DavClient` can call
//!    them; they're also used directly by the LLM-tool layer's
//!    tool wrappers.
//! 3. The `tool_*` functions in `dav::cal` and `dav::card` — the
//!    LLM-adapter layer. They iterate `config.caldav_clients`,
//!    build a `DavClient` per server, aggregate the per-server
//!    results, and serialise to the LLM-facing DTOs from
//!    `crate::tools::dtos`.

use fast_dav_rs::{CalDavClient, CardDavClient};

use crate::config::CalDavClient as CalDavClientConfig;
use crate::lib::dav::cal::{self, CalDavEventDetails};
use crate::lib::dav::card::{self, CardDavContactDetails};
use crate::tools::blocking::block_on;

/// One DAV server connection — both CalDAV and CardDAV over the
/// same base URL with the same credentials.
///
/// Construct with [`DavClient::new`], then call the protocol
/// methods (`search_calendar` / `get_calendar` /
/// `get_calendar_item` / `add_calendar_item` /
/// `update_calendar_item` / `delete_calendar_item` for CalDAV;
/// `search_contact` / `get_contact` / `add_contact` /
/// `update_contact` / `delete_contact` for CardDAV).
pub struct DavClient {
    name: String,
    base_url: String,
    username: String,
    cal: CalDavClient,
    card: CardDavClient,
}

impl DavClient {
    /// Build a `DavClient` from a name and a [`CalDavClientConfig`]
    /// entry. The struct name `CalDavClientConfig` is historical
    /// (the field predates CardDAV support) — the entry is shared
    /// by both protocols.
    ///
    /// Returns an error if either `CalDavClient::new` or
    /// `CardDavClient::new` cannot build the underlying
    /// connection (e.g. malformed URL). Both clients are built
    /// even though a server might only serve one protocol — the
    /// unused client simply errors on its first request, which
    /// the per-server aggregation in the tool layer records as a
    /// per-server error rather than aborting the whole call.
    pub fn new(name: String, config: &CalDavClientConfig) -> Result<Self, String> {
        let cal = CalDavClient::new(&config.url, Some(&config.username), Some(&config.password))
            .map_err(|e| format!("Client config error (CalDAV): {}", e))?;
        let card = CardDavClient::new(&config.url, Some(&config.username), Some(&config.password))
            .map_err(|e| format!("Client config error (CardDAV): {}", e))?;
        Ok(Self {
            name,
            base_url: config.url.clone(),
            username: config.username.clone(),
            cal,
            card,
        })
    }

    /// The friendly name from the config map. Returned in every
    /// `cal::CalDavEventDetails::client` /
    /// `card::CardDavContactDetails::client` field so the LLM
    /// can attribute results to a server.
    pub fn name(&self) -> &str {
        &self.name
    }

    // -----------------------------------------------------------------
    // CalDAV
    // -----------------------------------------------------------------

    /// Discover every calendar href this server exposes. Tries
    /// `list_calendars` first; on empty, falls through to
    /// `discover_calendar_home_set` → `discover_current_user_principal`,
    /// then a Fastmail-style `/dav/principals/user/{username}/` guess.
    async fn list_calendar_hrefs(&self) -> anyhow::Result<Vec<String>> {
        if let Ok(calendars) = self.cal.list_calendars(&self.base_url).await
            && !calendars.is_empty()
        {
            return Ok(calendars.into_iter().map(|c| c.href).collect());
        }

        if let Ok(homes) = self.cal.discover_calendar_home_set(&self.base_url).await
            && let Some(home) = homes.first()
            && let Ok(calendars) = self.cal.list_calendars(home).await
            && !calendars.is_empty()
        {
            return Ok(calendars.into_iter().map(|c| c.href).collect());
        }

        let mut principal_opt = self
            .cal
            .discover_current_user_principal()
            .await
            .ok()
            .flatten();

        // Fallback for Fastmail or other servers that use /dav/principals/user/username/
        if principal_opt.is_none() {
            let base_trimmed = self.base_url.trim_end_matches('/');
            let guess = format!("{}/dav/principals/user/{}/", base_trimmed, self.username);
            if let Ok(homes) = self.cal.discover_calendar_home_set(&guess).await
                && !homes.is_empty()
            {
                principal_opt = Some(guess);
            }
        }

        let principal = principal_opt.ok_or_else(|| anyhow::anyhow!("No principal found"))?;
        let homes = self.cal.discover_calendar_home_set(&principal).await?;
        let home = homes
            .first()
            .ok_or_else(|| anyhow::anyhow!("No calendar home found"))?;
        let calendars = self.cal.list_calendars(home).await?;
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
                    .cal
                    .calendar_query_timerange(&cal_path, "VEVENT", None, None, true)
                    .await?;
                for item in items {
                    if let Some(data) = &item.calendar_data
                        && data.to_lowercase().contains(&kw)
                    {
                        matches.push(cal::parse_ical_data(&name, &item.href, data));
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
                    .cal
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
                        matches.push(cal::parse_ical_data(&name, &item.href, data));
                    }
                }
            }
            anyhow::Result::<Vec<_>>::Ok(matches)
        })
        .map_err(|e| e.to_string())
    }

    /// Fetch a single calendar item by its href (`id`). The wire
    /// request is a plain `GET {id}`. Returns an error if the
    /// server responds non-2xx — the error string includes the
    /// status line and the body so the operator can diagnose 404
    /// vs auth failure.
    pub fn get_calendar_item(&self, id: &str) -> Result<CalDavEventDetails, String> {
        let name = self.name.clone();
        block_on(async {
            let resp = self.cal.get(id).await?;
            if !resp.status().is_success() {
                let status = resp.status();
                let bytes = resp.into_body();
                let body = String::from_utf8_lossy(&bytes).to_string();
                return Err(anyhow::anyhow!("Not found by href: {} - {}", status, body));
            }
            let bytes = resp.into_body();
            let body = String::from_utf8_lossy(&bytes).to_string();
            anyhow::Result::<CalDavEventDetails>::Ok(cal::parse_ical_data(&name, id, &body))
        })
        .map_err(|e| e.to_string())
    }

    /// Create a new VEVENT on the first calendar this server
    /// exposes (no "default calendar" concept in CalDAV; the first
    /// discovery hit is what every other tool does). `item_json`
    /// is the same LLM-facing JSON the [`cal::json_to_ical`]
    /// helper accepts.
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
            let ical_data = cal::json_to_ical(item_json, Some(&uid));
            let resp = self.cal.put(&path, ical_data.into_bytes().into()).await?;
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

    /// Update an existing VEVENT. GETs the current iCal body,
    /// merges `update_json` into it via
    /// [`cal::update_ical_string`], and PUTs the result back.
    /// Returns an error if the GET 404s.
    pub fn update_calendar_item(&self, id: &str, update_json: &str) -> Result<String, String> {
        block_on(async {
            let get_resp = self.cal.get(id).await?;
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
            let ical_data = cal::update_ical_string(&body, &update_parsed);

            let resp = self.cal.put(id, ical_data.into_bytes().into()).await?;
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

    /// DELETE a calendar item by href. Returns the error string
    /// from the server if the response is non-2xx.
    pub fn delete_calendar_item(&self, id: &str) -> Result<(), String> {
        block_on(async {
            let resp = self.cal.delete(id).await?;
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

    // -----------------------------------------------------------------
    // CardDAV
    // -----------------------------------------------------------------

    /// Search every addressbook on this server for contacts whose
    /// vCard body contains `keyword` (case-insensitive). Fail-fast
    /// on the first broken addressbook so the operator sees a
    /// real server error (e.g. a 403 from FastMail when a
    /// collection has been removed or renamed) instead of
    /// silently skipping it.
    pub fn search_contact(&self, keyword: &str) -> Result<Vec<CardDavContactDetails>, String> {
        let kw = keyword.to_lowercase();
        let name = self.name.clone();
        block_on(async {
            let books =
                card::get_all_addressbooks(&self.card, &self.base_url, &self.username).await?;
            let mut matches = Vec::new();
            let mut scanned = 0usize;
            for book_path in books {
                let contacts = card::fetch_contacts_from_book(&self.card, &book_path).await?;
                scanned += contacts.len();
                for (href, data) in contacts {
                    if data.to_lowercase().contains(&kw) {
                        matches.push(card::parse_vcard(&name, &href, &data));
                    }
                }
            }
            tracing::info!(
                name = "tool.carddav.search.summary",
                client = %name,
                keyword = %keyword,
                scanned = scanned,
                matched = matches.len(),
                "CardDAV search completed"
            );
            anyhow::Result::<Vec<_>>::Ok(matches)
        })
        .map_err(|e| e.to_string())
    }

    /// Fetch a single contact by its href (`id`). The wire request
    /// is a plain `GET {id}`. Returns an error if the server
    /// responds non-2xx — the error string includes the status
    /// line and the truncated body.
    pub fn get_contact(&self, id: &str) -> Result<CardDavContactDetails, String> {
        let name = self.name.clone();
        block_on(async {
            let resp = self.card.get(id).await?;
            let status = resp.status();
            let body_bytes = resp.into_body();
            let body_log = card::log_truncate(&body_bytes);
            if !status.is_success() {
                return Err(anyhow::anyhow!(
                    "Not found by href: {} - {}",
                    status,
                    body_log
                ));
            }
            Ok(card::parse_vcard(&name, id, &body_log))
        })
        .map_err(|e| e.to_string())
    }

    /// Create a new vCard on the first addressbook this server
    /// exposes. `contact_json` is the same LLM-facing JSON the
    /// `card::json_to_vcard` helper accepts. Uses
    /// `put_if_none_match` so the server rejects the PUT with 412
    /// (well, the DAV status for If-None-Match conflict — usually
    /// 412) if a contact with that UID already exists.
    pub fn add_contact(&self, contact_json: &str) -> Result<String, String> {
        block_on(async {
            let books =
                card::get_all_addressbooks(&self.card, &self.base_url, &self.username).await?;
            let default_book = books
                .first()
                .ok_or_else(|| anyhow::anyhow!("No addressbook found to add to"))?;
            let uid = format!(
                "{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
            );
            // Addressbook hrefs from PROPFIND typically end with `/`. The PUT
            // URL must be `<addressbook>/<uid>.vcf` (with a `/` separator) or
            // the server concatenates the resource name directly onto the
            // collection path and rejects the request as malformed
            // (FastMail responds `403 Forbidden - Mailbox does not exist`).
            // `build_contact_put_path` normalises the separator.
            let path = card::build_contact_put_path(default_book, &uid);
            let vcard_data = card::json_to_vcard(contact_json, Some(&uid));
            let vcard_bytes: bytes::Bytes = vcard_data.into_bytes().into();
            let resp = self.card.put_if_none_match(&path, vcard_bytes).await?;
            if !resp.status().is_success() {
                let status = resp.status();
                let body = String::from_utf8_lossy(&resp.into_body()).to_string();
                return Err(anyhow::anyhow!(
                    "Failed to PUT contact: {} - {}",
                    status,
                    body
                ));
            }
            Ok(format!("Created at {}", path))
        })
        .map_err(|e| e.to_string())
    }

    /// Update an existing vCard. GETs the current body, parses
    /// its properties, merges the JSON update via
    /// `card::merge_vcard_update` (preserves everything the LLM
    /// didn't touch), then PUTs the new vCard back. If the GET
    /// returns an ETag, the PUT uses `If-Match` for race
    /// detection; otherwise it falls back to an unconditional
    /// PUT.
    pub fn update_contact(&self, href: &str, contact_json: &str) -> Result<String, String> {
        block_on(async {
            let get_resp = self.card.get(href).await?;
            let get_status = get_resp.status();
            let get_etag = get_resp
                .headers()
                .get("etag")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            let get_body = card::log_truncate(&get_resp.into_body());
            if !get_status.is_success() {
                return Err(anyhow::anyhow!(
                    "Failed to fetch contact for update: {} - {}",
                    get_status,
                    get_body
                ));
            }

            let existing_uid = card::extract_vcard_uid(&get_body);
            let existing_props = card::parse_vcard_properties(&get_body);
            let new_vcard =
                card::merge_vcard_update(&existing_props, contact_json, existing_uid.as_deref());
            let vcard_bytes: bytes::Bytes = new_vcard.into_bytes().into();

            let put_resp = if let Some(ref tag) = get_etag {
                self.card.put_if_match(href, vcard_bytes, tag).await?
            } else {
                self.card.put(href, vcard_bytes).await?
            };
            let put_status = put_resp.status();
            let put_body = card::log_truncate(&put_resp.into_body());

            if !put_status.is_success() {
                return Err(anyhow::anyhow!(
                    "Failed to PUT updated contact: {} - {}",
                    put_status,
                    put_body
                ));
            }
            Ok(format!("Updated (status {})", put_status))
        })
        .map_err(|e| e.to_string())
    }

    /// DELETE a contact by href. 404 is treated as success so the
    /// LLM can call `delete_contact` idempotently.
    pub fn delete_contact(&self, href: &str) -> Result<String, String> {
        block_on(async {
            let resp = self.card.delete(href).await?;
            let status = resp.status();
            if status.as_u16() == 404 {
                return Ok("Already absent (404)".to_string());
            }
            if !status.is_success() {
                let body = card::log_truncate(&resp.into_body());
                return Err(anyhow::anyhow!(
                    "Failed to DELETE contact: {} - {}",
                    status,
                    body
                ));
            }
            Ok(format!("Deleted (status {})", status))
        })
        .map_err(|e| e.to_string())
    }
}
