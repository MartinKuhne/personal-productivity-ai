//! Unified DAV client.
//!
//! [`DavClient`] is the per-server entry point for everything the
//! `dav` integration exposes. It communicates with a DAV server for both
//! CalDAV and CardDAV over HTTP.
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

use crate::config::CalDavClient as CalDavClientConfig;
use crate::lib::dav::cal::{self, CalDavEventDetails};
use crate::lib::dav::card::{self, CardDavContactDetails};
use crate::lib::dav::xml;
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
    password: String,
    client: reqwest::Client,
}

impl DavClient {
    /// Build a `DavClient` from a name and a [`CalDavClientConfig`]
    /// entry. The struct name `CalDavClientConfig` is historical
    /// (the field predates CardDAV support) — the entry is shared
    /// by both protocols.
    ///
    /// Returns an error if the URL is invalid or the underlying HTTP
    /// client cannot be configured.
    pub fn new(name: String, config: &CalDavClientConfig) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("Client config error: {e}"))?;

        if !config.url.starts_with("http://") && !config.url.starts_with("https://") {
            return Err(format!("Client config error: invalid URL '{}'", config.url));
        }

        Ok(Self {
            name,
            base_url: config.url.clone(),
            username: config.username.clone(),
            password: config.password.clone(),
            client,
        })
    }

    /// The friendly name from the config map. Returned in every
    /// `cal::CalDavEventDetails::client` /
    /// `card::CardDavContactDetails::client` field so the LLM
    /// can attribute results to a server.
    pub fn name(&self) -> &str {
        &self.name
    }

    fn resolve_url(&self, path: &str) -> String {
        if path.starts_with("http://") || path.starts_with("https://") {
            return path.to_string();
        }
        if let Ok(joined) = url::Url::parse(&self.base_url).and_then(|base| base.join(path)) {
            return joined.to_string();
        }
        let base_trimmed = self.base_url.trim_end_matches('/');
        let path_clean = path.trim_start_matches('/');
        format!("{base_trimmed}/{path_clean}")
    }

    async fn send_request(
        &self,
        method: reqwest::Method,
        path: &str,
        headers: &[(&'static str, &str)],
        body: Option<String>,
    ) -> anyhow::Result<(
        reqwest::StatusCode,
        reqwest::header::HeaderMap,
        bytes::Bytes,
    )> {
        let full_url = self.resolve_url(path);
        let mut req = self
            .client
            .request(method, &full_url)
            .basic_auth(&self.username, Some(&self.password));
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        if let Some(b) = body {
            req = req.body(b);
        }
        let resp = req.send().await?;
        let status = resp.status();
        let headers = resp.headers().clone();
        let body_bytes = resp.bytes().await?;
        Ok((status, headers, body_bytes))
    }

    // -----------------------------------------------------------------
    // CalDAV
    // -----------------------------------------------------------------

    /// List CalDAV collections under a calendar home-set (`Depth: 1` PROPFIND).
    pub async fn list_calendars(&self, home_set_path: &str) -> anyhow::Result<Vec<String>> {
        let (status, _, body) = self
            .send_request(
                reqwest::Method::from_bytes(b"PROPFIND")?,
                home_set_path,
                &[
                    ("Depth", "1"),
                    ("Content-Type", "application/xml; charset=utf-8"),
                ],
                Some(xml::build_propfind_calendars().to_string()),
            )
            .await?;
        if status.as_u16() != 207 && !status.is_success() {
            return Err(anyhow::anyhow!("PROPFIND calendars failed: {status}"));
        }
        let hrefs = xml::parse_calendar_hrefs(&String::from_utf8_lossy(&body));
        Ok(hrefs)
    }

    /// Discover calendar-home-set collection(s) for a principal.
    pub async fn discover_calendar_home_set(&self, path: &str) -> anyhow::Result<Vec<String>> {
        let (status, _, body) = self
            .send_request(
                reqwest::Method::from_bytes(b"PROPFIND")?,
                path,
                &[
                    ("Depth", "0"),
                    ("Content-Type", "application/xml; charset=utf-8"),
                ],
                Some(xml::build_propfind_calendar_home_set().to_string()),
            )
            .await?;
        if status.as_u16() != 207 && !status.is_success() {
            return Err(anyhow::anyhow!(
                "PROPFIND calendar-home-set failed: {status}"
            ));
        }
        let homes = xml::parse_home_set(&String::from_utf8_lossy(&body), "calendar-home-set");
        Ok(homes)
    }

    /// Discover current user principal URL via `current-user-principal`.
    pub async fn discover_current_user_principal(&self) -> anyhow::Result<Option<String>> {
        let (status, _, body) = self
            .send_request(
                reqwest::Method::from_bytes(b"PROPFIND")?,
                &self.base_url,
                &[
                    ("Depth", "0"),
                    ("Content-Type", "application/xml; charset=utf-8"),
                ],
                Some(xml::build_propfind_current_user_principal().to_string()),
            )
            .await?;
        if status.as_u16() != 207 && !status.is_success() {
            return Ok(None);
        }
        Ok(xml::parse_current_user_principal(&String::from_utf8_lossy(
            &body,
        )))
    }

    /// Execute a CalDAV calendar-query with an optional time-range filter.
    pub async fn calendar_query_timerange(
        &self,
        calendar_path: &str,
        component: &str,
        start: Option<&str>,
        end: Option<&str>,
    ) -> anyhow::Result<Vec<xml::CalendarItem>> {
        let xml_body = xml::build_calendar_query(component, start, end);
        let (status, _, body) = self
            .send_request(
                reqwest::Method::from_bytes(b"REPORT")?,
                calendar_path,
                &[
                    ("Depth", "1"),
                    ("Content-Type", "application/xml; charset=utf-8"),
                ],
                Some(xml_body),
            )
            .await?;
        if status.as_u16() != 207 && !status.is_success() {
            return Err(anyhow::anyhow!("REPORT calendar-query failed: {status}"));
        }
        let items = xml::parse_calendar_query_response(&String::from_utf8_lossy(&body));
        Ok(items)
    }

    /// Discover every calendar href this server exposes. Tries
    /// `list_calendars` first; on empty, falls through to
    /// `discover_calendar_home_set` → `discover_current_user_principal`,
    /// then a Fastmail-style `/dav/principals/user/{username}/` guess.
    async fn list_calendar_hrefs(&self) -> anyhow::Result<Vec<String>> {
        if let Ok(calendars) = self.list_calendars(&self.base_url).await
            && !calendars.is_empty()
        {
            return Ok(calendars);
        }

        if let Ok(homes) = self.discover_calendar_home_set(&self.base_url).await
            && let Some(home) = homes.first()
            && let Ok(calendars) = self.list_calendars(home).await
            && !calendars.is_empty()
        {
            return Ok(calendars);
        }

        let mut principal_opt = self.discover_current_user_principal().await.ok().flatten();

        // Fallback for Fastmail or other servers that use /dav/principals/user/{username}/
        if principal_opt.is_none() {
            let base_trimmed = self.base_url.trim_end_matches('/');
            let guess = format!("{}/dav/principals/user/{}/", base_trimmed, self.username);
            if let Ok(homes) = self.discover_calendar_home_set(&guess).await
                && !homes.is_empty()
            {
                principal_opt = Some(guess);
            }
        }

        let principal = principal_opt.ok_or_else(|| anyhow::anyhow!("No principal found"))?;
        let homes = self.discover_calendar_home_set(&principal).await?;
        let home = homes
            .first()
            .ok_or_else(|| anyhow::anyhow!("No calendar home found"))?;
        let calendars = self.list_calendars(home).await?;
        Ok(calendars)
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
                    .calendar_query_timerange(&cal_path, "VEVENT", None, None)
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
                let clean = d.replace('-', "");
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
                    .calendar_query_timerange(&cal_path, "VEVENT", Some(&start_fmt), Some(&end_fmt))
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
            let (status, _, bytes) = self
                .send_request(reqwest::Method::GET, id, &[], None)
                .await?;
            let body = String::from_utf8_lossy(&bytes).to_string();
            if !status.is_success() {
                return Err(anyhow::anyhow!("Not found by href: {} - {}", status, body));
            }
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
            let (status, _, body) = self
                .send_request(
                    reqwest::Method::PUT,
                    &path,
                    &[("Content-Type", "text/calendar; charset=utf-8")],
                    Some(ical_data),
                )
                .await?;
            if !status.is_success() {
                let body_str = String::from_utf8_lossy(&body).to_string();
                return Err(anyhow::anyhow!(
                    "Failed to PUT event: {} - {}",
                    status,
                    body_str
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
            let (get_status, _, get_bytes) = self
                .send_request(reqwest::Method::GET, id, &[], None)
                .await?;
            let body = String::from_utf8_lossy(&get_bytes).to_string();
            if !get_status.is_success() {
                return Err(anyhow::anyhow!(
                    "Failed to fetch event for update: {} - {}",
                    get_status,
                    body
                ));
            }

            let update_parsed: serde_json::Value =
                serde_json::from_str(update_json).unwrap_or_else(|_| serde_json::json!({}));
            let ical_data = cal::update_ical_string(&body, &update_parsed);

            let (put_status, _, put_bytes) = self
                .send_request(
                    reqwest::Method::PUT,
                    id,
                    &[("Content-Type", "text/calendar; charset=utf-8")],
                    Some(ical_data),
                )
                .await?;
            if !put_status.is_success() {
                let put_body = String::from_utf8_lossy(&put_bytes).to_string();
                return Err(anyhow::anyhow!(
                    "Failed to PUT update event: {} - {}",
                    put_status,
                    put_body
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
            let (status, _, body) = self
                .send_request(reqwest::Method::DELETE, id, &[], None)
                .await?;
            if !status.is_success() {
                let body_str = String::from_utf8_lossy(&body).to_string();
                return Err(anyhow::anyhow!(
                    "Failed to DELETE event: {} - {}",
                    status,
                    body_str
                ));
            }
            anyhow::Result::<()>::Ok(())
        })
        .map_err(|e| e.to_string())
    }

    // -----------------------------------------------------------------
    // CardDAV
    // -----------------------------------------------------------------

    /// Discover every addressbook collection under a home set or base URL.
    pub async fn list_addressbooks(&self, path: &str) -> anyhow::Result<Vec<String>> {
        let (status, _, body) = self
            .send_request(
                reqwest::Method::from_bytes(b"PROPFIND")?,
                path,
                &[
                    ("Depth", "1"),
                    ("Content-Type", "application/xml; charset=utf-8"),
                ],
                Some(xml::build_propfind_addressbooks().to_string()),
            )
            .await?;
        if status.as_u16() != 207 && !status.is_success() {
            return Err(anyhow::anyhow!("PROPFIND addressbooks failed: {status}"));
        }
        let hrefs = xml::parse_addressbook_hrefs(&String::from_utf8_lossy(&body));
        Ok(hrefs)
    }

    /// Discover addressbook-home-set collections for a principal or base URL.
    pub async fn discover_addressbook_home_set(&self, path: &str) -> anyhow::Result<Vec<String>> {
        let (status, _, body) = self
            .send_request(
                reqwest::Method::from_bytes(b"PROPFIND")?,
                path,
                &[
                    ("Depth", "0"),
                    ("Content-Type", "application/xml; charset=utf-8"),
                ],
                Some(xml::build_propfind_addressbook_home_set().to_string()),
            )
            .await?;
        if status.as_u16() != 207 && !status.is_success() {
            return Err(anyhow::anyhow!(
                "PROPFIND addressbook-home-set failed: {status}"
            ));
        }
        let homes = xml::parse_home_set(&String::from_utf8_lossy(&body), "addressbook-home-set");
        Ok(homes)
    }

    /// Fetch all contacts from an addressbook collection.
    pub async fn fetch_contacts_from_book(
        &self,
        book_path: &str,
    ) -> anyhow::Result<Vec<(String, String)>> {
        let sync_body = xml::build_sync_collection(None, Some(10000));
        let res = self
            .send_request(
                reqwest::Method::from_bytes(b"REPORT")?,
                book_path,
                &[
                    ("Depth", "1"),
                    ("Content-Type", "application/xml; charset=utf-8"),
                ],
                Some(sync_body),
            )
            .await;

        let body_str = match res {
            Ok((status, _, body)) if status.as_u16() == 207 || status.is_success() => {
                String::from_utf8_lossy(&body).to_string()
            }
            _ => {
                let (status, _, body) = self
                    .send_request(
                        reqwest::Method::from_bytes(b"REPORT")?,
                        book_path,
                        &[
                            ("Depth", "1"),
                            ("Content-Type", "application/xml; charset=utf-8"),
                        ],
                        Some(xml::build_addressbook_query().to_string()),
                    )
                    .await?;
                if status.as_u16() != 207 && !status.is_success() {
                    return Err(anyhow::anyhow!("REPORT addressbook-query failed: {status}"));
                }
                String::from_utf8_lossy(&body).to_string()
            }
        };

        let items = xml::parse_addressbook_query_response(&body_str);
        let mut contacts = Vec::new();
        for item in items {
            if let Some(data) = item.address_data {
                contacts.push((item.href, data));
            }
        }
        Ok(contacts)
    }

    /// Search every addressbook on this server for contacts whose
    /// vCard body contains `keyword` (case-insensitive). Fail-fast
    /// on the first broken addressbook so the operator sees a
    /// real server error instead of silently skipping it.
    pub fn search_contact(&self, keyword: &str) -> Result<Vec<CardDavContactDetails>, String> {
        let kw = keyword.to_lowercase();
        let name = self.name.clone();
        block_on(async {
            let books = card::get_all_addressbooks(self, &self.base_url, &self.username).await?;
            let mut matches = Vec::new();
            let mut scanned = 0usize;
            for book_path in books {
                let contacts = card::fetch_contacts_from_book(self, &book_path).await?;
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
            let (status, _, body_bytes) = self
                .send_request(reqwest::Method::GET, id, &[], None)
                .await?;
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
    /// `If-None-Match: *` so the server rejects the PUT with 412
    /// if a contact with that UID already exists.
    pub fn add_contact(&self, contact_json: &str) -> Result<String, String> {
        block_on(async {
            let books = card::get_all_addressbooks(self, &self.base_url, &self.username).await?;
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
            let path = card::build_contact_put_path(default_book, &uid);
            let vcard_data = card::json_to_vcard(contact_json, Some(&uid));
            let (status, _, body) = self
                .send_request(
                    reqwest::Method::PUT,
                    &path,
                    &[
                        ("Content-Type", "text/vcard; charset=utf-8"),
                        ("If-None-Match", "*"),
                    ],
                    Some(vcard_data),
                )
                .await?;
            if !status.is_success() {
                let body_str = String::from_utf8_lossy(&body).to_string();
                return Err(anyhow::anyhow!(
                    "Failed to PUT contact: {} - {}",
                    status,
                    body_str
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
            let (get_status, get_headers, get_body_bytes) = self
                .send_request(reqwest::Method::GET, href, &[], None)
                .await?;
            let get_etag = get_headers
                .get("etag")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            let get_body = card::log_truncate(&get_body_bytes);
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

            let mut put_headers = vec![("Content-Type", "text/vcard; charset=utf-8")];
            if let Some(ref tag) = get_etag {
                put_headers.push(("If-Match", tag.as_str()));
            }

            let (put_status, _, put_body) = self
                .send_request(reqwest::Method::PUT, href, &put_headers, Some(new_vcard))
                .await?;

            if !put_status.is_success() {
                let body_str = card::log_truncate(&put_body);
                return Err(anyhow::anyhow!(
                    "Failed to PUT updated contact: {} - {}",
                    put_status,
                    body_str
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
            let (status, _, body) = self
                .send_request(reqwest::Method::DELETE, href, &[], None)
                .await?;
            if status.as_u16() == 404 {
                return Ok("Already absent (404)".to_string());
            }
            if !status.is_success() {
                let body_str = card::log_truncate(&body);
                return Err(anyhow::anyhow!(
                    "Failed to DELETE contact: {} - {}",
                    status,
                    body_str
                ));
            }
            Ok(format!("Deleted (status {})", status))
        })
        .map_err(|e| e.to_string())
    }
}
