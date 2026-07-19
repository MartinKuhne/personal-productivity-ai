use crate::config::{get_config_path, load_config};
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
    let mut system_prompt = format!("You are FastMD Agent, an autonomous assistant helper for managing the Markdown workspace. You can read, create, search, and edit files, fetch web pages, and manage tags using your tools. Help the user achieve their goal by using tools step by step. Respond to the user using Markdown format.\n\nToday's date and time is: {}", date_str);

    if let Some(name) = &config.user_name {
        system_prompt.push_str(&format!("\nUser's Name: {}", name));
    }
    if let Some(address) = &config.user_address {
        system_prompt.push_str(&format!("\nUser's Address: {}", address));
    }
    if let Some(age) = config.user_age {
        system_prompt.push_str(&format!("\nUser's Age: {}", age));
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
    tx_gui_agent: Sender<BackgroundMessage>,
    active_file: Option<PathBuf>,
    active_dir: Option<PathBuf>,
    prompt: String,
    cancel_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    history: Option<Vec<serde_json::Value>>,
    current_response: String,
) {
    std::thread::spawn(move || {
        let config = load_config();
        
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
            let _ = tx_gui_agent.send(BackgroundMessage::AgentFailed(format!(
                "API key not set. Please either configure your API key in {}, or use `/models` to view and switch to a configured model.",
                get_config_path().display()
            )));
            return;
        }

        let mut system_prompt = get_base_system_prompt(&config);
        
        let to_virtual = |path: &PathBuf| -> String {
            for lib in &config.content_libraries {
                if let Ok(rel) = path.strip_prefix(std::path::Path::new(&lib.root_folder)) {
                    return std::path::Path::new(&lib.name).join(rel).to_string_lossy().to_string();
                }
            }
            path.to_string_lossy().to_string()
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

        let tools_json = get_tools_schema(&config);
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
                    tracing::error!(name = "agent.api.failed", "AI API Error: Status {} - Body: {}", code, body);
                    let _ = tx_gui_agent.send(BackgroundMessage::AgentFailed(format!(
                        "HTTP Request failed with status {}: {}",
                        code, body
                    )));
                    return;
                }
                Err(e) => {
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

            if let Some(tool_calls) = message.get("tool_calls").and_then(|t| t.as_array()) {
                if tool_calls.is_empty() {
                    break;
                }

                messages.push(message.clone());

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

                for (_tool_call, call_id, func_name, func_args_str, result) in completed_results {
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
                        tracing::info!(name = "agent.tool.result", "--- Tool {} returned {} lines ---", func_name, content.lines().count());
                    } else {
                        tracing::info!(name = "agent.tool.result", "--- Tool {} returned ---\n{}", func_name, result);
                    }

                    let result_msg = if is_error {
                        format!("> **Result Error:** {}\n\n", error_msg)
                    } else if func_name == "list_files" {
                        let content = result_data.get("files").and_then(|f| f.as_str()).unwrap_or("");
                        let count = content.lines().count();
                        format!("> **Result:** {} files returned.\n\n", count)
                    } else if func_name == "web_fetch" {
                        let content = result_data.get("content").and_then(|f| f.as_str()).unwrap_or("");
                        let count = content.lines().count();
                        format!("> **Result:** {} markdown lines returned.\n\n", count)
                    } else if func_name == "web_search" {
                        let content = result_data.get("results").and_then(|f| f.as_str()).unwrap_or("");
                        let count = content.split("\n\n").filter(|s| !s.trim().is_empty()).count();
                        format!("> **Result:** {} search results returned.\n\n", count)
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
                            format!("> **Result:** Completed.\n\n")
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
                        format!("> **Result:** Completed.\n\n")
                    };
                    full_response.push_str(&result_msg);
                    let _ = tx_gui_agent.send(BackgroundMessage::AgentResponse(full_response.clone()));

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
        config.user_age = Some(30);
        config.user_gender = Some("female".to_string());
        let prompt = get_base_system_prompt(&config);
        assert!(prompt.contains("Alice"));
        assert!(prompt.contains("123 Main St"));
        assert!(prompt.contains("30"));
        assert!(prompt.contains("female"));
    }

    #[test]
    fn test_get_base_system_prompt_with_extension() {
        let mut config = crate::config::AppConfig::default();
        config.system_prompt_extension = Some("Custom instructions.".to_string());
        let prompt = get_base_system_prompt(&config);
        assert!(prompt.contains("Custom instructions."));
    }
}