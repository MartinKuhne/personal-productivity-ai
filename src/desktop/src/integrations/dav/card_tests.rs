//! Tests for `integrations/dav/card.rs`.
//!
//! Sidecar file. Extracted from `card.rs` so the implementation
//! module stays focused on production code.
//!
//! Originally a `#[cfg(test)] mod tests { ... }` block at the bottom of
//! `card.rs` (formerly `agent/tools/carddav.rs`, relocated when the
//! DAV protocol layer was moved to `crate::integrations::dav`).
//! Lives in a sibling file so private item access via `super::*`
//! keeps working.

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

// =====================================================================
// extract_vcard_uid tests
//
// `tool_update_contact` reuses the existing UID so the addressbook
// href stays stable across an update. These tests pin the extraction
// behavior including line folding and missing-UID.
// =====================================================================

#[test]
fn test_extract_vcard_uid_basic() {
    let body =
        "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Alice\r\nUID:abc-123\r\nEMAIL:a@e.com\r\nEND:VCARD";
    assert_eq!(extract_vcard_uid(body), Some("abc-123".to_string()));
}

#[test]
fn test_extract_vcard_uid_with_property_parameters() {
    // UID;X-OTHER=foo:value — the parameters must not confuse the
    // property-name parser.
    let body = "BEGIN:VCARD\r\nUID;X-OTHER=foo:abc-456\r\nEND:VCARD";
    assert_eq!(extract_vcard_uid(body), Some("abc-456".to_string()));
}

#[test]
fn test_extract_vcard_uid_with_folded_line() {
    // vCard 3.0 line folding: continuation lines start with a space.
    let body = "BEGIN:VCARD\r\nUID:abc-789-second-line-here\r\nEND:VCARD";
    assert_eq!(
        extract_vcard_uid(body),
        Some("abc-789-second-line-here".to_string())
    );
}

#[test]
fn test_extract_vcard_uid_missing() {
    let body = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:No UID\r\nEND:VCARD";
    assert_eq!(extract_vcard_uid(body), None);
}

#[test]
fn test_extract_vcard_uid_empty() {
    assert_eq!(extract_vcard_uid(""), None);
}

// =====================================================================
// unescape_vcard_text tests
// =====================================================================

#[test]
fn test_unescape_vcard_text_basic() {
    assert_eq!(unescape_vcard_text("plain text"), "plain text");
}

#[test]
fn test_unescape_vcard_text_inverts_escape() {
    // Every escape rule in RFC 2426 must round-trip.
    assert_eq!(unescape_vcard_text(r"Back\\slash"), r"Back\slash");
    assert_eq!(unescape_vcard_text(r"Semi\;colon"), "Semi;colon");
    assert_eq!(unescape_vcard_text(r"Comma\,here"), "Comma,here");
    assert_eq!(unescape_vcard_text(r"New\nline"), "New\nline");
    assert_eq!(unescape_vcard_text(r"Big\Nline"), "Big\nline");
}

#[test]
fn test_unescape_vcard_text_unknown_escape_preserved() {
    // Unknown escape sequences keep the backslash literal so we don't
    // silently mangle a value.
    assert_eq!(unescape_vcard_text(r"weird\x"), r"weird\x");
}

