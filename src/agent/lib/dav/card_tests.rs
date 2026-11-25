//! Tests for `agent/lib/dav/card.rs`.

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

/// `(label, vCard body, FN, EMAIL, TEL, ORG)` — the canonical scalar fields
/// `parse_vcard` must read back out of a vCard body.
type VcardScalarCase<'a> = (
    &'a str,
    &'a str,
    Option<&'a str>,
    Option<&'a str>,
    Option<&'a str>,
    Option<&'a str>,
);

#[test]
fn test_parse_vcard_reads_the_canonical_scalar_fields() {
    let cases: Vec<VcardScalarCase<'_>> = vec![
        (
            "property parameters on EMAIL/TEL are ignored",
            "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Bob\r\nEMAIL;TYPE=INTERNET,WORK:bob@example.com\r\nTEL;TYPE=CELL,VOICE:+9876543210\r\nEND:VCARD",
            Some("Bob"),
            Some("bob@example.com"),
            Some("+9876543210"),
            None,
        ),
        (
            "folded lines are concatenated without a separator",
            "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Very Long Name\r\n That Is Folded\r\nEMAIL:long@example.com\r\nEND:VCARD",
            Some("Very Long NameThat Is Folded"),
            Some("long@example.com"),
            None,
            None,
        ),
        (
            "absent optional fields stay None",
            "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:No Contact Info\r\nEND:VCARD",
            Some("No Contact Info"),
            None,
            None,
            None,
        ),
        (
            "empty values are kept as empty strings",
            "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:\r\nEMAIL:\r\nEND:VCARD",
            Some(""),
            Some(""),
            None,
            None,
        ),
        (
            "a line without a colon is skipped",
            "BEGIN:VCARD\r\nVERSION:3.0\r\nNOCOLON\r\nFN:Valid Name\r\nEND:VCARD",
            Some("Valid Name"),
            None,
            None,
            None,
        ),
        (
            "whitespace-only lines are skipped",
            "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Test\r\n \r\n\t\r\nEMAIL:test@test.com\r\nEND:VCARD",
            Some("Test"),
            Some("test@test.com"),
            None,
            None,
        ),
    ];

    for (label, data, fn_name, email, tel, org) in cases {
        let contact = parse_vcard("c", "/h", data);
        assert_eq!(
            contact.fn_name.as_deref(),
            fn_name,
            "case '{label}': FN must be read back"
        );
        assert_eq!(
            contact.email.as_deref(),
            email,
            "case '{label}': EMAIL must be read back"
        );
        assert_eq!(
            contact.tel.as_deref(),
            tel,
            "case '{label}': TEL must be read back"
        );
        assert_eq!(
            contact.org.as_deref(),
            org,
            "case '{label}': ORG must be read back"
        );
    }
}

// =====================================================================
// escape_vcard_text tests
// =====================================================================

#[test]
fn test_escape_vcard_text_escapes_every_special_character() {
    // One case per vCard 3.0 escape rule, plus the pass-through case and a
    // combined input that exercises all of them at once.
    let cases: Vec<(&str, &str, &str)> = vec![
        ("plain text is untouched", "Hello World", "Hello World"),
        ("semicolon", "Hello;World", "Hello\\;World"),
        ("comma", "Hello,World", "Hello\\,World"),
        ("newline", "Line1\nLine2", "Line1\\nLine2"),
        ("carriage return is dropped", "Line1\rLine2", "Line1Line2"),
        ("backslash", "Path\\to\\file", "Path\\\\to\\\\file"),
        (
            "all special characters at once",
            "Hello; World,\nLine2\rEnd\\",
            "Hello\\; World\\,\\nLine2End\\\\",
        ),
    ];

    for (label, input, expected) in cases {
        assert_eq!(
            escape_vcard_text(input),
            expected,
            "case '{label}': escape_vcard_text must produce the escaped value"
        );
    }
}

// =====================================================================
// json_to_vcard tests
// =====================================================================

/// One `json_to_vcard` render assertion set: the JSON payload and UID override
/// plus the fragments that must and must not appear in the produced vCard.
struct VcardRenderCase<'a> {
    label: &'a str,
    json: &'a str,
    uid: Option<&'a str>,
    must_contain: &'a [&'a str],
    must_not_contain: &'a [&'a str],
}

/// Assert one `json_to_vcard` render case set: every listed fragment must be
/// present in the produced vCard and every forbidden one must be absent.
fn assert_vcard_render(cases: Vec<VcardRenderCase<'_>>) {
    for case in cases {
        let VcardRenderCase {
            label,
            json,
            uid,
            must_contain,
            must_not_contain,
        } = case;
        let vcard = json_to_vcard(json, uid);
        for fragment in must_contain {
            assert!(
                vcard.contains(fragment),
                "case '{label}': expected the vCard to contain '{fragment}', got:\n{vcard}"
            );
        }
        for fragment in must_not_contain {
            assert!(
                !vcard.contains(fragment),
                "case '{label}': expected the vCard NOT to contain '{fragment}', got:\n{vcard}"
            );
        }
    }
}

#[test]
fn test_json_to_vcard_renders_the_expected_lines_for_each_payload() {
    assert_vcard_render(vec![
        VcardRenderCase {
            label: "the full legacy payload",
            json: r#"{"fn":"John Doe","email":"john@example.com","tel":"+1234567890","org":"Acme"}"#,
            uid: None,
            must_contain: &[
                "BEGIN:VCARD",
                "VERSION:3.0",
                "FN:John Doe",
                "EMAIL;TYPE=INTERNET:john@example.com",
                "TEL;TYPE=CELL:+1234567890",
                "ORG:Acme",
                "END:VCARD",
            ],
            must_not_contain: &[],
        },
        VcardRenderCase {
            label: "only FN is provided",
            json: r#"{"fn":"Anonymous"}"#,
            uid: None,
            must_contain: &["BEGIN:VCARD", "VERSION:3.0", "FN:Anonymous", "END:VCARD"],
            // No empty EMAIL/TEL/ORG lines for fields that were not provided.
            must_not_contain: &["EMAIL;", "TEL;", "ORG:"],
        },
        VcardRenderCase {
            label: "a missing FN defaults to Unknown",
            json: r#"{"email":"test@example.com"}"#,
            uid: None,
            must_contain: &["FN:Unknown"],
            must_not_contain: &[],
        },
        VcardRenderCase {
            label: "invalid JSON falls back to the defaults",
            json: "not json",
            uid: None,
            must_contain: &["BEGIN:VCARD", "FN:Unknown"],
            must_not_contain: &[],
        },
        VcardRenderCase {
            label: "an explicit UID override is emitted",
            json: r#"{"fn":"Test"}"#,
            uid: Some("custom-uid-12345"),
            must_contain: &["UID:custom-uid-12345"],
            must_not_contain: &[],
        },
        VcardRenderCase {
            label: "FN is vCard-escaped",
            json: r#"{"fn":"John; Doe","email":"test@example.com"}"#,
            uid: None,
            must_contain: &["FN:John\\; Doe"],
            must_not_contain: &[],
        },
        // Numbers/booleans are not strings, so `first_str` skips them and the
        // "Unknown" FN default kicks in.
        VcardRenderCase {
            label: "non-string field values are skipped by first_str",
            json: r#"{"name": 123, "email": 5, "phone": true}"#,
            uid: Some("u"),
            must_contain: &["FN:Unknown"],
            must_not_contain: &["EMAIL;", "TEL;"],
        },
    ]);
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
fn test_json_to_vcard_maps_natural_and_alias_field_names_to_vcard_lines() {
    // The LLM naturally sends `name`/`phone`/`company`/`title`/`notes`; the
    // original `fn`/`tel`/`org` names remain valid aliases, the canonical name
    // wins over its alias, empty strings count as "not provided", and TITLE /
    // NOTE / ORG / TEL are only emitted when a value exists (some CalDAV
    // servers reject empty lines).
    assert_vcard_render(vec![
        // Exactly the payload the LLM sends today: name, email, phone,
        // company, title, notes. Every field must be preserved.
        VcardRenderCase {
            label: "natural LLM payload keeps every field",
            json: r#"{
        "name": "Test User",
        "email": "test.user@example.com",
        "phone": "(555) 000-1111",
        "company": "Example Corp",
        "title": "Controller",
        "notes": "Handles payroll, timesheets, and compensation matters."
    }"#,
            uid: Some("test-uid"),
            must_contain: &[
                "FN:Test User",
                "EMAIL;TYPE=INTERNET:test.user@example.com",
                "TEL;TYPE=CELL:(555) 000-1111",
                "ORG:Example Corp",
                "TITLE:Controller",
                r"NOTE:Handles payroll\, timesheets\, and compensation matters.",
                "UID:test-uid",
            ],
            must_not_contain: &[],
        },
        // Original schema: `fn`, `tel`, `org`. Must still work for back-compat.
        VcardRenderCase {
            label: "legacy fn/tel/org aliases",
            json: r#"{"fn":"Jane Doe","email":"jane@example.com","tel":"+15551234567","org":"Acme"}"#,
            uid: None,
            must_contain: &[
                "FN:Jane Doe",
                "EMAIL;TYPE=INTERNET:jane@example.com",
                "TEL;TYPE=CELL:+15551234567",
                "ORG:Acme",
            ],
            // No empty TITLE/NOTE when those fields weren't provided.
            must_not_contain: &["TITLE:", "NOTE:"],
        },
        // Alternate names that the LLM might pick.
        VcardRenderCase {
            label: "mobile/organization/title aliases",
            json: r#"{"name":"Bob","mobile":"+1 555 9999","organization":"Initech","title":"PE"}"#,
            uid: None,
            must_contain: &["TEL;TYPE=CELL:+1 555 9999", "ORG:Initech", "TITLE:PE"],
            must_not_contain: &[],
        },
        // If both `name` and `fn` are present, `name` (canonical) wins.
        VcardRenderCase {
            label: "canonical name wins over the fn alias",
            json: r#"{"name":"Canonical Name","fn":"Legacy Name"}"#,
            uid: None,
            must_contain: &["FN:Canonical Name"],
            must_not_contain: &["Legacy Name"],
        },
        // Empty strings should be treated as "not provided" — we don't want
        // a contact with `ORG:` (empty) in the addressbook.
        VcardRenderCase {
            label: "empty strings are treated as not provided",
            json: r#"{"name":"X","email":"x@y.com","phone":"","company":""}"#,
            uid: None,
            must_contain: &["FN:X", "EMAIL;TYPE=INTERNET:x@y.com"],
            must_not_contain: &["TEL;", "ORG:", "TITLE:", "NOTE:"],
        },
        // vCard 3.0 escaping: `;` `,` `\n` `\\` all need backslash-escapes.
        VcardRenderCase {
            label: "special chars in title and note are escaped",
            json: r#"{"name":"E","title":"VP, Eng; Platform","notes":"Line1\nLine2; back\\slash"}"#,
            uid: None,
            must_contain: &[
                r"TITLE:VP\, Eng\; Platform",
                r"NOTE:Line1\nLine2\; back\\slash",
            ],
            must_not_contain: &[],
        },
        // Defensive: ensure we don't accidentally emit empty TITLE: or NOTE:
        // lines for missing fields (some CalDAV servers reject them).
        VcardRenderCase {
            label: "absent optional fields emit no lines",
            json: r#"{"name":"Plain","email":"p@e.com"}"#,
            uid: None,
            must_contain: &[],
            must_not_contain: &["TITLE:", "NOTE:", "ORG:", "TEL;"],
        },
        VcardRenderCase {
            label: "notes alias",
            json: r#"{"name":"N","notes":"via notes"}"#,
            uid: None,
            must_contain: &["NOTE:via notes"],
            must_not_contain: &[],
        },
        VcardRenderCase {
            label: "note alias",
            json: r#"{"name":"N","note":"via note"}"#,
            uid: None,
            must_contain: &["NOTE:via note"],
            must_not_contain: &[],
        },
    ]);
}

