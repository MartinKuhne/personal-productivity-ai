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
                Err(e) => Err(format!("Failed to convert HTML to Markdown: {}", e)),
            },
            Err(e) => Err(format!("Failed to read web response body: {}", e)),
        },
        Err(e) => Err(format!("Failed to fetch URL: {}", e)),
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
                            Ok(crate::tools::dtos::WebSearchResponse { results: "No results found.".to_string() })
                        } else {
                            Ok(crate::tools::dtos::WebSearchResponse { results: output })
                        }
                    } else {
                        Err("Failed to parse results array.".to_string())
                    }
                } else {
                    Err("Failed to parse JSON.".to_string())
                }
            }
            Err(e) => Err(format!("Failed to read body: {}", e)),
        },
        Err(e) => Err(format!("Failed to fetch URL: {}", e)),
    }
}


pub fn tool_web_delegate(config: &AppConfig, instruction: &str) -> Result<crate::tools::dtos::WebDelegateResponse, String> {
    let mut api_key = config.api_key.clone();
    let mut api_url = config.api_url.clone();
    let mut model_name = config.model.clone();

    if let Some(model_cfg) = config.models.get(&config.model) {
        api_key = model_cfg.api_key.clone();
        api_url = model_cfg.api_url.clone();
        model_name = model_cfg.model.clone();
    } else if (api_key == "your-api-key-here" || api_key.is_empty()) && !config.models.is_empty() {
        if let Some(model_cfg) = config.models.values().next() {
            api_key = model_cfg.api_key.clone();
            api_url = model_cfg.api_url.clone();
            model_name = model_cfg.model.clone();
        }
    }

    if api_key == "your-api-key-here" || api_key.is_empty() {
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
            Err(e) => return Err(format!("Delegate HTTP Request failed: {}", e)),
        };

        let resp_val: serde_json::Value = match response.into_json() {
            Ok(v) => v,
            Err(e) => return Err(format!("Failed to parse delegate JSON: {}", e)),
        };
        let choice = match resp_val.get("choices").and_then(|c| c.get(0)) {
            Some(c) => c,
            None => return Err("No choices in delegate response".to_string()),
        };
        let message = match choice.get("message") {
            Some(m) => m.clone(),
            None => return Err("No message in delegate choice".to_string()),
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

    // These tests require network access and are marked as ignored
    // Run with `cargo test -- --ignored` if needed
    
    #[test]
    #[ignore]
    fn test_tool_web_fetch() {
        let result = tool_web_fetch("https://example.com").unwrap().content;
        assert!(!result.is_empty());
    }
}