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

pub fn run_agent(
    tx_gui_agent: Sender<BackgroundMessage>,
    root_path_agent: PathBuf,
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

        let date_str = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let mut system_prompt = format!("You are FastMD Agent, an autonomous assistant helper for managing the Markdown workspace. You can read, create, search, and edit files, fetch web pages, and manage tags using your tools. Help the user achieve their goal by using tools step by step. Respond to the user using Markdown format.\n\nToday's date and time is: {}", date_str);
        if let Some(active) = active_file {
            let rel = active.strip_prefix(&root_path_agent).unwrap_or(&active);
            system_prompt.push_str(&format!(
                " The user is currently viewing the file: {}",
                rel.display()
            ));
        } else if let Some(dir) = active_dir {
            let rel = dir.strip_prefix(&root_path_agent).unwrap_or(&dir);
            system_prompt.push_str(&format!(
                " The user has selected the directory context: {}",
                rel.display()
            ));
        }

        let user_md_path = root_path_agent.join("USER.md");
        if user_md_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&user_md_path) {
                system_prompt.push_str(&format!(
                    "\n\nUser Context:\n{}",
                    content
                ));
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
                .timeout_connect(std::time::Duration::from_secs(30))
                .timeout_read(std::time::Duration::from_secs(120))
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

                    let (formatted_args, _is_empty_args) = match serde_json::from_str::<serde_json::Value>(func_args_str) {
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
                        if let Ok(args_val) = serde_json::from_str::<serde_json::Value>(func_args_str) {
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

                    println!("--- Calling tool {} with arguments ---\n{}", func_name, formatted_args);
                    let result = execute_tool(&config, &root_path_agent, func_name, func_args_str);
                    println!("--- Tool {} returned ---\n{}", func_name, result);

                    let result_msg = if result.starts_with("Error") {
                        format!("> **Result:** {}\n\n", result)
                    } else if func_name == "list_files" {
                        let count = result.lines().count();
                        format!("> **Result:** {} files returned.\n\n", count)
                    } else if func_name == "web_fetch" {
                        let count = result.lines().count();
                        format!("> **Result:** {} markdown lines returned.\n\n", count)
                    } else if func_name == "web_search" {
                        let count = result.split("\n\n").filter(|s| !s.trim().is_empty()).count();
                        format!("> **Result:** {} search results returned.\n\n", count)
                    } else if func_name == "get_email_by_id" {
                        let mut subject = String::new();
                        let mut date = String::new();
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&result) {
                            if let Some(list) = val.as_array() {
                                if let Some(email) = list.first().and_then(|e| e.as_object()) {
                                    subject = email.get("subject").and_then(|s| s.as_str()).unwrap_or("").to_string();
                                    date = email.get("date").and_then(|d| d.as_str()).unwrap_or("").to_string();
                                }
                            }
                        }
                        if !subject.is_empty() || !date.is_empty() {
                            format!("> **Result:** {} - {}\n\n", date, subject)
                        } else {
                            format!("> **Result:** Completed.\n\n")
                        }
                    } else if func_name.starts_with("search_") {
                        let mut count = 0;
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&result) {
                            if let Some(arr) = val.get("results").and_then(|r| r.as_array()) {
                                count = arr.len();
                            } else if let Some(arr) = val.as_array() {
                                count = arr.len();
                            }
                        } else {
                            for block in result.split("--- Client:") {
                                if let Some(idx) = block.find('\n') {
                                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(block[idx..].trim()) {
                                        if let Some(arr) = val.as_array() {
                                            count += arr.len();
                                        }
                                    }
                                }
                            }
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