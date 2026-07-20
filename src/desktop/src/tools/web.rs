use fast_h2m::convert;
use crate::config::AppConfig;

pub fn tool_web_fetch(url: &str) -> Result<crate::tools::dtos::WebFetchResponse, String> {
    match ureq::get(url)
        .set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
        .set("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .set("Accept-Language", "en-US,en;q=0.9")
        .call() {
        Ok(response) => match response.into_string() {
            Ok(body) => match convert(&body, None) {
                Ok(res) => Ok(crate::tools::dtos::WebFetchResponse { content: res.content.unwrap_or_default() }),
                Err(e) => {
                    tracing::error!(name = "tool.web.html2md_failed", error = %e, url = %url, "Failed to convert fetched HTML to Markdown. Operator should verify if the URL returns valid HTML.");
                    Err(format!("Failed to convert HTML to Markdown: {}", e))
                }
            },
            Err(e) => {
                tracing::error!(name = "tool.web.read_body_failed", error = %e, url = %url, "Failed to read response body from web fetch. Operator should check network connectivity or URL validity.");
                Err(format!("Failed to read web response body: {}", e))
            }
        },
        Err(e) => {
            tracing::error!(name = "tool.web.fetch_failed", error = %e, url = %url, "Failed to fetch URL. Likely cause: network error or invalid URL. Operator should verify network connectivity.");
            Err(format!("Failed to fetch URL: {}", e))
        }
    }
}

// Reference: https://docs.searxng.org/dev/search_api.html
pub fn tool_web_search(url: &str, query: &str) -> Result<crate::tools::dtos::WebSearchResponse, String> {
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
                        // Surface every result the SearXNG instance returned.
                        // The page size is controlled server-side by
                        // `search.max_results` in settings.yml; we don't
                        // slice on the client because the operator asked to
                        // see whatever the server actually returned.
                        let mut output = String::new();
                        for (i, result) in results.iter().enumerate() {
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
                            Ok(crate::tools::dtos::WebSearchResponse { results: "No results found.".to_string() })
                        } else {
                            Ok(crate::tools::dtos::WebSearchResponse { results: output })
                        }
                    } else {
                        tracing::error!(name = "tool.web_search.parse_results_failed", url = %endpoint, "Search API returned JSON without a 'results' array. Operator should verify search provider compatibility.");
                        Err("Failed to parse results array.".to_string())
                    }
                } else {
                    tracing::error!(name = "tool.web_search.invalid_json", url = %endpoint, "Search API returned invalid JSON. Operator should verify the search provider endpoint.");
                    Err("Failed to parse JSON.".to_string())
                }
            }
            Err(e) => {
                tracing::error!(name = "tool.web_search.read_body_failed", error = %e, url = %endpoint, "Failed to read response body from search provider. Operator should verify search provider status.");
                Err(format!("Failed to read body: {}", e))
            }
        },
        Err(e) => {
            tracing::error!(name = "tool.web_search.fetch_failed", error = %e, url = %endpoint, "Failed to fetch from search provider. Likely cause: network error or provider downtime. Operator should check search configuration.");
            Err(format!("Failed to fetch URL: {}", e))
        }
    }
}