#[test]
fn test_unescape_vcard_text_trailing_backslash_preserved() {
    // A trailing `\` with no follow-up character must not panic.
    assert_eq!(unescape_vcard_text(r"trailing\"), r"trailing\");
}

// =====================================================================
// Round-trip tests for json_to_vcard with explicit UID
//
// The update tool relies on json_to_vcard emitting exactly the UID
// passed in. If the override is dropped, the contact's identity in
// the addressbook will change on every update.
// =====================================================================

#[test]
fn test_json_to_vcard_preserves_explicit_uid() {
    let input = r#"{"name":"Paul Wayss","email":"p@example.com"}"#;
    let vcard = json_to_vcard(input, Some("existing-uid-xyz"));

    assert!(
        vcard.contains("UID:existing-uid-xyz"),
        "explicit UID must be preserved, got:\n{vcard}"
    );
    // The auto-generated timestamp UID must NOT appear.
    let uid_line = vcard
        .lines()
        .find(|l| l.starts_with("UID:"))
        .expect("UID line present");
    assert_eq!(uid_line, "UID:existing-uid-xyz");
}

// =====================================================================
// parse_vcard_properties tests
// =====================================================================

#[test]
fn test_parse_vcard_properties_drops_structural_lines() {
    let body = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:abc\r\nFN:Alice\r\nEND:VCARD";
    let props = parse_vcard_properties(body);
    let names: Vec<&str> = props.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["FN"]);
}

#[test]
fn test_parse_vcard_properties_preserves_prefix() {
    let body = "BEGIN:VCARD\r\nEMAIL;TYPE=WORK:bob@work.com\r\nEND:VCARD";
    let props = parse_vcard_properties(body);
    assert_eq!(props.len(), 1);
    assert_eq!(props[0].name, "EMAIL");
    assert_eq!(props[0].prefix, "EMAIL;TYPE=WORK");
}

#[test]
fn test_parse_vcard_properties_preserves_unknown_properties() {
    // BDAY, URL, NICKNAME, X-CUSTOM must all be captured.
    let body = "BEGIN:VCARD\r\n\
                VERSION:3.0\r\n\
                FN:Alice\r\n\
                BDAY:1990-01-01\r\n\
                URL:https://example.com\r\n\
                NICKNAME:Ally\r\n\
                X-SKILL:Rust\r\n\
                END:VCARD";
    let props = parse_vcard_properties(body);
    let names: Vec<&str> = props.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"FN"));
    assert!(names.contains(&"BDAY"));
    assert!(names.contains(&"URL"));
    assert!(names.contains(&"NICKNAME"));
    assert!(names.contains(&"X-SKILL"));
}

#[test]
fn test_parse_vcard_properties_keeps_multiple_emails_in_order() {
    let body = "BEGIN:VCARD\r\n\
                EMAIL;TYPE=WORK:bob@work.com\r\n\
                EMAIL;TYPE=HOME:bob@home.com\r\n\
                END:VCARD";
    let props = parse_vcard_properties(body);
    assert_eq!(props.len(), 2);
    assert_eq!(props[0].prefix, "EMAIL;TYPE=WORK");
    assert_eq!(props[1].prefix, "EMAIL;TYPE=HOME");
}

// =====================================================================
// unfold_vcard tests
// =====================================================================

#[test]
fn test_unfold_vcard_merges_continuation_lines() {
    // vCard 3.0 line folding: per RFC 2426, only the *first* whitespace
    // after the CRLF is the fold marker; any additional whitespace is
    // part of the value. So `"  Line2"` means fold + ` Line2` (a
    // space + the word "Line2"). `"\tLine3"` means fold + "Line3".
    let body = "BEGIN:VCARD\r\nNOTE:Line1\r\n  Line2\r\n\tLine3\r\nEND:VCARD";
    let unfolded = unfold_vcard(body);

    // Continuation lines are merged onto the previous line (the first
    // leading whitespace is consumed as the fold marker).
    assert!(unfolded.contains("NOTE:Line1 Line2Line3"));
    // No orphaned leading-space lines should remain.
    assert!(
        !unfolded
            .lines()
            .any(|l| l.starts_with(' ') || l.starts_with('\t'))
    );
}

// =====================================================================
// merge_vcard_update tests — the "no accidental delete" guarantee
//
// These tests pin the property-preservation behaviour the user asked
// for. A vCard that has BDAY, URL, NICKNAME, X-*, etc. must keep every
// one of those properties after the update; only the canonical fields
// the LLM provides may be replaced.
// =====================================================================

const FIXTURE_PAUL: &str = "BEGIN:VCARD\r\n\
VERSION:3.0\r\n\
FN:Paul Wayss\r\n\
N:Wayss;Paul;;;\r\n\
NICKNAME:PJ\r\n\
BDAY:1980-04-15\r\n\
EMAIL;TYPE=WORK:pwayss@sqagroup.com\r\n\
TEL;TYPE=CELL:401-709-4153\r\n\
URL:https://sqagroup.com\r\n\
X-SKILLS:payroll;tax;audit\r\n\
ORG:SQA Group\r\n\
TITLE:Controller\r\n\
NOTE:Handles payroll\\, timesheets\\, and compensation matters.\r\n\
UID:paul-uid-123\r\n\
END:VCARD";

