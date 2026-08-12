//! JMAP Email and EmailSubmission operations (RFC 8621).
//!
//! Uses `urn:ietf:params:jmap:mail` and `urn:ietf:params:jmap:submission` capabilities.
//! Core protocol handling per RFC 8620; error handling per RFC 8620 §3.6.2.
//!
//! All email operations use the typed `jmap_client` crate methods:
//! - `email_query()` — filter by text, mailbox, dates, keywords
//! - `email_get()` — fetch full email details by ID
//! - `Email/set` — create an email object for sending
//! - `email_submission_create()` — submit an email for delivery
//!
//! See: <https://www.rfc-editor.org/rfc/rfc8620>
//! See: <https://www.rfc-editor.org/rfc/rfc8621>
//!
//! Unit tests live in the sibling `email_tests.rs` sidecar.

use crate::config::AppConfig;
use fast_h2m::convert;
use jmap_client::core::query::Filter as CoreFilter;
use jmap_client::core::query::Filter as MailFilter;
use jmap_client::core::response::EmailSetResponse;
use jmap_client::core::set::SetObject;
use jmap_client::email::query::Filter;

use super::client::JmapSession;
use crate::agent::tools::registry::cache::{SearchEmailCacheEntry, SearchEmailItem};

/// Maximum number of bytes the JMAP server should inline for a single body
/// part in `bodyValues` (RFC 8621 §6.1.2 `maxBodyValueBytes`).
///
/// The RFC default of `0` makes servers return **no** body values at all, so
/// `email.body_value(part_id)` is `None` and `simplify_email` produces an
/// empty `body` field — which is what the LLM has been seeing. Setting a
/// non-zero cap is mandatory; we pick 10 MiB as a generous upper bound for a
/// single MIME part while still bounded enough to avoid pathological emails
/// inflating the agent's tool response.
pub const MAX_BODY_VALUE_BYTES: usize = 10 * 1024 * 1024;

/// Fetch a single email by ID with body content inlined.
///
/// Wraps `jmap_client::Client::email_get` but explicitly sets the
/// `fetchTextBodyValues` / `fetchHTMLBodyValues` / `maxBodyValueBytes`
/// arguments on the `Email/get` request (see [`MAX_BODY_VALUE_BYTES`]).
/// Without these, RFC 8621 §6.1.2 defaults `maxBodyValueBytes` to `0` and
/// the server returns an empty `bodyValues` map, so callers see an empty
/// body regardless of the actual message content.
fn email_get_full(
    session: &JmapSession,
    id: &str,
) -> Result<Option<jmap_client::email::Email<jmap_client::Get>>, String> {
    let mut request = session.inner().build();
    let get_request = request.get_email().ids([id]);
    get_request
        .arguments()
        .fetch_text_body_values(true)
        .fetch_html_body_values(true)
        .max_body_value_bytes(MAX_BODY_VALUE_BYTES);
    let mut response = request
        .send_get_email()
        .map_err(|e| format!("Email/get request failed: {e}"))?;
    Ok(response.take_list().into_iter().next())
}

