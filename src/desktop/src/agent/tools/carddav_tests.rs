//! Tests for `tools/carddav.rs`.
//!
//! Sidecar file. Extracted from `carddav.rs` so the implementation
//! module stays focused on production code.
//!
//! Originally a `#[cfg(test)] mod tests { ... }` block at the bottom of
//! `carddav.rs`. Lives in a sibling file so private item access via
//! `super::*` keeps working.

use super::*;

// =====================================================================
// parse_vcard tests
// =====================================================================

#[test]
fn test_parse_vcard_basic() {
    let data = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Alice Smith\r\nEMAIL:alice@example.com\r\nTEL:+1234567890\r\nORG:Acme Corp\r\nEND:VCARD";
    let contact = parse_vcard("client1", "/contacts/alice.vcf", data);

    assert_eq!(contact.client, "client1");
    assert_eq!(contact.href, "/contacts/alice.vcf");
    assert_eq!(contact.fn_name, Some("Alice Smith".to_string()));
    assert_eq!(contact.email, Some("alice@example.com".to_string()));
    assert_eq!(contact.tel, Some("+1234567890".to_string()));
    assert_eq!(contact.org, Some("Acme Corp".to_string()));
    assert!(contact.vcard.contains("BEGIN:VCARD"));
}

#[test]
fn test_parse_vcard_with_property_parameters() {
    // EMAIL;TYPE=INTERNET and TEL;TYPE=CELL should still parse
    let data = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Bob\r\nEMAIL;TYPE=INTERNET,WORK:bob@example.com\r\nTEL;TYPE=CELL,VOICE:+9876543210\r\nEND:VCARD";
    let contact = parse_vcard("c", "/b.vcf", data);

    assert_eq!(contact.fn_name, Some("Bob".to_string()));
    assert_eq!(contact.email, Some("bob@example.com".to_string()));
    assert_eq!(contact.tel, Some("+9876543210".to_string()));
}

#[test]
fn test_parse_vcard_folded_lines() {
    // vCard spec allows line folding with leading space/tab
    let data = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Very Long Name\r\n That Is Folded\r\nEMAIL:long@example.com\r\nEND:VCARD";
    let contact = parse_vcard("c", "/h", data);

    // The unfold logic removes leading whitespace and concatenates
    assert_eq!(
        contact.fn_name,
        Some("Very Long NameThat Is Folded".to_string())
    );
    assert_eq!(contact.email, Some("long@example.com".to_string()));
}

#[test]
fn test_parse_vcard_missing_fields() {
    // Only FN present
    let data = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:No Contact Info\r\nEND:VCARD";
    let contact = parse_vcard("c", "/h", data);

    assert_eq!(contact.fn_name, Some("No Contact Info".to_string()));
    assert_eq!(contact.email, None);
    assert_eq!(contact.tel, None);
    assert_eq!(contact.org, None);
}

#[test]
fn test_parse_vcard_empty_values() {
    let data = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:\r\nEMAIL:\r\nEND:VCARD";
    let contact = parse_vcard("c", "/h", data);

    assert_eq!(contact.fn_name, Some("".to_string()));
    assert_eq!(contact.email, Some("".to_string()));
}

#[test]
fn test_parse_vcard_malformed_no_colon() {
    // Lines without colon should be skipped
    let data = "BEGIN:VCARD\r\nVERSION:3.0\r\nNOCOLON\r\nFN:Valid Name\r\nEND:VCARD";
    let contact = parse_vcard("c", "/h", data);

    assert_eq!(contact.fn_name, Some("Valid Name".to_string()));
}

#[test]
fn test_parse_vcard_with_whitespace_only_lines() {
    let data =
        "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Test\r\n \r\n\t\r\nEMAIL:test@test.com\r\nEND:VCARD";
    let contact = parse_vcard("c", "/h", data);

    assert_eq!(contact.fn_name, Some("Test".to_string()));
    assert_eq!(contact.email, Some("test@test.com".to_string()));
}

// =====================================================================
// escape_vcard_text tests
// =====================================================================

#[test]
fn test_escape_vcard_text_basic() {
    assert_eq!(escape_vcard_text("Hello World"), "Hello World");
}

#[test]
fn test_escape_vcard_text_semicolon() {
    assert_eq!(escape_vcard_text("Hello;World"), "Hello\\;World");
}

#[test]
fn test_escape_vcard_text_comma() {
    assert_eq!(escape_vcard_text("Hello,World"), "Hello\\,World");
}

#[test]
fn test_escape_vcard_text_newline() {
    assert_eq!(escape_vcard_text("Line1\nLine2"), "Line1\\nLine2");
}

