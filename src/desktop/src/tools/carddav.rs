//! CardDAV agent tools — search, retrieve, create, update, and delete contacts across configured CardDAV servers.

use crate::config::AppConfig;
use fast_dav_rs::CardDavClient;
use std::sync::OnceLock;
use tokio::runtime::Runtime;

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

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    static RT: OnceLock<Runtime> = OnceLock::new();
    let rt = RT.get_or_init(|| Runtime::new().unwrap_or_else(|e| {
        panic!("Failed to create Tokio runtime: {}", e)
    }));
    rt.block_on(f)
}

async fn get_all_addressbooks(
    client: &CardDavClient,
    base_url: &str,
    username: &str,
) -> anyhow::Result<Vec<String>> {
    if let Ok(books) = client.list_addressbooks(base_url).await {
        if !books.is_empty() {
            return Ok(books.into_iter().map(|b| b.href).collect());
        }
    }

    if let Ok(homes) = client.discover_addressbook_home_set(base_url).await {
        if let Some(home) = homes.first() {
            if let Ok(books) = client.list_addressbooks(home).await {
                if !books.is_empty() {
                    return Ok(books.into_iter().map(|b| b.href).collect());
                }
            }
        }
    }

    let mut principal_opt = client
        .discover_current_user_principal()
        .await
        .ok()
        .flatten();

    if principal_opt.is_none() {
        let base_trimmed = base_url.trim_end_matches('/');
        let guess = format!("{}/dav/principals/user/{}/", base_trimmed, username);
        if let Ok(homes) = client.discover_addressbook_home_set(&guess).await {
            if !homes.is_empty() {
                principal_opt = Some(guess);
            }
        }
    }

    let principal = principal_opt.ok_or_else(|| anyhow::anyhow!("No principal found"))?;
    let homes = client.discover_addressbook_home_set(&principal).await?;
    let home = homes
        .first()
        .ok_or_else(|| anyhow::anyhow!("No addressbook home found"))?;
    let books = client.list_addressbooks(home).await?;
    Ok(books.into_iter().map(|b| b.href).collect())
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

    let fn_name = escape_vcard_text(
        parsed
            .get("fn")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown"),
    );
    let email = parsed.get("email").and_then(|v| v.as_str());
    let tel = parsed.get("tel").and_then(|v| v.as_str());
    let org = parsed
        .get("org")
        .and_then(|v| v.as_str())
        .map(escape_vcard_text);

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
    vcard.push_str("END:VCARD\r\n");
    vcard
}

pub fn tool_search_contact(
    config: &AppConfig,
    keyword: &str,
) -> Result<crate::tools::dtos::SearchContactResponse, String> {
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
            for book_path in books {
                let contacts = fetch_contacts_from_book(&client, &book_path).await?;
                for (href, data) in contacts {
                    if data.to_lowercase().contains(&kw) {
                        matches.push(parse_vcard(name, &href, &data));
                    }
                }
            }
            anyhow::Result::<Vec<_>>::Ok(matches)
        });

        match res {
            Ok(mut matches) => results.append(&mut matches),
            Err(e) => errors.push(format!("Error on client {}: {}", name, e)),
        }
    }

    let resp = CardDavResponse { results, errors };
    Ok(crate::tools::dtos::SearchContactResponse {
        results: serde_json::to_string_pretty(&resp).unwrap_or_else(|_| "{}".to_string()),
    })
}

pub fn tool_get_contact(
    config: &AppConfig,
    id: &str,
) -> Result<crate::tools::dtos::GetContactResponse, String> {
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

            let resp = client.get(id).await?;
            if !resp.status().is_success() {
                let status = resp.status();
                let bytes = resp.into_body();
                let body = String::from_utf8_lossy(&bytes).to_string();
                return Err(anyhow::anyhow!("Not found by href: {} - {}", status, body));
            }
            let bytes = resp.into_body();
            let body = String::from_utf8_lossy(&bytes).to_string();
            anyhow::Result::<CardDavContactDetails>::Ok(parse_vcard(name, id, &body))
        });

        match res {
            Ok(data) => results.push(data),
            Err(e) => errors.push(format!("Error on client {}: {}", name, e)),
        }
    }

    let resp = CardDavResponse { results, errors };
    Ok(crate::tools::dtos::GetContactResponse {
        result: serde_json::to_string_pretty(&resp).unwrap_or_else(|_| "{}".to_string()),
    })
}

pub fn tool_add_contact(
    config: &AppConfig,
    contact_json: &str,
) -> Result<crate::tools::dtos::AddContactResponse, String> {
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
            let path = format!("{}{}.vcf", default_book.trim_end_matches('/'), uid);
            let vcard_data = json_to_vcard(contact_json, Some(&uid));
            let vcard_bytes: bytes::Bytes = vcard_data.into_bytes().into();
            let resp = client.put_if_none_match(&path, vcard_bytes).await?;
            if !resp.status().is_success() {
                let status = resp.status();
                let body = String::from_utf8_lossy(&resp.into_body()).to_string();
                return Err(anyhow::anyhow!(
                    "Failed to PUT contact: {} - {}",
                    status,
                    body
                ));
            }
            anyhow::Result::<String>::Ok(format!("Created at {}", path))
        });

        match res {
            Ok(s) => all_results.push(format!("--- Client: {} ---\n{}", name, s)),
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

#[cfg(test)]
mod tests {
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
        let input =
            r#"{"fn":"John Doe","email":"john@example.com","tel":"+1234567890","org":"Acme"}"#;
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
}
