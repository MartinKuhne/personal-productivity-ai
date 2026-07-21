use crate::config::get_config_path;
use crate::file_events::{Bus, FileEvent};
use crate::messages::{BackgroundMessage, TokenUsageInfo};
use crate::tools::{execute_tool, get_tools_schema};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use ureq;

/// Best-effort parser for the `usage` block returned by an OpenAI-compatible
/// chat-completions endpoint.
///
/// OpenAI and most local OpenAI-compatible servers (LM Studio, vLLM, llama.cpp
/// server, etc.) emit:
/// ```json
/// "usage": {
///   "prompt_tokens": 123,
///   "completion_tokens": 45,
///   "total_tokens": 168,
///   "prompt_tokens_details":     { "cached_tokens": 0 },
///   "completion_tokens_details": { "reasoning_tokens": 0 }
/// }
/// ```
///
/// Anthropic's Messages API uses `input_tokens` / `output_tokens` instead; this
/// function maps those onto `prompt_tokens` / `completion_tokens` so downstream
/// code can stay provider-agnostic.
///
/// Returns `None` if no recognizable token field is present, so callers can
/// cheaply skip responses from providers that don't report usage (e.g. some
/// streaming-only endpoints).
pub fn parse_usage_block(usage: &serde_json::Value) -> Option<TokenUsageInfo> {
    let prompt_tokens = usage
        .get("prompt_tokens")
        .and_then(|v| v.as_u64())
        .or_else(|| usage.get("input_tokens").and_then(|v| v.as_u64()));
    let completion_tokens = usage
        .get("completion_tokens")
        .and_then(|v| v.as_u64())
        .or_else(|| usage.get("output_tokens").and_then(|v| v.as_u64()));
    let total_tokens = usage.get("total_tokens").and_then(|v| v.as_u64());

    if prompt_tokens.is_none() && completion_tokens.is_none() && total_tokens.is_none() {
        return None;
    }

    let cached_tokens = usage
        .get("prompt_tokens_details")
        .and_then(|d| d.get("cached_tokens"))
        .and_then(|v| v.as_u64());
    let reasoning_tokens = usage
        .get("completion_tokens_details")
        .and_then(|d| d.get("reasoning_tokens"))
        .and_then(|v| v.as_u64());

    Some(TokenUsageInfo {
        prompt_tokens: prompt_tokens.unwrap_or(0),
        completion_tokens: completion_tokens.unwrap_or(0),
        total_tokens: total_tokens.unwrap_or_else(|| {
            prompt_tokens
                .unwrap_or(0)
                .saturating_add(completion_tokens.unwrap_or(0))
        }),
        cached_tokens,
        reasoning_tokens,
    })
}

fn split_thinking_and_content(text: &str) -> (String, String) {
    let delim = "🤔";
    if let Some(start_idx) = text.find(delim) {
        if let Some(offset) = text[start_idx + delim.len()..].find(delim) {
            let end_idx = start_idx + delim.len() + offset;
            let thinking = text[start_idx + delim.len()..end_idx].to_string();
            let content = format!("{}{}", &text[..start_idx], &text[end_idx + delim.len()..]);
            return (thinking, content);
        }
    }
    ("".to_string(), text.to_string())
}

pub fn get_base_system_prompt(config: &crate::config::AppConfig) -> String {
    let date_str = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let mut system_prompt = format!(
        "You are FastMD Agent, an autonomous assistant helper for managing the Markdown workspace. You can read, create, search, and edit files, fetch web pages, and manage tags using your tools. Help the user achieve their goal by using tools step by step. Respond to the user using Markdown format.\n\nCRITICAL: Avoid context bloat! Do NOT use the `read_file` tool on multiple files in a single step. Always prefer `read_yaml_header` to survey documents, or `grep` to extract specific information without reading entire files.\n\nToday's date and time is: {}",
        date_str
    );

    if let Some(name) = &config.user_name {
        system_prompt.push_str(&format!("\nUser's Name: {}", name));
    }
    if let Some(address) = &config.user_address {
        system_prompt.push_str(&format!("\nUser's Address: {}", address));
    }
    if let Some(birthdate) = &config.user_birthdate {
        use chrono::Datelike;
        let mut age_str = None;
        if let Ok(parsed_date) = chrono::NaiveDate::parse_from_str(birthdate, "%Y-%m-%d")
            .or_else(|_| chrono::NaiveDate::parse_from_str(birthdate, "%m/%d/%Y"))
            .or_else(|_| chrono::NaiveDate::parse_from_str(birthdate, "%d/%m/%Y"))
            .or_else(|_| chrono::NaiveDate::parse_from_str(birthdate, "%d-%m-%Y"))
            .or_else(|_| chrono::NaiveDate::parse_from_str(birthdate, "%B %d, %Y"))
        {
            let today = chrono::Local::now().naive_local().date();
            let mut age = today.year() - parsed_date.year();
            if today.month() < parsed_date.month()
                || (today.month() == parsed_date.month() && today.day() < parsed_date.day())
            {
                age -= 1;
            }
            age_str = Some(age.to_string());
        } else if let Ok(num) = birthdate.trim().parse::<i32>() {
            let current_year = chrono::Local::now().year();
            if num > 1900 && num <= current_year {
                let age = current_year - num;
                age_str = Some(format!("~{}", age));
            } else if num > 0 && num < 150 {
                age_str = Some(num.to_string());
            }
        }

        if let Some(a) = age_str {
            system_prompt.push_str(&format!("\nUser's Age: {}", a));
        } else {
            system_prompt.push_str(&format!("\nUser's Birthdate/Age info: {}", birthdate));
        }
    }
    if let Some(gender) = &config.user_gender {
        system_prompt.push_str(&format!("\nUser's Gender: {}", gender));
    }
    if let Some(ext) = &config.system_prompt_extension {
        system_prompt.push_str(&format!("\n{}", ext));
    }
    system_prompt
}

