use crate::config::get_config_path;
use crate::tools::{execute_tool, get_tools_schema};
use crate::messages::BackgroundMessage;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use ureq;

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
    let mut system_prompt = format!("You are FastMD Agent, an autonomous assistant helper for managing the Markdown workspace. You can read, create, search, and edit files, fetch web pages, and manage tags using your tools. Help the user achieve their goal by using tools step by step. Respond to the user using Markdown format.\n\nCRITICAL: Avoid context bloat! Do NOT use the `read_file` tool on multiple files in a single step. Always prefer `read_yaml_header` to survey documents, or `grep` to extract specific information without reading entire files.\n\nToday's date and time is: {}", date_str);

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
            if today.month() < parsed_date.month() || (today.month() == parsed_date.month() && today.day() < parsed_date.day()) {
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
            system_prompt.push_str(&format!(
                " The user is currently viewing the file: {}",
                rel
            ));
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
                        lib.name,
                        content
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

            let response = match agent.post(&format!(
                "{}/chat/completions",
                api_url.trim_matches('"').trim_end_matches('/')
            ))
            .set("Authorization", &format!("Bearer {}", api_key))
            .set("Content-Type", "application/json")
            .send_json(request_body)
            {
                Ok(resp) => resp,
                Err(ureq::Error::Status(code, resp)) => {
                    let body = resp.into_string().unwrap_or_else(|_| "[Could not read body]".to_string());
                    tracing::error!(
                        name = "agent.api.failed",
                        status = code,
                        response = %body,
                        "Failed to get chat completion from AI API. This may cause the agent to stop responding. Likely cause: invalid API key, rate limiting, or API downtime. Operator should check the configured API key and API provider status."
                    );
                    let _ = tx_gui_agent.send(BackgroundMessage::AgentFailed(format!(
                        "HTTP Request failed with status {}: {}",
                        code, body
                    )));
                    return;
                }
                Err(e) => {
                    tracing::error!(
                        name = "agent.api.network_error",
                        error = %e,
                        "Network request to AI API failed completely. Agent cannot proceed. Likely cause: no internet connection or DNS failure. Operator should check network connectivity."
                    );
                    let _ = tx_gui_agent.send(BackgroundMessage::AgentFailed(format!(
                        "HTTP Request failed: {}",
                        e
                    )));
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
                    let func_name = tool_call.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()).unwrap_or("");
                    let is_safe = matches!(func_name, "grep" | "read_tags" | "list_files_by_tag" | "list_files" | "read_file" | "read_file_lines" | "web_fetch" | "read_yaml_header" | "web_search" | "search_calendar" | "get_calendar" | "get_calendar_item" | "search_email" | "get_email_by_id" | "get_email" | "search_contact" | "get_contact");
                    if is_safe {
                        safe_calls.push(tool_call.clone());
                    } else {
                        unsafe_calls.push(tool_call.clone());
                    }
                }

                let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
                let config_arc = std::sync::Arc::new(config.clone());
                let root_path_arc = std::sync::Arc::new(PathBuf::new());

                let mut completed_results = Vec::new();

                rt.block_on(async {
                    let mut join_set = tokio::task::JoinSet::new();
                    for tool_call in safe_calls {
                        let call_id = tool_call.get("id").and_then(|id| id.as_str()).unwrap_or("").to_string();
                        let func_name = tool_call.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()).unwrap_or("").to_string();
                        let func_args_str = tool_call.get("function").and_then(|f| f.get("arguments")).and_then(|a| a.as_str()).unwrap_or("{}").to_string();
                        let cfg = config_arc.clone();
                        let rp = root_path_arc.clone();
                        let tool_call_clone = tool_call.clone();
                        
                        join_set.spawn_blocking(move || {
                            let result = execute_tool(&cfg, &rp, &func_name, &func_args_str);
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
                    let call_id = tool_call.get("id").and_then(|id| id.as_str()).unwrap_or("").to_string();
                    let func_name = tool_call.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()).unwrap_or("").to_string();
                    let func_args_str = tool_call.get("function").and_then(|f| f.get("arguments")).and_then(|a| a.as_str()).unwrap_or("{}").to_string();
                    
                    let result = execute_tool(&config, &PathBuf::new(), &func_name, &func_args_str);
                    completed_results.push((tool_call.clone(), call_id, func_name, func_args_str, result));
                }

                let mut results_map = std::collections::HashMap::new();
                for (tool_call, call_id, func_name, func_args_str, result) in completed_results {
                    results_map.insert(call_id, (tool_call, func_name, func_args_str, result));
                }

                for tool_call in tool_calls {
                    let call_id = tool_call.get("id").and_then(|id| id.as_str()).unwrap_or("").to_string();
                    if let Some((_tc, func_name, func_args_str, result)) = results_map.remove(&call_id) {
                        let (formatted_args, _is_empty_args) = match serde_json::from_str::<serde_json::Value>(&func_args_str) {
                            Ok(val) => {
                                let empty = match &val {
                                    serde_json::Value::Object(o) => o.is_empty(),
                                    serde_json::Value::Array(a) => a.is_empty(),
                                    _ => false,
                                };
                                (serde_json::to_string_pretty(&val).unwrap_or_else(|_| func_args_str.to_string()), empty)
                            },
                            Err(_) => (func_args_str.to_string(), func_args_str.trim() == "{}"),
                        };

                        let formatted_args_quoted = formatted_args
                            .lines()
                            .map(|line| format!("> {}", line))
                            .collect::<Vec<_>>()
                            .join("\n");

                        let tool_msg = if func_name == "create_file" {
                            let mut msg = format!("> **Executing tool `{}`**\n", func_name);
                            if let Ok(args_val) = serde_json::from_str::<serde_json::Value>(&func_args_str) {
                                let path = args_val.get("path").and_then(|p| p.as_str()).unwrap_or("unknown");
                                msg.push_str(&format!("> Path: `{}`\n", path));
                            }
                            msg
                        } else {
                            format!("> **Executing tool `{}`**\n{}", func_name, formatted_args_quoted)
                        };
                        full_response.push_str(&tool_msg);
                        full_response.push_str("\n\n");
                        let _ = tx_gui_agent.send(BackgroundMessage::AgentResponse(full_response.clone()));

                        let mut is_error = false;
                        let mut error_msg = String::new();
                        let mut result_data = serde_json::Value::Null;
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&result) {
                            if let Some(status) = parsed.get("status").and_then(|s| s.as_str()) {
                                if status == "error" {
                                    is_error = true;
                                    error_msg = parsed.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error").to_string();
                                } else if status == "success" {
                                    if let Some(data) = parsed.get("data") {
                                        result_data = data.clone();
                                    }
                                }
                            }
                        }

                        if func_name == "read_file" && !is_error {
                            let content = result_data.get("content").and_then(|c| c.as_str()).unwrap_or("");
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
                            let size = result_data.get("size_bytes").and_then(|s| s.as_u64()).unwrap_or(0);
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
                            let size = result_data.get("size_bytes").and_then(|s| s.as_u64()).unwrap_or(0);
                            format!("> **Result:** File created ({} B).\n\n", size)
                        } else if func_name == "list_files" {
                            let content = result_data.get("files").and_then(|f| f.as_str()).unwrap_or("");
                            let count = content.lines().count();
                            format!("> **Result:** {} files returned.\n\n", count)
                        } else if func_name == "read_file" || func_name == "read_file_lines" {
                            let content = result_data.get("content").and_then(|f| f.as_str()).unwrap_or("");
                            let count = content.lines().count();
                            format!("> **Result:** {} line(s) read.\n\n", count)
                        } else if func_name == "web_fetch" {
                            let content = result_data.get("content").and_then(|f| f.as_str()).unwrap_or("");
                            let count = content.lines().count();
                            format!("> **Result:** {} markdown lines returned.\n\n", count)
                        } else if func_name == "web_search" {
                            let content = result_data.get("results").and_then(|f| f.as_str()).unwrap_or("");
                            let count = content.split("\n\n").filter(|s| !s.trim().is_empty()).count();
                            format!("> **Result:** {} search results returned.\n\n", count)
                        } else if func_name == "grep" {
                            let content = result_data.get("matches").and_then(|f| f.as_str()).unwrap_or("");
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
                            let subject = result_data.get("result").and_then(|v| v.as_str()).and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                                .and_then(|val| val.as_array().and_then(|a| a.first().cloned()))
                                .and_then(|obj| obj.get("subject").and_then(|s| s.as_str()).map(|s| s.to_string()))
                                .unwrap_or_default();
                            let date = result_data.get("result").and_then(|v| v.as_str()).and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                                .and_then(|val| val.as_array().and_then(|a| a.first().cloned()))
                                .and_then(|obj| obj.get("date").and_then(|s| s.as_str()).map(|s| s.to_string()))
                                .unwrap_or_default();
                            if !subject.is_empty() || !date.is_empty() {
                                format!("> **Result:** {} - {}\n\n", date, subject)
                            } else {
                                format!("> **Result:** Email content retrieved.\n\n")
                            }
                        } else if func_name.starts_with("search_") {
                            let mut count = 0;
                            if let Some(arr) = result_data.get("results").and_then(|r| r.as_str()).and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok()).and_then(|v| v.as_array().cloned()) {
                                count = arr.len();
                            } else if let Some(arr) = result_data.as_array() {
                                count = arr.len();
                            }
                            format!("> **Result:** {} item(s) found\n\n", count)
                        } else if result.len() < 100 && result.lines().count() <= 1 {
                            format!("> **Result:** {}\n\n", result)
                        } else {
                            let action = func_name.replace("_", " ");
                            format!("> **Result:** Tool '{}' completed successfully.\n\n", action)
                        };
                        full_response.push_str(&result_msg);
                        let _ = tx_gui_agent.send(BackgroundMessage::AgentResponse(full_response.clone()));

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
        config.models.insert("test".to_string(), crate::config::LlmConfig {
            model: "test".to_string(),
            api_url: "http://localhost".to_string(),
            api_key: "".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        });
        
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
        config.models.insert("test".to_string(), crate::config::LlmConfig {
            model: "test".to_string(),
            api_url: "http://127.0.0.1:0".to_string(),
            api_key: "valid-key".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        });
        
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
        );
        
        let mut got_failed = false;
        while let Ok(msg) = rx.recv() {
            match msg {
                BackgroundMessage::AgentFailed(err) => {
                    assert!(err.contains("HTTP Request failed"));
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
        config.models.insert("test".to_string(), crate::config::LlmConfig {
            model: "test".to_string(),
            api_url: format!("http://127.0.0.1:{}", port),
            api_key: "valid-key".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        });
        
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
        );
        
        let mut got_failed = false;
        while let Ok(msg) = rx.recv() {
            match msg {
                BackgroundMessage::AgentFailed(err) => {
                    assert!(err.contains("Failed to parse JSON response"), "Expected 'Failed to parse JSON response', got '{}'", err);
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
        config.models.insert("test".to_string(), crate::config::LlmConfig {
            model: "test".to_string(),
            api_url: format!("http://127.0.0.1:{}", port),
            api_key: "valid-key".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        });
        
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
        );
        
        let mut got_failed = false;
        while let Ok(msg) = rx.recv() {
            match msg {
                BackgroundMessage::AgentFailed(err) => {
                    assert!(err.contains("HTTP Request failed with status 400"), "Expected 'HTTP Request failed with status 400', got '{}'", err);
                    got_failed = true;
                    break;
                }
                _ => {}
            }
        }
        assert!(got_failed);
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
                let response = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}", body.len(), body);
                let _ = stream.write_all(response.as_bytes());
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        });

        let mut config = crate::config::AppConfig::default();
        config.models.insert("test".to_string(), crate::config::LlmConfig {
            model: "test".to_string(),
            api_url: format!("http://127.0.0.1:{}", port),
            api_key: "valid-key".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        });
        
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
        );
        
        let mut got_failed = false;
        while let Ok(msg) = rx.recv() {
            match msg {
                BackgroundMessage::AgentFailed(err) => {
                    assert!(err.contains("Invalid response schema"), "Expected 'Invalid response schema', got '{}'", err);
                    got_failed = true;
                    break;
                }
                _ => {}
            }
        }
        assert!(got_failed);
    }
}