// =====================================================================
// CardDAV tool integration tests
// Note: These tests verify the functions handle empty/missing configurations.
// Full integration tests with mock servers require async network handling
// which is better suited for integration tests rather than unit tests.
// =====================================================================

#[test]
fn test_tool_search_contact_handles_empty_clients_gracefully() {
    let config = crate::config::AgentConfig::default();
    let cache = crate::tools::registry::cache::ToolCache::new();
    let uuid_gen = crate::utils::uuid::SystemUuidGenerator;
    let res = tool_search_contact(&config, Some("test"), None, &cache, &uuid_gen);

    assert!(res.is_ok());
    let response = res.unwrap();
    assert!(response.results.is_empty());
    assert_eq!(response.total, 0);
    assert_eq!(response.hint.as_deref(), Some("Final page."));
    assert!(response.cursor.is_none());
}

#[test]
fn test_tool_search_contact_uses_cursor_and_ignores_keyword() {
    let config = crate::config::AgentConfig::default();
    let cache = crate::tools::registry::cache::ToolCache::new();
    let uuid_gen = crate::utils::uuid::SystemUuidGenerator;

    let items: Vec<CardDavContactDetails> = (0..35)
        .map(|i| CardDavContactDetails {
            client: "c1".to_string(),
            id: format!("id_{i}"),
            href: format!("/h/{i}.vcf"),
            fn_name: Some(format!("Contact {i}")),
            email: None,
            tel: None,
            org: None,
            bday: None,
            addresses: Vec::new(),
            vcard: String::new(),
        })
        .collect();

    let page = cache
        .contact_search_sessions
        .create_session(items, &uuid_gen);
    let cursor = page.cursor.expect("session should have cursor");

    let res = tool_search_contact(
        &config,
        Some("ignored_keyword"),
        Some(cursor),
        &cache,
        &uuid_gen,
    )
    .expect("tool_search_contact should succeed with cursor and ignore keyword");
    assert_eq!(res.count, 3);
    assert_eq!(res.total, 35);
}

#[test]
fn test_tool_search_contact_rejects_a_missing_or_blank_keyword() {
    let config = crate::config::AgentConfig::default();
    let cache = crate::tools::registry::cache::ToolCache::new();
    let uuid_gen = crate::utils::uuid::SystemUuidGenerator;
    let cases = [
        ("no keyword at all", None),
        ("a whitespace-only keyword", Some("   ")),
    ];
    for (label, keyword) in cases {
        let err = tool_search_contact(&config, keyword, None, &cache, &uuid_gen).unwrap_err();
        assert!(
            err.contains("Give `keyword`"),
            "case '{label}': unexpected error: {err}"
        );
    }
}

#[test]
fn test_carddav_tools_empty_config() {
    let config = crate::config::AgentConfig::default();

    assert_eq!(
        tool_add_contact(&config, None, "{}").unwrap_err(),
        "No CardDAV clients configured."
    );
    assert_eq!(
        tool_update_contact(&config, "/c.vcf", None, "{}").unwrap_err(),
        "No CardDAV clients configured."
    );
    assert_eq!(
        tool_delete_contact(&config, "/c.vcf", None).unwrap_err(),
        "No CardDAV clients configured."
    );

    let get_res = tool_get_contact(&config, "/c.vcf").unwrap();
    assert_eq!(get_res.contact, None);
    assert!(get_res.errors.is_empty());
}

#[test]
fn test_search_contact_cursor_sessions() {
    let cache = crate::tools::registry::cache::ToolCache::new();
    let uuid_gen = crate::utils::uuid::SystemUuidGenerator;
    let contacts: Vec<CardDavContactDetails> = (1..=70)
        .map(|i| CardDavContactDetails {
            client: "c".to_string(),
            id: format!("/c/{}.vcf", i),
            href: format!("/c/{}.vcf", i),
            fn_name: Some(format!("Contact {}", i)),
            email: None,
            tel: None,
            org: None,
            bday: None,
            addresses: Vec::new(),
            vcard: String::new(),
        })
        .collect();

    let page1 = cache
        .contact_search_sessions
        .create_session(contacts, &uuid_gen);
    assert_eq!(page1.total, 70);
    assert_eq!(page1.items.len(), 32);
    assert!(page1.cursor.is_some());
    assert!(page1.hint.is_none());
    let cursor1 = page1.cursor.unwrap();

    let page2 = cache.contact_search_sessions.next_page(&cursor1).unwrap();
    assert_eq!(page2.total, 70);
    assert_eq!(page2.items.len(), 32);
    assert!(page2.cursor.is_some());
    assert!(page2.hint.is_none());
    let cursor2 = page2.cursor.unwrap();

    let page3 = cache.contact_search_sessions.next_page(&cursor2).unwrap();
    assert_eq!(page3.total, 70);
    assert_eq!(page3.items.len(), 6);
    assert!(page3.cursor.is_none());
    assert_eq!(page3.hint.as_deref(), Some("Final page."));
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
fn test_build_contact_put_path_always_inserts_exactly_one_separator() {
    let cases = [
        // FastMail-style addressbook href: ends with `/`.
        (
            "trailing slash",
            "https://caldav.fastmail.com/dav/addressbooks/user/me@example.com/Default/",
            "https://caldav.fastmail.com/dav/addressbooks/user/me@example.com/Default/1234567890.vcf",
        ),
        // Some servers / clients return hrefs without a trailing slash; the
        // helper must still produce a well-formed URL.
        (
            "no trailing slash",
            "https://caldav.fastmail.com/dav/addressbooks/user/me@example.com/Default",
            "https://caldav.fastmail.com/dav/addressbooks/user/me@example.com/Default/1234567890.vcf",
        ),
        // Defensive: tolerate accidental double trailing slashes.
        (
            "multiple trailing slashes",
            "https://example.com/dav/user/me/Default//",
            "https://example.com/dav/user/me/Default/1234567890.vcf",
        ),
        // An empty collection is pathological, but the helper should not
        // produce a leading `/` and confuse the underlying HTTP client.
        ("empty addressbook", "", "/1234567890.vcf"),
    ];
    for (label, book, expected) in cases {
        assert_eq!(
            build_contact_put_path(book, "1234567890"),
            expected,
            "case '{label}'"
        );
    }
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

/// One `extract_vcard_uid` case: the vCard body and the expected UID.
type UidCase<'a> = (&'a str, &'a str, Option<&'a str>);

#[test]
fn test_extract_vcard_uid_reads_the_uid_line_in_all_its_shapes() {
    // The value is unescaped on the way out (`UID:abc\,def` -> `abc,def`),
    // surrounding whitespace is trimmed, an empty value is still `Some("")`
    // (distinct from a missing UID), and only the first UID line wins.
    // NOTE: the "long value" case is labelled as folding in the original
    // test but its body carries no continuation line.
    let cases: Vec<UidCase<'_>> = vec![
        (
            "bare UID line",
            "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Alice\r\nUID:abc-123\r\nEMAIL:a@e.com\r\nEND:VCARD",
            Some("abc-123"),
        ),
        // UID;X-OTHER=foo:value — the parameters must not confuse the
        // property-name parser.
        (
            "property parameters before the colon",
            "BEGIN:VCARD\r\nUID;X-OTHER=foo:abc-456\r\nEND:VCARD",
            Some("abc-456"),
        ),
        (
            "long value",
            "BEGIN:VCARD\r\nUID:abc-789-second-line-here\r\nEND:VCARD",
            Some("abc-789-second-line-here"),
        ),
        (
            "no UID line",
            "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:No UID\r\nEND:VCARD",
            None,
        ),
        ("empty body", "", None),
        (
            "unescapes the value",
            "BEGIN:VCARD\r\nUID:abc\\,def\r\nEND:VCARD",
            Some("abc,def"),
        ),
        (
            "returns the first of duplicates",
            "BEGIN:VCARD\r\nUID:first\r\nUID:second\r\nEND:VCARD",
            Some("first"),
        ),
        (
            "empty value is Some(\"\")",
            "BEGIN:VCARD\r\nUID:\r\nEND:VCARD",
            Some(""),
        ),
        (
            "trims surrounding whitespace",
            "UID:  spaced-uid  \r\n",
            Some("spaced-uid"),
        ),
    ];
    for (label, body, expected) in cases {
        assert_eq!(
            extract_vcard_uid(body).as_deref(),
            expected,
            "case '{label}'"
        );
    }
}