pub fn run_agent(
    config: crate::config::AppConfig,
    tx_gui_agent: Sender<BackgroundMessage>,
    active_file: Option<PathBuf>,
    active_dir: Option<PathBuf>,
    selected_files: std::collections::HashSet<PathBuf>,
    prompt: String,
    cancel_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    history: Option<Vec<serde_json::Value>>,
    current_response: String,
    file_event_bus: Bus<FileEvent>,
) {
    std::thread::spawn(move || {
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
            tracing::warn!(
                name = "agent.api_key.missing",
                "Agent run skipped because API key is missing or default. Operator should configure a valid API key in settings."
            );
            let _ = tx_gui_agent.send(BackgroundMessage::AgentFailed(format!(
                "API key not set. Please either configure your API key in {}, or use `/models` to view and switch to a configured model.",
                get_config_path().display()
            )));
            return;
        }

        let mut system_prompt = get_base_system_prompt(&config);

        let to_virtual = |path: &PathBuf| -> String {
            crate::config::library_display_label(&config.content_libraries, path)
                .unwrap_or_else(|| path.to_string_lossy().to_string())
        };

        if let Some(active) = active_file {
            let rel = to_virtual(&active);
            system_prompt.push_str(&format!(" The user is currently viewing the file: {}", rel));
        } else if let Some(dir) = active_dir {
            let rel = to_virtual(&dir);
            system_prompt.push_str(&format!(
                " The user has selected the directory context: {}",
                rel
            ));
        }

        if !selected_files.is_empty() {
            system_prompt.push_str(" The user has also selected the following files:");
            for f in &selected_files {
                system_prompt.push_str(&format!(" {}", to_virtual(f)));
            }
            system_prompt.push_str(".");
        }

        for lib in &config.content_libraries {
            let user_md_path = std::path::Path::new(&lib.root_folder).join("USER.md");
            if user_md_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&user_md_path) {
                    system_prompt.push_str(&format!(
                        "\n\nUser Context (from {}):\n{}",
                        lib.name, content
                    ));
                }
            }
        }

        let mut messages = if let Some(mut existing) = history {
            existing.push(serde_json::json!({
                "role": "user",
                "content": prompt
            }));
            existing
        } else {
            vec![
                serde_json::json!({
                    "role": "system",
                    "content": system_prompt
                }),
                serde_json::json!({
                    "role": "user",
                    "content": prompt
                }),
            ]
        };

        let tools_json = get_tools_schema(&config, &prompt);
        let mut full_response = current_response;

        'agent_loop: loop {
            if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
                break 'agent_loop;
            }
            let _ = tx_gui_agent.send(BackgroundMessage::AgentStatus(
                "Waiting for LLM completions...".to_string(),
            ));

            let agent = ureq::AgentBuilder::new()
                .timeout_connect(std::time::Duration::from_secs(60))
                .timeout_read(std::time::Duration::from_secs(1800))
                .timeout(std::time::Duration::from_secs(1800))
                .build();

            let request_body = serde_json::json!({
                "model": model_name,
                "messages": messages,
                "tools": tools_json,
                "tool_choice": "auto"
            });

            let max_retries = 3u32;
            let mut retry_attempt = 0u32;
            let response = 'retry: loop {
                let result = agent
                    .post(&format!(
                        "{}/chat/completions",
                        api_url.trim_matches('"').trim_end_matches('/')
                    ))
                    .set("Authorization", &format!("Bearer {}", api_key))
                    .set("Content-Type", "application/json")
                    .send_json(request_body.clone());

                match result {
                    Ok(resp) => break 'retry Ok(resp),
                    Err(ureq::Error::Status(code, resp)) if code >= 500 || code == 429 => {
                        if retry_attempt < max_retries {
                            let delay_secs = 1u64 << retry_attempt;
                            tracing::warn!(
                                name = "agent.api.retry",
                                status = code,
                                attempt = retry_attempt + 1,
                                delay_secs = delay_secs,
                                "Retryable HTTP error, will retry"
                            );
                            std::thread::sleep(std::time::Duration::from_secs(delay_secs));
                            retry_attempt += 1;
                            continue 'retry;
                        }
                        let body = resp
                            .into_string()
                            .unwrap_or_else(|_| "[Could not read body]".to_string());
                        tracing::error!(
                            name = "agent.api.failed",
                            status = code,
                            response = %body,
                            "Failed to get chat completion from AI API after all retries."
                        );
                        break 'retry Err(crate::error::AgentError::HttpError { status: code, body });
                    }
                    Err(ureq::Error::Status(code, resp)) => {
                        let body = resp
                            .into_string()
                            .unwrap_or_else(|_| "[Could not read body]".to_string());
                        tracing::error!(
                            name = "agent.api.failed",
                            status = code,
                            response = %body,
                            "Failed to get chat completion from AI API. Likely cause: invalid API key or bad request."
                        );
                        break 'retry Err(crate::error::AgentError::HttpError { status: code, body });
                    }
                    Err(ref e) => {
                        let err_str = e.to_string();
                        let is_timeout = err_str.contains("timed out")
                            || err_str.contains("Timeout")
                            || err_str.contains("Network is unreachable");
                        if is_timeout && retry_attempt < max_retries {
                            let delay_secs = 1u64 << retry_attempt;
                            tracing::warn!(
                                name = "agent.api.retry",
                                error = %e,
                                attempt = retry_attempt + 1,
                                delay_secs = delay_secs,
                                "Timeout, will retry"
                            );
                            std::thread::sleep(std::time::Duration::from_secs(delay_secs));
                            retry_attempt += 1;
                            continue 'retry;
                        }
                        if is_timeout {
                            break 'retry Err(crate::error::AgentError::Timeout);
                        }
                        break 'retry Err(crate::error::AgentError::NetworkError(err_str));
                    }
                }
            };

            let response = match response {
                Ok(resp) => resp,
                Err(e) => {
                    let _ = tx_gui_agent.send(BackgroundMessage::AgentFailed(e.user_message()));
                    return;
                }
            };

            if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
                break 'agent_loop;
            }

            let resp_val: serde_json::Value = match response.into_json() {
                Ok(val) => val,
                Err(e) => {
                    tracing::error!(
                        name = "agent.api.invalid_json",
                        error = %e,
                        "Failed to parse JSON response from AI API. Agent cannot proceed. Likely cause: API returned malformed response or non-JSON payload (e.g. 502 Bad Gateway HTML). Operator should check the API provider."
                    );
                    let _ = tx_gui_agent.send(BackgroundMessage::AgentFailed(format!(
                        "Failed to parse JSON response: {}",
                        e
                    )));
                    return;
                }
            };

            // Parse and surface the usage block (if the provider returned one).
            // This runs before the schema checks so we still capture token data
            // when the response is otherwise usable.
            if let Some(usage) = resp_val.get("usage") {
                if let Some(info) = parse_usage_block(usage) {
                    tracing::info!(
                        name = "agent.usage",
                        prompt_tokens = info.prompt_tokens,
                        completion_tokens = info.completion_tokens,
                        total_tokens = info.total_tokens,
                        cached_tokens = info.cached_tokens.unwrap_or(0),
                        reasoning_tokens = info.reasoning_tokens.unwrap_or(0),
                        "LLM usage for this turn. `prompt_tokens` is the full context sent; the operator can divide it by the model's context window to gauge headroom."
                    );
                    let _ = tx_gui_agent.send(BackgroundMessage::AgentTokenUsage(info));
                }
            }

            let choice = match resp_val.get("choices").and_then(|c| c.get(0)) {
                Some(c) => c,
                None => {
                    tracing::error!(
                        name = "agent.api.invalid_schema",
                        response = ?resp_val,
                        "AI API response did not contain expected 'choices' array. Agent cannot proceed. Likely cause: using an incompatible API endpoint or model. Operator should verify model configuration."
                    );
                    let _ = tx_gui_agent.send(BackgroundMessage::AgentFailed(format!(
                        "Invalid response schema from endpoint: {:?}",
                        resp_val
                    )));
                    return;
                }
            };

            let message = match choice.get("message") {
                Some(m) => m.clone(),
                None => {
                    tracing::error!(
                        name = "agent.api.missing_message",
                        choice = ?choice,
                        "AI API choice did not contain a 'message' field. Agent cannot proceed. Likely cause: incompatible API format. Operator should verify model compatibility."
                    );
                    let _ = tx_gui_agent.send(BackgroundMessage::AgentFailed(
                        "No message returned in choices.".to_string(),
                    ));
                    return;
                }
            };

            if let Some(reasoning) = message.get("reasoning_content").and_then(|r| r.as_str()) {
                let _ = tx_gui_agent.send(BackgroundMessage::AgentThinking(reasoning.to_string()));
            }

            let content_str = message
                .get("content")
                .and_then(|c| c.as_str())
                .unwrap_or("");
            let (thinking, content) = split_thinking_and_content(content_str);
            if !thinking.is_empty() {
                let _ = tx_gui_agent.send(BackgroundMessage::AgentThinking(thinking));
            }

            if !content.is_empty() {
                full_response.push_str(&content);
                full_response.push_str("\n\n");
                let _ = tx_gui_agent.send(BackgroundMessage::AgentResponse(full_response.clone()));
            }

            messages.push(message.clone());

            if let Some(tool_calls) = message.get("tool_calls").and_then(|t| t.as_array()) {
                if tool_calls.is_empty() {
                    break;
                }

                let mut safe_calls = Vec::new();
                let mut unsafe_calls = Vec::new();

                for tool_call in tool_calls {
                    let func_name = tool_call
                        .get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("");
                    let is_safe = matches!(
                        func_name,
                        "grep"
                            | "read_tags"
                            | "list_files_by_tag"
                            | "list_files"
                            | "read_file"
                            | "read_file_lines"
                            | "web_fetch"
                            | "read_yaml_header"
                            | "web_search"
                            | "search_calendar"
                            | "get_calendar"
                            | "get_calendar_item"
                            | "search_email"
                            | "get_email_by_id"
                            | "search_contact"
                            | "get_contact"
                    );
                    if is_safe {
                        safe_calls.push(tool_call.clone());
                    } else {
                        unsafe_calls.push(tool_call.clone());
                    }
                }

                let rt = match tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        tracing::error!(name = "agent.runtime.build_failed", error = %e, "Failed to build tokio runtime for tool execution");
                        let _ = tx_gui_agent.send(BackgroundMessage::AgentFailed(format!(
                            "Internal error: failed to create async runtime: {}", e
                        )));
                        return;
                    }
                };
                let config_arc = std::sync::Arc::new(config.clone());
                let root_path_arc = std::sync::Arc::new(PathBuf::new());

                let mut completed_results = Vec::new();

                rt.block_on(async {
                    let mut join_set = tokio::task::JoinSet::new();
                    for tool_call in safe_calls {
                        let call_id = tool_call
                            .get("id")
                            .and_then(|id| id.as_str())
                            .unwrap_or("")
                            .to_string();
                        let func_name = tool_call
                            .get("function")
                            .and_then(|f| f.get("name"))
                            .and_then(|n| n.as_str())
                            .unwrap_or("")
                            .to_string();
                        let func_args_str = tool_call
                            .get("function")
                            .and_then(|f| f.get("arguments"))
                            .and_then(|a| a.as_str())
                            .unwrap_or("{}")
                            .to_string();
                        let cfg = config_arc.clone();
                        let rp = root_path_arc.clone();
                        let tool_call_clone = tool_call.clone();
                        let bus = file_event_bus.clone();

                        join_set.spawn_blocking(move || {
                            let result = execute_tool(&cfg, &rp, &func_name, &func_args_str, &bus);
                            (tool_call_clone, call_id, func_name, func_args_str, result)
                        });
                    }
                    while let Some(res) = join_set.join_next().await {
                        if let Ok(data) = res {
                            completed_results.push(data);
                        }
                    }
                });

                for tool_call in unsafe_calls {
                    let call_id = tool_call
                        .get("id")
                        .and_then(|id| id.as_str())
                        .unwrap_or("")
                        .to_string();
                    let func_name = tool_call
                        .get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .to_string();
                    let func_args_str = tool_call
                        .get("function")
                        .and_then(|f| f.get("arguments"))
                        .and_then(|a| a.as_str())
                        .unwrap_or("{}")
                        .to_string();

                    let result = execute_tool(
                        &config,
                        &PathBuf::new(),
                        &func_name,
                        &func_args_str,
                        &file_event_bus,
                    );
                    completed_results.push((
                        tool_call.clone(),
                        call_id,
                        func_name,
                        func_args_str,
                        result,
                    ));
                }

                // Notify the UI about files created via tool calls so the
                // directory tree is refreshed without waiting for the file
                // watcher.
                for (_tool_call, _call_id, func_name, func_args_str, result) in &completed_results {
                    if func_name == "create_file" {
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(result) {
                            if parsed.get("status").and_then(|s| s.as_str()) == Some("success") {
                                if let Ok(args_val) =
                                    serde_json::from_str::<serde_json::Value>(func_args_str)
                                {
                                    if let Some(path_str) =
                                        args_val.get("path").and_then(|p| p.as_str())
                                    {
                                        let vpath = Path::new(path_str);
                                        let mut comps = vpath.components().peekable();
                                        while let Some(c) = comps.peek() {
                                            match c {
                                                std::path::Component::RootDir
                                                | std::path::Component::CurDir => {
                                                    comps.next();
                                                }
                                                _ => break,
                                            }
                                        }
                                        if let Some(std::path::Component::Normal(first)) =
                                            comps.next()
                                        {
                                            let lib_name = first.to_string_lossy();
                                            for lib in &config.content_libraries {
                                                if lib.name == lib_name {
                                                    let rest: PathBuf = comps.collect();
                                                    let abs_path =
                                                        Path::new(&lib.root_folder).join(rest);
                                                    let tags =
                                                        crate::utils::tags::extract_tags_from_file(
                                                            &abs_path,
                                                        );
                                                    let _ = tx_gui_agent.send(
                                                        BackgroundMessage::FileModified {
                                                            path: abs_path,
                                                            tags,
                                                        },
                                                    );
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                let mut results_map = std::collections::HashMap::new();
                for (tool_call, call_id, func_name, func_args_str, result) in completed_results {
                    results_map.insert(call_id, (tool_call, func_name, func_args_str, result));
                }

                for tool_call in tool_calls {
                    let call_id = tool_call
                        .get("id")
                        .and_then(|id| id.as_str())
                        .unwrap_or("")
                        .to_string();
                    if let Some((_tc, func_name, func_args_str, result)) =
                        results_map.remove(&call_id)
                    {
                        let (formatted_args, _is_empty_args) =
                            match serde_json::from_str::<serde_json::Value>(&func_args_str) {
                                Ok(val) => {
                                    let empty = match &val {
                                        serde_json::Value::Object(o) => o.is_empty(),
                                        serde_json::Value::Array(a) => a.is_empty(),
                                        _ => false,
                                    };
                                    (
                                        serde_json::to_string_pretty(&val)
                                            .unwrap_or_else(|_| func_args_str.to_string()),
                                        empty,
                                    )
                                }
                                Err(_) => (func_args_str.to_string(), func_args_str.trim() == "{}"),
                            };

                        let formatted_args_quoted = formatted_args
                            .lines()
                            .map(|line| format!("> {}", line))
                            .collect::<Vec<_>>()
                            .join("\n");

                        let tool_msg = if func_name == "create_file" {
                            let mut msg = format!("> **Executing tool `{}`**\n", func_name);
                            if let Ok(args_val) =
                                serde_json::from_str::<serde_json::Value>(&func_args_str)
                            {
                                let path = args_val
                                    .get("path")
                                    .and_then(|p| p.as_str())
                                    .unwrap_or("unknown");
                                msg.push_str(&format!("> Path: `{}`\n", path));
                            }
                            msg
                        } else {
                            format!(
                                "> **Executing tool `{}`**\n{}",
                                func_name, formatted_args_quoted
                            )
                        };
                        full_response.push_str(&tool_msg);
                        full_response.push_str("\n\n");
                        let _ = tx_gui_agent
                            .send(BackgroundMessage::AgentResponse(full_response.clone()));

                        let mut is_error = false;
                        let mut error_msg = String::new();
                        let mut result_data = serde_json::Value::Null;
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&result) {
                            if let Some(status) = parsed.get("status").and_then(|s| s.as_str()) {
                                if status == "error" {
                                    is_error = true;
                                    error_msg = parsed
                                        .get("message")
                                        .and_then(|m| m.as_str())
                                        .unwrap_or("Unknown error")
                                        .to_string();
                                } else if status == "success" {
                                    if let Some(data) = parsed.get("data") {
                                        result_data = data.clone();
                                    }
                                }
                            }
                        }

                        if func_name == "read_file" && !is_error {
                            let content = result_data
                                .get("content")
                                .and_then(|c| c.as_str())
                                .unwrap_or("");
                            tracing::info!(
                                name = "agent.tool.success",
                                tool = %func_name,
                                lines = content.lines().count(),
                                "Tool executed successfully and returned lines. Normal operation."
                            );
                        } else if is_error {
                            tracing::warn!(
                                name = "agent.tool.error",
                                tool = %func_name,
                                error = %error_msg,
                                result = %result,
                                "Tool execution returned an error status. The agent may attempt to recover or try another tool."
                            );
                        } else if func_name == "create_file" {
                            let size = result_data
                                .get("size_bytes")
                                .and_then(|s| s.as_u64())
                                .unwrap_or(0);
                            tracing::info!(
                                name = "agent.tool.success",
                                tool = %func_name,
                                size_bytes = size,
                                "File created successfully. Normal operation."
                            );
                        } else {
                            tracing::info!(
                                name = "agent.tool.success",
                                tool = %func_name,
                                result = %result,
                                "Tool executed successfully. Normal operation."
                            );
                        }

                        let result_msg = if is_error {
                            format!("> **Result Error:** {}\n\n", error_msg)
                        } else if func_name == "create_file" {
                            let size = result_data
                                .get("size_bytes")
                                .and_then(|s| s.as_u64())
                                .unwrap_or(0);
                            format!("> **Result:** File created ({} B).\n\n", size)
                        } else if func_name == "list_files" {
                            let count = result_data
                                .get("files")
                                .and_then(|f| f.as_array())
                                .map(|a| a.len())
                                .unwrap_or(0);
                            let total = result_data
                                .get("total")
                                .and_then(|t| t.as_u64())
                                .unwrap_or(count as u64);
                            format!(
                                "> **Result:** {} files returned (total: {}).\n\n",
                                count, total
                            )
                        } else if func_name == "list_files_by_tag" {
                            let count = result_data
                                .get("files")
                                .and_then(|f| f.as_array())
                                .map(|a| a.len())
                                .unwrap_or(0);
                            let total = result_data
                                .get("total")
                                .and_then(|t| t.as_u64())
                                .unwrap_or(count as u64);
                            format!(
                                "> **Result:** {} files returned (total: {}).\n\n",
                                count, total
                            )
                        } else if func_name == "read_tags" {
                            let count = result_data
                                .get("tags")
                                .and_then(|t| t.as_array())
                                .map(|a| a.len())
                                .unwrap_or(0);
                            format!("> **Result:** {} tag(s) found.\n\n", count)
                        } else if func_name == "read_file" || func_name == "read_file_lines" {
                            let content = result_data
                                .get("content")
                                .and_then(|f| f.as_str())
                                .unwrap_or("");
                            let count = content.lines().count();
                            format!("> **Result:** {} line(s) read.\n\n", count)
                        } else if func_name == "web_fetch" {
                            let content = result_data
                                .get("content")
                                .and_then(|f| f.as_str())
                                .unwrap_or("");
                            let count = content.lines().count();
                            format!("> **Result:** {} markdown lines returned.\n\n", count)
                        } else if func_name == "web_search" {
                            let content = result_data
                                .get("results")
                                .and_then(|f| f.as_str())
                                .unwrap_or("");
                            let count = content
                                .split("\n\n")
                                .filter(|s| !s.trim().is_empty())
                                .count();
                            format!("> **Result:** {} search results returned.\n\n", count)
                        } else if func_name == "grep" {
                            let content = result_data
                                .get("matches")
                                .and_then(|f| f.as_str())
                                .unwrap_or("");
                            if content == "No matches found." || content.is_empty() {
                                format!("> **Result:** 0 file(s) match\n\n")
                            } else {
                                let mut files = std::collections::HashSet::new();
                                for line in content.lines() {
                                    if let Some(colon_idx) = line.rfind(".md:") {
                                        files.insert(&line[..colon_idx + 3]);
                                    } else if let Some(colon_idx) = line.rfind(".markdown:") {
                                        files.insert(&line[..colon_idx + 9]);
                                    } else if let Some(colon_idx) = line.find(':') {
                                        files.insert(&line[..colon_idx]);
                                    }
                                }
                                format!("> **Result:** {} file(s) match\n\n", files.len())
                            }
                        } else if func_name == "get_email_by_id" {
                            let subject = result_data
                                .get("result")
                                .and_then(|v| v.as_str())
                                .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                                .and_then(|val| val.as_array().and_then(|a| a.first().cloned()))
                                .and_then(|obj| {
                                    obj.get("subject")
                                        .and_then(|s| s.as_str())
                                        .map(|s| s.to_string())
                                })
                                .unwrap_or_default();
                            let date = result_data
                                .get("result")
                                .and_then(|v| v.as_str())
                                .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                                .and_then(|val| val.as_array().and_then(|a| a.first().cloned()))
                                .and_then(|obj| {
                                    obj.get("date")
                                        .and_then(|s| s.as_str())
                                        .map(|s| s.to_string())
                                })
                                .unwrap_or_default();
                            if !subject.is_empty() || !date.is_empty() {
                                format!("> **Result:** {} - {}\n\n", date, subject)
                            } else {
                                format!("> **Result:** Email content retrieved.\n\n")
                            }
                        } else if func_name == "search_email" {
                            let total = result_data
                                .get("total")
                                .and_then(|t| t.as_u64())
                                .unwrap_or(0);
                            let hint = result_data
                                .get("hint")
                                .and_then(|h| h.as_str())
                                .unwrap_or("");
                            if !hint.is_empty() {
                                format!("> **Result:** {} item(s) found. {}\n\n", total, hint)
                            } else {
                                format!("> **Result:** {} item(s) found\n\n", total)
                            }
                        } else if func_name.starts_with("search_") {
                            let mut count = 0;
                            if let Some(arr) = result_data
                                .get("results")
                                .and_then(|r| r.as_str())
                                .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                                .and_then(|v| v.as_array().cloned())
                            {
                                count = arr.len();
                            } else if let Some(arr) = result_data.as_array() {
                                count = arr.len();
                            }
                            format!("> **Result:** {} item(s) found\n\n", count)
                        } else if result.len() < 100 && result.lines().count() <= 1 {
                            format!("> **Result:** {}\n\n", result)
                        } else {
                            let action = func_name.replace("_", " ");
                            format!(
                                "> **Result:** Tool '{}' completed successfully.\n\n",
                                action
                            )
                        };
                        full_response.push_str(&result_msg);
                        let _ = tx_gui_agent
                            .send(BackgroundMessage::AgentResponse(full_response.clone()));

                        messages.push(serde_json::json!({
                            "role": "tool",
                            "tool_call_id": call_id,
                            "content": result
                        }));
                    }
                }
            } else {
                break;
            }
        }

        // If the user did not cancel, the loop exited because the LLM
        // produced a final answer (no more tool calls). The last status the
        // UI saw was "Waiting for LLM completions..." from the most recent
        // iteration, so clear it now — otherwise the status line is stuck
        // after the agent has actually finished. We skip this on cancel
        // because the bottom panel already set "Aborted by user."
        // synchronously when the user clicked Stop, and we don't want to
        // clobber that.
        if !cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
            let _ = tx_gui_agent.send(BackgroundMessage::AgentStatus("Done".to_string()));
        }

        let _ = tx_gui_agent.send(BackgroundMessage::AgentFinished(messages));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_thinking_no_delimiter() {
        let text = "Hello world";
        let (thinking, content) = split_thinking_and_content(text);
        assert!(thinking.is_empty());
        assert_eq!(content, "Hello world");
    }

    #[test]
    fn test_split_thinking_with_delimiter() {
        let text = "Before🤔thinking content🤔After";
        let (thinking, content) = split_thinking_and_content(text);
        assert_eq!(thinking, "thinking content");
        assert_eq!(content, "BeforeAfter");
    }

    #[test]
    fn test_split_thinking_only_opening() {
        let text = "Before🤔no closing delimiter";
        let (thinking, content) = split_thinking_and_content(text);
        assert!(thinking.is_empty());
        assert_eq!(content, text);
    }

    #[test]
    fn test_split_thinking_empty_thinking() {
        let text = "Before🤔🤔After";
        let (thinking, content) = split_thinking_and_content(text);
        assert!(thinking.is_empty());
        assert_eq!(content, "BeforeAfter");
    }

    #[test]
    fn test_split_thinking_adjacent() {
        let text = "🤔think🤔";
        let (thinking, content) = split_thinking_and_content(text);
        assert_eq!(thinking, "think");
        assert!(content.is_empty());
    }

    #[test]
    fn test_split_thinking_no_surrounding_content() {
        let text = "🤔";
        let (thinking, content) = split_thinking_and_content(text);
        assert!(thinking.is_empty());
        assert_eq!(content, "🤔");
    }

    #[test]
    fn test_get_base_system_prompt_includes_date() {
        let config = crate::config::AppConfig::default();
        let prompt = get_base_system_prompt(&config);
        assert!(prompt.contains("FastMD Agent"));
        assert!(prompt.contains("Today's date and time is"));
    }

    #[test]
    fn test_get_base_system_prompt_with_user_info() {
        let mut config = crate::config::AppConfig::default();
        config.user_name = Some("Alice".to_string());
        config.user_address = Some("123 Main St".to_string());
        config.user_birthdate = Some("1990-01-01".to_string());
        config.user_gender = Some("female".to_string());
        let prompt = get_base_system_prompt(&config);
        assert!(prompt.contains("Alice"));
        assert!(prompt.contains("123 Main St"));
        assert!(prompt.contains("36"));
        assert!(prompt.contains("female"));
    }

    #[test]
    fn test_get_base_system_prompt_with_extension() {
        let mut config = crate::config::AppConfig::default();
        config.system_prompt_extension = Some("Custom instructions.".to_string());
        let prompt = get_base_system_prompt(&config);
        assert!(prompt.contains("Custom instructions."));
    }

    #[test]
    fn test_run_agent_missing_api_key() {
        let mut config = crate::config::AppConfig::default();
        config.models.insert(
            "test".to_string(),
            crate::config::LlmConfig {
                model: "test".to_string(),
                api_url: "http://localhost".to_string(),
                api_key: "".to_string(),
                cost: None,
                use_case: vec!["chat".to_string()],
            },
        );

        let (tx, rx) = std::sync::mpsc::channel();
        let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        run_agent(
            config,
            tx,
            None,
            None,
            std::collections::HashSet::new(),
            "Hello".to_string(),
            cancel_flag,
            None,
            "".to_string(),
            crate::file_events::Bus::new(),
        );

        let msg = rx.recv().unwrap();
        match msg {
            BackgroundMessage::AgentFailed(err) => {
                assert!(err.contains("API key not set"));
            }
            _ => panic!("Expected AgentFailed"),
        }
    }

    #[test]
    fn test_run_agent_network_error() {
        let mut config = crate::config::AppConfig::default();
        config.models.insert(
            "test".to_string(),
            crate::config::LlmConfig {
                model: "test".to_string(),
                api_url: "http://127.0.0.1:0".to_string(),
                api_key: "valid-key".to_string(),
                cost: None,
                use_case: vec!["chat".to_string()],
            },
        );

        let (tx, rx) = std::sync::mpsc::channel();
        let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        run_agent(
            config,
            tx,
            None,
            None,
            std::collections::HashSet::new(),
            "Hello".to_string(),
            cancel_flag,
            None,
            "".to_string(),
            crate::file_events::Bus::new(),
        );

        let mut got_failed = false;
        while let Ok(msg) = rx.recv() {
            match msg {
                BackgroundMessage::AgentFailed(err) => {
                    assert!(err.contains("Network error") || err.contains("timed out"));
                    got_failed = true;
                    break;
                }
                _ => {}
            }
        }
        assert!(got_failed);
    }

    #[test]
    fn test_run_agent_invalid_json_response() {
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                use std::io::{Read, Write};
                let mut buf = [0; 2048];
                let _ = stream.read(&mut buf);
                let response = "HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\n{";
                let _ = stream.write_all(response.as_bytes());
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        });

        let mut config = crate::config::AppConfig::default();
        config.models.insert(
            "test".to_string(),
            crate::config::LlmConfig {
                model: "test".to_string(),
                api_url: format!("http://127.0.0.1:{}", port),
                api_key: "valid-key".to_string(),
                cost: None,
                use_case: vec!["chat".to_string()],
            },
        );

        let (tx, rx) = std::sync::mpsc::channel();
        let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        let mut selected = std::collections::HashSet::new();
        selected.insert(PathBuf::from("other.md"));

        run_agent(
            config,
            tx,
            Some(PathBuf::from("test.md")), // Cover active_file
            None,
            selected, // Cover selected_files
            "Hello".to_string(),
            cancel_flag,
            None,
            "".to_string(),
            crate::file_events::Bus::new(),
        );

        let mut got_failed = false;
        while let Ok(msg) = rx.recv() {
            match msg {
                BackgroundMessage::AgentFailed(err) => {
                    assert!(
                        err.contains("Failed to parse JSON response"),
                        "Expected 'Failed to parse JSON response', got '{}'",
                        err
                    );
                    got_failed = true;
                    break;
                }
                _ => {}
            }
        }
        assert!(got_failed);
    }

    #[test]
    fn test_run_agent_http_status_error() {
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                use std::io::{Read, Write};
                let mut buf = [0; 2048];
                let _ = stream.read(&mut buf);
                let response = "HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\n\r\nbad request";
                let _ = stream.write_all(response.as_bytes());
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        });

        let mut config = crate::config::AppConfig::default();
        config.models.insert(
            "test".to_string(),
            crate::config::LlmConfig {
                model: "test".to_string(),
                api_url: format!("http://127.0.0.1:{}", port),
                api_key: "valid-key".to_string(),
                cost: None,
                use_case: vec!["chat".to_string()],
            },
        );

        let (tx, rx) = std::sync::mpsc::channel();
        let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        run_agent(
            config,
            tx,
            None,
            Some(PathBuf::from("dir")), // cover active_dir
            std::collections::HashSet::new(),
            "Hello".to_string(),
            cancel_flag,
            Some(vec![]), // cover history
            "".to_string(),
            crate::file_events::Bus::new(),
        );

        let mut got_failed = false;
        while let Ok(msg) = rx.recv() {
            match msg {
                BackgroundMessage::AgentFailed(err) => {
                    assert!(
                        err.contains("HTTP 400 error"),
                        "Expected 'HTTP 400 error', got '{}'",
                        err
                    );
                    got_failed = true;
                    break;
                }
                _ => {}
            }
        }
        assert!(got_failed);
    }

    #[test]
    fn test_parse_usage_block_openai_shape() {
        let usage = serde_json::json!({
            "prompt_tokens": 123,
            "completion_tokens": 45,
            "total_tokens": 168,
            "prompt_tokens_details":     { "cached_tokens": 10 },
            "completion_tokens_details": { "reasoning_tokens": 7 }
        });
        let info = parse_usage_block(&usage).expect("expected usage to be parsed");
        assert_eq!(info.prompt_tokens, 123);
        assert_eq!(info.completion_tokens, 45);
        assert_eq!(info.total_tokens, 168);
        assert_eq!(info.cached_tokens, Some(10));
        assert_eq!(info.reasoning_tokens, Some(7));
    }

    #[test]
    fn test_parse_usage_block_anthropic_shape() {
        // Anthropic Messages API style: input_tokens / output_tokens.
        let usage = serde_json::json!({
            "input_tokens": 200,
            "output_tokens": 50
        });
        let info = parse_usage_block(&usage).expect("expected usage to be parsed");
        assert_eq!(info.prompt_tokens, 200);
        assert_eq!(info.completion_tokens, 50);
        // total_tokens is synthesized when missing.
        assert_eq!(info.total_tokens, 250);
        assert_eq!(info.cached_tokens, None);
        assert_eq!(info.reasoning_tokens, None);
    }

    #[test]
    fn test_parse_usage_block_missing_returns_none() {
        let usage = serde_json::json!({});
        assert!(parse_usage_block(&usage).is_none());
    }

    #[test]
    fn test_parse_usage_block_partial_openai() {
        // Some providers only report prompt + completion.
        let usage = serde_json::json!({
            "prompt_tokens": 1,
            "completion_tokens": 2
        });
        let info = parse_usage_block(&usage).expect("expected usage to be parsed");
        assert_eq!(info.prompt_tokens, 1);
        assert_eq!(info.completion_tokens, 2);
        assert_eq!(info.total_tokens, 3);
    }

    #[test]
    fn test_run_agent_missing_choices() {
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                use std::io::{Read, Write};
                let mut buf = [0; 2048];
                let _ = stream.read(&mut buf);
                let body = "{}";
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        });

        let mut config = crate::config::AppConfig::default();
        config.models.insert(
            "test".to_string(),
            crate::config::LlmConfig {
                model: "test".to_string(),
                api_url: format!("http://127.0.0.1:{}", port),
                api_key: "valid-key".to_string(),
                cost: None,
                use_case: vec!["chat".to_string()],
            },
        );

        let (tx, rx) = std::sync::mpsc::channel();
        let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        run_agent(
            config,
            tx,
            None,
            None,
            std::collections::HashSet::new(),
            "Hello".to_string(),
            cancel_flag,
            None,
            "".to_string(),
            crate::file_events::Bus::new(),
        );

        let mut got_failed = false;
        while let Ok(msg) = rx.recv() {
            match msg {
                BackgroundMessage::AgentFailed(err) => {
                    assert!(
                        err.contains("Invalid response schema"),
                        "Expected 'Invalid response schema', got '{}'",
                        err
                    );
                    got_failed = true;
                    break;
                }
                _ => {}
            }
        }
        assert!(got_failed);
    }

    /// Regression test: when the LLM returns a final answer (no tool calls),
    /// the agent must emit an `AgentStatus("Done")` before `AgentFinished`
    /// so the UI's status line is not stuck on "Waiting for LLM
    /// completions..." after the agent has actually completed.
    #[test]
    fn test_run_agent_emits_done_status_on_natural_completion() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        // Body is a valid OpenAI-compatible chat completion with no
        // tool_calls, which causes the agent loop to break and reach the
        // final AgentFinished message.
        let body = serde_json::json!({
            "id": "chatcmpl-test",
            "object": "chat.completion",
            "created": 0,
            "model": "test",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "All done."
                },
                "finish_reason": "stop"
            }]
        })
        .to_string();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        std::thread::spawn(move || {
            // Serve one request and exit; the agent only needs one turn
            // because the response has no tool_calls.
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0; 8192];
                let _ = stream.read(&mut buf);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        });

        let mut config = crate::config::AppConfig::default();
        config.models.insert(
            "test".to_string(),
            crate::config::LlmConfig {
                model: "test".to_string(),
                api_url: format!("http://127.0.0.1:{}", port),
                api_key: "valid-key".to_string(),
                cost: None,
                use_case: vec!["chat".to_string()],
            },
        );

        let (tx, rx) = std::sync::mpsc::channel();
        let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        run_agent(
            config,
            tx,
            None,
            None,
            std::collections::HashSet::new(),
            "Hello".to_string(),
            cancel_flag,
            None,
            "".to_string(),
            crate::file_events::Bus::new(),
        );

        // Collect all messages until the agent signals finished.
        let mut statuses = Vec::new();
        let mut saw_finished = false;
        while let Ok(msg) = rx.recv() {
            match msg {
                BackgroundMessage::AgentStatus(s) => statuses.push(s),
                BackgroundMessage::AgentFinished(_) => {
                    saw_finished = true;
                    // Keep draining in case more messages were queued,
                    // but stop after a short while.
                    break;
                }
                BackgroundMessage::AgentFailed(err) => {
                    panic!("agent failed unexpectedly: {}", err);
                }
                _ => {}
            }
        }
        assert!(saw_finished, "expected AgentFinished to be sent");
        assert!(
            statuses
                .iter()
                .any(|s| s == "Waiting for LLM completions..."),
            "expected the per-iteration 'Waiting for LLM completions...' status, got {:?}",
            statuses
        );
        assert!(
            statuses.iter().any(|s| s == "Done"),
            "expected a terminal 'Done' status to clear the waiting state, got {:?}",
            statuses
        );
        // The 'Done' status must come AFTER the waiting status, so the UI
        // sees the transition in the right order.
        let waiting_idx = statuses
            .iter()
            .position(|s| s == "Waiting for LLM completions...")
            .unwrap();
        let done_idx = statuses.iter().position(|s| s == "Done").unwrap();
        assert!(
            done_idx > waiting_idx,
            "'Done' ({}) must come after 'Waiting' ({})",
            done_idx,
            waiting_idx
        );
    }

    /// Regression test: when the user cancels, the agent must NOT emit a
    /// terminal "Done" status — the bottom panel already set "Aborted by
    /// user." synchronously and the agent should leave that status alone.
    #[test]
    fn test_run_agent_skips_done_status_when_cancelled() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        // Same body as the natural-completion test: a clean final answer.
        let body = serde_json::json!({
            "id": "chatcmpl-test",
            "object": "chat.completion",
            "created": 0,
            "model": "test",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "All done."
                },
                "finish_reason": "stop"
            }]
        })
        .to_string();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0; 8192];
                let _ = stream.read(&mut buf);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        });

        let mut config = crate::config::AppConfig::default();
        config.models.insert(
            "test".to_string(),
            crate::config::LlmConfig {
                model: "test".to_string(),
                api_url: format!("http://127.0.0.1:{}", port),
                api_key: "valid-key".to_string(),
                cost: None,
                use_case: vec!["chat".to_string()],
            },
        );

        let (tx, rx) = std::sync::mpsc::channel();
        // Pre-set the cancel flag so the agent loop's first guard trips
        // and we go straight to the post-loop code path.
        let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));

        run_agent(
            config,
            tx,
            None,
            None,
            std::collections::HashSet::new(),
            "Hello".to_string(),
            cancel_flag,
            None,
            "".to_string(),
            crate::file_events::Bus::new(),
        );

        let mut saw_done = false;
        let mut saw_finished = false;
        while let Ok(msg) = rx.recv() {
            match msg {
                BackgroundMessage::AgentStatus(s) if s == "Done" => saw_done = true,
                BackgroundMessage::AgentFinished(_) => saw_finished = true,
                _ => {}
            }
        }
        assert!(saw_finished, "expected AgentFinished to be sent");
        assert!(
            !saw_done,
            "agent must not emit 'Done' status when cancelled — \
             the UI's 'Aborted by user.' would be clobbered"
        );
    }
}
