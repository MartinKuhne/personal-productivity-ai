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

/// Structured postal address for a vCard `ADR` property.
///
/// vCard 3.0 §3.2.2 ADR is a 7-component semicolon-separated value
/// (post-office-box;extended-address;street-address;locality;region;
/// postal-code;country-name). The LLM-facing JSON form lifts each
/// component to its own field so the model can address them
/// individually. The `kind` field carries the `TYPE=` parameter
/// (`HOME`, `WORK`, …) so multiple addresses can coexist on a single
/// contact. Empty components are preserved as `None` for clean
/// round-tripping — the formatter writes them as empty `;`-separated
/// slots, the parser turns the empty slots back into `None`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct StructuredAddress {
    /// vCard `TYPE=` value, e.g. `"HOME"`, `"WORK"`. `None` means
    /// no TYPE= parameter (uncommon but valid).
    kind: Option<String>,
    po_box: Option<String>,
    ext: Option<String>,
    street: Option<String>,
    city: Option<String>,
    region: Option<String>,
    postal_code: Option<String>,
    country: Option<String>,
}

impl serde::Serialize for StructuredAddress {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        // 8 fields, but the LLM doesn't need empty strings for missing
        // components. Use `skip_none` semantics by hand: always emit
        // `type`/`street`/`city` if present, omit the rest when None.
        // Trade-off: the LLM can't distinguish "omitted" from "empty
        // string". For our use case that's fine — both mean "no value".
        let mut s = ser.serialize_struct("StructuredAddress", 8)?;
        if let Some(k) = &self.kind {
            s.serialize_field("type", k)?;
        }
        if let Some(v) = &self.street {
            s.serialize_field("street", v)?;
        }
        if let Some(v) = &self.city {
            s.serialize_field("city", v)?;
        }
        if let Some(v) = &self.region {
            s.serialize_field("region", v)?;
        }
        if let Some(v) = &self.postal_code {
            s.serialize_field("postal_code", v)?;
        }
        if let Some(v) = &self.country {
            s.serialize_field("country", v)?;
        }
        if let Some(v) = &self.po_box {
            s.serialize_field("po_box", v)?;
        }
        if let Some(v) = &self.ext {
            s.serialize_field("ext", v)?;
        }
        s.end()
    }
}

/// Extract the `UID` property value from a vCard body.
///
/// vCard 3.0 line folding is handled (lines starting with space/tab are
/// continuations of the previous property). The value is unescaped per
/// RFC 2426 §4 (`\\` → `\`, `\n`/`\N` → newline).
///
/// Returns `None` if the body has no UID property, in which case the
/// caller should generate a fresh one. vCard 3.0 doesn't strictly require
/// a UID but CardDAV servers reject resources without one, so missing
/// UID is a real-world error worth logging.
fn extract_vcard_uid(body: &str) -> Option<String> {
    let unfolded = unfold_vcard(body);
    for line in unfolded.lines() {
        let (prop, value) = line.split_once(':')?;
        let prop_name = prop.split(';').next()?.trim();
        if prop_name == "UID" {
            return Some(unescape_vcard_text(value.trim()));
        }
    }
    None
}

/// Reverse of [`escape_vcard_text`]. Used when round-tripping existing
/// vCard values back into a fresh property line.
fn unescape_vcard_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('\\') => out.push('\\'),
                Some(',') => out.push(','),
                Some(';') => out.push(';'),
                Some('n') | Some('N') => out.push('\n'),
                Some(other) => {
                    // Unknown escape — keep the literal backslash and the
                    // character so we don't silently mangle the value.
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Unfold a vCard body so each logical line is a single string.
///
/// vCard 3.0 §2.6 line folding: a CRLF followed by a single whitespace
/// (space or tab) is removed and the continuation is appended to the
/// previous line. This is the inverse of what producers do when a value
/// exceeds 75 octets.
fn unfold_vcard(body: &str) -> String {
    let mut unfolded = String::new();
    for line in body.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            // Continuation of previous logical line.
            unfolded.push_str(&line[1..]);
        } else {
            if !unfolded.is_empty() {
                unfolded.push('\n');
            }
            unfolded.push_str(line);
        }
    }
    unfolded
}

/// One non-structural property of a vCard body.
///
/// `name` is the bare property name (`EMAIL`, `TEL`, `BDAY`, `X-SKILL`,
/// …) used to decide whether a property is one of the canonical fields
/// the update tool knows how to touch. `prefix` is everything up to the
/// first colon (so the original TYPE= parameters are preserved when a
/// canonical property is replaced). `line` is the unfolded original
/// line, re-emitted verbatim for properties the update doesn't touch.
#[derive(Debug, Clone)]
struct VcardProp {
    name: String,
    prefix: String,
    line: String,
}

