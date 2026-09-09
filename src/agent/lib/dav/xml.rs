//! XML query construction and multistatus response parsing for CalDAV and CardDAV.
//! Unit tests live in the sibling `xml_tests.rs` sidecar.

/// An event or calendar object returned from a CalDAV query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarItem {
    /// The relative or absolute URL (`href`) of the calendar item.
    pub href: String,
    /// The raw iCalendar payload, if requested and returned by the server.
    pub calendar_data: Option<String>,
}

/// A contact or vCard object returned from a CardDAV query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContactItem {
    /// The relative or absolute URL (`href`) of the contact item.
    pub href: String,
    /// The raw vCard payload, if requested and returned by the server.
    pub address_data: Option<String>,
}

/// Returns a static XML body for a `PROPFIND` querying calendar collections.
pub fn build_propfind_calendars() -> &'static str {
    r#"<?xml version="1.0" encoding="utf-8"?>
<D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop>
    <D:resourcetype/>
    <D:displayname/>
  </D:prop>
</D:propfind>"#
}

/// Returns a static XML body for a `PROPFIND` querying addressbook collections.
pub fn build_propfind_addressbooks() -> &'static str {
    r#"<?xml version="1.0" encoding="utf-8"?>
<D:propfind xmlns:D="DAV:" xmlns:CARD="urn:ietf:params:xml:ns:carddav">
  <D:prop>
    <D:resourcetype/>
    <D:displayname/>
  </D:prop>
</D:propfind>"#
}

/// Returns a static XML body for a `PROPFIND` querying `current-user-principal`.
pub fn build_propfind_current_user_principal() -> &'static str {
    r#"<?xml version="1.0" encoding="utf-8"?>
<D:propfind xmlns:D="DAV:">
  <D:prop>
    <D:current-user-principal/>
  </D:prop>
</D:propfind>"#
}

/// Returns a static XML body for a `PROPFIND` querying `calendar-home-set`.
pub fn build_propfind_calendar_home_set() -> &'static str {
    r#"<?xml version="1.0" encoding="utf-8"?>
<D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop>
    <C:calendar-home-set/>
  </D:prop>
</D:propfind>"#
}

/// Returns a static XML body for a `PROPFIND` querying `addressbook-home-set`.
pub fn build_propfind_addressbook_home_set() -> &'static str {
    r#"<?xml version="1.0" encoding="utf-8"?>
<D:propfind xmlns:D="DAV:" xmlns:CARD="urn:ietf:params:xml:ns:carddav">
  <D:prop>
    <CARD:addressbook-home-set/>
  </D:prop>
</D:propfind>"#
}

/// Builds a CalDAV `REPORT` XML query body for calendar events.
pub fn build_calendar_query(component: &str, start: Option<&str>, end: Option<&str>) -> String {
    let comp_filter = match (start, end) {
        (Some(s), Some(e)) => {
            format!(
                "      <C:comp-filter name=\"{component}\">\n        <C:time-range start=\"{s}\" end=\"{e}\"/>\n      </C:comp-filter>"
            )
        }
        (Some(s), None) => {
            format!(
                "      <C:comp-filter name=\"{component}\">\n        <C:time-range start=\"{s}\"/>\n      </C:comp-filter>"
            )
        }
        (None, Some(e)) => {
            format!(
                "      <C:comp-filter name=\"{component}\">\n        <C:time-range end=\"{e}\"/>\n      </C:comp-filter>"
            )
        }
        (None, None) => format!("      <C:comp-filter name=\"{component}\"/>"),
    };

    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<C:calendar-query xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop>
    <C:calendar-data/>
  </D:prop>
  <C:filter>
    <C:comp-filter name="VCALENDAR">
{comp_filter}
    </C:comp-filter>
  </C:filter>
</C:calendar-query>"#
    )
}

/// Returns a static XML body for a CardDAV `REPORT` querying addressbook data.
pub fn build_addressbook_query() -> &'static str {
    r#"<?xml version="1.0" encoding="utf-8"?>
<CARD:addressbook-query xmlns:D="DAV:" xmlns:CARD="urn:ietf:params:xml:ns:carddav">
  <D:prop>
    <CARD:address-data/>
  </D:prop>
</CARD:addressbook-query>"#
}