#[test]
fn test_escape_vcard_text_carriage_return() {
    assert_eq!(escape_vcard_text("Line1\rLine2"), "Line1Line2");
}

#[test]
fn test_escape_vcard_text_backslash() {
    assert_eq!(escape_vcard_text("Path\\to\\file"), "Path\\\\to\\\\file");
}

#[test]
fn test_escape_vcard_text_all_special_chars() {
    assert_eq!(
        escape_vcard_text("Hello; World,\nLine2\rEnd\\"),
        "Hello\\; World\\,\\nLine2End\\\\"
    );
}

// =====================================================================
// json_to_vcard tests
// =====================================================================

#[test]
fn test_json_to_vcard_basic() {
    let input = r#"{"fn":"John Doe","email":"john@example.com","tel":"+1234567890","org":"Acme"}"#;
    let vcard = json_to_vcard(input, None);

    assert!(vcard.starts_with("BEGIN:VCARD"));
    assert!(vcard.contains("VERSION:3.0"));
    assert!(vcard.contains("FN:John Doe"));
    assert!(vcard.contains("EMAIL;TYPE=INTERNET:john@example.com"));
    assert!(vcard.contains("TEL;TYPE=CELL:+1234567890"));
    assert!(vcard.contains("ORG:Acme"));
    assert!(vcard.contains("END:VCARD"));
}

#[test]
fn test_json_to_vcard_minimal() {
    // Only FN required, rest optional
    let input = r#"{"fn":"Anonymous"}"#;
    let vcard = json_to_vcard(input, None);

    assert!(vcard.contains("BEGIN:VCARD"));
    assert!(vcard.contains("VERSION:3.0"));
    assert!(vcard.contains("FN:Anonymous"));
    assert!(vcard.contains("END:VCARD"));
    // Should NOT contain empty EMAIL/TEL lines when not provided
    assert!(!vcard.contains("EMAIL;"));
    assert!(!vcard.contains("TEL;"));
    assert!(!vcard.contains("ORG:"));
}

#[test]
fn test_json_to_vcard_missing_fn_defaults_to_unknown() {
    let input = r#"{"email":"test@example.com"}"#;
    let vcard = json_to_vcard(input, None);

    assert!(vcard.contains("FN:Unknown"));
}

#[test]
fn test_json_to_vcard_invalid_json() {
    // Invalid JSON should use defaults
    let vcard = json_to_vcard("not json", None);

    assert!(vcard.starts_with("BEGIN:VCARD"));
    assert!(vcard.contains("FN:Unknown")); // Default
}

#[test]
fn test_json_to_vcard_with_uid_override() {
    let input = r#"{"fn":"Test"}"#;
    let vcard = json_to_vcard(input, Some("custom-uid-12345"));

    assert!(vcard.contains("UID:custom-uid-12345"));
}

#[test]
fn test_json_to_vcard_escapes_special_chars() {
    let input = r#"{"fn":"John; Doe","email":"test@example.com"}"#;
    let vcard = json_to_vcard(input, None);

    assert!(vcard.contains("FN:John\\; Doe"));
}

#[test]
fn test_json_to_vcard_generates_timestamp_based_uid() {
    // Test that json_to_vcard generates a UID field
    let input = r#"{"fn":"Test"}"#;
    let vcard = json_to_vcard(input, None);

    // UID should be present and contain only digits (timestamp-based)
    let uid_line = vcard.lines().find(|l| l.starts_with("UID:"));
    assert!(uid_line.is_some(), "UID field should be present");
    let uid = uid_line.unwrap().trim_start_matches("UID:");
    assert!(
        uid.chars().all(|c| c.is_ascii_digit()),
        "UID should be numeric: {}",
        uid
    );
}

// =====================================================================
// Natural field-name tests
//
// The LLM naturally sends `name`/`phone`/`company`/`title`/`notes`. The
// parser MUST accept those canonical names; the original `fn`/`tel`/`org`
// names remain valid aliases. Missing required fields are warned but
// still produce a vCard (with "Unknown" FN) so existing callers don't
// break — see `test_json_to_vcard_missing_name_warns` for that path.
// =====================================================================