// =====================================================================
// unescape_vcard_text tests
// =====================================================================

#[test]
fn test_unescape_vcard_text_unescapes_every_documented_case() {
    // Every escape rule in RFC 2426 must round-trip; unknown escapes and a
    // trailing `\` keep the backslash literal so no value is mangled or
    // panics.
    let cases = [
        ("plain text", "plain text"),
        (r"Back\\slash", r"Back\slash"),
        (r"Semi\;colon", "Semi;colon"),
        (r"Comma\,here", "Comma,here"),
        (r"New\nline", "New\nline"),
        (r"Big\Nline", "Big\nline"),
        (r"weird\x", r"weird\x"),
        (r"trailing\", r"trailing\"),
    ];
    for (input, expected) in cases {
        assert_eq!(unescape_vcard_text(input), expected, "input '{input}'");
    }
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
    let input = r#"{"name":"Test User","email":"p@example.com"}"#;
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
FN:Test User\r\n\
N:User;Test;;;\r\n\
NICKNAME:TJ\r\n\
BDAY:1990-01-01\r\n\
EMAIL;TYPE=WORK:test.user@example.com\r\n\
TEL;TYPE=CELL:(555) 000-1111\r\n\
URL:https://example.com\r\n\
X-SKILLS:payroll;tax;audit\r\n\
ORG:Example Corp\r\n\
TITLE:Controller\r\n\
NOTE:Handles payroll\\, timesheets\\, and compensation matters.\r\n\
UID:test-uid-456\r\n\
END:VCARD";

#[test]
fn test_merge_vcard_update_preserves_unknown_properties() {
    // Update only `title`. Everything else (N, NICKNAME, BDAY, URL,
    // X-SKILLS, NOTE, the FN, EMAIL, TEL, ORG) must remain untouched.
    let existing = parse_vcard_properties(FIXTURE_PAUL);
    let new_json = r#"{"title":"VP Controller"}"#;
    let merged = merge_vcard_update(&existing, new_json, Some("test-uid-456"));

    // New value applied
    assert!(merged.contains("TITLE:VP Controller"), "got:\n{merged}");
    // Preserved
    assert!(merged.contains("FN:Test User"));
    assert!(merged.contains("N:User;Test;;;"));
    assert!(merged.contains("NICKNAME:TJ"));
    assert!(merged.contains("BDAY:1990-01-01"));
    assert!(merged.contains("EMAIL;TYPE=WORK:test.user@example.com"));
    assert!(merged.contains("TEL;TYPE=CELL:(555) 000-1111"));
    assert!(merged.contains("URL:https://example.com"));
    assert!(merged.contains("X-SKILLS:payroll;tax;audit"));
    assert!(merged.contains("ORG:Example Corp"));
    assert!(merged.contains(r"NOTE:Handles payroll\, timesheets\, and compensation matters."));
    // UID
    assert!(merged.contains("UID:test-uid-456"));
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
    let new_json = r#"{"name":"Test J. User","phone":"+1-555-000-0199"}"#;
    let merged = merge_vcard_update(&existing, new_json, Some("test-uid-456"));

    assert!(merged.contains("FN:Test J. User"));
    assert!(merged.contains("TEL;TYPE=CELL:+1-555-000-0199"));
    // Preserved canonical fields
    assert!(merged.contains("EMAIL;TYPE=WORK:test.user@example.com"));
    assert!(merged.contains("ORG:Example Corp"));
    assert!(merged.contains("TITLE:Controller"));
    assert!(merged.contains(r"NOTE:Handles payroll\, timesheets\, and compensation matters."));
}

#[test]
fn test_merge_vcard_update_empty_value_is_treated_as_not_provided() {
    // The schema's first_str drops empty strings, so an LLM that sends
    // `"phone": ""` is saying "don't change the phone" — the existing
    // TEL must survive.
    let existing = parse_vcard_properties(FIXTURE_PAUL);
    let merged = merge_vcard_update(&existing, r#"{"phone":""}"#, Some("test-uid-456"));
    assert!(
        merged.contains("TEL;TYPE=CELL:(555) 000-1111"),
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
    let json = r#"{"name":"Test User","email":"test.user@example.com"}"#;
    let merged = merge_vcard_update(&existing, json, Some("test-uid-456"));
    // No fields dropped, no new values introduced.
    assert!(merged.contains("FN:Test User"));
    assert!(merged.contains("EMAIL;TYPE=WORK:test.user@example.com"));
    assert!(merged.contains("TEL;TYPE=CELL:(555) 000-1111"));
    assert!(merged.contains("BDAY:1990-01-01"));
    assert!(merged.contains("URL:https://example.com"));
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
    let data = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:A\r\nBDAY:1990-01-01\r\nEND:VCARD";
    let contact = parse_vcard("c", "/h", data);
    assert_eq!(contact.bday, Some("1990-01-01".to_string()));
}

#[test]
fn test_parse_vcard_missing_bday_is_none() {
    let data = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:A\r\nEND:VCARD";
    let contact = parse_vcard("c", "/h", data);
    assert_eq!(contact.bday, None);
}

#[test]
fn test_json_to_vcard_emits_bday_for_every_accepted_key() {
    // The LLM might send any of: birthday, bday, dob, date_of_birth. Without
    // one of those keys no BDAY line may appear.
    assert_vcard_render(vec![
        VcardRenderCase {
            label: "birthday",
            json: r#"{"name":"A","birthday":"1990-01-15"}"#,
            uid: Some("u"),
            must_contain: &["BDAY:1990-01-15"],
            must_not_contain: &[],
        },
        VcardRenderCase {
            label: "bday",
            json: r#"{"name":"A","bday":"1990-01-15"}"#,
            uid: None,
            must_contain: &["BDAY:1990-01-15"],
            must_not_contain: &[],
        },
        VcardRenderCase {
            label: "dob",
            json: r#"{"name":"A","dob":"1990-01-15"}"#,
            uid: None,
            must_contain: &["BDAY:1990-01-15"],
            must_not_contain: &[],
        },
        VcardRenderCase {
            label: "date_of_birth",
            json: r#"{"name":"A","date_of_birth":"1990-01-15"}"#,
            uid: None,
            must_contain: &["BDAY:1990-01-15"],
            must_not_contain: &[],
        },
        VcardRenderCase {
            label: "no bday key emits no line",
            json: r#"{"name":"A"}"#,
            uid: None,
            must_contain: &[],
            must_not_contain: &["BDAY:"],
        },
    ]);
}

#[test]
fn test_merge_vcard_update_replaces_bday_when_provided() {
    let body = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:A\r\nBDAY:1990-01-01\r\nEND:VCARD";
    let existing = parse_vcard_properties(body);
    let merged = merge_vcard_update(&existing, r#"{"birthday":"1990-12-31"}"#, Some("u"));
    assert!(merged.contains("BDAY:1990-12-31"));
    assert!(!merged.contains("BDAY:1990-01-01"));
}

#[test]
fn test_merge_vcard_update_preserves_bday_when_not_provided() {
    let body = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:A\r\nBDAY:1990-01-01\r\nEND:VCARD";
    let existing = parse_vcard_properties(body);
    let merged = merge_vcard_update(&existing, r#"{"name":"A2"}"#, Some("u"));
    assert!(merged.contains("BDAY:1990-01-01"));
}

#[test]
fn test_merge_vcard_update_collapses_duplicate_bdays() {
    // BDAY is single-valued; duplicates must collapse like FN/EMAIL/…
    let body = "BEGIN:VCARD\r\nBDAY:1990-01-01\r\nBDAY:1990-01-01\r\nEND:VCARD";
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
    assert_eq!(addr.addr_type, Some("HOME".to_string()));
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
    assert_eq!(contact.addresses[0].addr_type, Some("HOME".to_string()));
    assert_eq!(contact.addresses[0].city, Some("Springfield".to_string()));
    assert_eq!(contact.addresses[1].addr_type, Some("WORK".to_string()));
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
    assert_eq!(addr.addr_type, None);
    assert_eq!(addr.street, Some("123 Main".to_string()));
}

#[test]
fn test_json_to_vcard_renders_addresses_as_adr_lines() {
    assert_vcard_render(vec![
        VcardRenderCase {
            label: "addresses array",
            json: r#"{
        "name": "Alice",
        "addresses": [
            {"type": "home", "street": "1 Main", "city": "Town", "region": "ST", "postal_code": "00000", "country": "US"}
        ]
    }"#,
            uid: Some("u"),
            must_contain: &["ADR;TYPE=HOME:;;1 Main;Town;ST;00000;US"],
            must_not_contain: &[],
        },
        VcardRenderCase {
            label: "address object alias",
            json: r#"{
        "name": "Alice",
        "address": {"type": "work", "city": "Metropolis"}
    }"#,
            uid: Some("u"),
            // Only `city` is set; the other 6 ADR components are empty.
            must_contain: &["ADR;TYPE=WORK:;;;Metropolis;;;"],
            must_not_contain: &[],
        },
        // Convenience: a single-line address string is heuristically split
        // into the 5 standard components (street, city, region, postal_code,
        // country). The vCard ADR has 7 components (po-box, ext, then the 5
        // standard ones), so the first two are empty.
        VcardRenderCase {
            label: "address string form",
            json: r#"{
        "name": "Alice",
        "address": "123 Main St, Springfield, IL, 62701, USA"
    }"#,
            uid: Some("u"),
            must_contain: &["ADR:;;123 Main St;Springfield;IL;62701;USA"],
            must_not_contain: &[],
        },
        VcardRenderCase {
            label: "multiple addresses",
            json: r#"{
        "name": "Alice",
        "addresses": [
            {"type": "home", "city": "Springfield"},
            {"type": "work", "city": "Metropolis"}
        ]
    }"#,
            uid: Some("u"),
            must_contain: &[
                "ADR;TYPE=HOME:;;;Springfield;;;",
                "ADR;TYPE=WORK:;;;Metropolis;;;",
            ],
            must_not_contain: &[],
        },
        VcardRenderCase {
            label: "address without a type has no TYPE prefix",
            json: r#"{
        "name": "Alice",
        "address": {"city": "Springfield"}
    }"#,
            uid: Some("u"),
            // No TYPE= since the JSON didn't include a type.
            must_contain: &["ADR:;;;Springfield;;;"],
            must_not_contain: &["ADR;TYPE"],
        },
        VcardRenderCase {
            label: "no address emits no ADR line",
            json: r#"{"name":"A"}"#,
            uid: None,
            must_contain: &[],
            must_not_contain: &["ADR"],
        },
        VcardRenderCase {
            label: "empty addresses array emits no ADR line",
            json: r#"{"name":"A","addresses":[]}"#,
            uid: Some("u"),
            must_contain: &[],
            must_not_contain: &["ADR"],
        },
        VcardRenderCase {
            label: "blank-only address object emits an empty ADR",
            json: r#"{"name":"A","address":{"type":"  "}}"#,
            uid: Some("u"),
            // Seven empty components, no TYPE= parameter.
            must_contain: &["ADR:;;;;;;\r\n"],
            must_not_contain: &[],
        },
        VcardRenderCase {
            label: "every component of an address is escaped",
            json: r#"{"name":"A","addresses":[{"type":"home","street":"1; Main","city":"Town, TX"}]}"#,
            uid: Some("u"),
            // vCard 3.0 §3.2.2 orders ADR as
            // po-box;extended-address;street-address;locality;region;postal-code;country,
            // so street is the *third* slot: exactly two empty components precede
            // it and the four trailing ones are empty. The `;` inside the street
            // and the `,` inside the locality must both come back escaped.
            must_contain: &[r"ADR;TYPE=HOME:;;1\; Main;Town\, TX;;;"],
            must_not_contain: &[],
        },
    ]);
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
        r#"{"address":{"type":"work","city":"Example HQ","country":"US"}}"#,
        Some("test-uid-456"),
    );
    assert!(merged.contains("ADR;TYPE=WORK:;;;Example HQ;;;US"));
}

