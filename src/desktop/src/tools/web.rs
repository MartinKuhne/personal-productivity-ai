use fast_h2m::convert;

pub fn tool_web_fetch(url: &str) -> String {
    match ureq::get(url)
        .set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
        .set("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .set("Accept-Language", "en-US,en;q=0.9")
        .call() {
        Ok(response) => match response.into_string() {
            Ok(body) => match convert(&body, None) {
                Ok(res) => res.content.unwrap_or_default(),
                Err(e) => format!("Error converting HTML to Markdown: {}", e),
            },
            Err(e) => format!("Error reading web response body: {}", e),
        },
        Err(e) => format!("Error fetching URL: {}", e),
    }
}

pub fn tool_web_search(url: &str, query: &str) -> String {
    let endpoint = format!("{}/search", url);
    match ureq::get(&endpoint)
        .set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
        .set("Accept", "application/json")
        .set("Accept-Language", "en-US,en;q=0.9")
        .set("X-Forwarded-For", "127.0.0.1")
        .set("X-Real-IP", "127.0.0.1")
        .query("q", query)
        .query("format", "json")
        .call()
    {
        Ok(response) => match response.into_string() {
            Ok(body) => {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                    if let Some(results) = json.get("results").and_then(|r| r.as_array()) {
                        let mut output = String::new();
                        for (i, result) in results.iter().take(5).enumerate() {
                            let title = result.get("title").and_then(|t| t.as_str()).unwrap_or("");
                            let url = result.get("url").and_then(|u| u.as_str()).unwrap_or("");
                            let content = result.get("content").and_then(|c| c.as_str()).unwrap_or("");
                            output.push_str(&format!(
                                "{}. [{}]({})\n{}\n\n",
                                i + 1,
                                title,
                                url,
                                content
                            ));
                        }
                        if output.is_empty() {
                            "No results found.".to_string()
                        } else {
                            output
                        }
                    } else {
                        "Error parsing results array.".to_string()
                    }
                } else {
                    "Error parsing JSON.".to_string()
                }
            }
            Err(e) => format!("Error reading body: {}", e),
        },
        Err(e) => format!("Error fetching URL: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These tests require network access and are marked as ignored
    // Run with `cargo test -- --ignored` if needed
    
    #[test]
    #[ignore]
    fn test_tool_web_fetch() {
        let result = tool_web_fetch("https://example.com");
        assert!(!result.is_empty());
    }
}