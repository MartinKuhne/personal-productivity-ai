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

use crate::config::AppConfig;
use fast_h2m::convert;
use jmap_client::core::query::Filter as CoreFilter;
use jmap_client::core::query::Filter as MailFilter;
use jmap_client::core::response::EmailSetResponse;
use jmap_client::core::set::SetObject;
use jmap_client::email::query::Filter;

use super::client::JmapSession;

/// Convert HTML body values in a JMAP response to Markdown using `fast_h2m`.
fn convert_html_in_jmap(mut res: serde_json::Value) -> serde_json::Value {
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
    } else {
        simplified.insert("date".to_string(), serde_json::Value::Null);
    }

    simplified.insert("from".to_string(), serialize_address_list(email.from()));
    simplified.insert("to".to_string(), serialize_address_list(email.to()));
    simplified.insert("cc".to_string(), serialize_address_list(email.cc()));
    simplified.insert("bcc".to_string(), serialize_address_list(email.bcc()));

    // Extract body: prefer htmlBody, fall back to textBody
    let mut body_str = String::new();
    let mut is_truncated = false;

    if let Some(html_parts) = email.html_body()
        && let Some(first) = html_parts.first()
        && let Some(part_id) = first.part_id()
        && let Some(body_val) = email.body_value(part_id)
    {
        body_str = body_val.value().to_string();
        is_truncated = body_val.is_truncated();
    }

    if body_str.is_empty()
        && let Some(text_parts) = email.text_body()
        && let Some(first) = text_parts.first()
        && let Some(part_id) = first.part_id()
        && let Some(body_val) = email.body_value(part_id)
    {
        body_str = body_val.value().to_string();
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
) -> serde_json::Value {
    match addrs {
        Some(list) => {
            let json: Vec<serde_json::Value> = list
                .iter()
                .map(|addr| {
                    let mut obj = serde_json::Map::new();
                    obj.insert(
                        "name".to_string(),
                        serde_json::Value::String(addr.name().unwrap_or("").to_string()),
                    );
                    obj.insert(
                        "email".to_string(),
                        serde_json::Value::String(addr.email().to_string()),
                    );
                    serde_json::Value::Object(obj)
                })
                .collect();
            serde_json::Value::Array(json)
        }
        None => serde_json::Value::Null,
    }
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

/// Pagination for `tool_search_email`.
#[derive(Debug, Clone, Copy)]
pub struct SearchEmailPagination {
    pub page: usize,
    pub page_size: usize,
}

impl Default for SearchEmailPagination {
    fn default() -> Self {
        Self {
            page: 1,
            page_size: 10,
        }
    }
}

/// Search emails across all configured JMAP clients.
pub fn tool_search_email(
    config: &AppConfig,
    filters: SearchEmailFilters<'_>,
    pagination: SearchEmailPagination,
) -> Result<crate::agent::tools::dtos::SearchEmailResponse, String> {
    let keyword = filters.keyword;
    let folder = filters.folder;
    let start_date = filters.start_date;
    let end_date = filters.end_date;
    let from = filters.from;
    let to = filters.to;
    let is_unread = filters.is_unread;
    let is_flagged = filters.is_flagged;
    let page = pagination.page.max(1);
    let page_size = pagination.page_size.max(1);

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

    let mut all_items: Vec<(String, serde_json::Value)> = Vec::new();
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

        let mut emails_json = Vec::new();
        for email_id in &email_ids {
            match session
                .inner()
                .email_get(email_id, None::<Vec<jmap_client::email::Property>>)
            {
                Ok(Some(mut email)) => {
                    tracing::debug!(
                        client = %name,
                        email_id = %email_id,
                        "[email] email_get succeeded for id={}",
                        email_id
                    );
                    let email_json = convert_html_in_jmap(simplify_email(&mut email, Some(10)));
                    emails_json.push(email_json);
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

        for email_json in emails_json {
            all_items.push((name.clone(), email_json));
        }
    }

    let total = all_items.len();

    let (page_items, hint) = if total == 0 {
        let hint = if error_messages.is_empty() {
            Some("No matching emails found.".to_string())
        } else {
            None
        };
        (Vec::new(), hint)
    } else {
        let start = page.saturating_sub(1).saturating_mul(page_size);
        if start >= total {
            (
                Vec::new(),
                Some(format!(
                    "No emails on page {page} (showing 0 of {total} total, page_size: {page_size})."
                )),
            )
        } else {
            let end = (start + page_size).min(total);
            (all_items[start..end].to_vec(), None)
        }
    };

    use std::collections::BTreeMap;
    let mut client_items: BTreeMap<&str, Vec<&serde_json::Value>> = BTreeMap::new();
    for (client, item) in &page_items {
        client_items.entry(client.as_str()).or_default().push(item);
    }

    let mut result_parts: Vec<String> = Vec::new();
    for (client, items) in &client_items {
        result_parts.push(format!(
            "--- Client: {} ---\n{}",
            client,
            serde_json::to_string_pretty(items).unwrap_or_default()
        ));
    }
    result_parts.extend(error_messages);

    if config.jmap_clients.is_empty() {
        tracing::warn!(
            name = "tool.email.search.no_clients",
            "No JMAP clients configured."
        );
        Err("No JMAP clients configured.".to_string())
    } else {
        Ok(crate::agent::tools::dtos::SearchEmailResponse {
            results: result_parts.join("\n\n"),
            total,
            hint,
        })
    }
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

        match session
            .inner()
            .email_get(id, None::<Vec<jmap_client::email::Property>>)
        {
            Ok(Some(mut email)) => {
                let email_json = convert_html_in_jmap(simplify_email(&mut email, None));
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
pub const AI_AGENT_FOOTER: &str = "\n---\nSent by an AI agent";

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
fn simplify_jmap_emails(res: serde_json::Value, max_lines: Option<usize>) -> serde_json::Value {
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{convert_html_in_jmap, simplify_jmap_emails};

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
        assert!(
            result["methodResponses"][0][1]["list"][0]["subject"]
                .as_str()
                .is_some()
        );
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
        convert_html_in_jmap(res);
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
        convert_html_in_jmap(res);
    }

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
                            "from": [{"email": "a@t.com"}],
                            "to": [{"email": "b@t.com"}],
                            "htmlBody": [{"partId": "p1"}],
                            "bodyValues": { "p1": { "value": "Body 1", "isTruncated": false } }
                        },
                        {
                            "id": "e2",
                            "subject": "Second",
                            "receivedAt": "2026-07-19T11:00:00Z",
                            "from": [{"email": "c@t.com"}],
                            "to": [{"email": "d@t.com"}],
                            "htmlBody": [{"partId": "p2"}],
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
                        "from": [{"email": "a@t.com"}],
                        "to": [{"email": "b@t.com"}],
                        "htmlBody": [{"partId": "p1"}],
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
                        "from": [{"email": "a@t.com"}],
                        "to": [{"email": "b@t.com"}],
                        "htmlBody": [{"partId": "p1"}],
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
                        "from": [{"email": "a@t.com"}],
                        "to": [{"email": "b@t.com"}],
                        "htmlBody": [{"partId": "p1"}],
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
                        "from": [{"email": "a@t.com"}],
                        "to": [{"email": "b@t.com"}],
                        "htmlBody": [{"partId": "p1"}],
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
                        "from": [{"email": "a@t.com"}],
                        "to": [{"email": "b@t.com"}],
                        "cc": [{"email": "cc@t.com"}],
                        "bcc": [{"email": "bcc@t.com"}],
                        "htmlBody": [{"partId": "p1"}],
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

    use super::{
        SearchEmailFilters, SearchEmailPagination, tool_get_email_by_id, tool_search_email,
        tool_send_email,
    };
    use crate::agent::tools::jmap::spawn_mock_server;
    use crate::config::{AppConfig, JmapClient};
    #[test]
    fn test_tool_search_email_no_clients() {
        let config = AppConfig::default();
        let res = tool_search_email(
            &config,
            SearchEmailFilters {
                keyword: Some("test"),
                ..Default::default()
            },
            SearchEmailPagination {
                page: 1,
                page_size: 10,
            },
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_tool_search_email_success() {
        rustls::crypto::ring::default_provider()
            .install_default()
            .ok();
        let body = "{\
            \"apiUrl\": \"{API_URL}\",\
            \"primaryAccounts\": {\"urn:ietf:params:jmap:mail\": \"acc1\"},\
            \"methodResponses\": [\
                [\"Email/query\", {\"ids\": [\"e1\"]}, \"0\"],\
                [\"Email/get\", {\"list\": [{\"id\": \"e1\", \"subject\": \"Test\"}]}, \"1\"]\
            ]\
        }";
        let url = spawn_mock_server(body);
        let mut config = AppConfig::default();
        config.jmap_clients.insert(
            "test".to_string(),
            JmapClient {
                url,
                token: "tok".to_string(),
            },
        );
        let res = tool_search_email(
            &config,
            SearchEmailFilters {
                keyword: Some("test"),
                ..Default::default()
            },
            SearchEmailPagination {
                page: 1,
                page_size: 10,
            },
        );
        assert!(res.is_ok());
    }

    #[test]
    fn test_tool_get_email_by_id_success() {
        rustls::crypto::ring::default_provider()
            .install_default()
            .ok();
        let body = "{\
            \"apiUrl\": \"{API_URL}\",\
            \"primaryAccounts\": {\"urn:ietf:params:jmap:mail\": \"acc1\"},\
            \"methodResponses\": [\
                [\"Email/get\", {\"list\": [{\"id\": \"e1\", \"subject\": \"Test\"}]}, \"0\"]\
            ]\
        }";
        let url = spawn_mock_server(body);
        let mut config = AppConfig::default();
        config.jmap_clients.insert(
            "test".to_string(),
            JmapClient {
                url,
                token: "tok".to_string(),
            },
        );
        let res = tool_get_email_by_id(&config, "e1");
        assert!(res.is_ok(), "Error: {}", res.unwrap_err());
    }

    #[test]
    fn test_tool_search_email_with_status_filters_success() {
        rustls::crypto::ring::default_provider()
            .install_default()
            .ok();
        let body = "{\
            \"apiUrl\": \"{API_URL}\",\
            \"primaryAccounts\": {\"urn:ietf:params:jmap:mail\": \"acc1\"},\
            \"methodResponses\": [\
                [\"Email/query\", {\"ids\": [\"e1\"]}, \"0\"],\
                [\"Email/get\", {\"list\": [{\"id\": \"e1\", \"subject\": \"Unread\"}]}, \"1\"]\
            ]\
        }";
        let url = spawn_mock_server(body);
        let mut config = AppConfig::default();
        config.jmap_clients.insert(
            "test".to_string(),
            JmapClient {
                url,
                token: "tok".to_string(),
            },
        );
        let res = tool_search_email(
            &config,
            SearchEmailFilters {
                keyword: None,
                folder: None,
                start_date: Some("2026-07-01"),
                end_date: Some("2026-07-10"),
                from: Some("s@test.com"),
                to: Some("r@test.com"),
                is_unread: Some(true),
                is_flagged: Some(false),
            },
            SearchEmailPagination {
                page: 1,
                page_size: 10,
            },
        );
        assert!(res.is_ok());
    }

    #[test]
    fn test_tool_send_email_success() {
        rustls::crypto::ring::default_provider()
            .install_default()
            .ok();
        let body = "{\
            \"apiUrl\": \"{API_URL}\",\
            \"primaryAccounts\": {\"urn:ietf:params:jmap:mail\": \"acc1\", \"urn:ietf:params:jmap:submission\": \"acc1\"},\
            \"methodResponses\": [\
                [\"Mailbox/query\", {\"ids\": [\"inbox-id\"]}, \"0\"],\
                [\"Identity/get\", {\"state\": \"id-state-0\", \"list\": [{\"id\": \"ident-1\", \"email\": \"sender@test.com\"}], \"notFound\": []}, \"1\"],\
                [\"Email/set\", {\"created\": {\"c0\": {\"id\": \"email-1\"}}}, \"2\"],\
                [\"EmailSubmission/set\", {\"created\": {\"c0\": {\"id\": \"sub-1\"}}}, \"3\"]\
            ]\
        }";
        let url = spawn_mock_server(body);
        let mut config = AppConfig::default();
        config.jmap_clients.insert(
            "test".to_string(),
            JmapClient {
                url,
                token: "tok".to_string(),
            },
        );
        let res = tool_send_email(&config, "to@test.com", "Subject", "Body");
        assert!(res.is_ok(), "Error: {}", res.unwrap_err());
    }

    #[test]
    fn test_tool_send_email_ai_agent_footer() {
        use super::AI_AGENT_FOOTER;
        assert_eq!(AI_AGENT_FOOTER, "\n---\nSent by an AI agent");
    }

    #[test]
    fn test_tool_search_email_empty_filters_errors() {
        let config = AppConfig::default();
        let res = tool_search_email(
            &config,
            SearchEmailFilters {
                ..Default::default()
            },
            SearchEmailPagination {
                page: 1,
                page_size: 10,
            },
        );
        assert!(res.is_err());
        let msg = res.unwrap_err();
        assert!(msg.contains("At least one filter field must be provided"));
    }

    #[test]
    fn test_tool_search_email_empty_filters_with_client_errors() {
        rustls::crypto::ring::default_provider()
            .install_default()
            .ok();
        let mut config = AppConfig::default();
        let body = "{\"apiUrl\": \"{API_URL}\", \"primaryAccounts\": {\"urn:ietf:params:jmap:mail\": \"acc1\"}}";
        let url = spawn_mock_server(body);
        config.jmap_clients.insert(
            "test".to_string(),
            JmapClient {
                url,
                token: "tok".to_string(),
            },
        );
        let res = tool_search_email(
            &config,
            SearchEmailFilters {
                ..Default::default()
            },
            SearchEmailPagination {
                page: 1,
                page_size: 10,
            },
        );
        assert!(res.is_err());
        assert!(
            res.unwrap_err()
                .contains("At least one filter field must be provided")
        );
    }

    #[test]
    fn test_tool_search_email_pagination_default_page_size_is_10() {
        rustls::crypto::ring::default_provider()
            .install_default()
            .ok();
        let body = r#"{
                "apiUrl": "{API_URL}",
                "primaryAccounts": {"urn:ietf:params:jmap:mail": "acc1"},
                "methodResponses": [
                    ["Email/query", {"ids": ["e1","e2","e3"]}, "0"],
                    ["Email/get", {
                        "list": [
                            {"id": "e1", "subject": "First", "receivedAt": "2026-07-19T10:00:00Z", "from": [{"email":"a@t.com"}], "to": [{"email":"b@t.com"}], "htmlBody": [{"partId":"p1"}], "bodyValues": {"p1": {"value":"Body 1","isTruncated":false}}},
                            {"id": "e2", "subject": "Second", "receivedAt": "2026-07-19T11:00:00Z", "from": [{"email":"c@t.com"}], "to": [{"email":"d@t.com"}], "htmlBody": [{"partId":"p2"}], "bodyValues": {"p2": {"value":"Body 2","isTruncated":false}}},
                            {"id": "e3", "subject": "Third", "receivedAt": "2026-07-19T12:00:00Z", "from": [{"email":"e@t.com"}], "to": [{"email":"f@t.com"}], "htmlBody": [{"partId":"p3"}], "bodyValues": {"p3": {"value":"Body 3","isTruncated":false}}}
                        ],
                        "notFound": []
                    }, "1"]
                ]
            }"#
        .to_string();
        let url = spawn_mock_server(body);
        let mut config = AppConfig::default();
        config.jmap_clients.insert(
            "test".to_string(),
            JmapClient {
                url,
                token: "tok".to_string(),
            },
        );
        let res = tool_search_email(
            &config,
            SearchEmailFilters {
                keyword: Some("test"),
                ..Default::default()
            },
            SearchEmailPagination {
                page: 1,
                page_size: 10,
            },
        );
        assert!(res.is_ok());
        let response = res.unwrap();
        assert_eq!(response.total, 3);
        assert!(response.hint.is_none());
        assert!(!response.results.is_empty());
    }

    #[test]
    fn test_tool_search_email_pagination_second_page() {
        rustls::crypto::ring::default_provider()
            .install_default()
            .ok();
        let body = r#"{
                "apiUrl": "{API_URL}",
                "primaryAccounts": {"urn:ietf:params:jmap:mail": "acc1"},
                "methodResponses": [
                    ["Email/query", {"ids": ["e1","e2","e3","e4","e5"]}, "0"],
                    ["Email/get", {
                        "list": [
                            {"id": "e1", "subject": "S1", "receivedAt": "2026-07-19T10:00:00Z", "from": [{"email":"a@t.com"}], "to": [{"email":"b@t.com"}], "htmlBody": [{"partId":"p1"}], "bodyValues": {"p1": {"value":"B1","isTruncated":false}}},
                            {"id": "e2", "subject": "S2", "receivedAt": "2026-07-19T11:00:00Z", "from": [{"email":"c@t.com"}], "to": [{"email":"d@t.com"}], "htmlBody": [{"partId":"p2"}], "bodyValues": {"p2": {"value":"B2","isTruncated":false}}},
                            {"id": "e3", "subject": "S3", "receivedAt": "2026-07-19T12:00:00Z", "from": [{"email":"e@t.com"}], "to": [{"email":"f@t.com"}], "htmlBody": [{"partId":"p3"}], "bodyValues": {"p3": {"value":"B3","isTruncated":false}}},
                            {"id": "e4", "subject": "S4", "receivedAt": "2026-07-19T13:00:00Z", "from": [{"email":"g@t.com"}], "to": [{"email":"h@t.com"}], "htmlBody": [{"partId":"p4"}], "bodyValues": {"p4": {"value":"B4","isTruncated":false}}},
                            {"id": "e5", "subject": "S5", "receivedAt": "2026-07-19T14:00:00Z", "from": [{"email":"i@t.com"}], "to": [{"email":"j@t.com"}], "htmlBody": [{"partId":"p5"}], "bodyValues": {"p5": {"value":"B5","isTruncated":false}}}
                        ],
                        "notFound": []
                    }, "1"]
                ]
            }"#
        .to_string();
        let url = spawn_mock_server(body);
        let mut config = AppConfig::default();
        config.jmap_clients.insert(
            "test".to_string(),
            JmapClient {
                url,
                token: "tok".to_string(),
            },
        );
        let res = tool_search_email(
            &config,
            SearchEmailFilters {
                keyword: Some("test"),
                ..Default::default()
            },
            SearchEmailPagination {
                page: 2,
                page_size: 2,
            },
        );
        assert!(res.is_ok());
        let response = res.unwrap();
        assert_eq!(response.total, 5);
        assert!(response.results.contains("S3") || response.results.contains("S4"));
    }

    #[test]
    fn test_tool_search_email_page_beyond_total() {
        rustls::crypto::ring::default_provider()
            .install_default()
            .ok();
        let body = r#"{
                "apiUrl": "{API_URL}",
                "primaryAccounts": {"urn:ietf:params:jmap:mail": "acc1"},
                "methodResponses": [
                    ["Email/query", {"ids": ["e1"]}, "0"],
                    ["Email/get", {
                        "list": [
                            {"id": "e1", "subject": "Only", "receivedAt": "2026-07-19T10:00:00Z", "from": [{"email":"a@t.com"}], "to": [{"email":"b@t.com"}], "htmlBody": [{"partId":"p1"}], "bodyValues": {"p1": {"value":"Body","isTruncated":false}}}
                        ],
                        "notFound": []
                    }, "1"]
                ]
            }"#
        .to_string();
        let url = spawn_mock_server(body);
        let mut config = AppConfig::default();
        config.jmap_clients.insert(
            "test".to_string(),
            JmapClient {
                url,
                token: "tok".to_string(),
            },
        );
        let res = tool_search_email(
            &config,
            SearchEmailFilters {
                keyword: Some("test"),
                ..Default::default()
            },
            SearchEmailPagination {
                page: 5,
                page_size: 2,
            },
        );
        assert!(res.is_ok());
        let response = res.unwrap();
        assert_eq!(response.total, 1);
        assert!(response.hint.is_some());
        assert!(response.hint.unwrap().contains("No emails on page 5"));
    }

    #[test]
    fn test_tool_search_email_multiple_clients() {
        rustls::crypto::ring::default_provider()
            .install_default()
            .ok();
        let body = r#"{
                "apiUrl": "{API_URL}",
                "primaryAccounts": {"urn:ietf:params:jmap:mail": "acc1"},
                "methodResponses": [
                    ["Email/query", {"ids": ["e1"]}, "0"],
                    ["Email/get", {
                        "list": [
                            {"id": "e1", "subject": "Multi", "receivedAt": "2026-07-19T10:00:00Z", "from": [{"email":"a@t.com"}], "to": [{"email":"b@t.com"}], "htmlBody": [{"partId":"p1"}], "bodyValues": {"p1": {"value":"Body","isTruncated":false}}}
                        ],
                        "notFound": []
                    }, "1"]
                ]
            }"#
        .to_string();
        let url = spawn_mock_server(body);
        let mut config = AppConfig::default();
        config.jmap_clients.insert(
            "client1".to_string(),
            JmapClient {
                url: url.clone(),
                token: "tok1".to_string(),
            },
        );
        config.jmap_clients.insert(
            "client2".to_string(),
            JmapClient {
                url,
                token: "tok2".to_string(),
            },
        );
        let res = tool_search_email(
            &config,
            SearchEmailFilters {
                keyword: Some("test"),
                ..Default::default()
            },
            SearchEmailPagination {
                page: 1,
                page_size: 10,
            },
        );
        assert!(res.is_ok());
        let response = res.unwrap();
        assert_eq!(response.total, 2);
        assert!(response.results.contains("client1"));
        assert!(response.results.contains("client2"));
    }

    #[test]
    fn test_tool_search_email_logs_tracing() {
        rustls::crypto::ring::default_provider()
            .install_default()
            .ok();
        let body = r#"{
                "apiUrl": "{API_URL}",
                "primaryAccounts": {"urn:ietf:params:jmap:mail": "acc1"},
                "methodResponses": [
                    ["Email/query", {"ids": ["e1", "e2"]}, "0"],
                    ["Email/get", {
                        "list": [
                            {"id": "e1", "subject": "First"},
                            {"id": "e2", "subject": "Second"}
                        ],
                        "notFound": []
                    }, "1"]
                ]
            }"#
        .to_string();
        let url = spawn_mock_server(body);
        let mut config = AppConfig::default();
        config.jmap_clients.insert(
            "fastmail".to_string(),
            JmapClient {
                url,
                token: "tok".to_string(),
            },
        );
        let res = tool_search_email(
            &config,
            SearchEmailFilters {
                keyword: Some("fastmail"),
                ..Default::default()
            },
            SearchEmailPagination {
                page: 1,
                page_size: 10,
            },
        );
        assert!(res.is_ok());
        let response = res.unwrap();
        assert_eq!(response.total, 2);
    }
}