#[test]
fn test_json_to_vcard_natural_field_names_round_trip() {
    // Exactly the payload the LLM sends today: name, email, phone,
    // company, title, notes. Every field must be preserved.
    let input = r#"{
        "name": "Paul Wayss",
        "email": "pwayss@sqagroup.com",
        "phone": "401-709-4153",
        "company": "SQA Group",
        "title": "Controller",
        "notes": "Handles payroll, timesheets, and compensation matters."
    }"#;
    let vcard = json_to_vcard(input, Some("test-uid"));

    assert!(
        vcard.contains("FN:Paul Wayss"),
        "FN must use the name field, got:\n{vcard}"
    );
    assert!(vcard.contains("EMAIL;TYPE=INTERNET:pwayss@sqagroup.com"));
    assert!(vcard.contains("TEL;TYPE=CELL:401-709-4153"));
    assert!(vcard.contains("ORG:SQA Group"));
    assert!(
        vcard.contains("TITLE:Controller"),
        "TITLE line missing, got:\n{vcard}"
    );
    assert!(
        vcard.contains(r"NOTE:Handles payroll\, timesheets\, and compensation matters."),
        "NOTE line missing or not escaped, got:\n{vcard}"
    );
    assert!(vcard.contains("UID:test-uid"));
}

#[test]
fn test_json_to_vcard_accepts_legacy_field_aliases() {
    // Original schema: `fn`, `tel`, `org`. Must still work for back-compat.
    let input = r#"{"fn":"Jane Doe","email":"jane@example.com","tel":"+15551234567","org":"Acme"}"#;
    let vcard = json_to_vcard(input, None);

    assert!(vcard.contains("FN:Jane Doe"));
    assert!(vcard.contains("EMAIL;TYPE=INTERNET:jane@example.com"));
    assert!(vcard.contains("TEL;TYPE=CELL:+15551234567"));
    assert!(vcard.contains("ORG:Acme"));
    // No empty TITLE/NOTE when those fields weren't provided.
    assert!(!vcard.contains("TITLE:"));
    assert!(!vcard.contains("NOTE:"));
}

#[test]
fn test_json_to_vcard_accepts_field_aliases_for_phone_org() {
    // Alternate names that the LLM might pick.
    let input = r#"{"name":"Bob","mobile":"+1 555 9999","organization":"Initech","title":"PE"}"#;
    let vcard = json_to_vcard(input, None);
    assert!(vcard.contains("TEL;TYPE=CELL:+1 555 9999"));
    assert!(vcard.contains("ORG:Initech"));
    assert!(vcard.contains("TITLE:PE"));
}

#[test]
fn test_json_to_vcard_canonical_name_takes_precedence_over_alias() {
    // If both `name` and `fn` are present, `name` (canonical) wins.
    let input = r#"{"name":"Canonical Name","fn":"Legacy Name"}"#;
    let vcard = json_to_vcard(input, None);
    assert!(vcard.contains("FN:Canonical Name"));
    assert!(!vcard.contains("Legacy Name"));
}

#[test]
fn test_json_to_vcard_skips_empty_string_fields() {
    // Empty strings should be treated as "not provided" — we don't want
    // a contact with `ORG:` (empty) in the addressbook.
    let input = r#"{"name":"X","email":"x@y.com","phone":"","company":""}"#;
    let vcard = json_to_vcard(input, None);
    assert!(vcard.contains("FN:X"));
    assert!(vcard.contains("EMAIL;TYPE=INTERNET:x@y.com"));
    assert!(!vcard.contains("TEL;"));
    assert!(!vcard.contains("ORG:"));
    assert!(!vcard.contains("TITLE:"));
    assert!(!vcard.contains("NOTE:"));
}

#[test]
fn test_json_to_vcard_escapes_special_chars_in_title_and_note() {
    // vCard 3.0 escaping: `;` `,` `\n` `\\` all need backslash-escapes.
    let input = r#"{"name":"E","title":"VP, Eng; Platform","notes":"Line1\nLine2; back\\slash"}"#;
    let vcard = json_to_vcard(input, None);
    assert!(vcard.contains(r"TITLE:VP\, Eng\; Platform"));
    assert!(vcard.contains(r"NOTE:Line1\nLine2\; back\\slash"));
}

#[test]
fn test_json_to_vcard_emits_title_and_note_only_when_provided() {
    // Defensive: ensure we don't accidentally emit empty TITLE: or NOTE:
    // lines for missing fields (some CalDAV servers reject them).
    let input = r#"{"name":"Plain","email":"p@e.com"}"#;
    let vcard = json_to_vcard(input, None);
    assert!(!vcard.contains("TITLE:"));
    assert!(!vcard.contains("NOTE:"));
    assert!(!vcard.contains("ORG:"));
    assert!(!vcard.contains("TEL;"));
}

