//! CardDAV agent tools — search, retrieve, create, update, and delete contacts across configured CardDAV servers.
//!
//! Every network round-trip is logged via `tracing` so that failures on the
//! server (e.g. FastMail returning `403 Forbidden - Mailbox does not exist`
//! for a malformed PUT path) are visible in the application log with the
//! request URL, the response status, the relevant response headers
//! (`Location`, `ETag`), and the response body.
//!
//! Unit tests live in the sibling `carddav_tests.rs` sidecar.

use crate::agent::tools::blocking::block_on;
use crate::config::AppConfig;
use fast_dav_rs::CardDavClient;

/// Cap on the number of body bytes echoed into a single tracing event.
/// CardDAV error bodies are typically small (XML error envelopes), but
/// pathological responses can be large; 4 KiB is plenty for diagnosis
/// without flooding the log.
const LOG_BODY_LIMIT: usize = 4096;

/// Truncate `body` to at most [`LOG_BODY_LIMIT`] bytes for safe logging.
fn log_truncate(body: &[u8]) -> String {
    if body.len() <= LOG_BODY_LIMIT {
        String::from_utf8_lossy(body).to_string()
    } else {
        let mut s = String::from_utf8_lossy(&body[..LOG_BODY_LIMIT]).to_string();
        s.push_str(&format!("...<truncated, total {} bytes>", body.len()));
        s
    }
}

/// Build the PUT URL for a new contact resource inside `addressbook_href`.
///
/// CardDAV hrefs returned by `PROPFIND` typically end with `/`. If we
/// concatenate the resource name directly onto the collection path without
/// a separator, the resulting URL is malformed and the server rejects the
/// PUT (FastMail responds `403 Forbidden - Mailbox does not exist`).
/// Strip any trailing `/` from the collection and re-insert a single one.
fn build_contact_put_path(addressbook_href: &str, uid: &str) -> String {
    format!("{}/{}.vcf", addressbook_href.trim_end_matches('/'), uid)
}

#[derive(serde::Serialize)]
struct CardDavContactDetails {
    client: String,
    href: String,
    fn_name: Option<String>,
    email: Option<String>,
    tel: Option<String>,
    org: Option<String>,
    vcard: String,
}

#[derive(serde::Serialize)]
struct CardDavResponse {
    results: Vec<CardDavContactDetails>,
    errors: Vec<String>,
}

async fn get_all_addressbooks(
    client: &CardDavClient,
    base_url: &str,
    username: &str,
) -> anyhow::Result<Vec<String>> {
    if let Ok(books) = client.list_addressbooks(base_url).await
        && !books.is_empty()
    {
        let hrefs: Vec<String> = books.into_iter().map(|b| b.href).collect();
        tracing::info!(
            name = "tool.carddav.addressbook.discovered",
            base_url = %base_url,
            strategy = "list_addressbooks",
            count = hrefs.len(),
            hrefs = ?hrefs,
            "Discovered addressbooks via direct PROPFIND on base URL"
        );
        return Ok(hrefs);
    }

    if let Ok(homes) = client.discover_addressbook_home_set(base_url).await
        && let Some(home) = homes.first()
        && let Ok(books) = client.list_addressbooks(home).await
        && !books.is_empty()
    {
        let hrefs: Vec<String> = books.into_iter().map(|b| b.href).collect();
        tracing::info!(
            name = "tool.carddav.addressbook.discovered",
            base_url = %base_url,
            strategy = "home_set",
            home = %home,
            count = hrefs.len(),
            hrefs = ?hrefs,
            "Discovered addressbooks via addressbook-home-set on base URL"
        );
        return Ok(hrefs);
    }

    let mut principal_opt = client
        .discover_current_user_principal()
        .await
        .ok()
        .flatten();

    if principal_opt.is_none() {
        let base_trimmed = base_url.trim_end_matches('/');
        let guess = format!("{}/dav/principals/user/{}/", base_trimmed, username);
        if let Ok(homes) = client.discover_addressbook_home_set(&guess).await
            && !homes.is_empty()
        {
            principal_opt = Some(guess);
        }
    }

    let principal = principal_opt.ok_or_else(|| {
        tracing::warn!(
            name = "tool.carddav.addressbook.no_principal",
            base_url = %base_url,
            "Could not discover a current-user-principal for CardDAV"
        );
        anyhow::anyhow!("No principal found")
    })?;
    let homes = client.discover_addressbook_home_set(&principal).await?;
    let home = homes.first().ok_or_else(|| {
        tracing::warn!(
            name = "tool.carddav.addressbook.no_home",
            principal = %principal,
            "Principal discovered but addressbook-home-set is empty"
        );
        anyhow::anyhow!("No addressbook home found")
    })?;
    let books = client.list_addressbooks(home).await?;
    let hrefs: Vec<String> = books.into_iter().map(|b| b.href).collect();
    tracing::info!(
        name = "tool.carddav.addressbook.discovered",
        base_url = %base_url,
        strategy = "principal",
        principal = %principal,
        home = %home,
        count = hrefs.len(),
        hrefs = ?hrefs,
        "Discovered addressbooks via principal URL"
    );
    Ok(hrefs)
}