/// Properties that `merge_vcard_update` understands and may replace.
/// Any other property name (N, NICKNAME, URL, PHOTO, X-*, …) is
/// preserved verbatim.
///
/// `ADR` and `BDAY` are listed here for documentation but are handled
/// separately in [`merge_vcard_update`] because their merge semantics
/// differ: `BDAY` is single-valued (first match replaced, others
/// dropped — same as FN/EMAIL/…), while `ADR` is list-valued (all
/// existing ADRs are dropped when the JSON provides a new list).
const CANONICAL_PROP_NAMES: &[&str] =
    &["FN", "EMAIL", "TEL", "ORG", "TITLE", "NOTE", "BDAY", "ADR"];

/// Parse a vCard body into its non-structural properties.
///
/// `BEGIN`, `END`, `VERSION`, and `UID` are filtered out — `BEGIN`,
/// `END`, `VERSION` are re-emitted by the merger, and `UID` is supplied
/// separately by the caller.
fn parse_vcard_properties(body: &str) -> Vec<VcardProp> {
    let unfolded = unfold_vcard(body);
    let mut out = Vec::new();
    for line in unfolded.lines() {
        let Some((prefix, _value)) = line.split_once(':') else {
            continue;
        };
        let name = prefix.split(';').next().unwrap_or("").trim().to_uppercase();
        if name.is_empty() || matches!(name.as_str(), "BEGIN" | "END" | "VERSION" | "UID") {
            continue;
        }
        out.push(VcardProp {
            name,
            prefix: prefix.to_string(),
            line: line.to_string(),
        });
    }
    out
}

/// Parse the LLM's `addresses` / `address` field into a list of
/// `StructuredAddress`. Returns `None` when the JSON provides no
/// address at all (so the caller can leave existing ADRs intact).
///
/// Accepted forms:
///   * `"addresses": [ {…}, {…} ]` — canonical array form
///   * `"address": {…}` — single object, treated as a 1-element list
///   * `"address": "123 Main St, …"` — convenience string, parsed
///     heuristically into the 5 standard components
fn parse_addresses_from_json(parsed: &serde_json::Value) -> Option<Vec<StructuredAddress>> {
    // Canonical array form
    if let Some(arr) = parsed.get("addresses").and_then(|v| v.as_array()) {
        if arr.is_empty() {
            // Explicitly empty list — caller wants to clear addresses.
            return Some(Vec::new());
        }
        let out: Vec<StructuredAddress> = arr
            .iter()
            .filter_map(parse_single_address_from_json)
            .collect();
        if !out.is_empty() {
            return Some(out);
        }
    }
    // Single object form
    if let Some(obj) = parsed.get("address").and_then(|v| v.as_object()) {
        let value = serde_json::Value::Object(obj.clone());
        if let Some(addr) = parse_single_address_from_json(&value) {
            return Some(vec![addr]);
        }
    }
    // Single string form
    if let Some(s) = parsed.get("address").and_then(|v| v.as_str()) {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            return Some(vec![parse_address_from_string(trimmed)]);
        }
    }
    None
}