#[test]
fn test_merge_vcard_update_address_string_alias() {
    let existing = parse_vcard_properties(FIXTURE_PAUL);
    let merged = merge_vcard_update(
        &existing,
        r#"{"address":"123 Main St, Springfield, IL 62701"}"#,
        Some("test-uid-456"),
    );
    // Heuristic split: the 3 non-empty comma-separated parts fill 3
    // of the 7 ADR components; the rest (po_box, ext, postal_code,
    // country) stay empty. vCard 7-component ADR shape:
    // po_box;ext;street;city;region;postal_code;country
    assert!(merged.contains("ADR:;;123 Main St;Springfield;IL 62701;;"));
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
fn test_split_vcard_value_splits_on_unescaped_separators_only() {
    // The vCard ADR value uses `;` as the component separator AND uses `\;` to
    // embed a literal `;` inside a component, so the helper must not break on
    // the escaped one. An ADR with fewer than 7 components is valid, so the
    // split must not crash on short input.
    let cases: Vec<(&str, &str, Vec<&str>)> = vec![
        (
            "escaped separator stays inside a part",
            r"a\;b;c",
            vec![r"a\;b", "c"],
        ),
        (
            "short input is not padded",
            ";;only street",
            vec!["", "", "only street"],
        ),
        (
            "trailing backslash has no character to consume",
            "abc\\",
            vec!["abc\\"],
        ),
        (
            "escape consumes the next char even if it is the separator",
            r"a\;b;c\;d",
            vec![r"a\;b", r"c\;d"],
        ),
        ("empty string yields one empty part", "", vec![""]),
    ];
    for (label, input, expected) in cases {
        assert_eq!(split_vcard_value(input, ';'), expected, "case '{label}'");
    }
}

// =====================================================================
// WireMock integration tests for CardDAV agent tools
// =====================================================================

use wiremock::matchers::{method, path as wm_path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct WiremockGuard {
    server: MockServer,
    _runtime: tokio::runtime::Runtime,
}

impl WiremockGuard {
    fn start() -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");
        let server = runtime.block_on(MockServer::start());
        Self {
            server,
            _runtime: runtime,
        }
    }

    fn uri(&self) -> String {
        self.server.uri()
    }

    fn register(&self, mock: Mock) {
        self._runtime.block_on(self.server.register(mock));
    }
}

const CARD_PROPFIND_BODY: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:carddav">
 <d:response>
  <d:href>/addressbooks/primary/</d:href>
  <d:propstat>
   <d:prop>
    <d:resourcetype><d:collection/><c:addressbook/></d:resourcetype>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>
</d:multistatus>"#;

const CARD_REPORT_BODY: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:carddav">
 <d:response>
  <d:href>/addressbooks/primary/alice.vcf</d:href>
  <d:propstat>
   <d:prop>
    <c:address-data>BEGIN:VCARD
VERSION:3.0
FN:Alice Example
UID:alice-1
EMAIL:alice@example.com
TEL:555-1234
END:VCARD</c:address-data>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>
</d:multistatus>"#;

const ALICE_VCARD_BODY: &str = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Alice Example\r\nUID:alice-1\r\nEMAIL:alice@example.com\r\nTEL:555-1234\r\nEND:VCARD";

fn register_carddav_tool_stubs(mock: &WiremockGuard) {
    mock.register(
        Mock::given(method("PROPFIND")).respond_with(
            ResponseTemplate::new(207)
                .insert_header("content-type", "application/xml")
                .set_body_string(CARD_PROPFIND_BODY),
        ),
    );
    mock.register(
        Mock::given(method("REPORT")).respond_with(
            ResponseTemplate::new(207)
                .insert_header("content-type", "application/xml")
                .set_body_string(CARD_REPORT_BODY),
        ),
    );
    mock.register(
        Mock::given(method("GET"))
            .and(wm_path("/addressbooks/primary/alice.vcf"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/vcard")
                    .set_body_string(ALICE_VCARD_BODY),
            ),
    );
    mock.register(
        Mock::given(method("GET"))
            .and(wm_path("/notfound.vcf"))
            .respond_with(
                ResponseTemplate::new(404)
                    .insert_header("content-type", "text/plain")
                    .set_body_string("Not Found"),
            ),
    );
    mock.register(
        Mock::given(method("PUT")).respond_with(
            ResponseTemplate::new(201)
                .insert_header("ETag", "\"tag123\"")
                .set_body_string(""),
        ),
    );
    mock.register(
        Mock::given(method("DELETE"))
            .and(wm_path("/addressbooks/primary/alice.vcf"))
            .respond_with(ResponseTemplate::new(204).set_body_string("")),
    );
    mock.register(
        Mock::given(method("DELETE"))
            .and(wm_path("/fail.vcf"))
            .respond_with(
                ResponseTemplate::new(500)
                    .insert_header("content-type", "text/plain")
                    .set_body_string("Error"),
            ),
    );
}

fn card_config_for(uri: String) -> crate::config::AgentConfig {
    let mut config = crate::config::AgentConfig::default();
    config.caldav_clients.insert(
        "mock_card".to_string(),
        crate::config::CalDavClient {
            url: uri,
            username: "user".to_string(),
            password: "password".to_string(),
        },
    );
    config
}

#[test]
fn test_carddav_tools_mock_server() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock = WiremockGuard::start();
    register_carddav_tool_stubs(&mock);
    let config = card_config_for(mock.uri());
    let cache = crate::tools::registry::cache::ToolCache::new();
    let uuid_gen = crate::utils::uuid::SystemUuidGenerator;

    // 1. Search contact
    let search_res = tool_search_contact(&config, Some("Alice"), None, &cache, &uuid_gen).unwrap();
    assert!(
        search_res
            .results
            .iter()
            .any(|c| c.fn_name.as_deref() == Some("Alice Example"))
    );

    // 2. Get contact success
    let get_res = tool_get_contact(&config, "/addressbooks/primary/alice.vcf").unwrap();
    assert_eq!(
        get_res.contact.as_ref().and_then(|c| c.fn_name.as_deref()),
        Some("Alice Example")
    );

    // 3. Get contact 404
    let get_res_404 = tool_get_contact(&config, "/notfound.vcf").unwrap();
    assert!(get_res_404.contact.is_none());
    assert!(
        get_res_404
            .errors
            .iter()
            .any(|e| e.contains("Not found by href"))
    );

    // 4. Add contact
    let add_res = tool_add_contact(
        &config,
        None,
        r#"{"name":"Bob Example","email":"b@example.com"}"#,
    )
    .unwrap();
    assert!(add_res.result.contains("Created at /addressbooks/primary/"));

    // 5. Update contact success
    let update_res = tool_update_contact(
        &config,
        "/addressbooks/primary/alice.vcf",
        None,
        r#"{"email":"new@example.com"}"#,
    )
    .unwrap();
    assert!(update_res.result.contains("Updated"));

    // 6. Delete contact success
    let delete_res = tool_delete_contact(&config, "/addressbooks/primary/alice.vcf", None).unwrap();
    assert!(delete_res.result.contains("Deleted"));

    // 7. Delete contact 500 error
    let delete_res_err = tool_delete_contact(&config, "/fail.vcf", None).unwrap();
    assert!(delete_res_err.result.contains("Failed to DELETE"));
}

#[test]
fn test_carddav_tools_targeted_client_routing() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock_personal = WiremockGuard::start();
    let mock_work = WiremockGuard::start();
    register_carddav_tool_stubs(&mock_personal);
    register_carddav_tool_stubs(&mock_work);

    let mut config = crate::config::AgentConfig::default();
    config.caldav_clients.insert(
        "personal".to_string(),
        crate::config::CalDavClient {
            url: mock_personal.uri(),
            username: "user".to_string(),
            password: "password".to_string(),
        },
    );
    config.caldav_clients.insert(
        "work".to_string(),
        crate::config::CalDavClient {
            url: mock_work.uri(),
            username: "user".to_string(),
            password: "password".to_string(),
        },
    );

    // 1. Add contact targeted to work
    let add_res = tool_add_contact(&config, Some("work"), r#"{"name":"Work Colleague"}"#).unwrap();
    assert!(add_res.result.contains("--- Client: work ---"));
    assert!(!add_res.result.contains("--- Client: personal ---"));

    // 2. Add contact targeted to invalid client
    let add_err = tool_add_contact(&config, Some("nonexistent"), r#"{"name":"X"}"#).unwrap_err();
    assert!(add_err.contains("CardDAV client 'nonexistent' not found"));

    // 3. Update contact targeted to personal
    let update_res = tool_update_contact(
        &config,
        "/addressbooks/primary/alice.vcf",
        Some("personal"),
        r#"{"email":"alice@p.com"}"#,
    )
    .unwrap();
    assert!(update_res.result.contains("--- Client: personal ---"));
    assert!(!update_res.result.contains("--- Client: work ---"));

    // 4. Update contact targeted to invalid client
    let update_err = tool_update_contact(
        &config,
        "/addressbooks/primary/alice.vcf",
        Some("nonexistent"),
        r#"{"email":"x@x.com"}"#,
    )
    .unwrap_err();
    assert!(update_err.contains("CardDAV client 'nonexistent' not found"));

    // 5. Delete contact targeted to work
    let delete_res =
        tool_delete_contact(&config, "/addressbooks/primary/alice.vcf", Some("work")).unwrap();
    assert!(delete_res.result.contains("--- Client: work ---"));
    assert!(!delete_res.result.contains("--- Client: personal ---"));

    // 6. Delete contact targeted to invalid client
    let delete_err = tool_delete_contact(
        &config,
        "/addressbooks/primary/alice.vcf",
        Some("nonexistent"),
    )
    .unwrap_err();
    assert!(delete_err.contains("CardDAV client 'nonexistent' not found"));
}

#[test]
fn test_carddav_tools_multi_server_404_suppression() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock_found = WiremockGuard::start();
    let mock_missing = WiremockGuard::start();

    register_carddav_tool_stubs(&mock_found);

    mock_missing.register(
        Mock::given(method("PROPFIND")).respond_with(
            ResponseTemplate::new(207)
                .insert_header("content-type", "application/xml")
                .set_body_string(CARD_PROPFIND_BODY),
        ),
    );
    mock_missing.register(
        Mock::given(method("GET"))
            .and(wm_path("/addressbooks/primary/alice.vcf"))
            .respond_with(ResponseTemplate::new(404).set_body_string("Not Found")),
    );
    mock_missing.register(
        Mock::given(method("DELETE"))
            .and(wm_path("/addressbooks/primary/alice.vcf"))
            .respond_with(ResponseTemplate::new(404).set_body_string("Not Found")),
    );

    let mut config = crate::config::AgentConfig::default();
    config.caldav_clients.insert(
        "found_client".to_string(),
        crate::config::CalDavClient {
            url: mock_found.uri(),
            username: "user".to_string(),
            password: "password".to_string(),
        },
    );
    config.caldav_clients.insert(
        "missing_client".to_string(),
        crate::config::CalDavClient {
            url: mock_missing.uri(),
            username: "user".to_string(),
            password: "password".to_string(),
        },
    );

    // 1. Get contact: found_client succeeds, missing_client returns 404.
    // 404 error from missing_client should be suppressed!
    let get_res = tool_get_contact(&config, "/addressbooks/primary/alice.vcf").unwrap();
    assert_eq!(
        get_res.contact.as_ref().and_then(|c| c.fn_name.as_deref()),
        Some("Alice Example")
    );
    assert!(
        get_res.errors.is_empty(),
        "404 error should be suppressed, got: {:?}",
        get_res.errors
    );

    // 2. Update contact: found_client succeeds, missing_client returns 404.
    // Result should contain found_client success, and missing_client 404 is suppressed!
    let update_res = tool_update_contact(
        &config,
        "/addressbooks/primary/alice.vcf",
        None,
        r#"{"email":"updated@a.com"}"#,
    )
    .unwrap();
    assert!(update_res.result.contains("--- Client: found_client ---"));
    assert!(update_res.result.contains("Updated"));
    assert!(
        !update_res.result.contains("missing_client"),
        "404 from missing_client should be suppressed, got: {}",
        update_res.result
    );

    // 3. Delete contact: found_client succeeds, missing_client returns 404.
    // Result should contain found_client success, and missing_client 404 is suppressed!
    let delete_res = tool_delete_contact(&config, "/addressbooks/primary/alice.vcf", None).unwrap();
    assert!(delete_res.result.contains("--- Client: found_client ---"));
    assert!(delete_res.result.contains("Deleted"));
    assert!(
        !delete_res.result.contains("missing_client"),
        "404 from missing_client should be suppressed, got: {}",
        delete_res.result
    );

    // 4. Update when BOTH fail with 404: error is NOT suppressed.
    let update_all_fail =
        tool_update_contact(&config, "/notfound.vcf", None, r#"{"email":"x@x.com"}"#).unwrap();
    assert!(update_all_fail.result.contains("found_client"));
    assert!(update_all_fail.result.contains("missing_client"));
}

// =====================================================================
// extract_vcard_uid — remaining branches
// =====================================================================

/// Contract: a line without a colon is not a vCard property, so it is skipped and
/// the scan keeps going — a `UID` later in the body is still found. (Was: the
/// first colonless line aborted the whole scan and the function reported "no
/// UID", unlike the sibling parser `parse_vcard_properties`.)
#[test]
fn extract_vcard_uid_skips_colonless_lines_and_finds_a_later_uid() {
    let body = "BEGIN:VCARD\r\nGARBAGE WITHOUT COLON\r\nUID:abc-123\r\nEND:VCARD";
    assert_eq!(extract_vcard_uid(body), Some("abc-123".to_string()));

    // The same body with a well-formed filler line finds the UID as well.
    let clean = "BEGIN:VCARD\r\nFN:x\r\nUID:abc-123\r\nEND:VCARD";
    assert_eq!(extract_vcard_uid(clean), Some("abc-123".to_string()));

    // Skipping is not inventing: a body without any UID stays `None` even when
    // every line is unusable.
    let no_uid = "BEGIN:VCARD\r\nGARBAGE WITHOUT COLON\r\nMORE GARBAGE\r\nEND:VCARD";
    assert_eq!(extract_vcard_uid(no_uid), None);
}

// =====================================================================
// parse_vcard_properties — remaining branches
// =====================================================================

#[test]
fn test_parse_vcard_properties_keeps_only_named_non_structural_lines() {
    // Both the BEGIN/VERSION/END envelope and a line with no property name
    // at all (`:novalue`) must be dropped rather than becoming a property.
    let cases: Vec<(&str, &str, &[&str])> = vec![
        (
            "structural lines",
            "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:abc\r\nFN:Alice\r\nEND:VCARD",
            &["FN"],
        ),
        (
            "line with an empty name",
            "BEGIN:VCARD\r\n:novalue\r\nFN:Alice\r\nEND:VCARD",
            &["FN"],
        ),
    ];
    for (label, body, expected) in cases {
        let props = parse_vcard_properties(body);
        let names: Vec<&str> = props.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names.as_slice(), expected, "case '{label}'");
    }
}

#[test]
fn test_parse_vcard_properties_trims_and_uppercases_the_name() {
    // The name is taken before the first `;`, trimmed and upper-cased,
    // while the prefix keeps the original spacing and parameters.
    let body = "BEGIN:VCARD\r\nfn :Alice\r\nemail ;TYPE=WORK:a@b.com\r\nEND:VCARD";
    let props = parse_vcard_properties(body);
    assert_eq!(props.len(), 2, "got: {props:?}");
    assert_eq!(props[0].name, "FN");
    assert_eq!(props[0].prefix, "fn ");
    assert_eq!(props[1].name, "EMAIL");
    assert_eq!(props[1].prefix, "email ;TYPE=WORK");
}

#[test]
fn test_parse_vcard_properties_merges_folded_continuations() {
    let body = "BEGIN:VCARD\r\nNOTE:Line1\r\n Line2\r\nEND:VCARD";
    let props = parse_vcard_properties(body);
    assert_eq!(props.len(), 1);
    assert_eq!(props[0].name, "NOTE");
    assert_eq!(props[0].line, "NOTE:Line1Line2");
}

#[test]
fn test_parse_vcard_properties_on_empty_body() {
    assert!(parse_vcard_properties("").is_empty());
}

// =====================================================================
// unfold_vcard — remaining branches
// =====================================================================

#[test]
fn test_unfold_vcard_returns_the_expected_text_for_each_body() {
    let cases = [
        // A body that *starts* with a fold marker has nothing to fold into; the
        // helper must not emit a stray leading newline. Unfolding removes exactly
        // ONE whitespace as the fold marker (vCard 3.0 §2.6, per `unfold_vcard`),
        // so the second space is payload and is kept.
        ("leading continuation", "  orphan", " orphan"),
        ("empty body", "", ""),
        // `str::lines` already drops `\r` before the `\n`.
        ("carriage returns", "A:1\r\nB:2\r\n", "A:1\nB:2"),
    ];
    for (label, body, expected) in cases {
        assert_eq!(unfold_vcard(body), expected, "case '{label}'");
    }
}

// =====================================================================
// log_truncate — boundary
// =====================================================================

#[test]
fn test_log_truncate_at_exactly_the_limit_is_not_truncated() {
    let body = vec![b'a'; LOG_BODY_LIMIT];
    let out = log_truncate(&body);
    assert_eq!(out.len(), LOG_BODY_LIMIT);
    assert!(!out.contains("truncated"), "got: {out}");
}

#[test]
fn test_log_truncate_one_byte_over_the_limit_reports_the_total() {
    let body = vec![b'b'; LOG_BODY_LIMIT + 1];
    let out = log_truncate(&body);
    assert!(
        out.ends_with(&format!(
            "...<truncated, total {} bytes>",
            LOG_BODY_LIMIT + 1
        )),
        "got: {out}"
    );
    assert!(
        out.starts_with(&"b".repeat(LOG_BODY_LIMIT)),
        "the first {LOG_BODY_LIMIT} bytes must survive verbatim"
    );
}

// =====================================================================
// parse_address_from_string tests
// =====================================================================

#[test]
fn test_parse_address_from_string_uses_only_the_first_five_parts() {
    let addr = parse_address_from_string("a, b, c, d, e, f");
    assert_eq!(addr.street.as_deref(), Some("a"));
    assert_eq!(addr.city.as_deref(), Some("b"));
    assert_eq!(addr.region.as_deref(), Some("c"));
    assert_eq!(addr.postal_code.as_deref(), Some("d"));
    assert_eq!(addr.country.as_deref(), Some("e"));
    assert_eq!(addr.kind, None);
    assert_eq!(addr.po_box, None);
    assert_eq!(addr.ext, None);
}

#[test]
fn test_parse_address_from_string_single_part_fills_street_only() {
    let addr = parse_address_from_string("123 Main St");
    assert_eq!(addr.street.as_deref(), Some("123 Main St"));
    assert_eq!(addr.city, None);
    assert_eq!(addr.region, None);
    assert_eq!(addr.postal_code, None);
    assert_eq!(addr.country, None);
}

#[test]
fn test_parse_address_from_string_all_empty_parts_yield_no_components() {
    let addr = parse_address_from_string(",,,");
    assert_eq!(addr.street, None);
    assert_eq!(addr.city, None);
    assert_eq!(addr.region, None);
    assert_eq!(addr.postal_code, None);
    assert_eq!(addr.country, None);
}

// =====================================================================
// parse_addresses_from_json tests
// =====================================================================

#[test]
fn test_parse_addresses_from_json_returns_none_when_no_address_can_be_read() {
    // Every case must report "nothing provided" (`None`) so the caller leaves
    // existing ADR lines alone.
    let cases: Vec<(&str, serde_json::Value)> = vec![
        ("no address keys", serde_json::json!({ "name": "A" })),
        // Every element is filtered out, the array result stays empty and the
        // helper falls through to the other accepted forms — of which there
        // are none, so the answer is `None` (leave existing ADRs alone).
        (
            "non-object array items",
            serde_json::json!({ "addresses": ["just a string", 7] }),
        ),
        (
            "a non-array addresses value",
            serde_json::json!({ "addresses": "123 Main St" }),
        ),
        (
            "a blank address string",
            serde_json::json!({ "address": "   " }),
        ),
    ];
    for (label, parsed) in cases {
        assert_eq!(parse_addresses_from_json(&parsed), None, "case '{label}'");
    }
}

#[test]
fn test_parse_addresses_from_json_empty_array_is_an_explicit_clear() {
    // An explicit `[]` wins over a sibling `address` string.
    let parsed = serde_json::json!({ "addresses": [], "address": "1 Main" });
    assert_eq!(
        parse_addresses_from_json(&parsed),
        Some(Vec::<StructuredAddress>::new())
    );
}

#[test]
fn test_parse_addresses_from_json_object_with_only_blank_fields_yields_an_empty_address() {
    let parsed = serde_json::json!({ "address": { "type": "  ", "city": "" } });
    let out = parse_addresses_from_json(&parsed).expect("object form yields one address");
    assert_eq!(out.len(), 1);
    assert_eq!(
        out[0],
        StructuredAddress {
            kind: None,
            po_box: None,
            ext: None,
            street: None,
            city: None,
            region: None,
            postal_code: None,
            country: None,
        }
    );
}

#[test]
fn test_parse_addresses_from_json_object_form_uppercases_the_type() {
    let parsed = serde_json::json!({ "address": { "type": "home", "street": "1 Main" } });
    let out = parse_addresses_from_json(&parsed).expect("one address");
    assert_eq!(out[0].kind.as_deref(), Some("HOME"));
    assert_eq!(out[0].street.as_deref(), Some("1 Main"));
}

#[test]
fn test_parse_addresses_from_json_array_form_skips_the_blank_entries() {
    // A non-object element is dropped, the object element survives.
    let parsed = serde_json::json!({ "addresses": [ "junk", { "city": "Town" } ] });
    let out = parse_addresses_from_json(&parsed).expect("one address");
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].city.as_deref(), Some("Town"));
}