/// Convert HTML body values in a JMAP response to Markdown using `fast_h2m`.
#[allow(dead_code)]
pub(crate) fn convert_html_in_jmap(mut res: serde_json::Value) -> serde_json::Value {
    fn process(val: &mut serde_json::Value) {
        match val {
            serde_json::Value::Object(map) => {
                if let Some(body_values) = map.get_mut("bodyValues")
                    && let serde_json::Value::Object(parts) = body_values
                {
                    for (_, part_obj) in parts.iter_mut() {
                        if let serde_json::Value::Object(part_map) = part_obj
                            && let Some(serde_json::Value::String(val_str)) =
                                part_map.get_mut("value")
                            && val_str.contains('<')
                            && val_str.contains('>')
                            && let Ok(conv) = convert(val_str, None)
                            && let Some(md) = conv.content
                        {
                            *val_str = md;
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

/// Look up a mailbox by folder name using the typed crate API.
fn lookup_mailbox_id(session: &JmapSession, folder_name: &str) -> Result<String, String> {
    let lower_name = folder_name.to_lowercase();
    let mut mailboxes = session
        .inner()
        .mailbox_query(
            None::<MailFilter<jmap_client::mailbox::query::Filter>>,
            None::<Vec<_>>,
        )
        .map_err(|e| format!("Failed to query mailboxes: {}", e))?;
    for id in mailboxes.take_ids() {
        if let Some(mailbox) = session
            .inner()
            .mailbox_get(&id, None::<Vec<_>>)
            .map_err(|e| format!("Failed to get mailbox: {}", e))?
            && let Some(name) = mailbox.name()
            && name.to_lowercase() == lower_name
        {
            return Ok(id);
        }
    }
    Err(format!("Mailbox not found with name: {}", folder_name))
}

/// Simplify a single `Email<Get>` into a flat JSON object.
fn simplify_email(
    email: &mut jmap_client::email::Email<jmap_client::Get>,
    max_lines: Option<usize>,
) -> serde_json::Value {
    let mut simplified = serde_json::Map::new();

    simplified.insert("id".to_string(), serde_json::Value::String(email.take_id()));

    simplified.insert(
        "subject".to_string(),
        email
            .subject()
            .map(|s| serde_json::Value::String(s.to_string()))
            .unwrap_or(serde_json::Value::Null),
    );

    if let Some(ts) = email.received_at() {
        simplified.insert(
            "date".to_string(),
            serde_json::Value::String(
                chrono::DateTime::<chrono::Utc>::from_timestamp(ts, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_else(|| ts.to_string()),
            ),
        );
    }

    if let Some(val) = serialize_address_list(email.from()) {
        simplified.insert("from".to_string(), val);
    }
    if let Some(val) = serialize_address_list(email.to()) {
        simplified.insert("to".to_string(), val);
    }
    if let Some(val) = serialize_address_list(email.cc()) {
        simplified.insert("cc".to_string(), val);
    }
    if let Some(val) = serialize_address_list(email.bcc()) {
        simplified.insert("bcc".to_string(), val);
    }

    // Extract body: prefer htmlBody, fall back to textBody
    let mut body_str = String::new();
    let mut is_truncated = false;

    if let Some(html_parts) = email.html_body()
        && let Some(first) = html_parts.first()
        && let Some(part_id) = first.part_id()
        && let Some(body_val) = email.body_value(part_id)
    {
        let mut raw = body_val.value().to_string();
        if raw.contains('<')
            && raw.contains('>')
            && let Ok(conv) = convert(&raw, None)
            && let Some(md) = conv.content
        {
            raw = md;
        }
        body_str = raw;
        is_truncated = body_val.is_truncated();
    }

    if body_str.is_empty()
        && let Some(text_parts) = email.text_body()
        && let Some(first) = text_parts.first()
        && let Some(part_id) = first.part_id()
        && let Some(body_val) = email.body_value(part_id)
    {
        let mut raw = body_val.value().to_string();
        if raw.contains('<')
            && raw.contains('>')
            && let Ok(conv) = convert(&raw, None)
            && let Some(md) = conv.content
        {
            raw = md;
        }
        body_str = raw;
        is_truncated = body_val.is_truncated();
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
    serde_json::Value::Object(simplified)
}

fn serialize_address_list(
    addrs: Option<&[jmap_client::email::EmailAddress<jmap_client::Get>]>,
) -> Option<serde_json::Value> {
    let list = addrs?;
    if list.is_empty() {
        return None;
    }
    let json: Vec<serde_json::Value> = list
        .iter()
        .map(|addr| {
            let email = addr.email();
            let val = match addr.name() {
                Some(name) if !name.trim().is_empty() => format!("{} <{}>", name, email),
                _ => email.to_string(),
            };
            serde_json::Value::String(val)
        })
        .collect();
    Some(serde_json::Value::Array(json))
}

/// Optional filters an LLM can pass to `tool_search_email`.
#[derive(Debug, Default, Clone)]
pub struct SearchEmailFilters<'a> {
    pub keyword: Option<&'a str>,
    pub folder: Option<&'a str>,
    pub start_date: Option<&'a str>,
    pub end_date: Option<&'a str>,
    pub from: Option<&'a str>,
    pub to: Option<&'a str>,
    pub is_unread: Option<bool>,
    pub is_flagged: Option<bool>,
}

/// Page size for `tool_search_email` (cursor mode). 100 per the
/// `doc/planning/tool-paging-audit-and-migration.md` plan.
pub const SEARCH_EMAIL_PAGE_SIZE: usize = 25;

/// Hint string emitted on the final page of a `search_email` cursor
/// walk.
pub const SEARCH_EMAIL_FINAL_PAGE_HINT: &str = "Final page.";

/// Generate a new opaque cursor string (UUID v4). The cursor is the
/// cache key in the shared `ToolCache`; the LLM treats it as opaque.
pub fn new_search_email_cursor(uuid_gen: &dyn crate::utils::uuid::UuidGenerator) -> String {
    uuid_gen.new_v4().to_string()
}

/// Fetch the full server result set for a search. Used by the
/// cursor flow when the caller has not supplied a cursor (or when
/// the cache lookup misses). The result is cached in the shared
/// `ToolCache` under a fresh UUID cursor.
fn fetch_full_search_result(
    config: &AppConfig,
    filters: &SearchEmailFilters<'_>,
) -> Result<SearchEmailCacheEntry, String> {
    let keyword = filters.keyword;
    let folder = filters.folder;
    let start_date = filters.start_date;
    let end_date = filters.end_date;
    let from = filters.from;
    let to = filters.to;
    let is_unread = filters.is_unread;
    let is_flagged = filters.is_flagged;

    // Build non-folder filter conditions using typed Filter enum
    let mut conditions: Vec<Filter> = Vec::new();
    if let Some(k) = keyword
        && !k.is_empty()
    {
        conditions.push(Filter::text(k));
    }
    if let Some(s) = start_date
        && !s.is_empty()
    {
        let ts = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .ok()
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|dt| dt.and_utc().timestamp())
            .unwrap_or(0);
        conditions.push(Filter::after(ts));
    }
    if let Some(e) = end_date
        && !e.is_empty()
    {
        let ts = chrono::NaiveDate::parse_from_str(e, "%Y-%m-%d")
            .ok()
            .and_then(|d| d.and_hms_opt(23, 59, 59))
            .map(|dt| dt.and_utc().timestamp())
            .unwrap_or(i64::MAX);
        conditions.push(Filter::before(ts));
    }
    if let Some(f) = from
        && !f.is_empty()
    {
        conditions.push(Filter::from(f));
    }
    if let Some(t) = to
        && !t.is_empty()
    {
        conditions.push(Filter::to(t));
    }
    if let Some(u) = is_unread {
        if u {
            conditions.push(Filter::not_keyword("$seen"));
        } else {
            conditions.push(Filter::has_keyword("$seen"));
        }
    }
    if let Some(f) = is_flagged {
        if f {
            conditions.push(Filter::has_keyword("$flagged"));
        } else {
            conditions.push(Filter::not_keyword("$flagged"));
        }
    }

    if conditions.is_empty() && folder.is_none() {
        return Err("At least one filter field must be provided \
              (keyword, folder, start_date, end_date, from, to, is_unread, is_flagged)"
            .to_string());
    }

    if config.jmap_clients.is_empty() {
        return Err("No JMAP clients configured.".to_string());
    }

    let mut all_items: Vec<SearchEmailItem> = Vec::new();
    let mut error_messages: Vec<String> = Vec::new();

    for (name, client) in &config.jmap_clients {
        let mut session = match JmapSession::connect(client) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(name = "tool.email.search.session_failed", client = %name, error = %e, "Failed to fetch JMAP session. Operator should check email account credentials.");
                error_messages.push(format!("Error fetching JMAP session for {}: {}", name, e));
                continue;
            }
        };
        let _account_id = session.account_id("urn:ietf:params:jmap:mail");

        let mut client_conditions = conditions.clone();
        if let Some(f) = folder {
            let mailbox_id = match lookup_mailbox_id(&session, f) {
                Ok(id) => id,
                Err(e) => {
                    error_messages.push(format!("Error for {}: {}", name, e));
                    continue;
                }
            };
            client_conditions.push(Filter::in_mailbox(&mailbox_id));
        }

        if client_conditions.is_empty() {
            error_messages.push(format!("Error for {}: At least one filter field must be provided (keyword, folder, start_date, end_date, from, to, is_unread, is_flagged)", name));
            continue;
        }

        let query_filter = if client_conditions.len() == 1 {
            CoreFilter::from(client_conditions.remove(0))
        } else {
            CoreFilter::and(client_conditions)
        };

        let mut query_response = match session
            .inner()
            .email_query(Some(query_filter), None::<Vec<_>>)
        {
            Ok(q) => q,
            Err(e) => {
                tracing::error!(name = "tool.email.search.query_failed", client = %name, error = %e, "[email] email_query error for {}: {}", name, e);
                error_messages.push(format!("Error querying email for {}: {}", name, e));
                continue;
            }
        };

        let email_ids = query_response.take_ids();
        tracing::debug!(
            client = %name,
            count = email_ids.len(),
            "[email] email_query returned {} ids for {}",
            email_ids.len(),
            name
        );
        if email_ids.is_empty() {
            continue;
        }

        for email_id in &email_ids {
            match email_get_full(&session, email_id) {
                Ok(Some(mut email)) => {
                    tracing::debug!(
                        client = %name,
                        email_id = %email_id,
                        "[email] email_get succeeded for id={}",
                        email_id
                    );
                    let email_json = simplify_email(&mut email, Some(10));
                    all_items.push(SearchEmailItem {
                        client: name.clone(),
                        email: email_json,
                    });
                }
                Ok(None) => {
                    tracing::warn!(
                        client = %name,
                        email_id = %email_id,
                        "[email] email_get returned None for id={}",
                        email_id
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        client = %name,
                        email_id = %email_id,
                        error = %e,
                        "[email] email_get error for id={}: {}",
                        email_id,
                        e
                    );
                }
            }
        }
    }

    let total = all_items.len();
    Ok(SearchEmailCacheEntry {
        items: all_items,
        cursor_offset: 0,
        total,
        fetched_at: std::time::Instant::now(),
        errors: error_messages,
    })
}

/// Format a slice of cached search-email items as the
/// `SearchEmailResponse.results` payload: a `--- Client: X ---`
/// block per client, followed by per-client error messages.
fn format_search_page(page_items: &[&SearchEmailItem], errors: &[String]) -> String {
    use std::collections::BTreeMap;
    let mut client_items: BTreeMap<&str, Vec<&serde_json::Value>> = BTreeMap::new();
    for item in page_items {
        client_items
            .entry(item.client.as_str())
            .or_default()
            .push(&item.email);
    }

    let mut result_parts: Vec<String> = Vec::new();
    for (client, items) in &client_items {
        result_parts.push(format!(
            "--- Client: {} ---\n{}",
            client,
            serde_json::to_string_pretty(items).unwrap_or_default()
        ));
    }
    result_parts.extend(errors.iter().cloned());
    result_parts.join("\n\n")
}

/// Search emails across all configured JMAP clients using a
/// cursor-based paging model backed by the shared `ToolCache`. The
/// first call (no cursor) queries JMAP once, caches the full result
/// set under a fresh UUID cursor, and returns the first page. Each
/// subsequent call with the same cursor slices the next page from
/// the cache without re-querying JMAP. On an unknown or expired
/// cursor, the helper returns an error instructing the LLM to
/// re-run the search with no cursor.
pub fn tool_search_email(
    config: &AppConfig,
    filters: SearchEmailFilters<'_>,
    cursor: Option<String>,
    cache: &crate::agent::tools::registry::cache::ToolCache,
    uuid_gen: &dyn crate::utils::uuid::UuidGenerator,
) -> Result<crate::agent::tools::dtos::SearchEmailResponse, String> {
    use crate::agent::tools::registry::cache::{CacheEntry, SearchEmailCacheEntry, SearchEmailItem};

    // First call: query JMAP, populate the cache, return first page + cursor.
    let Some(cursor) = cursor else {
        let entry = fetch_full_search_result(config, &filters)?;
        let total = entry.total;
        let first_page: Vec<SearchEmailItem> = entry
            .items
            .iter()
            .take(SEARCH_EMAIL_PAGE_SIZE)
            .cloned()
            .collect();
        let next_offset = first_page.len();
        let new_cursor = new_search_email_cursor(uuid_gen);

        // Store the cache entry (a clone of the items we will keep
        // serving from). The cursor_offset is set so the next call
        // returns the items immediately after this batch.
        let cache_entry = SearchEmailCacheEntry {
            items: entry.items,
            cursor_offset: next_offset,
            total,
            fetched_at: entry.fetched_at,
            errors: entry.errors.clone(),
        };
        cache.put(new_cursor.clone(), CacheEntry::SearchEmail(cache_entry));

        // Empty result set: no cursor, hint says "no matches". We
        // do not need to keep the cache entry we just inserted, so
        // invalidate it before returning.
        if total == 0 {
            cache.invalidate(&new_cursor);
            return Ok(crate::agent::tools::dtos::SearchEmailResponse {
                results: "No matching emails found.".to_string(),
                total: 0,
                cursor: None,
                hint: Some("No matching emails found.".to_string()),
            });
        }

        // If we served every item in this batch, the cursor is
        // final: emit a hint instead of a cursor so the LLM stops
        // walking. If there are more, the LLM gets the cursor and
        // makes a follow-up call.
        let (cursor_out, hint_out) = if next_offset >= total {
            // All items already returned in this first page; remove
            // the cache entry since there will be no follow-up.
            cache.invalidate(&new_cursor);
            (None, Some(SEARCH_EMAIL_FINAL_PAGE_HINT.to_string()))
        } else {
            (Some(new_cursor), None)
        };

        let first_refs: Vec<&SearchEmailItem> = first_page.iter().collect();
        let results = format_search_page(&first_refs, &entry.errors);

        return Ok(crate::agent::tools::dtos::SearchEmailResponse {
            results,
            total,
            cursor: cursor_out,
            hint: hint_out,
        });
    };

    // Subsequent call: look up the cache, slice, return next page.
    let entry = match cache.get(&cursor) {
        Some(CacheEntry::SearchEmail(e)) => e,
        _ => {
            return Err("Cursor expired or unknown; re-run the search with no cursor.".to_string());
        }
    };

    let total = entry.total;
    let start = entry.cursor_offset;
    if start >= total {
        // Cursor used after final page was already returned.
        // This indicates the LLM incorrectly reused a cursor.
        // Return the same error as for an unknown/expired cursor.
        return Err("Cursor expired or unknown; re-run the search with no cursor.".to_string());
    }

    let end = (start + SEARCH_EMAIL_PAGE_SIZE).min(total);
    let page_refs: Vec<&SearchEmailItem> = entry.items[start..end].iter().collect();
    let results = format_search_page(&page_refs, &entry.errors);

    // Advance the cached offset for the next call.
    let new_offset = end;
    let (cursor_out, hint_out) = if new_offset >= total {
        // Final page.
        (None, Some(SEARCH_EMAIL_FINAL_PAGE_HINT.to_string()))
    } else {
        // More pages exist: same cursor, advanced offset.
        (Some(cursor.clone()), None)
    };

    // Update the cached entry with the new offset. Re-put is the
    // simplest way; the cache key is unchanged so the entry stays
    // in the same slot.
    cache.put(
        cursor,
        CacheEntry::SearchEmail(SearchEmailCacheEntry {
            items: entry.items,
            cursor_offset: new_offset,
            total,
            fetched_at: entry.fetched_at,
            errors: entry.errors,
        }),
    );

    Ok(crate::agent::tools::dtos::SearchEmailResponse {
        results,
        total,
        cursor: cursor_out,
        hint: hint_out,
    })
}

/// Get a single email by its JMAP ID.
pub fn tool_get_email_by_id(
    config: &AppConfig,
    id: &str,
) -> Result<crate::agent::tools::dtos::GetEmailByIdResponse, String> {
    for (name, client) in &config.jmap_clients {
        let session = match JmapSession::connect(client) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(name = "tool.email.get_by_id.session_failed", client = %name, error = %e, "Failed to fetch JMAP session. Operator should check email account credentials.");
                continue;
            }
        };

        match email_get_full(&session, id) {
            Ok(Some(mut email)) => {
                let email_json = simplify_email(&mut email, None);
                return Ok(crate::agent::tools::dtos::GetEmailByIdResponse {
                    result: serde_json::to_string_pretty(&email_json).unwrap_or_default(),
                });
            }
            Ok(None) => {
                tracing::warn!(client = %name, email_id = %id, "Email not found in response");
            }
            Err(e) => {
                tracing::error!(name = "tool.email.get_by_id.api_failed", client = %name, error = %e, "Failed to fetch email by ID via JMAP.");
            }
        }
    }
    tracing::warn!(name = "tool.email.get_by_id.not_found", id = %id, "Email not found in any client or no clients configured.");
    Err("Email not found in any client or no clients configured.".to_string())
}

/// Footer appended to all emails sent by the AI agent.
pub const AI_AGENT_FOOTER: &str = "\n---\nSent by FastMD on behalf of the user";

/// Send an email using `Email/set` to create the message and `EmailSubmission/set`
/// to submit it for delivery via the typed crate API.
pub fn tool_send_email(
    config: &AppConfig,
    to: &str,
    subject: &str,
    body: &str,
) -> Result<crate::agent::tools::dtos::SendEmailResponse, String> {
    let mut all_results = Vec::new();
    if let Some((name, client)) = config.jmap_clients.iter().next() {
        let mut session = match JmapSession::connect(client) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(name = "tool.email.send.session_failed", client = %name, error = %e, "Failed to fetch JMAP session. Operator should check email account credentials.");
                all_results.push(format!("Error fetching JMAP session for {}: {}", name, e));
                return Err(all_results.join("\n\n"));
            }
        };

        let account_id = session.account_id("urn:ietf:params:jmap:mail");

        let inbox_id = match resolve_inbox(&session, &account_id) {
            Ok(id) => id,
            Err(e) => {
                tracing::error!(name = "tool.email.send.resolve_inbox_failed", client = %name, error = %e, "Failed to resolve inbox mailbox ID.");
                all_results.push(format!("Error resolving inbox for {}: {}", name, e));
                return Err(all_results.join("\n\n"));
            }
        };

        let identity_email = match get_first_identity_email(&session) {
            Ok(email) => email,
            Err(e) => {
                tracing::error!(name = "tool.email.send.identity_failed", client = %name, error = %e, "Failed to retrieve identity for sending.");
                all_results.push(format!("Error retrieving identity for {}: {}", name, e));
                return Err(all_results.join("\n\n"));
            }
        };

        let full_body = format!("{body}{AI_AGENT_FOOTER}");
        let email_id = match create_email_via_set(
            &session,
            &inbox_id,
            &identity_email,
            to,
            subject,
            &full_body,
        ) {
            Ok(id) => id,
            Err(e) => {
                tracing::error!(name = "tool.email.send.create_failed", client = %name, error = %e, "Failed to create email via JMAP Email/set.");
                all_results.push(format!("Error creating email for {}: {}", name, e));
                return Err(all_results.join("\n\n"));
            }
        };

        let identity_id = match get_first_identity_id(&session) {
            Ok(id) => id,
            Err(e) => {
                tracing::error!(name = "tool.email.send.identity_id_failed", client = %name, error = %e, "Failed to retrieve identity ID for submission.");
                all_results.push(format!("Error retrieving identity ID for {}: {}", name, e));
                return Err(all_results.join("\n\n"));
            }
        };

        match session
            .inner()
            .email_submission_create(&email_id, &identity_id)
        {
            Ok(_) => {
                all_results.push(format!(
                    "--- Client: {} ---\nEmail sent successfully with ID: {}",
                    name, email_id
                ));
            }
            Err(e) => {
                tracing::error!(name = "tool.email.send.submission_failed", client = %name, error = %e, "Failed to submit email for delivery via JMAP.");
                all_results.push(format!("Error submitting email for {}: {}", name, e));
            }
        }
    }
    if all_results.is_empty() {
        tracing::warn!(
            name = "tool.email.send.no_clients",
            "No JMAP clients configured."
        );
        Err("No JMAP clients configured.".to_string())
    } else {
        Ok(crate::agent::tools::dtos::SendEmailResponse {
            result: all_results.join("\n\n"),
        })
    }
}

/// Create an email via `Email/set` using the typed crate request builder.
fn create_email_via_set(
    session: &JmapSession,
    inbox_id: &str,
    from_email: &str,
    to: &str,
    subject: &str,
    body: &str,
) -> Result<String, String> {
    let mut request = session.inner().build();
    let create_id = request
        .set_email()
        .create()
        .mailbox_ids([inbox_id])
        .from([from_email])
        .to([to])
        .subject(subject)
        .text_body(jmap_client::email::EmailBodyPart::new().part_id("1"))
        .body_value("1".to_string(), body)
        .create_id()
        .ok_or_else(|| "Failed to obtain create ID for Email/set".to_string())?;

    let mut response: EmailSetResponse = request
        .send_single()
        .map_err(|e| format!("Email/set request failed: {e}"))?;

    let email = response
        .created(&create_id)
        .map_err(|e| format!("Email/set creation failed: {e}"))?;

    email
        .id()
        .map(|s| s.to_string())
        .ok_or_else(|| "Email/set response missing created email ID".to_string())
}

/// Retrieve the email address of the first identity via `Identity/get`.
fn get_first_identity_email(session: &JmapSession) -> Result<String, String> {
    let mut request = session.inner().build();
    request.add_capability(jmap_client::URI::Submission);
    request.get_identity();
    let mut response = request
        .send_get_identity()
        .map_err(|e| format!("Identity/get request failed: {e}"))?;

    let identities = response.take_list();
    identities
        .first()
        .and_then(|id| id.email())
        .map(|s| s.to_string())
        .ok_or_else(|| "No identity with an email address found".to_string())
}

/// Retrieve the ID of the first identity via `Identity/get`.
fn get_first_identity_id(session: &JmapSession) -> Result<String, String> {
    let mut request = session.inner().build();
    request.add_capability(jmap_client::URI::Submission);
    request.get_identity();
    let mut response = request
        .send_get_identity()
        .map_err(|e| format!("Identity/get request failed: {e}"))?;

    let identities = response.take_list();
    identities
        .first()
        .and_then(|id| id.id())
        .map(|s| s.to_string())
        .ok_or_else(|| "No identity found".to_string())
}

/// Resolve the Inbox mailbox ID using the typed crate `mailbox_query` with a role filter.
fn resolve_inbox(session: &JmapSession, _account_id: &str) -> Result<String, String> {
    let mut query = session
        .inner()
        .mailbox_query(
            Some(jmap_client::mailbox::query::Filter::role(
                jmap_client::mailbox::Role::Inbox,
            )),
            None::<Vec<_>>,
        )
        .map_err(|e| format!("Mailbox/query error: {e}"))?;

    let ids = query.take_ids();
    if ids.is_empty() {
        tracing::warn!(
            name = "tool.email.resolve_inbox.not_found",
            "Mailbox/query returned no inbox IDs"
        );
        return Err("Inbox mailbox not found".to_string());
    }
    Ok(ids.into_iter().next().unwrap())
}

/// Simplify JMAP `methodResponses` containing `Email/get` results.
#[allow(dead_code)]
pub(crate) fn simplify_jmap_emails(
    res: serde_json::Value,
    max_lines: Option<usize>,
) -> serde_json::Value {
    let mut simplified_emails = Vec::new();

    if let Some(method_responses) = res.get("methodResponses").and_then(|mr| mr.as_array()) {
        for resp in method_responses {
            if let Some(resp_arr) = resp.as_array()
                && resp_arr.first().and_then(|n| n.as_str()) == Some("Email/get")
                && let Some(args) = resp_arr.get(1).and_then(|a| a.as_object())
                && let Some(list) = args.get("list").and_then(|l| l.as_array())
            {
                for email_val in list {
                    let mut simplified = serde_json::Map::new();

                    let id = email_val
                        .get("id")
                        .unwrap_or(&serde_json::Value::Null)
                        .clone();
                    simplified.insert("id".to_string(), id);

                    let subject = email_val
                        .get("subject")
                        .unwrap_or(&serde_json::Value::Null)
                        .clone();
                    simplified.insert("subject".to_string(), subject);

                    let date = email_val
                        .get("receivedAt")
                        .unwrap_or(&serde_json::Value::Null)
                        .clone();
                    simplified.insert("date".to_string(), date);

                    let from = email_val
                        .get("from")
                        .unwrap_or(&serde_json::Value::Null)
                        .clone();
                    simplified.insert("from".to_string(), from);

                    let to = email_val
                        .get("to")
                        .unwrap_or(&serde_json::Value::Null)
                        .clone();
                    simplified.insert("to".to_string(), to);

                    let cc = email_val
                        .get("cc")
                        .unwrap_or(&serde_json::Value::Null)
                        .clone();
                    simplified.insert("cc".to_string(), cc);

                    let bcc = email_val
                        .get("bcc")
                        .unwrap_or(&serde_json::Value::Null)
                        .clone();
                    simplified.insert("bcc".to_string(), bcc);

                    let mut body_str = String::new();
                    let mut is_truncated = false;
                    if let Some(body_values) =
                        email_val.get("bodyValues").and_then(|bv| bv.as_object())
                    {
                        let mut found_html = false;
                        if let Some(html_bodies) =
                            email_val.get("htmlBody").and_then(|h| h.as_array())
                            && let Some(first_html) =
                                html_bodies.first().and_then(|h| h.as_object())
                            && let Some(part_id) = first_html.get("partId").and_then(|p| p.as_str())
                            && let Some(part_val) =
                                body_values.get(part_id).and_then(|v| v.as_object())
                            && let Some(val) = part_val.get("value").and_then(|v| v.as_str())
                        {
                            body_str = val.to_string();
                            is_truncated = part_val
                                .get("isTruncated")
                                .and_then(|t| t.as_bool())
                                .unwrap_or(false);
                            found_html = true;
                        }

                        if !found_html
                            && let Some(text_bodies) =
                                email_val.get("textBody").and_then(|t| t.as_array())
                            && let Some(first_text) =
                                text_bodies.first().and_then(|t| t.as_object())
                            && let Some(part_id) = first_text.get("partId").and_then(|p| p.as_str())
                            && let Some(part_val) =
                                body_values.get(part_id).and_then(|v| v.as_object())
                            && let Some(val) = part_val.get("value").and_then(|v| v.as_str())
                        {
                            body_str = val.to_string();
                            is_truncated = part_val
                                .get("isTruncated")
                                .and_then(|t| t.as_bool())
                                .unwrap_or(false);
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

    serde_json::Value::Array(simplified_emails)
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `email_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "email_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "email_proptests.rs"]
mod email_proptests;