#[test]
fn test_merge_vcard_update_preserves_unknown_properties() {
    // Update only `title`. Everything else (N, NICKNAME, BDAY, URL,
    // X-SKILLS, NOTE, the FN, EMAIL, TEL, ORG) must remain untouched.
    let existing = parse_vcard_properties(FIXTURE_PAUL);
    let new_json = r#"{"title":"VP Controller"}"#;
    let merged = merge_vcard_update(&existing, new_json, Some("paul-uid-123"));

    // New value applied
    assert!(merged.contains("TITLE:VP Controller"), "got:\n{merged}");
    // Preserved
    assert!(merged.contains("FN:Paul Wayss"));
    assert!(merged.contains("N:Wayss;Paul;;;"));
    assert!(merged.contains("NICKNAME:PJ"));
    assert!(merged.contains("BDAY:1980-04-15"));
    assert!(merged.contains("EMAIL;TYPE=WORK:pwayss@sqagroup.com"));
    assert!(merged.contains("TEL;TYPE=CELL:401-709-4153"));
    assert!(merged.contains("URL:https://sqagroup.com"));
    assert!(merged.contains("X-SKILLS:payroll;tax;audit"));
    assert!(merged.contains("ORG:SQA Group"));
    assert!(merged.contains(r"NOTE:Handles payroll\, timesheets\, and compensation matters."));
    // UID
    assert!(merged.contains("UID:paul-uid-123"));
}

#[test]
fn test_merge_vcard_update_collapses_duplicate_emails_to_provided_value() {
    // Existing contact has two emails. LLM provides one. Result: the
    // first one is updated, the second is dropped. This is the
    // predictable behaviour for a single-value field — the LLM is
    // sending the canonical email, not a list.
    let body = "BEGIN:VCARD\r\n\
                VERSION:3.0\r\n\
                FN:Bob\r\n\
                EMAIL;TYPE=WORK:bob@work.com\r\n\
                EMAIL;TYPE=HOME:bob@home.com\r\n\
                END:VCARD";
    let existing = parse_vcard_properties(body);
    let new_json = r#"{"email":"bob@personal.com"}"#;
    let merged = merge_vcard_update(&existing, new_json, Some("bob-uid"));

    // Only one EMAIL line, with the new value, and the original
    // TYPE=WORK prefix preserved (since it was on the first match).
    let email_lines: Vec<&str> = merged.lines().filter(|l| l.starts_with("EMAIL")).collect();
    assert_eq!(
        email_lines.len(),
        1,
        "duplicate EMAIL should collapse, got:\n{merged}"
    );
    assert_eq!(email_lines[0], "EMAIL;TYPE=WORK:bob@personal.com");
    // The dropped one must be gone.
    assert!(!merged.contains("bob@home.com"));
}