fn parse_single_address_from_json(item: &serde_json::Value) -> Option<StructuredAddress> {
    let obj = item.as_object()?;
    let s = |k: &str| -> Option<String> {
        obj.get(k)
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    Some(StructuredAddress {
        kind: s("type").map(|t| t.to_uppercase()),
        po_box: s("po_box"),
        ext: s("ext"),
        street: s("street"),
        city: s("city"),
        region: s("region"),
        postal_code: s("postal_code"),
        country: s("country"),
    })
}

/// Build a fresh vCard that preserves every property of `existing` and
/// only replaces the canonical fields (FN / EMAIL / TEL / ORG / TITLE /
/// NOTE / BDAY) for which `contact_json` provides a non-empty value.
///
/// Semantics:
///   * For each canonical single-valued property the JSON provides, the
///     **first** matching existing property is updated in place; any
///     subsequent matching properties are dropped (so a contact with two
///     EMAILs collapses to the one the LLM provided, which is the
///     predictable behaviour for a single-value field).
///   * The replacement keeps the original prefix (`EMAIL;TYPE=WORK:…`)
///     so we don't drop TYPE parameters the LLM doesn't know about.
///   * `ADR` is list-valued: when the JSON provides addresses, the new
///     list replaces every existing ADR; when it doesn't, every
///     existing ADR is preserved verbatim.
///   * All non-canonical properties (N, NICKNAME, URL, PHOTO, X-*, …)
///     are preserved verbatim. **This is the no-accidental-delete
///     guarantee.**
///   * If the JSON provides a canonical field that the existing vCard
///     didn't have, a new property is appended with the standard TYPE
///     default for that field.
///   * Empty strings in the JSON are treated as "not provided"
///     (consistent with [`first_str`]) so an LLM that forgets a field
///     doesn't wipe it out.
fn merge_vcard_update(existing: &[VcardProp], contact_json: &str, uid: Option<&str>) -> String {
    let parsed: serde_json::Value =
        serde_json::from_str(contact_json).unwrap_or_else(|_| serde_json::json!({}));

    // Single-valued updates the LLM actually provided. Empty/missing
    // values are filtered out by first_str, so we only see real
    // updates here. BDAY is single-valued like FN/EMAIL/…, so it goes
    // through the same path.
    let updates: Vec<(&str, &str)> = [
        (
            "FN",
            first_str(&parsed, &["name", "fn", "full_name", "displayName"]),
        ),
        (
            "EMAIL",
            first_str(&parsed, &["email", "email_address", "mail"]),
        ),
        (
            "TEL",
            first_str(
                &parsed,
                &["phone", "tel", "telephone", "mobile", "phone_number"],
            ),
        ),
        (
            "ORG",
            first_str(&parsed, &["company", "org", "organization", "organisation"]),
        ),
        ("TITLE", first_str(&parsed, &["title", "job_title", "role"])),
        ("NOTE", first_str(&parsed, &["notes", "note", "comment"])),
        (
            "BDAY",
            first_str(&parsed, &["birthday", "bday", "dob", "date_of_birth"]),
        ),
    ]
    .into_iter()
    .filter_map(|(k, v)| v.map(|vv| (k, vv)))
    .collect();

    // ADR is list-valued: when the JSON provides addresses, the whole
    // list replaces every existing ADR. When the JSON doesn't, every
    // existing ADR is preserved.
    let new_addresses = parse_addresses_from_json(&parsed);
    let replace_addresses = new_addresses.is_some();

    let mut out = String::new();
    out.push_str("BEGIN:VCARD\r\n");
    out.push_str("VERSION:3.0\r\n");

    // Track which canonical names have been replaced so duplicates in
    // the existing vCard collapse to one. ADR is excluded because
    // duplicates of ADR are legitimate (HOME + WORK) and we handle
    // them by list-replacement, not single-value replacement.
    let mut replaced: std::collections::HashSet<&str> = std::collections::HashSet::new();

    for prop in existing {
        if prop.name == "ADR" {
            if replace_addresses {
                // Drop — the new list (if any) is appended at the end.
            } else {
                // Preserve verbatim.
                out.push_str(&format!("{}\r\n", prop.line));
            }
            continue;
        }

        let is_canonical = CANONICAL_PROP_NAMES.contains(&prop.name.as_str());
        if is_canonical {
            if let Some((_, new_value)) = updates.iter().find(|(n, _)| *n == prop.name) {
                if !replaced.contains(prop.name.as_str()) {
                    // First occurrence of this canonical property —
                    // replace the value, preserve the original prefix
                    // (so TYPE= parameters survive).
                    out.push_str(&format!(
                        "{}:{}\r\n",
                        prop.prefix,
                        escape_vcard_text(new_value)
                    ));
                    replaced.insert(prop.name.as_str());
                }
                // Subsequent occurrences of the same canonical name are
                // dropped on purpose: a single-value field shouldn't
                // accumulate duplicates.
            } else {
                // Canonical but not being updated — preserve verbatim.
                out.push_str(&format!("{}\r\n", prop.line));
            }
        } else {
            // Non-canonical property: always preserve verbatim. This is
            // the no-accidental-delete guarantee for NICKNAME, URL, X-*,
            // and any other property the LLM doesn't know about.
            out.push_str(&format!("{}\r\n", prop.line));
        }
    }

    // Append canonical single-valued properties the JSON provided but
    // that didn't exist in the original vCard. Pick a sensible default
    // prefix.
    for (name, value) in &updates {
        if replaced.contains(name) {
            continue;
        }
        let prefix = match *name {
            "EMAIL" => "EMAIL;TYPE=INTERNET",
            "TEL" => "TEL;TYPE=CELL",
            _ => *name,
        };
        out.push_str(&format!("{}:{}\r\n", prefix, escape_vcard_text(value)));
    }

    // Append the new addresses (if any). Each address uses its own
    // kind as TYPE= (or no TYPE= when kind is None).
    if let Some(addrs) = new_addresses {
        for addr in addrs {
            let value = format_vcard_adr_value(&addr);
            let prefix = match &addr.kind {
                Some(t) if !t.is_empty() => format!("ADR;TYPE={}", t),
                _ => "ADR".to_string(),
            };
            out.push_str(&format!("{}:{}\r\n", prefix, value));
        }
    }

    if let Some(uid) = uid {
        out.push_str(&format!("UID:{}\r\n", uid));
    }

    out.push_str("END:VCARD\r\n");
    out
}

#[derive(serde::Serialize)]
struct CardDavContactDetails {
    client: String,
    href: String,
    fn_name: Option<String>,
    email: Option<String>,
    tel: Option<String>,
    org: Option<String>,
    /// vCard `BDAY` (typically `YYYY-MM-DD`).
    bday: Option<String>,
    /// All vCard `ADR` properties on the contact, in source order.
    /// Empty when the contact has no structured address.
    addresses: Vec<StructuredAddress>,
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
        bday: None,
        addresses: Vec::new(),
        vcard: data.to_string(),
    };

    let unfolded = unfold_vcard(data);
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
            "BDAY" => contact.bday = Some(value.trim().to_string()),
            "ADR" => {
                let kind = extract_type_from_prefix(prop);
                let mut addr = parse_vcard_adr_value(value);
                addr.kind = kind;
                contact.addresses.push(addr);
            }
            _ => {}
        }
    }

    contact
}