// =====================================================================
// merge_vcard_update — remaining branches
// =====================================================================

#[test]
fn test_merge_vcard_update_invalid_json_preserves_everything() {
    let body = "BEGIN:VCARD\r\n\
                VERSION:3.0\r\n\
                FN:Alice\r\n\
                EMAIL;a@b.com\r\n\
                X-CUSTOM:keep\r\n\
                END:VCARD";
    let existing = parse_vcard_properties(body);
    let merged = merge_vcard_update(&existing, "this is not json", Some("u"));
    assert!(merged.contains("FN:Alice"), "got:\n{merged}");
    assert!(merged.contains("X-CUSTOM:keep"), "got:\n{merged}");
    assert!(merged.contains("UID:u"), "got:\n{merged}");
}

#[test]
fn test_merge_vcard_update_appends_new_canonical_fields_with_plain_prefixes() {
    // None of these existed, so each gets its bare name as prefix —
    // except EMAIL and TEL which get the standard TYPE defaults.
    let merged = merge_vcard_update(
        &[],
        r#"{"name":"A","email":"a@b.com","phone":"+1","company":"Acme",
            "title":"Boss","notes":"hi","birthday":"1990-01-01"}"#,
        None,
    );
    assert!(merged.contains("FN:A\r\n"), "got:\n{merged}");
    assert!(
        merged.contains("EMAIL;TYPE=INTERNET:a@b.com\r\n"),
        "got:\n{merged}"
    );
    assert!(merged.contains("TEL;TYPE=CELL:+1\r\n"), "got:\n{merged}");
    assert!(merged.contains("ORG:Acme\r\n"), "got:\n{merged}");
    assert!(merged.contains("TITLE:Boss\r\n"), "got:\n{merged}");
    assert!(merged.contains("NOTE:hi\r\n"), "got:\n{merged}");
    assert!(merged.contains("BDAY:1990-01-01\r\n"), "got:\n{merged}");
    // No UID line when the caller passes None.
    assert!(!merged.contains("UID:"), "got:\n{merged}");
}