async fn fetch_contacts_from_book(
    client: &CardDavClient,
    book_path: &str,
) -> anyhow::Result<Vec<(String, String)>> {
    let sync = client
        .sync_collection(book_path, None, Some(10000), true)
        .await?;
    let mut contacts = Vec::new();
    for item in sync.items {
        if item.is_deleted {
            continue;
        }
        if let Some(data) = item.address_data {
            contacts.push((item.href, data));
        }
    }
    Ok(contacts)
}

fn parse_vcard(client: &str, href: &str, data: &str) -> CardDavContactDetails {
    let mut contact = CardDavContactDetails {
        client: client.to_string(),
        href: href.to_string(),
        fn_name: None,
        email: None,
        tel: None,
        org: None,
        vcard: data.to_string(),
    };

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
        let (prop, value) = match line.split_once(':') {
            Some((p, v)) => (p, v),
            None => continue,
        };
        let prop_name = prop.split(';').next().unwrap_or("").trim();
        match prop_name {
            "FN" => contact.fn_name = Some(value.trim().to_string()),
            "EMAIL" => contact.email = Some(value.trim().to_string()),
            "TEL" => contact.tel = Some(value.trim().to_string()),
            "ORG" => contact.org = Some(value.trim().to_string()),
            _ => {}
        }
    }

    contact
}

fn escape_vcard_text(text: &str) -> String {
    text.replace("\\", "\\\\")
        .replace(";", "\\;")
        .replace(",", "\\,")
        .replace("\n", "\\n")
        .replace("\r", "")
}

/// Pull the first non-empty string value for any of `keys` from `parsed`.
///
/// CardDAV LLM callers send a wide variety of natural-language key names
/// (`name`, `fn`, `phone`, `tel`, `mobile`, `company`, `org`,
/// `organization`, `title`, `notes`, `note`). Returning the first match
/// keeps the parser liberal in what it accepts while still letting the
/// tool description point at one canonical name per field.
fn first_str<'a>(parsed: &'a serde_json::Value, keys: &[&str]) -> Option<&'a str> {
    for key in keys {
        if let Some(v) = parsed.get(*key).and_then(|v| v.as_str()) {
            let trimmed = v.trim();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }
    None
}