/// Pull the `TYPE=` value out of a vCard property prefix.
///
/// `prefix` is the part before the first colon, e.g.
/// `"ADR;TYPE=HOME;CHARSET=UTF-8"`. Returns the first `TYPE=…` token
/// (upper-cased per vCard 3.0 convention) or `None` if no TYPE=
/// parameter is present.
fn extract_type_from_prefix(prefix: &str) -> Option<String> {
    for token in prefix.split(';').skip(1) {
        if let Some(value) = token.strip_prefix("TYPE=") {
            return Some(value.to_uppercase());
        }
    }
    None
}

/// Split a vCard-escaped value on `sep`, respecting backslash-escapes.
///
/// vCard 3.0 §4 uses `\` to escape the separator inside a value, so
/// `"a\;b;c"` splits into `["a;b", "c"]`, not `["a\", "b", "c"]`. This
/// helper is used to split the 7-component ADR value.
fn split_vcard_value(text: &str, sep: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            current.push(c);
            if let Some(next) = chars.next() {
                current.push(next);
            }
        } else if c == sep {
            out.push(std::mem::take(&mut current));
        } else {
            current.push(c);
        }
    }
    out.push(current);
    out
}

/// Parse a vCard `ADR` value (the part after the colon) into a
/// `StructuredAddress`. Empty components become `None` so the LLM
/// can see "this contact has no street" without the JSON carrying an
/// empty string. The `kind` (TYPE=) is left as `None` here; the
/// caller sets it from the property prefix.
fn parse_vcard_adr_value(value: &str) -> StructuredAddress {
    let parts = split_vcard_value(value, ';');
    let at = |i: usize| -> Option<String> {
        parts
            .get(i)
            .map(|s| unescape_vcard_text(s))
            .filter(|s| !s.is_empty())
    };
    StructuredAddress {
        kind: None,
        po_box: at(0),
        ext: at(1),
        street: at(2),
        city: at(3),
        region: at(4),
        postal_code: at(5),
        country: at(6),
    }
}

/// Format a `StructuredAddress` as the vCard `ADR` value (the part
/// after the colon). Missing components become empty strings, so the
/// result always has exactly 7 `;`-separated components — the
/// canonical vCard ADR shape. Each component is escaped per RFC 2426
/// so `;` and `,` inside a field don't break the split.
fn format_vcard_adr_value(addr: &StructuredAddress) -> String {
    let components = [
        addr.po_box.as_deref().unwrap_or(""),
        addr.ext.as_deref().unwrap_or(""),
        addr.street.as_deref().unwrap_or(""),
        addr.city.as_deref().unwrap_or(""),
        addr.region.as_deref().unwrap_or(""),
        addr.postal_code.as_deref().unwrap_or(""),
        addr.country.as_deref().unwrap_or(""),
    ];
    let escaped: Vec<String> = components.iter().map(|c| escape_vcard_text(c)).collect();
    escaped.join(";")
}