#[test]
fn test_merge_vcard_update_collapses_duplicates_for_every_canonical_name() {
    let body = "BEGIN:VCARD\r\n\
                FN:one\r\nFN:two\r\n\
                TEL:1\r\nTEL:2\r\n\
                ORG:o1\r\nORG:o2\r\n\
                TITLE:t1\r\nTITLE:t2\r\n\
                NOTE:n1\r\nNOTE:n2\r\n\
                END:VCARD";
    let existing = parse_vcard_properties(body);
    let merged = merge_vcard_update(
        &existing,
        r#"{"name":"NEWFN","phone":"NEWTEL","company":"NEWORG",
            "title":"NEWTITLE","notes":"NEWNOTE"}"#,
        Some("u"),
    );
    for (case, line) in [
        ("FN", "FN:NEWFN"),
        ("TEL", "TEL:NEWTEL"),
        ("ORG", "ORG:NEWORG"),
        ("TITLE", "TITLE:NEWTITLE"),
        ("NOTE", "NOTE:NEWNOTE"),
    ] {
        let hits = merged.lines().filter(|l| l.starts_with(line)).count();
        assert_eq!(hits, 1, "{case} should appear exactly once, got:\n{merged}");
    }
    // The first original prefix is reused, so the TYPE= parameters stay.
    assert!(!merged.contains("two"), "got:\n{merged}");
}

