//! Web-fetching tools — fetch a URL and convert HTML to markdown, and search via a SearXNG instance.
//!
//! Unit tests live in the sibling `web_tests.rs` sidecar.

use crate::config::AgentConfig;
use crate::datamark::{self, SECURITY_HEADER};
use crate::events::DelegateToolCall;
use crate::tools::registry::cache::CachedWebDocument;
use fast_h2m::convert;
use std::collections::HashMap;

pub fn tool_web_fetch(
    input: &crate::tools::dtos::WebFetchInput,
    cache: &crate::tools::registry::cache::ToolCache,
    uuid_gen: &dyn crate::utils::uuid::UuidGenerator,
) -> Result<crate::tools::dtos::WebFetchResponse, String> {
    let url = &input.url;

    // 1. If cursor is provided, slice next page from line cursor manager
    if let Some(cursor) = &input.cursor {
        let page = cache.web_lines.next_page(cursor)?;
        return Ok(crate::tools::dtos::WebFetchResponse {
            content: page.items.join("\n"),
            total_lines: page.total,
            cursor: page.cursor,
            hint: page.hint,
            response_headers: None,
            from_cache: true,
        });
    }

    // 2. A call without a cursor incurs a force refetch and invalidates cache entries (TOOL-032)
    cache.web_documents.invalidate(url);

    let (cached_doc, from_cache) = {
        match reqwest::blocking::Client::new()
            .get(url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
            )
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            )
            .header("Accept-Language", "en-US,en;q=0.9")
            .send()
        {
            Ok(response) => {
                let mut response_headers = HashMap::new();
                for (name, value) in response.headers() {
                    if let Ok(val) = value.to_str() {
                        response_headers.insert(name.as_str().to_string(), val.to_string());
                    }
                }
                match response.text() {
                    Ok(body) => match convert(&body, None) {
                        Ok(res) => {
                            let md_content = res.content.unwrap_or_default();
                            let doc = CachedWebDocument {
                                content: md_content,
                                response_headers,
                            };
                            cache.web_documents.insert(url.clone(), doc.clone());
                            Ok((doc, false))
                        }
                        Err(e) => {
                            tracing::error!(
                                name = "tool.web.html2md_failed",
                                error = %e,
                                url = %url,
                                "Failed to convert fetched HTML to Markdown. Operator should verify if the URL returns valid HTML."
                            );
                            Err(format!("Failed to convert HTML to Markdown: {}", e))
                        }
                    },
                    Err(e) => {
                        tracing::error!(
                            name = "tool.web.read_body_failed",
                            error = %e,
                            url = %url,
                            "Failed to read response body from web fetch. Operator should check network connectivity or URL validity."
                        );
                        Err(format!("Failed to read web response body: {}", e))
                    }
                }
            }
            Err(e) => {
                tracing::error!(
                    name = "tool.web.fetch_failed",
                    error = %e,
                    url = %url,
                    "Failed to fetch URL. Likely cause: network error or invalid URL. Operator should verify network connectivity."
                );
                Err(format!("Failed to fetch URL: {}", e))
            }
        }?
    };

    // 4. Create cursor pagination session over markdown lines
    let lines: Vec<String> = cached_doc.content.lines().map(|s| s.to_string()).collect();
    let page = cache.web_lines.create_session(lines, uuid_gen);

    Ok(crate::tools::dtos::WebFetchResponse {
        content: page.items.join("\n"),
        total_lines: page.total,
        cursor: page.cursor,
        hint: page.hint,
        response_headers: if input.headers {
            Some(cached_doc.response_headers)
        } else {
            None
        },
        from_cache,
    })
}