/// Heuristic: turn a single-line address string into a
/// `StructuredAddress`. Splits on `,` and fills the first 5
/// `StructuredAddress` components (street, city, region, postal_code,
/// country). The other 2 (po_box, ext) stay `None`. This is for the
/// convenience of an LLM that has a one-line address and doesn't want
/// to enumerate the fields.
fn parse_address_from_string(s: &str) -> StructuredAddress {
    let mut parts: Vec<String> = s
        .split(',')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();
    while parts.len() < 5 {
        parts.push(String::new());
    }
    StructuredAddress {
        kind: None,
        po_box: None,
        ext: None,
        street: parts.first().cloned().filter(|s| !s.is_empty()),
        city: parts.get(1).cloned().filter(|s| !s.is_empty()),
        region: parts.get(2).cloned().filter(|s| !s.is_empty()),
        postal_code: parts.get(3).cloned().filter(|s| !s.is_empty()),
        country: parts.get(4).cloned().filter(|s| !s.is_empty()),
    }
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
    let bday = first_str(&parsed, &["birthday", "bday", "dob", "date_of_birth"]);
    let addresses = parse_addresses_from_json(&parsed);

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
    if let Some(b) = bday {
        vcard.push_str(&format!("BDAY:{}\r\n", b));
    }
    if let Some(addrs) = addresses {
        for addr in addrs {
            let value = format_vcard_adr_value(&addr);
            let prefix = match &addr.kind {
                Some(t) if !t.is_empty() => format!("ADR;TYPE={}", t),
                _ => "ADR".to_string(),
            };
            vcard.push_str(&format!("{}:{}\r\n", prefix, value));
        }
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

/// Update an existing contact at `href` with new data from `contact_json`.
///
/// Flow:
/// 1. `GET` the current vCard at `href` to capture the existing `UID`
///    (so the vCard identity is preserved across the update) and the
///    current `ETag` (so we can do an `If-Match` conditional write).
/// 2. Build a fresh vCard from `contact_json` (same schema as
///    [`tool_add_contact`]) using the existing `UID`.
/// 3. `PUT` the new vCard back to the same `href` with `If-Match: <etag>`.
///    If the server has a newer ETag (someone else touched the contact),
///    the PUT fails with 412 and the caller must `GET` again and retry.
///
/// If the `GET` returns no ETag header (some CalDAV servers omit it),
/// we fall back to an unconditional `PUT`. The vCard is still
/// regenerated, so the worst case is a last-writer-wins race.
pub fn tool_update_contact(
    config: &AppConfig,
    href: &str,
    contact_json: &str,
) -> Result<crate::agent::tools::dtos::UpdateContactResponse, String> {
    let mut all_results = Vec::new();
    for (name, client_config) in &config.caldav_clients {
        let res = block_on(async {
            let client = CardDavClient::new(
                &client_config.url,
                Some(&client_config.username),
                Some(&client_config.password),
            )
            .map_err(|e| anyhow::anyhow!("Client config error: {}", e))?;

            tracing::info!(
                name = "tool.carddav.update.fetch",
                client = %name,
                href = %href,
                "Fetching existing contact for update"
            );
            let get_resp = client.get(href).await?;
            let get_status = get_resp.status();
            let get_etag = get_resp
                .headers()
                .get("etag")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            let get_body = log_truncate(&get_resp.into_body());
            if !get_status.is_success() {
                tracing::warn!(
                    name = "tool.carddav.update.fetch_failed",
                    client = %name,
                    href = %href,
                    status = %get_status,
                    body = %get_body,
                    "Failed to fetch existing contact for update"
                );
                return Err(anyhow::anyhow!(
                    "Failed to fetch contact for update: {} - {}",
                    get_status,
                    get_body
                ));
            }

            let existing_uid = extract_vcard_uid(&get_body);
            if existing_uid.is_none() {
                tracing::warn!(
                    name = "tool.carddav.update.no_uid",
                    client = %name,
                    href = %href,
                    "Existing vCard has no UID; the regenerated vCard will get a fresh one and the href may change"
                );
            }

            // Property-preserving merge: every property the LLM doesn't
            // touch (N, NICKNAME, ADR, BDAY, URL, PHOTO, X-*, …) is kept
            // verbatim. Only the canonical fields (FN, EMAIL, TEL, ORG,
            // TITLE, NOTE) get replaced when the LLM provides a value.
            let existing_props = parse_vcard_properties(&get_body);
            let new_vcard =
                merge_vcard_update(&existing_props, contact_json, existing_uid.as_deref());
            let vcard_bytes: bytes::Bytes = new_vcard.into_bytes().into();

            tracing::info!(
                name = "tool.carddav.update.request",
                client = %name,
                href = %href,
                if_match = ?get_etag,
                vcard_bytes = vcard_bytes.len(),
                preserved_uid = ?existing_uid,
                "PUT updated contact (If-Match when ETag is present)"
            );

            let put_resp = if let Some(ref tag) = get_etag {
                client.put_if_match(href, vcard_bytes, tag).await?
            } else {
                // No ETag means no race detection; fall back to a plain PUT.
                client.put(href, vcard_bytes).await?
            };
            let put_status = put_resp.status();
            let put_location = put_resp
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            let put_etag = put_resp
                .headers()
                .get("etag")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            let put_body = log_truncate(&put_resp.into_body());

            if !put_status.is_success() {
                tracing::error!(
                    name = "tool.carddav.update.failed",
                    client = %name,
                    href = %href,
                    status = %put_status,
                    location = ?put_location,
                    etag = ?put_etag,
                    body = %put_body,
                    "PUT of updated contact returned non-success status"
                );
                return Err(anyhow::anyhow!(
                    "Failed to PUT updated contact: {} - {}",
                    put_status,
                    put_body
                ));
            }

            tracing::info!(
                name = "tool.carddav.update.success",
                client = %name,
                href = %href,
                status = %put_status,
                location = ?put_location,
                etag = ?put_etag,
                "Updated contact"
            );
            Ok((put_status, put_location, put_etag))
        });

        match res {
            Ok((status, location, etag)) => {
                let mut summary = format!(
                    "--- Client: {} ---\nUpdated {} (status {})",
                    name, href, status
                );
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
        Ok(crate::agent::tools::dtos::UpdateContactResponse {
            result: all_results.join("\n\n"),
        })
    }
}

/// Delete the contact at `href`.
///
/// Returns Ok with "Deleted" on 2xx (typically 204 No Content) and 404
/// (already gone — treat as success so the LLM can retry idempotently).
/// Other non-success statuses are propagated as errors with the
/// truncated response body.
pub fn tool_delete_contact(
    config: &AppConfig,
    href: &str,
) -> Result<crate::agent::tools::dtos::DeleteContactResponse, String> {
    let mut all_results = Vec::new();
    for (name, client_config) in &config.caldav_clients {
        let res = block_on(async {
            let client = CardDavClient::new(
                &client_config.url,
                Some(&client_config.username),
                Some(&client_config.password),
            )
            .map_err(|e| anyhow::anyhow!("Client config error: {}", e))?;

            tracing::info!(
                name = "tool.carddav.delete.request",
                client = %name,
                href = %href,
                "Deleting CardDAV contact"
            );
            let resp = client.delete(href).await?;
            let status = resp.status();
            let body = log_truncate(&resp.into_body());

            // 404 is treated as a successful no-op so the LLM can call
            // delete_contact idempotently (e.g. "delete Paul Wayss, then
            // re-create with the correct data" should not error if the
            // first delete already succeeded).
            if status.as_u16() == 404 {
                tracing::info!(
                    name = "tool.carddav.delete.already_gone",
                    client = %name,
                    href = %href,
                    "Contact was already absent (404); treating as success"
                );
                return Ok::<_, anyhow::Error>("Already absent (404)".to_string());
            }

            if !status.is_success() {
                tracing::error!(
                    name = "tool.carddav.delete.failed",
                    client = %name,
                    href = %href,
                    status = %status,
                    body = %body,
                    "DELETE of contact returned non-success status"
                );
                return Err(anyhow::anyhow!(
                    "Failed to DELETE contact: {} - {}",
                    status,
                    body
                ));
            }

            tracing::info!(
                name = "tool.carddav.delete.success",
                client = %name,
                href = %href,
                status = %status,
                "Deleted contact"
            );
            Ok(format!("Deleted (status {})", status))
        });

        match res {
            Ok(s) => all_results.push(format!("--- Client: {} ---\n{}", name, s)),
            Err(e) => all_results.push(format!("Error on client {}: {}", name, e)),
        }
    }

    if all_results.is_empty() {
        Err("No CardDAV clients configured.".to_string())
    } else {
        Ok(crate::agent::tools::dtos::DeleteContactResponse {
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