#[test]
fn test_merge_vcard_update_keeps_adrs_when_addresses_is_not_a_list() {
    // `"addresses": "x"` is not an array, so no replacement happens and
    // the existing ADR survives.
    let body = "BEGIN:VCARD\r\nFN:A\r\nADR;TYPE=HOME:;;1 Main;Town;;;Country\r\nEND:VCARD";
    let existing = parse_vcard_properties(body);
    let merged = merge_vcard_update(&existing, r#"{"addresses":"x"}"#, Some("u"));
    assert!(
        merged.contains("ADR;TYPE=HOME:;;1 Main;Town;;;Country"),
        "got:\n{merged}"
    );
}

#[test]
fn test_merge_vcard_update_escapes_values_it_appends() {
    let merged = merge_vcard_update(&[], r#"{"name":"A; B","notes":"x\ny"}"#, Some("u"));
    assert!(merged.contains("FN:A\\; B"), "got:\n{merged}");
    assert!(merged.contains(r"NOTE:x\ny"), "got:\n{merged}");
}

// =====================================================================
// StructuredAddress <-> AddressInput conversions
// =====================================================================

#[test]
fn test_structured_address_converts_into_address_input() {
    let addr = StructuredAddress {
        kind: Some("HOME".to_string()),
        po_box: Some("PO 1".to_string()),
        ext: Some("Ext 2".to_string()),
        street: Some("1 Main".to_string()),
        city: Some("Town".to_string()),
        region: Some("ST".to_string()),
        postal_code: Some("00000".to_string()),
        country: Some("Country".to_string()),
    };
    let input: crate::tools::dtos::AddressInput = addr.into();
    assert_eq!(input.addr_type.as_deref(), Some("HOME"));
    assert_eq!(input.po_box.as_deref(), Some("PO 1"));
    assert_eq!(input.ext.as_deref(), Some("Ext 2"));
    assert_eq!(input.street.as_deref(), Some("1 Main"));
    assert_eq!(input.city.as_deref(), Some("Town"));
    assert_eq!(input.region.as_deref(), Some("ST"));
    assert_eq!(input.postal_code.as_deref(), Some("00000"));
    assert_eq!(input.country.as_deref(), Some("Country"));
}

#[test]
fn test_address_input_converts_into_structured_address() {
    let input = crate::tools::dtos::AddressInput {
        addr_type: Some("WORK".to_string()),
        street: Some("2 Office".to_string()),
        city: Some("City".to_string()),
        region: Some("NY".to_string()),
        postal_code: Some("10001".to_string()),
        country: Some("USA".to_string()),
        po_box: None,
        ext: None,
    };
    let addr = StructuredAddress::from(input);
    assert_eq!(addr.kind.as_deref(), Some("WORK"));
    assert_eq!(addr.street.as_deref(), Some("2 Office"));
    assert_eq!(addr.city.as_deref(), Some("City"));
    assert_eq!(addr.region.as_deref(), Some("NY"));
    assert_eq!(addr.postal_code.as_deref(), Some("10001"));
    assert_eq!(addr.country.as_deref(), Some("USA"));
    assert_eq!(addr.po_box, None);
    assert_eq!(addr.ext, None);
}

#[test]
fn test_structured_address_round_trips_through_address_input() {
    let original = StructuredAddress {
        kind: Some("HOME".to_string()),
        po_box: None,
        ext: Some("Apt 3".to_string()),
        street: Some("1 Main".to_string()),
        city: None,
        region: Some("ST".to_string()),
        postal_code: None,
        country: Some("Country".to_string()),
    };
    let back = StructuredAddress::from(crate::tools::dtos::AddressInput::from(original.clone()));
    assert_eq!(back, original);
}

// =====================================================================
// get_all_addressbooks discovery strategies
//
// The helper walks four strategies in order: direct PROPFIND on the base
// URL, addressbook-home-set on the base URL, current-user-principal, and
// finally a Fastmail-style `/dav/principals/user/{username}/` guess.
// =====================================================================

use crate::tools::blocking::block_on;

/// A multistatus with no `<d:response>` children: every collection
// parser returns an empty list for it.
const EMPTY_MULTISTATUS: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:carddav">
</d:multistatus>"#;

fn xml_207(body: &str) -> ResponseTemplate {
    ResponseTemplate::new(207)
        .insert_header("content-type", "application/xml")
        .set_body_string(body.to_string())
}

fn propfind_at(mock: &WiremockGuard, path: &str, body: &str) {
    mock.register(
        Mock::given(method("PROPFIND"))
            .and(wm_path(path))
            .respond_with(xml_207(body)),
    );
}

fn addressbook_home_set(href: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:carddav">
 <d:response><d:propstat><d:prop>
  <c:addressbook-home-set><d:href>{href}</d:href></c:addressbook-home-set>
 </d:prop></d:propstat></d:response>
</d:multistatus>"#
    )
}

fn current_user_principal(href: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:carddav">
 <d:response><d:propstat><d:prop>
  <d:current-user-principal><d:href>{href}</d:href></d:current-user-principal>
 </d:prop></d:propstat></d:response>
</d:multistatus>"#
    )
}

fn client_at(uri: String) -> crate::lib::dav::client::DavClient {
    let cfg = crate::config::CalDavClient {
        url: uri,
        username: "user".to_string(),
        password: "password".to_string(),
    };
    crate::lib::dav::client::DavClient::new("mock_card".to_string(), &cfg).expect("build client")
}

#[test]
fn test_get_all_addressbooks_direct_propfind_strategy() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock = WiremockGuard::start();
    propfind_at(&mock, "/", CARD_PROPFIND_BODY);
    let client = client_at(mock.uri());

    let books = block_on(get_all_addressbooks(&client, &mock.uri(), "user")).unwrap();
    assert_eq!(books, vec!["/addressbooks/primary/".to_string()]);
}

#[test]
fn test_get_all_addressbooks_home_set_strategy() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock = WiremockGuard::start();
    // Base URL advertises a home set but is not itself an addressbook.
    propfind_at(&mock, "/", &addressbook_home_set("/addressbooks/user/"));
    propfind_at(&mock, "/addressbooks/user/", CARD_PROPFIND_BODY);
    let client = client_at(mock.uri());

    let books = block_on(get_all_addressbooks(&client, &mock.uri(), "user")).unwrap();
    assert_eq!(books, vec!["/addressbooks/primary/".to_string()]);
}

#[test]
fn test_get_all_addressbooks_principal_strategy() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock = WiremockGuard::start();
    // Base URL yields neither an addressbook nor a home set, only a principal.
    propfind_at(&mock, "/", &current_user_principal("/principals/user/"));
    propfind_at(
        &mock,
        "/principals/user/",
        &addressbook_home_set("/addressbooks/user/"),
    );
    propfind_at(&mock, "/addressbooks/user/", CARD_PROPFIND_BODY);
    let client = client_at(mock.uri());

    let books = block_on(get_all_addressbooks(&client, &mock.uri(), "user")).unwrap();
    assert_eq!(books, vec!["/addressbooks/primary/".to_string()]);
}