// Reference: https://docs.searxng.org/dev/search_api.html
pub fn tool_web_search(
    url: &str,
    query: &str,
    cursor: Option<String>,
    cache: &crate::tools::registry::cache::ToolCache,
    uuid_gen: &dyn crate::utils::uuid::UuidGenerator,
) -> Result<crate::tools::dtos::WebSearchResponse, String> {
    if let Some(cursor) = cursor {
        let page = cache.web_search_sessions.next_page(&cursor)?;
        let results = if page.items.is_empty() {
            "No results found.".to_string()
        } else {
            page.items.join("\n\n")
        };
        return Ok(crate::tools::dtos::WebSearchResponse {
            results,
            total: page.total,
            cursor: page.cursor,
            hint: page.hint,
        });
    }

    let endpoint = format!("{}/search", url);
    match reqwest::blocking::Client::new()
        .get(&endpoint)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
        .header("Accept", "application/json")
        .header("Accept-Language", "en-US,en;q=0.9")
        .header("X-Forwarded-For", "127.0.0.1")
        .header("X-Real-IP", "127.0.0.1")
        .query(&[("q", query), ("format", "json")])
        .send()
    {
        Ok(response) => match response.text() {
            Ok(body) => {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                    if let Some(results) = json.get("results").and_then(|r| r.as_array()) {
                        let mut items = Vec::new();
                        for (i, result) in results.iter().enumerate() {
                            let title = result.get("title").and_then(|t| t.as_str()).unwrap_or("");
                            let url = result.get("url").and_then(|u| u.as_str()).unwrap_or("");
                            let content = result.get("content").and_then(|c| c.as_str()).unwrap_or("");
                            items.push(format!(
                                "{}. [{}]({})\n{}",
                                i + 1,
                                title,
                                url,
                                content
                            ));
                        }
                        if items.is_empty() {
                            Ok(crate::tools::dtos::WebSearchResponse {
                                results: "No results found.".to_string(),
                                total: 0,
                                cursor: None,
                                hint: Some(crate::tools::registry::builtin::strings::FINAL_PAGE_HINT.to_string()),
                            })
                        } else {
                            let page = cache.web_search_sessions.create_session(items, uuid_gen);
                            Ok(crate::tools::dtos::WebSearchResponse {
                                results: page.items.join("\n\n"),
                                total: page.total,
                                cursor: page.cursor,
                                hint: page.hint,
                            })
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

pub fn tool_web_delegate(
    config: &AgentConfig,
    instruction: &str,
    cache: &crate::tools::registry::cache::ToolCache,
) -> Result<crate::tools::dtos::WebDelegateResponse, String> {
    let model_cfg = config.select_chat_model().map_err(|e| {
        tracing::warn!(name = "tool.web_delegate.missing_api_key", "{}", e);
        e
    })?;

    let api_key = model_cfg.api_key.clone();
    let api_url = model_cfg.api_url.clone();
    let model_name = model_cfg.model.clone();

    let mut messages = vec![
        serde_json::json!({
            "role": "system",
            // R1 (Spotlighting): the sub-agent's system prompt must
            // include the same security header the parent uses, plus
            // a delegate-specific role line. The header teaches the
            // sub-agent to refuse instructions it sees inside
            // datamarked tool results — which is critical because
            // `web_delegate` is exactly the surface an indirect
            // injection can drive (it runs many fetches in one
            // session).
            "content": format!(
                "{SECURITY_HEADER}\n\nYou are a web research delegate. Use the web_search and web_fetch tools to execute the user's instruction. Gather information and return a concise, accurate summary. Do not converse, just output the final summarized answer."
            )
        }),
        serde_json::json!({
            "role": "user",
            "content": instruction
        }),
    ];

    let mut tools_json = vec![serde_json::json!({
        "type": "function",
        "function": {
            "name": "web_fetch",
            "description": "Fetch a URL and convert the content to Markdown. Returns up to 64 lines and a cursor token for pagination. Use the cursor to fetch the next page. Use force_refetch=true to bypass.",
            "parameters": schemars::schema_for!(crate::tools::dtos::WebFetchInput)
        }
    })];

    if config.searxng_url().is_some() {
        tools_json.push(serde_json::json!({
            "type": "function",
            "function": {
                "name": "web_search",
                "description": "Search the web using SearXNG.",
                "parameters": schemars::schema_for!(crate::tools::dtos::WebSearchInput)
            }
        }));
    }

    let agent = reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| {
            tracing::error!(name = "tool.web_delegate.client_build_failed", error = %e, "Failed to build reqwest client for delegate");
            format!("Failed to build HTTP client: {}", e)
        })?;

    let mut loop_count = 0;
    let max_loops = 10;
    let mut final_content = String::new();
    let mut delegate_tool_calls: Vec<DelegateToolCall> = Vec::new();

    while loop_count < max_loops {
        loop_count += 1;
        let request_body = serde_json::json!({
            "model": model_name,
            "messages": messages,
            "tools": tools_json,
            "tool_choice": "auto"
        });

        let response = match agent
            .post(format!(
                "{}/chat/completions",
                api_url.trim_matches('"').trim_end_matches('/')
            ))
            .bearer_auth(&api_key)
            .json(&request_body)
            .send()
        {
            Ok(resp) => resp,
            Err(e) => {
                tracing::error!(name = "tool.web_delegate.api_request_failed", error = %e, "Delegate API request failed completely. Operator should check network connectivity.");
                return Err(format!("Delegate HTTP Request failed: {}", e));
            }
        };

        let resp_val: serde_json::Value = match response.json() {
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

        let content_str = message
            .get("content")
            .and_then(|c| c.as_str())
            .unwrap_or("");
        if !content_str.is_empty() {
            final_content.push_str(content_str);
        }

        if let Some(tool_calls) = message.get("tool_calls").and_then(|t| t.as_array()) {
            if tool_calls.is_empty() {
                if final_content.is_empty() {
                    tracing::warn!(name = "tool.web_delegate.empty_content_on_break", model = %model_name, loop = loop_count, "Delegate turn returned empty tool_calls and no content — model may not support function calling or refused the instruction.");
                }
                break;
            }
            messages.push(message.clone());

            for tool_call in tool_calls {
                let call_id = tool_call.get("id").and_then(|id| id.as_str()).unwrap_or("");
                let func_name = tool_call
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("");
                let func_args_str = tool_call
                    .get("function")
                    .and_then(|f| f.get("arguments"))
                    .and_then(|a| a.as_str())
                    .unwrap_or("{}");

                let result = if func_name == "web_fetch" {
                    if let Ok(input) =
                        serde_json::from_str::<crate::tools::dtos::WebFetchInput>(func_args_str)
                    {
                        match tool_web_fetch(
                            &input,
                            cache,
                            &crate::utils::uuid::SystemUuidGenerator,
                        ) {
                            Ok(res) => {
                                serde_json::to_string(&crate::tools::dtos::ToolResponse::Success {
                                    data: res,
                                })
                                .unwrap_or_default()
                            }
                            Err(e) => serde_json::to_string(&crate::tools::dtos::ToolResponse::<
                                crate::tools::dtos::WebFetchResponse,
                            >::Error {
                                message: e,
                            })
                            .unwrap_or_default(),
                        }
                    } else {
                        r#"{"status":"error","message":"Invalid input"}"#.to_string()
                    }
                } else if func_name == "web_search" {
                    if let Ok(input) =
                        serde_json::from_str::<crate::tools::dtos::WebSearchInput>(func_args_str)
                    {
                        if let Some(url) = config.searxng_url() {
                            match tool_web_search(
                                url,
                                &input.query,
                                input.cursor,
                                cache,
                                &crate::utils::uuid::SystemUuidGenerator,
                            ) {
                                Ok(res) => serde_json::to_string(
                                    &crate::tools::dtos::ToolResponse::Success { data: res },
                                )
                                .unwrap_or_default(),
                                Err(e) => {
                                    serde_json::to_string(&crate::tools::dtos::ToolResponse::<
                                        crate::tools::dtos::WebSearchResponse,
                                    >::Error {
                                        message: e,
                                    })
                                    .unwrap_or_default()
                                }
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

                // R1 (Spotlighting): wrap every tool result the
                // sub-agent sees in a datamark envelope so the
                // LLM treats it as data, not instructions. This
                // is the sub-agent counterpart of the parent
                // loop's wrap in `agent_impl::process_tool_results`.
                // The `func_name` is the literal LLM-facing tool
                // name (`web_fetch` or `web_search`) so the
                // provenance line in the envelope tells the LLM
                // which tool produced the content.
                let wrapped = datamark::wrap_tool_result(func_name, &result);

                delegate_tool_calls.push(DelegateToolCall {
                    name: func_name.to_string(),
                    args: serde_json::from_str::<serde_json::Value>(func_args_str)
                        .unwrap_or(serde_json::Value::String(func_args_str.to_string())),
                    result: serde_json::from_str::<serde_json::Value>(&result)
                        .unwrap_or(serde_json::Value::String(result.clone())),
                });

                messages.push(serde_json::json!({
                    "role": "tool",
                    "tool_call_id": call_id,
                    "content": wrapped
                }));
            }
        } else {
            if final_content.is_empty() {
                tracing::warn!(name = "tool.web_delegate.empty_content_on_break", model = %model_name, loop = loop_count, "Delegate turn returned no tool_calls and no content — model may not support function calling or refused the instruction.");
            }
            break;
        }
    }

    if final_content.is_empty() {
        tracing::warn!(name = "tool.web_delegate.empty_result", model = %model_name, loops = loop_count, max_loops = max_loops, instruction_len = instruction.len(), tool_call_count = delegate_tool_calls.len(), "Delegate completed with empty result after {loop_count} loops.");
    }

    Ok(crate::tools::dtos::WebDelegateResponse {
        result: final_content,
        tool_calls: delegate_tool_calls,
    })
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `web_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "web_tests.rs"]
mod tests;