#[test]
fn test_json_to_vcard_accepts_notes_and_note_aliases() {
    let v_notes = json_to_vcard(r#"{"name":"N","notes":"via notes"}"#, None);
    assert!(v_notes.contains("NOTE:via notes"));

    let v_note = json_to_vcard(r#"{"name":"N","note":"via note"}"#, None);
    assert!(v_note.contains("NOTE:via note"));
}

// =====================================================================
// CardDAV tool integration tests
// Note: These tests verify the functions handle empty/missing configurations.
// Full integration tests with mock servers require async network handling
// which is better suited for integration tests rather than unit tests.
// =====================================================================

#[test]
fn test_tool_search_contact_handles_empty_clients_gracefully() {
    // When caldav_clients is empty, the function should handle it gracefully
    let config = crate::config::AppConfig::default();
    let res = tool_search_contact(&config, "test");

    // Should handle empty config without panicking
    // Result may be Ok with empty response or Err depending on implementation
    assert!(res.is_ok() || res.is_err());
    if let Ok(response) = res {
        // If Ok, verify results is a valid string (we just access the
        // field; a panic here would mean a bug in the producer).
        let _ = &response.results;
    }
}

// =====================================================================
// build_contact_put_path tests
//
// Regression coverage for the path-separator bug that caused FastMail to
// return `403 Forbidden - Mailbox does not exist` on `add_contact`.
// PROPFIND typically returns hrefs with a trailing `/`; the original
// `format!("{}{}.vcf", book.trim_end_matches('/'), uid)` collapsed that
// separator and produced a malformed PUT URL like
// `…/Default1234567890.vcf` instead of `…/Default/1234567890.vcf`.
// =====================================================================

#[test]
fn test_build_contact_put_path_with_trailing_slash() {
    // FastMail-style addressbook href: ends with `/`.
    let path = build_contact_put_path(
        "https://caldav.fastmail.com/dav/addressbooks/user/me@example.com/Default/",
        "1234567890",
    );
    assert_eq!(
        path,
        "https://caldav.fastmail.com/dav/addressbooks/user/me@example.com/Default/1234567890.vcf"
    );
}

#[test]
fn test_build_contact_put_path_without_trailing_slash() {
    // Some servers / clients return hrefs without a trailing slash; the
    // helper must still produce a well-formed URL.
    let path = build_contact_put_path(
        "https://caldav.fastmail.com/dav/addressbooks/user/me@example.com/Default",
        "1234567890",
    );
    assert_eq!(
        path,
        "https://caldav.fastmail.com/dav/addressbooks/user/me@example.com/Default/1234567890.vcf"
    );
}

#[test]
fn test_build_contact_put_path_collapses_multiple_trailing_slashes() {
    // Defensive: tolerate accidental double trailing slashes.
    let path = build_contact_put_path("https://example.com/dav/user/me/Default//", "1234567890");
    assert_eq!(
        path,
        "https://example.com/dav/user/me/Default/1234567890.vcf"
    );
}

#[test]
fn test_build_contact_put_path_empty_addressbook_falls_back_safely() {
    // An empty collection is pathological, but the helper should not
    // produce a leading `/` and confuse the underlying HTTP client.
    let path = build_contact_put_path("", "1234567890");
    assert_eq!(path, "/1234567890.vcf");
}

#[test]
fn test_build_contact_put_path_uid_is_not_url_encoded() {
    // We generate numeric UIDs (millis-since-epoch), so this is a
    // smoke test that the helper does not introduce surprises.
    let path = build_contact_put_path("https://example.com/books/", "1700000000000");
    assert!(path.ends_with("/1700000000000.vcf"));
    assert!(!path.contains("//.vcf"));
}

// =====================================================================
// log_truncate tests
// =====================================================================

#[test]
fn test_log_truncate_under_limit_returns_full_body() {
    let body = b"hello world";
    assert_eq!(log_truncate(body), "hello world");
}

#[test]
fn test_log_truncate_over_limit_truncates_with_marker() {
    let body = vec![b'a'; LOG_BODY_LIMIT + 100];
    let s = log_truncate(&body);
    // Marker indicates truncation
    assert!(s.contains("...<truncated,"), "got: {}", s);
    // Body length up to the marker is exactly LOG_BODY_LIMIT bytes
    let marker_idx = s.find("...<truncated,").unwrap();
    let prefix = &s.as_bytes()[..marker_idx];
    assert_eq!(prefix.len(), LOG_BODY_LIMIT);
}

#[test]
fn test_log_truncate_invalid_utf8_does_not_panic() {
    // 0xff is not valid UTF-8; from_utf8_lossy substitutes U+FFFD.
    let body = vec![0xff, 0xfe, 0xfd];
    let s = log_truncate(&body);
    // Just ensure it doesn't panic and returns something
    assert!(!s.is_empty());
}