#[test]
fn test_get_all_addressbooks_fastmail_guess_strategy() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock = WiremockGuard::start();
    // Nothing discoverable on the base URL, but the guessed
    // /dav/principals/user/{username}/ path advertises a home set.
    propfind_at(&mock, "/", EMPTY_MULTISTATUS);
    propfind_at(
        &mock,
        "/dav/principals/user/user/",
        &addressbook_home_set("/addressbooks/user/"),
    );
    propfind_at(&mock, "/addressbooks/user/", CARD_PROPFIND_BODY);
    let client = client_at(mock.uri());

    let books = block_on(get_all_addressbooks(&client, &mock.uri(), "user")).unwrap();
    assert_eq!(books, vec!["/addressbooks/primary/".to_string()]);
}

#[test]
fn test_get_all_addressbooks_errors_when_no_principal_is_found() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock = WiremockGuard::start();
    propfind_at(&mock, "/", EMPTY_MULTISTATUS);
    propfind_at(&mock, "/dav/principals/user/user/", EMPTY_MULTISTATUS);
    let client = client_at(mock.uri());

    let err = block_on(get_all_addressbooks(&client, &mock.uri(), "user")).unwrap_err();
    assert_eq!(err.to_string(), "No principal found");
}

#[test]
fn test_get_all_addressbooks_errors_when_home_set_is_empty() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock = WiremockGuard::start();
    // A principal exists but has no addressbook-home-set.
    propfind_at(&mock, "/", &current_user_principal("/principals/user/"));
    propfind_at(&mock, "/principals/user/", EMPTY_MULTISTATUS);
    let client = client_at(mock.uri());

    let err = block_on(get_all_addressbooks(&client, &mock.uri(), "user")).unwrap_err();
    assert_eq!(err.to_string(), "No addressbook home found");
}

#[test]
fn test_get_all_addressbooks_returns_an_empty_list_when_the_home_has_no_books() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock = WiremockGuard::start();
    propfind_at(&mock, "/", &current_user_principal("/principals/user/"));
    propfind_at(
        &mock,
        "/principals/user/",
        &addressbook_home_set("/addressbooks/user/"),
    );
    propfind_at(&mock, "/addressbooks/user/", EMPTY_MULTISTATUS);
    let client = client_at(mock.uri());

    let books = block_on(get_all_addressbooks(&client, &mock.uri(), "user")).unwrap();
    assert!(books.is_empty(), "got: {books:?}");
}

#[test]
fn test_fetch_contacts_from_book_wrapper_delegates_to_the_client() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock = WiremockGuard::start();
    mock.register(Mock::given(method("REPORT")).respond_with(xml_207(CARD_REPORT_BODY)));
    let client = client_at(mock.uri());

    let contacts = block_on(fetch_contacts_from_book(&client, "/addressbooks/primary/")).unwrap();
    assert_eq!(contacts.len(), 1);
    assert_eq!(contacts[0].0, "/addressbooks/primary/alice.vcf");
    assert!(contacts[0].1.contains("FN:Alice Example"));
}

// =====================================================================
// tool_* aggregation branches
// =====================================================================

#[test]
fn test_for_each_card_client_records_invalid_url_as_a_client_error() {
    // `DavClient::new` rejects a non-HTTP URL, and the aggregation helper
    // must surface that as a per-server error rather than panicking.
    let mut config = crate::config::AgentConfig::default();
    config.caldav_clients.insert(
        "broken".to_string(),
        crate::config::CalDavClient {
            url: "ftp://example.invalid/dav".to_string(),
            username: "u".to_string(),
            password: "p".to_string(),
        },
    );

    let res = tool_get_contact(&config, "/c.vcf").unwrap();
    assert_eq!(res.contact, None);
    assert_eq!(res.errors.len(), 1, "got: {:?}", res.errors);
    assert_eq!(
        res.errors[0],
        "Error on client broken: Client config error: invalid URL 'ftp://example.invalid/dav'"
    );
}

#[test]
fn test_tool_search_contact_reports_client_errors_on_an_empty_result_set() {
    let mut config = crate::config::AgentConfig::default();
    config.caldav_clients.insert(
        "broken".to_string(),
        crate::config::CalDavClient {
            url: "ftp://example.invalid/dav".to_string(),
            username: "u".to_string(),
            password: "p".to_string(),
        },
    );
    let cache = crate::tools::registry::cache::ToolCache::new();
    let uuid_gen = crate::utils::uuid::SystemUuidGenerator;

    let res = tool_search_contact(&config, Some("alice"), None, &cache, &uuid_gen).unwrap();
    assert!(res.results.is_empty());
    assert_eq!(res.total, 0);
    assert_eq!(res.hint.as_deref(), Some("Final page."));
    assert_eq!(res.errors.len(), 1, "got: {:?}", res.errors);
    assert!(res.errors[0].starts_with("Error on client broken: "));
}

#[test]
fn test_tool_add_contact_targeted_client_error_is_fatal() {
    // A targeted add cannot fall back to another server, so the per-client
    // failure is returned as the tool error.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock = WiremockGuard::start();
    register_carddav_put_failure_stub(&mock);
    let mut config = crate::config::AgentConfig::default();
    config.caldav_clients.insert(
        "work".to_string(),
        crate::config::CalDavClient {
            url: mock.uri(),
            username: "user".to_string(),
            password: "password".to_string(),
        },
    );

    let err = tool_add_contact(&config, Some("work"), r#"{"name":"Bob"}"#).unwrap_err();
    assert!(
        err.starts_with("Error on client work: Failed to PUT contact"),
        "got: {err}"
    );
}

#[test]
fn test_tool_add_contact_untargeted_error_is_reported_in_the_result() {
    // Without a target the failure is collected rather than returned, so
    // the tool still answers with Ok.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock = WiremockGuard::start();
    register_carddav_put_failure_stub(&mock);
    let config = card_config_for(mock.uri());

    let res = tool_add_contact(&config, None, r#"{"name":"Bob"}"#).unwrap();
    assert!(
        res.result.contains("Error on client mock_card"),
        "got: {}",
        res.result
    );
    assert!(
        res.result.contains("Failed to PUT contact"),
        "got: {}",
        res.result
    );
}

/// A CardDAV service whose discovery succeeds but whose PUT returns 500.
fn register_carddav_put_failure_stub(mock: &WiremockGuard) {
    mock.register(Mock::given(method("PROPFIND")).respond_with(xml_207(CARD_PROPFIND_BODY)));
    mock.register(
        Mock::given(method("PUT")).respond_with(
            ResponseTemplate::new(500)
                .insert_header("content-type", "text/plain")
                .set_body_string("put failed"),
        ),
    );
}

#[test]
fn test_tool_delete_contact_keeps_already_absent_when_nothing_was_deleted() {
    // Every server answers 404: `has_actual_delete` is false, so the
    // "Already absent" lines are kept instead of being filtered out.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock = WiremockGuard::start();
    mock.register(
        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(404).set_body_string("no")),
    );
    let config = card_config_for(mock.uri());

    let res = tool_delete_contact(&config, "/gone.vcf", None).unwrap();
    assert!(
        res.result.contains("Already absent (404)"),
        "got: {}",
        res.result
    );
}

#[test]
fn test_tool_update_contact_reports_a_failed_get_for_every_client() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock = WiremockGuard::start();
    mock.register(
        Mock::given(method("GET")).respond_with(
            ResponseTemplate::new(500)
                .insert_header("content-type", "text/plain")
                .set_body_string("boom"),
        ),
    );
    let config = card_config_for(mock.uri());

    let res = tool_update_contact(&config, "/c.vcf", None, r#"{"email":"a@b.com"}"#).unwrap();
    assert!(
        res.result.contains("Failed to fetch contact for update"),
        "got: {}",
        res.result
    );
    assert!(res.result.contains("500"), "got: {}", res.result);
}

#[test]
fn test_tool_update_contact_uses_if_match_when_the_server_returns_an_etag() {
    // The GET carries an ETag; the merged vCard must still be PUT back
    // successfully and the summary must report the status.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mock = WiremockGuard::start();
    mock.register(
        Mock::given(method("GET")).respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/vcard")
                .insert_header("ETag", "\"etag-value\"")
                .set_body_string(ALICE_VCARD_BODY),
        ),
    );
    mock.register(
        Mock::given(method("PUT")).respond_with(
            ResponseTemplate::new(204)
                .insert_header("ETag", "\"etag-value-2\"")
                .set_body_string(""),
        ),
    );
    let config = card_config_for(mock.uri());

    let res =
        tool_update_contact(&config, "/c.vcf", None, r#"{"email":"new@example.com"}"#).unwrap();
    // `reqwest::StatusCode`'s Display includes the canonical reason phrase,
    // so the summary reads "Updated (status 204 No Content)".
    assert!(
        res.result.contains("Updated (status 204 No Content)"),
        "got: {}",
        res.result
    );
}