fn json_to_vcard(json_str: &str, uid_override: Option<&str>) -> String {
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

    // Canonical key set per the tool description, plus common aliases the
    // LLM tends to send on its own.
    let fn_name = first_str(&parsed, &["name", "fn", "full_name", "displayName"])
        .map(escape_vcard_text)
        .unwrap_or_else(|| {
            // Surface the silent-default case so the operator can see when
            // a contact was created without a real name.
            tracing::warn!(
                name = "tool.carddav.add.missing_name",
                "add_contact payload had no name/fn field; vCard FN will be \"Unknown\""
            );
            "Unknown".to_string()
        });
    let email = first_str(&parsed, &["email", "email_address", "mail"]);
    let tel = first_str(
        &parsed,
        &["phone", "tel", "telephone", "mobile", "phone_number"],
    );
    let org = first_str(&parsed, &["company", "org", "organization", "organisation"])
        .map(escape_vcard_text);
    let title = first_str(&parsed, &["title", "job_title", "role"]).map(escape_vcard_text);
    let note = first_str(&parsed, &["notes", "note", "comment"]).map(escape_vcard_text);

    if email.is_none() {
        tracing::warn!(
            name = "tool.carddav.add.missing_email",
            "add_contact payload had no email/email_address field; vCard will be created without EMAIL"
        );
    }

    let mut vcard = String::new();
    vcard.push_str("BEGIN:VCARD\r\n");
    vcard.push_str("VERSION:3.0\r\n");
    vcard.push_str(&format!("FN:{}\r\n", fn_name));
    vcard.push_str(&format!("UID:{}\r\n", uid));
    if let Some(e) = email {
        vcard.push_str(&format!("EMAIL;TYPE=INTERNET:{}\r\n", e));
    }
    if let Some(t) = tel {
        vcard.push_str(&format!("TEL;TYPE=CELL:{}\r\n", t));
    }
    if let Some(o) = org {
        vcard.push_str(&format!("ORG:{}\r\n", o));
    }
    if let Some(t) = title {
        vcard.push_str(&format!("TITLE:{}\r\n", t));
    }
    if let Some(n) = note {
        vcard.push_str(&format!("NOTE:{}\r\n", n));
    }
    vcard.push_str("END:VCARD\r\n");
    vcard
}

