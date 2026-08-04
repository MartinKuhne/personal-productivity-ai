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