/// Builds a WebDAV/CardDAV sync-collection `REPORT` XML body.
pub fn build_sync_collection(sync_token: Option<&str>, limit: Option<u32>) -> String {
    let token = sync_token.unwrap_or_default();
    let limit_xml = match limit {
        Some(lim) => format!("<D:limit><D:nresults>{lim}</D:nresults></D:limit>"),
        None => String::new(),
    };

    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<D:sync-collection xmlns:D="DAV:" xmlns:CARD="urn:ietf:params:xml:ns:carddav">
  <D:sync-token>{token}</D:sync-token>
  <D:sync-level>1</D:sync-level>
  {limit_xml}
  <D:prop>
    <CARD:address-data/>
  </D:prop>
</D:sync-collection>"#
    )
}

/// Parses every calendar collection `href` from a WebDAV multistatus XML string.
pub fn parse_calendar_hrefs(xml: &str) -> Vec<String> {
    parse_collections_by_type(xml, "calendar")
}

/// Parses every addressbook collection `href` from a WebDAV multistatus XML string.
pub fn parse_addressbook_hrefs(xml: &str) -> Vec<String> {
    parse_collections_by_type(xml, "addressbook")
}

fn parse_collections_by_type(xml: &str, target_type: &str) -> Vec<String> {
    let Ok(doc) = roxmltree::Document::parse(xml) else {
        return Vec::new();
    };
    let mut hrefs = Vec::new();
    for resp in doc.descendants().filter(|n| n.has_tag_name("response")) {
        let is_target = resp.descendants().any(|n| {
            n.has_tag_name("resourcetype") && n.children().any(|c| c.has_tag_name(target_type))
        });
        if !is_target {
            continue;
        }
        if let Some(href) = resp
            .descendants()
            .find(|n| n.has_tag_name("href"))
            .and_then(|n| n.text())
        {
            hrefs.push(href.trim().to_string());
        }
    }
    hrefs
}

/// Parses home set collection `href`s matching the specified property tag name.
pub fn parse_home_set(xml: &str, home_set_tag: &str) -> Vec<String> {
    let Ok(doc) = roxmltree::Document::parse(xml) else {
        return Vec::new();
    };
    let mut hrefs = Vec::new();
    for prop in doc.descendants().filter(|n| n.has_tag_name(home_set_tag)) {
        for href in prop.descendants().filter(|n| n.has_tag_name("href")) {
            if let Some(text) = href.text() {
                hrefs.push(text.trim().to_string());
            }
        }
    }
    hrefs.sort();
    hrefs.dedup();
    hrefs
}

/// Parses the `current-user-principal` URL from a WebDAV multistatus response.
pub fn parse_current_user_principal(xml: &str) -> Option<String> {
    let doc = roxmltree::Document::parse(xml).ok()?;
    let prop = doc
        .descendants()
        .find(|n| n.has_tag_name("current-user-principal"))?;
    let href = prop
        .descendants()
        .find(|n| n.has_tag_name("href"))?
        .text()?;
    Some(href.trim().to_string())
}

/// Parses calendar items (`href` and `calendar-data`) from a CalDAV `REPORT` response.
pub fn parse_calendar_query_response(xml: &str) -> Vec<CalendarItem> {
    let Ok(doc) = roxmltree::Document::parse(xml) else {
        return Vec::new();
    };
    let mut items = Vec::new();
    for resp in doc.descendants().filter(|n| n.has_tag_name("response")) {
        let href = resp
            .descendants()
            .find(|n| n.has_tag_name("href"))
            .and_then(|n| n.text())
            .unwrap_or_default()
            .trim()
            .to_string();
        let cal_data = resp
            .descendants()
            .find(|n| n.has_tag_name("calendar-data"))
            .and_then(|n| n.text())
            .map(|s| s.to_string());
        if !href.is_empty() {
            items.push(CalendarItem {
                href,
                calendar_data: cal_data,
            });
        }
    }
    items
}

/// Parses contact items (`href` and `address-data`) from a CardDAV `REPORT` response.
pub fn parse_addressbook_query_response(xml: &str) -> Vec<ContactItem> {
    let Ok(doc) = roxmltree::Document::parse(xml) else {
        return Vec::new();
    };
    let mut items = Vec::new();
    for resp in doc.descendants().filter(|n| n.has_tag_name("response")) {
        let href = resp
            .descendants()
            .find(|n| n.has_tag_name("href"))
            .and_then(|n| n.text())
            .unwrap_or_default()
            .trim()
            .to_string();
        let addr_data = resp
            .descendants()
            .find(|n| n.has_tag_name("address-data"))
            .and_then(|n| n.text())
            .map(|s| s.to_string());
        if !href.is_empty() {
            items.push(ContactItem {
                href,
                address_data: addr_data,
            });
        }
    }
    items
}

#[cfg(test)]
#[path = "xml_tests.rs"]
mod tests;
