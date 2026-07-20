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
    let rt = RT.get_or_init(|| Runtime::new().expect("Failed to create Tokio runtime"));
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

    let mut principal_opt = client.discover_current_user_principal().await.ok().flatten();

    if principal_opt.is_none() {
        let base_trimmed = base_url.trim_end_matches('/');
        let guess = format!("{}/dav/principals/user/{}/", base_trimmed, username);
        if let Ok(homes) = client.discover_addressbook_home_set(&guess).await {
            if !homes.is_empty() {
                principal_opt = Some(guess);
            }
        }
    }

    let principal =
        principal_opt.ok_or_else(|| anyhow::anyhow!("No principal found"))?;
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

    let uid = uid_override
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            format!(
                "{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
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
    let org = parsed.get("org").and_then(|v| v.as_str()).map(escape_vcard_text);

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
                    .unwrap()
                    .as_millis()
            );
            let path = format!("{}{}.vcf", default_book.trim_end_matches('/'), uid);
            let vcard_data = json_to_vcard(contact_json, Some(&uid));
            let vcard_bytes: bytes::Bytes = vcard_data.into_bytes().into();
            let resp = client.put_if_none_match(&path, vcard_bytes).await?;
            if !resp.status().is_success() {
                let status = resp.status();
                let body = String::from_utf8_lossy(&resp.into_body()).to_string();
                return Err(anyhow::anyhow!("Failed to PUT contact: {} - {}", status, body));
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