pub fn tool_search_contact(
    config: &AppConfig,
    keyword: &str,
) -> Result<crate::agent::tools::dtos::SearchContactResponse, String> {
    let mut results = Vec::new();
    let mut errors = Vec::new();
    let kw = keyword.to_lowercase();

    for (name, client_config) in &config.caldav_clients {
        let res = block_on(async {
            let client = CardDavClient::new(
                &client_config.url,
                Some(&client_config.username),
                Some(&client_config.password),
            )
            .map_err(|e| anyhow::anyhow!("Client config error: {}", e))?;

            let books =
                get_all_addressbooks(&client, &client_config.url, &client_config.username).await?;
            let mut matches = Vec::new();
            let mut scanned = 0usize;
            for book_path in books {
                match fetch_contacts_from_book(&client, &book_path).await {
                    Ok(contacts) => {
                        scanned += contacts.len();
                        for (href, data) in contacts {
                            if data.to_lowercase().contains(&kw) {
                                matches.push(parse_vcard(name, &href, &data));
                            }
                        }
                    }
                    Err(e) => {
                        // Fail-fast on the first broken addressbook so the
                        // operator can see a real server error (e.g. a 403
                        // from FastMail when a collection has been removed
                        // or renamed) instead of silently skipping it.
                        tracing::warn!(
                            name = "tool.carddav.search.book_failed",
                            client = %name,
                            book = %book_path,
                            error = %e,
                            "CardDAV sync_collection failed for an addressbook; aborting search"
                        );
                        return Err(e);
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
        });

        match res {
            Ok(mut matches) => results.append(&mut matches),
            Err(e) => errors.push(format!("Error on client {}: {}", name, e)),
        }
    }

    let resp = CardDavResponse { results, errors };
    Ok(crate::agent::tools::dtos::SearchContactResponse {
        results: serde_json::to_string_pretty(&resp).unwrap_or_else(|_| "{}".to_string()),
    })
}

pub fn tool_get_contact(
    config: &AppConfig,
    id: &str,
) -> Result<crate::agent::tools::dtos::GetContactResponse, String> {
    let mut results = Vec::new();
    let mut errors = Vec::new();

    for (name, client_config) in &config.caldav_clients {
        let res = block_on(async {
            let client = CardDavClient::new(
                &client_config.url,
                Some(&client_config.username),
                Some(&client_config.password),
            )
            .map_err(|e| anyhow::anyhow!("Client config error: {}", e))?;

            tracing::info!(
                name = "tool.carddav.get.request",
                client = %name,
                href = %id,
                "Fetching CardDAV contact by href"
            );
            let resp = client.get(id).await?;
            let status = resp.status();
            let body_bytes = resp.into_body();
            let body_log = log_truncate(&body_bytes);
            if !status.is_success() {
                tracing::warn!(
                    name = "tool.carddav.get.failed",
                    client = %name,
                    href = %id,
                    status = %status,
                    body = %body_log,
                    "CardDAV GET returned non-success status"
                );
                return Err(anyhow::anyhow!(
                    "Not found by href: {} - {}",
                    status,
                    body_log
                ));
            }
            Ok(parse_vcard(name, id, &body_log))
        });

        match res {
            Ok(data) => results.push(data),
            Err(e) => errors.push(format!("Error on client {}: {}", name, e)),
        }
    }

    let resp = CardDavResponse { results, errors };
    Ok(crate::agent::tools::dtos::GetContactResponse {
        result: serde_json::to_string_pretty(&resp).unwrap_or_else(|_| "{}".to_string()),
    })
}

pub fn tool_add_contact(
    config: &AppConfig,
    contact_json: &str,
) -> Result<crate::agent::tools::dtos::AddContactResponse, String> {
    let mut all_results = Vec::new();
    if let Some((name, client_config)) = config.caldav_clients.iter().next() {
        let res = block_on(async {
            let client = CardDavClient::new(
                &client_config.url,
                Some(&client_config.username),
                Some(&client_config.password),
            )
            .map_err(|e| anyhow::anyhow!("Client config error: {}", e))?;

            let books =
                get_all_addressbooks(&client, &client_config.url, &client_config.username).await?;
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
            let path = build_contact_put_path(default_book, &uid);
            let vcard_data = json_to_vcard(contact_json, Some(&uid));

            tracing::info!(
                name = "tool.carddav.add.request",
                client = %name,
                addressbook = %default_book,
                uid = %uid,
                path = %path,
                vcard_bytes = vcard_data.len(),
                "Sending CardDAV PUT (If-None-Match: *) to create contact"
            );
            tracing::debug!(
                name = "tool.carddav.add.vcard",
                client = %name,
                vcard = %vcard_data,
                "vCard body for contact creation"
            );
            let vcard_bytes: bytes::Bytes = vcard_data.into_bytes().into();

            let resp = client.put_if_none_match(&path, vcard_bytes).await?;
            let status = resp.status();
            // Capture Location/ETag headers BEFORE consuming the body — they
            // are critical for diagnosing "server said 2xx but the contact
            // isn't there" cases. Use string literals so we don't need a
            // direct `http` crate dependency.
            let location = resp
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            let etag = resp
                .headers()
                .get("etag")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            let body_bytes = resp.into_body();
            let body_log = log_truncate(&body_bytes);

            if !status.is_success() {
                tracing::error!(
                    name = "tool.carddav.add.failed",
                    client = %name,
                    path = %path,
                    status = %status,
                    location = ?location,
                    etag = ?etag,
                    body = %body_log,
                    "CardDAV PUT returned non-success status; contact was NOT created"
                );
                return Err(anyhow::anyhow!(
                    "Failed to PUT contact: {} - {}",
                    status,
                    body_log
                ));
            }

            // Even on 2xx, log enough detail that the operator can confirm
            // the server actually accepted the resource. FastMail and other
            // providers occasionally return 2xx for a no-op or for a put
            // that landed at a different URL than the one we sent.
            tracing::info!(
                name = "tool.carddav.add.success",
                client = %name,
                path = %path,
                status = %status,
                location = ?location,
                etag = ?etag,
                "CardDAV PUT succeeded"
            );
            Ok((path, location, etag))
        });

        match res {
            Ok((path, location, etag)) => {
                let mut summary = format!("--- Client: {} ---\nCreated at {}", name, path);
                if let Some(loc) = location {
                    summary.push_str(&format!("\nLocation: {}", loc));
                }
                if let Some(tag) = etag {
                    summary.push_str(&format!("\nETag: {}", tag));
                }
                all_results.push(summary);
            }
            Err(e) => all_results.push(format!("Error on client {}: {}", name, e)),
        }
    }

    if all_results.is_empty() {
        Err("No CardDAV clients configured.".to_string())
    } else {
        Ok(crate::agent::tools::dtos::AddContactResponse {
            result: all_results.join("\n\n"),
        })
    }
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `carddav_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "carddav_tests.rs"]
mod tests;
