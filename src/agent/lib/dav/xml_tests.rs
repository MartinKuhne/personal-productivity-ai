//! Unit tests for `xml.rs`.

use super::*;

const CALENDAR_PROPFIND_FIXTURE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
 <d:response>
  <d:href>/calendars/primary/</d:href>
  <d:propstat>
   <d:prop>
    <d:resourcetype><d:collection/><c:calendar/></d:resourcetype>
    <d:displayname>Primary</d:displayname>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>
 <d:response>
  <d:href>/calendars/work/</d:href>
  <d:propstat>
   <d:prop>
    <d:resourcetype><d:collection/><c:calendar/></d:resourcetype>
    <d:displayname>Work</d:displayname>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>
 <d:response>
  <d:href>/calendars/not-a-cal/</d:href>
  <d:propstat>
   <d:prop>
    <d:resourcetype><d:collection/></d:resourcetype>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>
</d:multistatus>"#;

const ADDRESSBOOK_PROPFIND_FIXTURE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:card="urn:ietf:params:xml:ns:carddav">
 <d:response>
  <d:href>/addressbooks/default/</d:href>
  <d:propstat>
   <d:prop>
    <d:resourcetype><d:collection/><card:addressbook/></d:resourcetype>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>
</d:multistatus>"#;

const CALENDAR_REPORT_FIXTURE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
 <d:response>
  <d:href>/calendars/primary/event1.ics</d:href>
  <d:propstat>
   <d:prop>
    <c:calendar-data>BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
SUMMARY:Meeting with Bob
END:VEVENT
END:VCALENDAR</c:calendar-data>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>
</d:multistatus>"#;

const ADDRESSBOOK_REPORT_FIXTURE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:card="urn:ietf:params:xml:ns:carddav">
 <d:response>
  <d:href>/addressbooks/default/alice.vcf</d:href>
  <d:propstat>
   <d:prop>
    <card:address-data>BEGIN:VCARD
VERSION:3.0
FN:Alice
END:VCARD</card:address-data>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>
</d:multistatus>"#;

const PRINCIPAL_FIXTURE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<D:multistatus xmlns:D="DAV:">
  <D:response>
    <D:href>/dav/</D:href>
    <D:propstat>
      <D:prop>
        <D:current-user-principal>
          <D:href>/dav/principals/user/alice/</D:href>
        </D:current-user-principal>
      </D:prop>
      <D:status>HTTP/1.1 200 OK</D:status>
    </D:propstat>
  </D:response>
</D:multistatus>"#;

const HOME_SET_FIXTURE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:response>
    <D:href>/dav/principals/user/alice/</D:href>
    <D:propstat>
      <D:prop>
        <C:calendar-home-set>
          <D:href>/dav/calendars/user/alice/</D:href>
        </C:calendar-home-set>
      </D:prop>
      <D:status>HTTP/1.1 200 OK</D:status>
    </D:propstat>
  </D:response>
</D:multistatus>"#;

#[test]
fn test_parse_calendar_hrefs() {
    let hrefs = parse_calendar_hrefs(CALENDAR_PROPFIND_FIXTURE);
    assert_eq!(hrefs, vec!["/calendars/primary/", "/calendars/work/"]);
}

#[test]
fn test_parse_addressbook_hrefs() {
    let hrefs = parse_addressbook_hrefs(ADDRESSBOOK_PROPFIND_FIXTURE);
    assert_eq!(hrefs, vec!["/addressbooks/default/"]);
}

#[test]
fn test_parse_calendar_report() {
    let items = parse_calendar_query_response(CALENDAR_REPORT_FIXTURE);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].href, "/calendars/primary/event1.ics");
    assert!(
        items[0]
            .calendar_data
            .as_deref()
            .unwrap()
            .contains("Meeting with Bob")
    );
}

#[test]
fn test_parse_addressbook_report() {
    let items = parse_addressbook_query_response(ADDRESSBOOK_REPORT_FIXTURE);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].href, "/addressbooks/default/alice.vcf");
    assert!(
        items[0]
            .address_data
            .as_deref()
            .unwrap()
            .contains("FN:Alice")
    );
}

#[test]
fn test_parse_principal() {
    let p = parse_current_user_principal(PRINCIPAL_FIXTURE);
    assert_eq!(p, Some("/dav/principals/user/alice/".to_string()));
}

#[test]
fn test_parse_home_set() {
    let homes = parse_home_set(HOME_SET_FIXTURE, "calendar-home-set");
    assert_eq!(homes, vec!["/dav/calendars/user/alice/"]);
}

#[test]
fn test_corrupted_xml_returns_empty() {
    assert!(parse_calendar_hrefs("not xml").is_empty());
    assert!(parse_addressbook_hrefs("<unclosed").is_empty());
    assert!(parse_calendar_query_response("").is_empty());
    assert!(parse_addressbook_query_response("hello world").is_empty());
    assert_eq!(parse_current_user_principal(""), None);
}

#[test]
fn test_build_calendar_query_with_timerange() {
    let q = build_calendar_query("VEVENT", Some("20240101T000000Z"), Some("20240102T000000Z"));
    assert!(q.contains(r#"<C:time-range start="20240101T000000Z" end="20240102T000000Z"/>"#));
    assert!(q.contains(r#"<C:comp-filter name="VEVENT">"#));
}

#[test]
fn test_build_calendar_query_without_timerange() {
    let q = build_calendar_query("VEVENT", None, None);
    assert!(!q.contains("time-range"));
    assert!(q.contains(r#"<C:comp-filter name="VEVENT"/>"#));
}

#[test]
fn test_build_sync_collection() {
    let sync = build_sync_collection(Some("token123"), Some(100));
    assert!(sync.contains("<D:sync-token>token123</D:sync-token>"));
    assert!(sync.contains("<D:nresults>100</D:nresults>"));
}