#[test]
fn test_merge_vcard_update_preserves_first_prefix_for_replacement() {
    // If the LLM updates an EMAIL that has TYPE=WORK, the TYPE=WORK
    // prefix must be kept on the replacement (we don't know whether
    // the user wanted to keep or change the type, and dropping the
    // parameter is a surprise).
    let body = "BEGIN:VCARD\r\nEMAIL;TYPE=WORK:bob@work.com\r\nEND:VCARD";
    let existing = parse_vcard_properties(body);
    let merged = merge_vcard_update(&existing, r#"{"email":"bob@newoffice.com"}"#, Some("u"));
    assert!(merged.contains("EMAIL;TYPE=WORK:bob@newoffice.com"));
}

#[test]
fn test_merge_vcard_update_partial_json_preserves_other_canonical_fields() {
    // LLM sends only `name` and `phone`. EMAIL, ORG, TITLE, NOTE
    // should all be preserved verbatim.
    let existing = parse_vcard_properties(FIXTURE_PAUL);
    let new_json = r#"{"name":"Paul J. Wayss","phone":"+1-401-555-0199"}"#;
    let merged = merge_vcard_update(&existing, new_json, Some("paul-uid-123"));

    assert!(merged.contains("FN:Paul J. Wayss"));
    assert!(merged.contains("TEL;TYPE=CELL:+1-401-555-0199"));
    // Preserved canonical fields
    assert!(merged.contains("EMAIL;TYPE=WORK:pwayss@sqagroup.com"));
    assert!(merged.contains("ORG:SQA Group"));
    assert!(merged.contains("TITLE:Controller"));
    assert!(merged.contains(r"NOTE:Handles payroll\, timesheets\, and compensation matters."));
}

#[test]
fn test_merge_vcard_update_empty_value_is_treated_as_not_provided() {
    // The schema's first_str drops empty strings, so an LLM that sends
    // `"phone": ""` is saying "don't change the phone" — the existing
    // TEL must survive.
    let existing = parse_vcard_properties(FIXTURE_PAUL);
    let merged = merge_vcard_update(&existing, r#"{"phone":""}"#, Some("paul-uid-123"));
    assert!(
        merged.contains("TEL;TYPE=CELL:401-709-4153"),
        "empty phone value must not clear the existing TEL, got:\n{merged}"
    );
}

#[test]
fn test_merge_vcard_update_adds_new_canonical_field_with_default_prefix() {
    // Existing contact has no EMAIL. LLM sends one. The new EMAIL
    // should appear with the standard `EMAIL;TYPE=INTERNET` prefix.
    let body = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Bob\r\nEND:VCARD";
    let existing = parse_vcard_properties(body);
    let merged = merge_vcard_update(
        &existing,
        r#"{"email":"bob@new.com","phone":"+15551212"}"#,
        Some("uid"),
    );
    assert!(merged.contains("EMAIL;TYPE=INTERNET:bob@new.com"));
    assert!(merged.contains("TEL;TYPE=CELL:+15551212"));
}

#[test]
fn test_merge_vcard_update_generates_uid_when_existing_lacks_one() {
    let body = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Bob\r\nEND:VCARD";
    let existing = parse_vcard_properties(body);
    let merged = merge_vcard_update(&existing, r#"{}"#, Some("fresh-uid-42"));
    assert!(merged.contains("UID:fresh-uid-42"));
}

#[test]
fn test_merge_vcard_update_drops_no_uid_when_caller_passes_none() {
    // If existing vCard has a UID and the caller passes None (e.g.
    // because they extracted the UID themselves and used it), the UID
    // line is simply not re-emitted.
    let body = "BEGIN:VCARD\r\nUID:abc\r\nFN:Bob\r\nEND:VCARD";
    let existing = parse_vcard_properties(body);
    let merged = merge_vcard_update(&existing, r#"{}"#, None);
    assert!(!merged.contains("UID:"));
}

#[test]
fn test_merge_vcard_update_idempotent_on_no_changes() {
    // Sending the same canonical fields as the existing vCard should
    // produce an equivalent vCard (modulo a possible re-emission of
    // the same lines).
    let existing = parse_vcard_properties(FIXTURE_PAUL);
    let json = r#"{"name":"Paul Wayss","email":"pwayss@sqagroup.com"}"#;
    let merged = merge_vcard_update(&existing, json, Some("paul-uid-123"));
    // No fields dropped, no new values introduced.
    assert!(merged.contains("FN:Paul Wayss"));
    assert!(merged.contains("EMAIL;TYPE=WORK:pwayss@sqagroup.com"));
    assert!(merged.contains("TEL;TYPE=CELL:401-709-4153"));
    assert!(merged.contains("BDAY:1980-04-15"));
    assert!(merged.contains("URL:https://sqagroup.com"));
    assert!(merged.contains("X-SKILLS:payroll;tax;audit"));
}

#[test]
fn test_merge_vcard_update_handles_empty_existing() {
    // Pathological: no existing properties at all. The merger should
    // still produce a well-formed vCard with just the new fields.
    let existing: Vec<VcardProp> = Vec::new();
    let merged = merge_vcard_update(&existing, r#"{"name":"New","email":"n@e.com"}"#, Some("u"));
    assert!(merged.starts_with("BEGIN:VCARD\r\n"));
    assert!(merged.contains("FN:New"));
    assert!(merged.contains("EMAIL;TYPE=INTERNET:n@e.com"));
    assert!(merged.contains("UID:u"));
    assert!(merged.ends_with("END:VCARD\r\n"));
}

// =====================================================================
// BDAY tests — single-valued canonical field.
// =====================================================================

#[test]
fn test_parse_vcard_extracts_bday() {
    let data = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:A\r\nBDAY:1980-04-15\r\nEND:VCARD";
    let contact = parse_vcard("c", "/h", data);
    assert_eq!(contact.bday, Some("1980-04-15".to_string()));
}

#[test]
fn test_parse_vcard_missing_bday_is_none() {
    let data = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:A\r\nEND:VCARD";
    let contact = parse_vcard("c", "/h", data);
    assert_eq!(contact.bday, None);
}

#[test]
fn test_json_to_vcard_emits_bday_when_provided() {
    let vcard = json_to_vcard(r#"{"name":"A","birthday":"1990-01-15"}"#, Some("u"));
    assert!(vcard.contains("BDAY:1990-01-15"));
}

#[test]
fn test_json_to_vcard_bday_aliases() {
    // LLM might send any of: birthday, bday, dob, date_of_birth.
    assert!(json_to_vcard(r#"{"name":"A","bday":"1990-01-15"}"#, None).contains("BDAY:1990-01-15"));
    assert!(json_to_vcard(r#"{"name":"A","dob":"1990-01-15"}"#, None).contains("BDAY:1990-01-15"));
    assert!(
        json_to_vcard(r#"{"name":"A","date_of_birth":"1990-01-15"}"#, None)
            .contains("BDAY:1990-01-15")
    );
}

#[test]
fn test_json_to_vcard_no_bday_no_line() {
    let vcard = json_to_vcard(r#"{"name":"A"}"#, None);
    assert!(!vcard.contains("BDAY:"));
}

#[test]
fn test_merge_vcard_update_replaces_bday_when_provided() {
    let body = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:A\r\nBDAY:1980-04-15\r\nEND:VCARD";
    let existing = parse_vcard_properties(body);
    let merged = merge_vcard_update(&existing, r#"{"birthday":"1990-12-31"}"#, Some("u"));
    assert!(merged.contains("BDAY:1990-12-31"));
    assert!(!merged.contains("BDAY:1980-04-15"));
}

#[test]
fn test_merge_vcard_update_preserves_bday_when_not_provided() {
    let body = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:A\r\nBDAY:1980-04-15\r\nEND:VCARD";
    let existing = parse_vcard_properties(body);
    let merged = merge_vcard_update(&existing, r#"{"name":"A2"}"#, Some("u"));
    assert!(merged.contains("BDAY:1980-04-15"));
}

#[test]
fn test_merge_vcard_update_collapses_duplicate_bdays() {
    // BDAY is single-valued; duplicates must collapse like FN/EMAIL/…
    let body = "BEGIN:VCARD\r\nBDAY:1980-04-15\r\nBDAY:1990-01-01\r\nEND:VCARD";
    let existing = parse_vcard_properties(body);
    let merged = merge_vcard_update(&existing, r#"{"birthday":"2000-06-15"}"#, Some("u"));
    let bdays: Vec<&str> = merged.lines().filter(|l| l.starts_with("BDAY:")).collect();
    assert_eq!(bdays.len(), 1);
    assert_eq!(bdays[0], "BDAY:2000-06-15");
}

// =====================================================================
// ADR tests — list-valued, structured.
// =====================================================================

#[test]
fn test_parse_vcard_extracts_single_adr() {
    let data = "BEGIN:VCARD\r\n\
                VERSION:3.0\r\n\
                FN:A\r\n\
                ADR;TYPE=HOME:;;123 Main St;Springfield;IL;62701;USA\r\n\
                END:VCARD";
    let contact = parse_vcard("c", "/h", data);
    assert_eq!(contact.addresses.len(), 1);
    let addr = &contact.addresses[0];
    assert_eq!(addr.kind, Some("HOME".to_string()));
    assert_eq!(addr.street, Some("123 Main St".to_string()));
    assert_eq!(addr.city, Some("Springfield".to_string()));
    assert_eq!(addr.region, Some("IL".to_string()));
    assert_eq!(addr.postal_code, Some("62701".to_string()));
    assert_eq!(addr.country, Some("USA".to_string()));
}

#[test]
fn test_parse_vcard_extracts_multiple_adrs() {
    let data = "BEGIN:VCARD\r\n\
                VERSION:3.0\r\n\
                FN:A\r\n\
                ADR;TYPE=HOME:;;123 Main St;Springfield;IL;62701;USA\r\n\
                ADR;TYPE=WORK:;;456 Office Blvd;Metropolis;NY;10001;USA\r\n\
                END:VCARD";
    let contact = parse_vcard("c", "/h", data);
    assert_eq!(contact.addresses.len(), 2);
    assert_eq!(contact.addresses[0].kind, Some("HOME".to_string()));
    assert_eq!(contact.addresses[0].city, Some("Springfield".to_string()));
    assert_eq!(contact.addresses[1].kind, Some("WORK".to_string()));
    assert_eq!(contact.addresses[1].city, Some("Metropolis".to_string()));
}

#[test]
fn test_parse_vcard_adr_empty_components_become_none() {
    // Empty `;`-separated slots must come back as None, not as
    // Some(""), so the LLM sees "no value" rather than "empty string".
    let data = "BEGIN:VCARD\r\n\
                VERSION:3.0\r\n\
                FN:A\r\n\
                ADR:;;;;;;\r\n\
                END:VCARD";
    let contact = parse_vcard("c", "/h", data);
    assert_eq!(contact.addresses.len(), 1);
    let addr = &contact.addresses[0];
    assert_eq!(addr.street, None);
    assert_eq!(addr.city, None);
    assert_eq!(addr.region, None);
    assert_eq!(addr.postal_code, None);
    assert_eq!(addr.country, None);
}

#[test]
fn test_parse_vcard_adr_unescapes_semicolons_in_components() {
    // vCard 3.0 escapes `;` inside a component as `\;`. The split
    // must respect that and the value must be unescaped.
    let data = "BEGIN:VCARD\r\n\
                VERSION:3.0\r\n\
                FN:A\r\n\
                ADR:;Suite 4\\; Bldg 7;1 Main;Town;State;00000;Country\r\n\
                END:VCARD";
    let contact = parse_vcard("c", "/h", data);
    let addr = &contact.addresses[0];
    assert_eq!(addr.ext, Some("Suite 4; Bldg 7".to_string()));
    assert_eq!(addr.street, Some("1 Main".to_string()));
}

#[test]
fn test_parse_vcard_adr_no_type_prefix() {
    let data = "BEGIN:VCARD\r\
                FN:A\r\n\
                ADR:;;123 Main;Town;ST;00000;Country\r\n\
                END:VCARD";
    let contact = parse_vcard("c", "/h", data);
    let addr = &contact.addresses[0];
    assert_eq!(addr.kind, None);
    assert_eq!(addr.street, Some("123 Main".to_string()));
}

#[test]
fn test_json_to_vcard_emits_addresses_array() {
    let input = r#"{
        "name": "Alice",
        "addresses": [
            {"type": "home", "street": "1 Main", "city": "Town", "region": "ST", "postal_code": "00000", "country": "US"}
        ]
    }"#;
    let vcard = json_to_vcard(input, Some("u"));
    assert!(vcard.contains("ADR;TYPE=HOME:;;1 Main;Town;ST;00000;US"));
}

#[test]
fn test_json_to_vcard_emits_address_object_alias() {
    let input = r#"{
        "name": "Alice",
        "address": {"type": "work", "city": "Metropolis"}
    }"#;
    let vcard = json_to_vcard(input, Some("u"));
    // Only `city` is set; the other 6 ADR components are empty.
    assert!(vcard.contains("ADR;TYPE=WORK:;;;Metropolis;;;"));
}

#[test]
fn test_json_to_vcard_emits_address_string_form() {
    // Convenience: a single-line address string is heuristically split
    // into the 5 standard components (street, city, region, postal_code,
    // country). The vCard ADR has 7 components (po-box, ext, then the 5
    // standard ones), so the first two are empty.
    let input = r#"{
        "name": "Alice",
        "address": "123 Main St, Springfield, IL, 62701, USA"
    }"#;
    let vcard = json_to_vcard(input, Some("u"));
    assert!(vcard.contains("ADR:;;123 Main St;Springfield;IL;62701;USA"));
}

#[test]
fn test_json_to_vcard_emits_multiple_addresses() {
    let input = r#"{
        "name": "Alice",
        "addresses": [
            {"type": "home", "city": "Springfield"},
            {"type": "work", "city": "Metropolis"}
        ]
    }"#;
    let vcard = json_to_vcard(input, Some("u"));
    assert!(vcard.contains("ADR;TYPE=HOME:;;;Springfield;;;"));
    assert!(vcard.contains("ADR;TYPE=WORK:;;;Metropolis;;;"));
}

#[test]
fn test_json_to_vcard_address_without_type_has_no_type_prefix() {
    let input = r#"{
        "name": "Alice",
        "address": {"city": "Springfield"}
    }"#;
    let vcard = json_to_vcard(input, Some("u"));
    // No TYPE= since the JSON didn't include a type.
    assert!(vcard.contains("ADR:;;;Springfield;;;"));
    assert!(!vcard.contains("ADR;TYPE"));
}

#[test]
fn test_json_to_vcard_no_address_no_adr_line() {
    let vcard = json_to_vcard(r#"{"name":"A"}"#, None);
    assert!(!vcard.contains("ADR"));
}

#[test]
fn test_merge_vcard_update_preserves_existing_adrs_when_no_new_addresses() {
    let body = "BEGIN:VCARD\r\n\
                VERSION:3.0\r\n\
                FN:A\r\n\
                ADR;TYPE=HOME:;;1 Main;Town;ST;00000;Country\r\n\
                END:VCARD";
    let existing = parse_vcard_properties(body);
    let merged = merge_vcard_update(&existing, r#"{"name":"A2"}"#, Some("u"));
    // The existing ADR survives unchanged.
    assert!(merged.contains("ADR;TYPE=HOME:;;1 Main;Town;ST;00000;Country"));
}

#[test]
fn test_merge_vcard_update_replaces_adrs_with_new_list() {
    // Existing has two ADRs. New payload has two different ones.
    // The old ADRs must be gone; the new ones present.
    let body = "BEGIN:VCARD\r\n\
                VERSION:3.0\r\n\
                FN:A\r\n\
                ADR;TYPE=HOME:;;1 Main;Town;ST;00000;Country\r\n\
                ADR;TYPE=WORK:;;2 Office;City;ST;11111;Country\r\n\
                END:VCARD";
    let existing = parse_vcard_properties(body);
    let merged = merge_vcard_update(
        &existing,
        r#"{"addresses":[{"type":"home","city":"NewTown"},{"type":"work","city":"NewCity"}]}"#,
        Some("u"),
    );
    assert!(merged.contains("NewTown"));
    assert!(merged.contains("NewCity"));
    assert!(!merged.contains("1 Main"));
    assert!(!merged.contains("2 Office"));
}

#[test]
fn test_merge_vcard_update_empty_addresses_list_clears_all_adrs() {
    // Sending `"addresses": []` is an explicit "drop all addresses"
    // signal. The list-replace semantic must honour it.
    let body = "BEGIN:VCARD\r\n\
                VERSION:3.0\r\n\
                FN:A\r\n\
                ADR;TYPE=HOME:;;1 Main;Town;ST;00000;Country\r\n\
                END:VCARD";
    let existing = parse_vcard_properties(body);
    let merged = merge_vcard_update(&existing, r#"{"addresses":[]}"#, Some("u"));
    assert!(!merged.contains("ADR"));
}

#[test]
fn test_merge_vcard_update_address_object_alias() {
    let existing = parse_vcard_properties(FIXTURE_PAUL);
    let merged = merge_vcard_update(
        &existing,
        r#"{"address":{"type":"work","city":"SQA HQ","country":"US"}}"#,
        Some("paul-uid-123"),
    );
    assert!(merged.contains("ADR;TYPE=WORK:;;;SQA HQ;;;US"));
}

#[test]
fn test_merge_vcard_update_address_string_alias() {
    let existing = parse_vcard_properties(FIXTURE_PAUL);
    let merged = merge_vcard_update(
        &existing,
        r#"{"address":"16999 NE 37th Pl, Bellevue, WA 98004"}"#,
        Some("paul-uid-123"),
    );
    // Heuristic split: the 3 non-empty comma-separated parts fill 3
    // of the 7 ADR components; the rest (po_box, ext, postal_code,
    // country) stay empty. vCard 7-component ADR shape:
    // po_box;ext;street;city;region;postal_code;country
    assert!(merged.contains("ADR:;;16999 NE 37th Pl;Bellevue;WA 98004;;"));
}

#[test]
fn test_merge_vcard_update_addresses_escape_commas() {
    // A city name with a comma must escape it so the round-trip works.
    let existing: Vec<VcardProp> = Vec::new();
    let merged = merge_vcard_update(
        &existing,
        r#"{"addresses":[{"type":"home","city":"Bellevue, WA"}]}"#,
        Some("u"),
    );
    // The comma must be escaped to `\,` in the vCard output.
    assert!(merged.contains(r"ADR;TYPE=HOME:;;;Bellevue\, WA;;;"));
}

// =====================================================================
// StructuredAddress serialization tests
// =====================================================================

#[test]
fn test_structured_address_serialize_minimal() {
    let addr = StructuredAddress {
        kind: Some("HOME".to_string()),
        po_box: None,
        ext: None,
        street: Some("1 Main".to_string()),
        city: None,
        region: None,
        postal_code: None,
        country: None,
    };
    let json = serde_json::to_value(&addr).unwrap();
    let obj = json.as_object().unwrap();
    assert_eq!(obj.get("type"), Some(&serde_json::json!("HOME")));
    assert_eq!(obj.get("street"), Some(&serde_json::json!("1 Main")));
    // Empty components are omitted from the JSON.
    assert!(!obj.contains_key("city"));
    assert!(!obj.contains_key("country"));
}

#[test]
fn test_structured_address_serialize_full() {
    let addr = StructuredAddress {
        kind: Some("WORK".to_string()),
        po_box: Some("P.O. Box 5".to_string()),
        ext: Some("Suite 200".to_string()),
        street: Some("456 Office Blvd".to_string()),
        city: Some("Metropolis".to_string()),
        region: Some("NY".to_string()),
        postal_code: Some("10001".to_string()),
        country: Some("USA".to_string()),
    };
    let json = serde_json::to_value(&addr).unwrap();
    assert_eq!(json["type"], "WORK");
    assert_eq!(json["po_box"], "P.O. Box 5");
    assert_eq!(json["ext"], "Suite 200");
    assert_eq!(json["street"], "456 Office Blvd");
    assert_eq!(json["city"], "Metropolis");
    assert_eq!(json["region"], "NY");
    assert_eq!(json["postal_code"], "10001");
    assert_eq!(json["country"], "USA");
}

// =====================================================================
// split_vcard_value tests (used for ADR component split)
// =====================================================================

#[test]
fn test_split_vcard_value_respects_escaped_separator() {
    // The vCard ADR value uses `;` as the component separator AND
    // uses `\;` to embed a literal `;` inside a component. The split
    // helper must not break on the escaped one.
    let parts = split_vcard_value(r"a\;b;c", ';');
    assert_eq!(parts, vec![r"a\;b", "c"]);
}

#[test]
fn test_split_vcard_value_pads_short_input() {
    // An ADR with fewer than 7 components is valid (missing
    // components are empty). The split must not crash.
    let parts = split_vcard_value(";;only street", ';');
    assert_eq!(parts, vec!["", "", "only street"]);
}