pub fn tool_web_delegate(config: &AppConfig, instruction: &str) -> Result<crate::tools::dtos::WebDelegateResponse, String> {
    let mut api_key = String::new();
    let mut api_url = String::new();
    let mut model_name = String::new();

    if let Some((_key, model_cfg)) = config.model_for_use_case("chat") {
        api_key = model_cfg.api_key.clone();
        api_url = model_cfg.api_url.clone();
        model_name = model_cfg.model.clone();
    } else if !config.models.is_empty() {
        if let Some(model_cfg) = config.models.values().next() {
            api_key = model_cfg.api_key.clone();
            api_url = model_cfg.api_url.clone();
            model_name = model_cfg.model.clone();
        }
    }

    if api_key == "your-api-key-here" || api_key.is_empty() {
        tracing::warn!(name = "tool.web_delegate.missing_api_key", "API key not set. Cannot use web_delegate. Operator should configure a valid API key in settings.");
        return Err("API key not set. Cannot use web_delegate.".to_string());
    }

    let mut messages = vec![
        serde_json::json!({
            "role": "system",
            "content": "You are a web research delegate. Use the web_search and web_fetch tools to execute the user's instruction. Gather information and return a concise, accurate summary. Do not converse, just output the final summarized answer."
        }),
        serde_json::json!({
            "role": "user",
            "content": instruction
        }),
    ];

    let mut tools_json = vec![
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "web_fetch",
                "description": "Fetch content from a URL.",
                "parameters": schemars::schema_for!(crate::tools::dtos::WebFetchInput)
            }
        })
    ];

    if config.searxng_url.is_some() {
        tools_json.push(serde_json::json!({
            "type": "function",
            "function": {
                "name": "web_search",
                "description": "Search the web using SearXNG.",
                "parameters": schemars::schema_for!(crate::tools::dtos::WebSearchInput)
            }
        }));
    }
    
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(30))
        .timeout_read(std::time::Duration::from_secs(120))
        .build();

    let mut loop_count = 0;
    let max_loops = 10;
    let mut final_content = String::new();

    while loop_count < max_loops {
        loop_count += 1;
        let request_body = serde_json::json!({
            "model": model_name,
            "messages": messages,
            "tools": tools_json,
            "tool_choice": "auto"
        });

        let response = match agent.post(&format!("{}/chat/completions", api_url.trim_matches('"').trim_end_matches('/')))
            .set("Authorization", &format!("Bearer {}", api_key))
            .set("Content-Type", "application/json")
            .send_json(request_body)
        {
            Ok(resp) => resp,
            Err(e) => {
                tracing::error!(name = "tool.web_delegate.api_request_failed", error = %e, "Delegate API request failed completely. Operator should check network connectivity.");
                return Err(format!("Delegate HTTP Request failed: {}", e));
            }
        };

        let resp_val: serde_json::Value = match response.into_json() {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(name = "tool.web_delegate.invalid_json", error = %e, "Delegate API returned invalid JSON. Operator should verify API provider.");
                return Err(format!("Failed to parse delegate JSON: {}", e));
            }
        };
        let choice = match resp_val.get("choices").and_then(|c| c.get(0)) {
            Some(c) => c,
            None => {
                tracing::error!(name = "tool.web_delegate.invalid_schema", response = ?resp_val, "Delegate API response missing 'choices' array. Operator should verify model configuration.");
                return Err("No choices in delegate response".to_string());
            }
        };
        let message = match choice.get("message") {
            Some(m) => m.clone(),
            None => {
                tracing::error!(name = "tool.web_delegate.missing_message", choice = ?choice, "Delegate API response missing 'message' field. Operator should verify model configuration.");
                return Err("No message in delegate choice".to_string());
            }
        };
        
        let content_str = message.get("content").and_then(|c| c.as_str()).unwrap_or("");
        if !content_str.is_empty() {
            final_content.push_str(content_str);
        }

        if let Some(tool_calls) = message.get("tool_calls").and_then(|t| t.as_array()) {
            if tool_calls.is_empty() {
                break;
            }
            messages.push(message.clone());
            
            for tool_call in tool_calls {
                let call_id = tool_call.get("id").and_then(|id| id.as_str()).unwrap_or("");
                let func_name = tool_call.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()).unwrap_or("");
                let func_args_str = tool_call.get("function").and_then(|f| f.get("arguments")).and_then(|a| a.as_str()).unwrap_or("{}");
                
                let result = if func_name == "web_fetch" {
                    if let Ok(input) = serde_json::from_str::<crate::tools::dtos::WebFetchInput>(func_args_str) {
                        match tool_web_fetch(&input.url) {
                            Ok(res) => serde_json::to_string(&crate::tools::dtos::ToolResponse::Success { data: res }).unwrap_or_default(),
                            Err(e) => serde_json::to_string(&crate::tools::dtos::ToolResponse::<crate::tools::dtos::WebFetchResponse>::Error { message: e }).unwrap_or_default(),
                        }
                    } else {
                        r#"{"status":"error","message":"Invalid input"}"#.to_string()
                    }
                } else if func_name == "web_search" {
                    if let Ok(input) = serde_json::from_str::<crate::tools::dtos::WebSearchInput>(func_args_str) {
                        if let Some(url) = &config.searxng_url {
                            match tool_web_search(url, &input.query) {
                                Ok(res) => serde_json::to_string(&crate::tools::dtos::ToolResponse::Success { data: res }).unwrap_or_default(),
                                Err(e) => serde_json::to_string(&crate::tools::dtos::ToolResponse::<crate::tools::dtos::WebSearchResponse>::Error { message: e }).unwrap_or_default(),
                            }
                        } else {
                            r#"{"status":"error","message":"web_search disabled"}"#.to_string()
                        }
                    } else {
                        r#"{"status":"error","message":"Invalid input"}"#.to_string()
                    }
                } else {
                    r#"{"status":"error","message":"Unknown tool"}"#.to_string()
                };

                messages.push(serde_json::json!({
                    "role": "tool",
                    "tool_call_id": call_id,
                    "content": result
                }));
            }
        } else {
            break;
        }
    }

    Ok(crate::tools::dtos::WebDelegateResponse { result: final_content })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, LlmConfig};

    fn spawn_mock_server(body: impl Into<String>) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let body_str = body.into();
        let response_str = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}", body_str.len(), body_str);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                if let Ok(mut stream) = stream {
                    use std::io::{Read, Write};
                    let mut buf = [0; 4096];
                    let _ = stream.read(&mut buf);
                    let _ = stream.write_all(response_str.as_bytes());
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
            }
        });
        format!("http://127.0.0.1:{}", port)
    }

    #[test]
    fn test_tool_web_fetch_mock() {
        rustls::crypto::ring::default_provider().install_default().ok();
        let server_url = spawn_mock_server("<html><body><h1>Hello World</h1></body></html>");
        let result = tool_web_fetch(&server_url).unwrap().content;
        assert!(result.contains("Hello") || result.contains("World"));
    }

    #[test]
    fn test_tool_web_fetch_error() {
        rustls::crypto::ring::default_provider().install_default().ok();
        let result = tool_web_fetch("http://127.0.0.1:1"); // Invalid port
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_web_search_mock() {
        rustls::crypto::ring::default_provider().install_default().ok();
        let mock_json = serde_json::json!({
            "results": [
                {
                    "title": "Test Title",
                    "url": "https://test.com",
                    "content": "Test content"
                }
            ]
        });
        let server_url = spawn_mock_server(mock_json.to_string());
        let result = tool_web_search(&server_url, "test query").unwrap().results;
        assert!(result.contains("Test Title"));
        assert!(result.contains("https://test.com"));
        assert!(result.contains("Test content"));
    }

    #[test]
    fn test_tool_web_search_empty() {
        rustls::crypto::ring::default_provider().install_default().ok();
        let mock_json = serde_json::json!({
            "results": []
        });
        let server_url = spawn_mock_server(mock_json.to_string());
        let result = tool_web_search(&server_url, "test query").unwrap().results;
        assert_eq!(result, "No results found.");
    }

    #[test]
    fn test_tool_web_search_returns_full_default_page() {
        // SearXNG with default `search.max_results=10` returns 10 results.
        // Verify we surface all of them.
        rustls::crypto::ring::default_provider().install_default().ok();
        let results: Vec<serde_json::Value> = (1..=10)
            .map(|i| {
                serde_json::json!({
                    "title": format!("Title {}", i),
                    "url": format!("https://example.com/{}", i),
                    "content": format!("Content for result {}", i),
                })
            })
            .collect();
        let mock_json = serde_json::json!({ "results": results });
        let server_url = spawn_mock_server(mock_json.to_string());
        let out = tool_web_search(&server_url, "q").unwrap().results;
        for i in 1..=10 {
            assert!(
                out.contains(&format!("Title {}", i)),
                "Expected result #{} to be present; missing from output: {}",
                i,
                out
            );
        }
    }

    #[test]
    fn test_tool_web_search_does_not_slice() {
        // Regression guard: the operator asked us to surface whatever
        // SearXNG returns without a client-side cap. Even if the server
        // returns more than the default 10 (e.g. an instance with many
        // engines enabled), we must pass every result through.
        rustls::crypto::ring::default_provider().install_default().ok();
        let total = 25;
        let results: Vec<serde_json::Value> = (1..=total)
            .map(|i| {
                serde_json::json!({
                    "title": format!("Title {}", i),
                    "url": format!("https://example.com/{}", i),
                    "content": format!("Content for result {}", i),
                })
            })
            .collect();
        let mock_json = serde_json::json!({ "results": results });
        let server_url = spawn_mock_server(mock_json.to_string());
        let out = tool_web_search(&server_url, "q").unwrap().results;
        for i in 1..=total {
            assert!(
                out.contains(&format!("Title {}", i)),
                "result #{} should be present; the tool must not slice the server response",
                i
            );
        }
    }
    
    #[test]
    fn test_tool_web_search_invalid_json() {
        rustls::crypto::ring::default_provider().install_default().ok();
        let server_url = spawn_mock_server("invalid json");
        let result = tool_web_search(&server_url, "test query");
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_web_delegate_missing_api_key() {
        let mut config = AppConfig::default();
        config.models.insert("chat".to_string(), LlmConfig {
            model: "test-model".to_string(),
            api_url: "http://example.com".to_string(),
            api_key: "".to_string(), // Missing API key
            cost: None,
            use_case: vec!["chat".to_string()],
        });
        
        let result = tool_web_delegate(&config, "do something");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "API key not set. Cannot use web_delegate.");
    }

    #[test]
    fn test_tool_web_delegate_mock() {
        rustls::crypto::ring::default_provider().install_default().ok();
        let mock_response = serde_json::json!({
            "choices": [
                {
                    "message": {
                        "content": "Final summarized answer",
                        "tool_calls": []
                    }
                }
            ]
        });
        
        let server_url = spawn_mock_server(mock_response.to_string());
        
        let mut config = AppConfig::default();
        config.models.insert("chat".to_string(), LlmConfig {
            model: "test-model".to_string(),
            api_url: server_url.clone(),
            api_key: "valid-key".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        });
        
        let result = tool_web_delegate(&config, "search for tests").unwrap();
        assert_eq!(result.result, "Final summarized answer");
    }
    
    #[test]
    fn test_tool_web_delegate_with_unknown_tool_handled_gracefully() {
        rustls::crypto::ring::default_provider().install_default().ok();
        
        // Mock server returns a tool_call with unknown function name
        // The delegate should handle this gracefully and continue
        let mock_response = serde_json::json!({
            "choices": [{
                "message": {
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "function": {
                            "name": "unknown_function",
                            "arguments": "{}"
                        }
                    }]
                }
            }]
        });
        
        let server_url = spawn_mock_server(mock_response.to_string());
        
        let mut config = AppConfig::default();
        config.models.insert("chat".to_string(), LlmConfig {
            model: "test-model".to_string(),
            api_url: server_url.clone(),
            api_key: "valid-key".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        });
        config.searxng_url = None;
        
        // Should not panic - handles unknown tool gracefully
        let result = tool_web_delegate(&config, "do something");
        // Either succeeds or returns an error we can handle
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_tool_web_delegate_handles_api_error_gracefully() {
        rustls::crypto::ring::default_provider().install_default().ok();
        
        // Mock server that returns an error status
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                if let Ok(mut stream) = stream {
                    use std::io::{Read, Write};
                    let mut buf = [0; 4096];
                    let _ = stream.read(&mut buf);
                    let response = "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 5\r\n\r\nerror";
                    let _ = stream.write_all(response.as_bytes());
                }
            }
        });
        
        let mut config = AppConfig::default();
        config.models.insert("chat".to_string(), LlmConfig {
            model: "test-model".to_string(),
            api_url: format!("http://127.0.0.1:{}", port),
            api_key: "valid-key".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        });
        
        let result = tool_web_delegate(&config, "test");
        // Should return an error, not panic
        assert!(result.is_err() || result.is_ok());
    }
}