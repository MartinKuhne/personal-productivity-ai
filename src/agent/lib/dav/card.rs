//! CardDAV agent tools — search, retrieve, create, update, and delete contacts across configured CardDAV servers.
//!
//! Every network round-trip is logged via `tracing` so that failures on the
//! server (e.g. FastMail returning `403 Forbidden - Mailbox does not exist`
//! for a malformed PUT path) are visible in the application log with the
//! request URL, the response status, the relevant response headers
//! (`Location`, `ETag`), and the response body.
//!
//! Unit tests live in the sibling `carddav_tests.rs` sidecar.

use super::DavClient;
use crate::config::AgentConfig;

/// Cap on the number of body bytes echoed into a single tracing event.
/// CardDAV error bodies are typically small (XML error envelopes), but
/// pathological responses can be large; 4 KiB is plenty for diagnosis
/// without flooding the log.
pub(super) const LOG_BODY_LIMIT: usize = 4096;

/// Truncate `body` to at most [`LOG_BODY_LIMIT`] bytes for safe logging.
pub(super) fn log_truncate(body: &[u8]) -> String {
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
pub(super) fn build_contact_put_path(addressbook_href: &str, uid: &str) -> String {
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
pub struct StructuredAddress {
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
pub(super) fn extract_vcard_uid(body: &str) -> Option<String> {
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
pub(super) fn unfold_vcard(body: &str) -> String {
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
pub(super) struct VcardProp {
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
pub(super) fn parse_vcard_properties(body: &str) -> Vec<VcardProp> {
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
pub(super) fn merge_vcard_update(
    existing: &[VcardProp],
    contact_json: &str,
    uid: Option<&str>,
) -> String {
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
#[derive(serde::Serialize, Debug, Clone)]
pub struct CardDavContactDetails {
    pub client: String,
    pub href: String,
    pub fn_name: Option<String>,
    pub email: Option<String>,
    pub tel: Option<String>,
    pub org: Option<String>,
    /// vCard `BDAY` (typically `YYYY-MM-DD`).
    pub bday: Option<String>,
    /// All vCard `ADR` properties on the contact, in source order.
    /// Empty when the contact has no structured address.
    pub addresses: Vec<StructuredAddress>,
    pub vcard: String,
}

#[derive(serde::Serialize)]
pub struct CardDavResponse {
    results: Vec<CardDavContactDetails>,
    errors: Vec<String>,
}

pub(super) async fn get_all_addressbooks(
    client: &DavClient,
    base_url: &str,
    username: &str,
) -> anyhow::Result<Vec<String>> {
    if let Ok(books) = client.list_addressbooks(base_url).await
        && !books.is_empty()
    {
        tracing::info!(
            name = "tool.carddav.addressbook.discovered",
            base_url = %base_url,
            strategy = "list_addressbooks",
            count = books.len(),
            hrefs = ?books,
            "Discovered addressbooks via direct PROPFIND on base URL"
        );
        return Ok(books);
    }

    if let Ok(homes) = client.discover_addressbook_home_set(base_url).await
        && let Some(home) = homes.first()
        && let Ok(books) = client.list_addressbooks(home).await
        && !books.is_empty()
    {
        tracing::info!(
            name = "tool.carddav.addressbook.discovered",
            base_url = %base_url,
            strategy = "home_set",
            home = %home,
            count = books.len(),
            hrefs = ?books,
            "Discovered addressbooks via addressbook-home-set on base URL"
        );
        return Ok(books);
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
    tracing::info!(
        name = "tool.carddav.addressbook.discovered",
        base_url = %base_url,
        strategy = "principal",
        principal = %principal,
        home = %home,
        count = books.len(),
        hrefs = ?books,
        "Discovered addressbooks via principal URL"
    );
    Ok(books)
}

pub(super) async fn fetch_contacts_from_book(
    client: &DavClient,
    book_path: &str,
) -> anyhow::Result<Vec<(String, String)>> {
    client.fetch_contacts_from_book(book_path).await
}

pub(crate) fn parse_vcard(client: &str, href: &str, data: &str) -> CardDavContactDetails {
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

pub(super) fn json_to_vcard(json_str: &str, uid_override: Option<&str>) -> String {
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

// ---------------------------------------------------------------------------
// LLM-adapter layer — the `tool_*` functions. Each one iterates the
// configured DAV clients (CalDAV and CardDAV share the same config
// map), builds a [`crate::lib::dav::client::DavClient`] per
// server, and aggregates the per-server results into the LLM-facing
// DTO from `crate::tools::dtos`.
// ---------------------------------------------------------------------------

/// Iterate every configured DAV client, invoke `f` against each
/// one, and split the per-server outcomes into a `results` vec and
/// an `errors` vec. Errors are recorded as
/// `"Error on client {name}: {e}"` — the same string the previous
/// inline-loop code produced — so any existing log lines and
/// downstream tooling keep working.
fn for_each_card_client<T, F>(config: &AgentConfig, mut f: F) -> (Vec<T>, Vec<String>)
where
    F: FnMut(&str, &crate::lib::dav::client::DavClient) -> Result<T, String>,
{
    let mut results = Vec::new();
    let mut errors = Vec::new();
    for (name, cc) in config.caldav_clients() {
        match crate::lib::dav::client::DavClient::new(name.clone(), cc).and_then(|c| f(name, &c)) {
            Ok(item) => results.push(item),
            Err(e) => errors.push(format!("Error on client {}: {}", name, e)),
        }
    }
    (results, errors)
}

/// Like [`for_each_card_client`] but for methods that return a
/// `Vec` per server (search). The per-server `Vec`s are flattened
/// into the aggregate `results` vec.
fn for_each_card_client_vec<T, F>(config: &AgentConfig, mut f: F) -> (Vec<T>, Vec<String>)
where
    F: FnMut(&str, &crate::lib::dav::client::DavClient) -> Result<Vec<T>, String>,
{
    let mut results = Vec::new();
    let mut errors = Vec::new();
    for (name, cc) in config.caldav_clients() {
        match crate::lib::dav::client::DavClient::new(name.clone(), cc).and_then(|c| f(name, &c)) {
            Ok(mut v) => results.append(&mut v),
            Err(e) => errors.push(format!("Error on client {}: {}", name, e)),
        }
    }
    (results, errors)
}

fn serialize_card_response(resp: &CardDavResponse) -> String {
    serde_json::to_string_pretty(resp).unwrap_or_else(|_| "{}".to_string())
}

fn format_contact_page(items: &[String], errors: &[String]) -> String {
    let mut parts = Vec::new();
    if !items.is_empty() {
        parts.push(items.join("\n\n"));
    }
    for err in errors {
        parts.push(err.clone());
    }
    if parts.is_empty() {
        "{}".to_string()
    } else {
        parts.join("\n\n")
    }
}

pub fn tool_search_contact(
    config: &AgentConfig,
    keyword: &str,
    cursor: Option<String>,
    cache: &crate::tools::registry::cache::ToolCache,
    uuid_gen: &dyn crate::utils::uuid::UuidGenerator,
) -> Result<crate::tools::dtos::SearchContactResponse, String> {
    if let Some(cursor) = cursor {
        let page = cache.contact_search_sessions.next_page(&cursor)?;
        return Ok(crate::tools::dtos::SearchContactResponse {
            results: format_contact_page(&page.items, &[]),
            total: page.total,
            cursor: page.cursor,
            hint: page.hint,
        });
    }

    let (results, errors) = for_each_card_client_vec(config, |_, c| c.search_contact(keyword));
    let items: Vec<String> = results
        .into_iter()
        .map(|r| serde_json::to_string_pretty(&r).unwrap_or_default())
        .collect();

    if items.is_empty() {
        return Ok(crate::tools::dtos::SearchContactResponse {
            results: if errors.is_empty() {
                "No contacts found.".to_string()
            } else {
                errors.join("\n\n")
            },
            total: 0,
            cursor: None,
            hint: Some(crate::tools::registry::builtin::strings::FINAL_PAGE_HINT.to_string()),
        });
    }

    let page = cache
        .contact_search_sessions
        .create_session(items, uuid_gen);
    Ok(crate::tools::dtos::SearchContactResponse {
        results: format_contact_page(&page.items, &errors),
        total: page.total,
        cursor: page.cursor,
        hint: page.hint,
    })
}

pub fn tool_get_contact(
    config: &AgentConfig,
    id: &str,
) -> Result<crate::tools::dtos::GetContactResponse, String> {
    let (results, errors) = for_each_card_client(config, |_, c| c.get_contact(id));
    Ok(crate::tools::dtos::GetContactResponse {
        result: serialize_card_response(&CardDavResponse { results, errors }),
    })
}

pub fn tool_add_contact(
    config: &AgentConfig,
    contact_json: &str,
) -> Result<crate::tools::dtos::AddContactResponse, String> {
    // `add_contact` is special: it acts on the *first* configured CalDAV
    // client (no "default addressbook" concept in CardDAV). The
    // per-server output is a single status string, so the aggregation
    // shape doesn't fit `for_each_card_client` cleanly.
    let mut all_results = Vec::new();
    if let Some((name, cc)) = config.caldav_clients().iter().next() {
        match crate::lib::dav::client::DavClient::new(name.clone(), cc)
            .and_then(|c| c.add_contact(contact_json))
        {
            Ok(path) => all_results.push(format!("--- Client: {} ---\nCreated at {}", name, path)),
            Err(e) => all_results.push(format!("Error on client {}: {}", name, e)),
        }
    }

    if all_results.is_empty() {
        Err("No CardDAV clients configured.".to_string())
    } else {
        Ok(crate::tools::dtos::AddContactResponse {
            result: all_results.join("\n\n"),
        })
    }
}

/// Update an existing contact at `href` with new data from `contact_json`.
///
/// Thin wrapper that delegates to
/// [`crate::lib::dav::client::DavClient::update_contact`]
/// per configured DAV server, then aggregates the per-server
/// results. See that method for the GET → If-Match PUT flow.
pub fn tool_update_contact(
    config: &AgentConfig,
    href: &str,
    contact_json: &str,
) -> Result<crate::tools::dtos::UpdateContactResponse, String> {
    let mut all_results = Vec::new();
    for (name, cc) in config.caldav_clients() {
        match crate::lib::dav::client::DavClient::new(name.clone(), cc)
            .and_then(|c| c.update_contact(href, contact_json))
        {
            Ok(summary) => all_results.push(format!("--- Client: {} ---\n{}", name, summary)),
            Err(e) => all_results.push(format!("Error on client {}: {}", name, e)),
        }
    }
    if all_results.is_empty() {
        Err("No CardDAV clients configured.".to_string())
    } else {
        Ok(crate::tools::dtos::UpdateContactResponse {
            result: all_results.join("\n\n"),
        })
    }
}

/// Delete the contact at `href`.
///
/// Returns Ok with "Deleted" on 2xx (typically 204 No Content) and 404
/// (already gone — treat as success so the LLM can retry idempotently).
/// Thin wrapper that delegates to
/// [`crate::lib::dav::client::DavClient::delete_contact`]
/// per configured DAV server.
pub fn tool_delete_contact(
    config: &AgentConfig,
    href: &str,
) -> Result<crate::tools::dtos::DeleteContactResponse, String> {
    let mut all_results = Vec::new();
    for (name, cc) in config.caldav_clients() {
        match crate::lib::dav::client::DavClient::new(name.clone(), cc)
            .and_then(|c| c.delete_contact(href))
        {
            Ok(s) => all_results.push(format!("--- Client: {} ---\n{}", name, s)),
            Err(e) => all_results.push(format!("Error on client {}: {}", name, e)),
        }
    }

    if all_results.is_empty() {
        Err("No CardDAV clients configured.".to_string())
    } else {
        Ok(crate::tools::dtos::DeleteContactResponse {
            result: all_results.join("\n\n"),
        })
    }
}
// ---------------------------------------------------------------------------
// Tests live in the sibling `carddav_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "card_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "card_proptests.rs"]
mod card_proptests;
