//! Top-level agent orchestration — builds the system prompt, sends requests, executes tool calls, and streams results back to the UI.

use crate::agent::context::AgentContext;
use crate::agent::llm_client::{parse_usage_block, LLMClient};
use crate::agent::prompt_builder::SystemPromptBuilder;
use crate::agent::response_formatter::{
    format_tool_call_message, format_tool_result_message, split_thinking_and_content,
};
use crate::agent::tool_executor::ToolExecutor;
use crate::config::get_config_path;
use crate::messages::BackgroundMessage;
use crate::tools::get_tools_schema;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;

pub fn run_agent(ctx: AgentContext) {
    std::thread::spawn(move || run_agent_inner(ctx));
}
fn run_agent_inner(ctx: AgentContext) {
    let llm = match resolve_ll_client(&ctx) {
        Some(c) => c,
        None => return,
    };
    let system_prompt = SystemPromptBuilder::new(&ctx.config)
        .with_active_file(ctx.active_file.clone())
        .with_active_dir(ctx.active_dir.clone())
        .with_selected_files(ctx.selected_files.clone())
        .build(&ctx.config);
    let mut messages = build_messages(system_prompt, &ctx.prompt, ctx.history.clone());
    let tools_json = get_tools_schema(&ctx.config, &ctx.prompt);
    let mut full_response = ctx.current_response.clone();
    let executor = ToolExecutor::new(ctx.config.clone(), ctx.file_event_bus.clone());
    loop {
        if ctx.cancel_flag.load(Ordering::SeqCst) {
            break;
        }
        match process_turn(
            &llm,
            &ctx,
            &mut messages,
            &tools_json,
            &mut full_response,
            &executor,
        ) {
            Turn::Continue => {}
            Turn::Done => break,
            Turn::Failed => return,
        }
    }
    if !ctx.cancel_flag.load(Ordering::SeqCst) {
        let _ = ctx
            .tx_gui
            .send(BackgroundMessage::AgentStatus("Done".into()));
    }
    let _ = ctx.tx_gui.send(BackgroundMessage::AgentFinished(messages));
}
enum Turn {
    Continue,
    Done,
    Failed,
}
fn process_turn(
    llm: &LLMClient,
    ctx: &AgentContext,
    messages: &mut Vec<serde_json::Value>,
    tools_json: &serde_json::Value,
    full_response: &mut String,
    executor: &ToolExecutor,
) -> Turn {
    let _ = ctx.tx_gui.send(BackgroundMessage::AgentStatus(
        "Waiting for LLM completions...".into(),
    ));
    let resp_val = match llm.chat_completion(messages, tools_json) {
        Ok(v) => v,
        Err(e) => {
            let _ = ctx
                .tx_gui
                .send(BackgroundMessage::AgentFailed(e.user_message()));
            return Turn::Failed;
        }
    };
    emit_usage(&resp_val, &ctx.tx_gui);
    let message = match extract_message(&resp_val) {
        Some(m) => m,
        None => {
            let _ = ctx.tx_gui.send(BackgroundMessage::AgentFailed(
                "Invalid response schema".into(),
            ));
            return Turn::Failed;
        }
    };
    handle_reasoning(&message, &ctx.tx_gui);
    handle_content(&message, full_response, &ctx.tx_gui);
    messages.push(message.clone());
    match message.get("tool_calls").and_then(|t| t.as_array()) {
        Some(tc) if !tc.is_empty() => {
            let results = executor.execute_all(tc, &ctx.tx_gui);
            process_tool_results(&results, tc, messages, full_response, &ctx.tx_gui);
            Turn::Continue
        }
        _ => Turn::Done,
    }
}
fn resolve_ll_client(ctx: &AgentContext) -> Option<LLMClient> {
    let client = LLMClient::from_config(&ctx.config, ctx.model_name.as_deref())?;
    if !client.api_key_valid() {
        tracing::warn!(name = "agent.api_key.missing", "Agent run skipped.");
        let _ = ctx.tx_gui.send(BackgroundMessage::AgentFailed(format!(
            "API key not set. Configure in {} or use `/models`.",
            get_config_path().display()
        )));
        return None;
    }
    Some(client)
}
fn build_messages(
    system_prompt: String,
    prompt: &str,
    history: Option<Vec<serde_json::Value>>,
) -> Vec<serde_json::Value> {
    if let Some(mut existing) = history {
        existing.push(serde_json::json!({"role": "user", "content": prompt}));
        existing
    } else {
        vec![
            serde_json::json!({"role": "system", "content": system_prompt}),
            serde_json::json!({"role": "user", "content": prompt}),
        ]
    }
}
fn emit_usage(resp: &serde_json::Value, tx: &Sender<BackgroundMessage>) {
    if let Some(info) = resp.get("usage").and_then(|u| parse_usage_block(u)) {
        tracing::info!(
            name = "agent.usage",
            prompt_tokens = info.prompt_tokens,
            completion_tokens = info.completion_tokens,
            total_tokens = info.total_tokens,
            "LLM usage."
        );
        let _ = tx.send(BackgroundMessage::AgentTokenUsage(info));
    }
}
fn extract_message(resp: &serde_json::Value) -> Option<serde_json::Value> {
    resp.get("choices")?.get(0)?.get("message").cloned()
}
fn handle_reasoning(message: &serde_json::Value, tx: &Sender<BackgroundMessage>) {
    if let Some(r) = message.get("reasoning_content").and_then(|r| r.as_str()) {
        let _ = tx.send(BackgroundMessage::AgentThinking(r.to_string()));
    }
}
fn handle_content(
    message: &serde_json::Value,
    full_response: &mut String,
    tx: &Sender<BackgroundMessage>,
) {
    let content_str = message
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("");
    let (thinking, content) = split_thinking_and_content(content_str);
    if !thinking.is_empty() {
        let _ = tx.send(BackgroundMessage::AgentThinking(thinking));
    }
    if !content.is_empty() {
        full_response.push_str(&content);
        full_response.push_str("\n\n");
        let _ = tx.send(BackgroundMessage::AgentResponse(full_response.clone()));
    }
}
fn process_tool_results(
    results: &[(String, String, String, String)],
    tool_calls: &[serde_json::Value],
    messages: &mut Vec<serde_json::Value>,
    full_response: &mut String,
    tx: &Sender<BackgroundMessage>,
) {
    let mut map: std::collections::HashMap<String, (String, String, String)> =
        std::collections::HashMap::new();
    for (cid, fn_name, args, result) in results {
        map.insert(cid.clone(), (fn_name.clone(), args.clone(), result.clone()));
    }
    for tc in tool_calls {
        let cid = tc
            .get("id")
            .and_then(|id| id.as_str())
            .unwrap_or("")
            .to_string();
        if let Some((fn_name, args, result)) = map.remove(&cid) {
            log_tool_result(&fn_name, &result);
            full_response.push_str(&format_tool_call_message(&fn_name, &args));
            full_response.push_str("\n\n");
            let _ = tx.send(BackgroundMessage::AgentResponse(full_response.clone()));
            full_response.push_str(&format_tool_result_message(&fn_name, &result));
            let _ = tx.send(BackgroundMessage::AgentResponse(full_response.clone()));
            messages
                .push(serde_json::json!({"role": "tool", "tool_call_id": cid, "content": result}));
        }
    }
}
fn log_tool_result(func_name: &str, result: &str) {
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(result) {
        if parsed.get("status").and_then(|s| s.as_str()) == Some("error") {
            let msg = parsed
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown");
            tracing::warn!(name = "agent.tool.error", tool = %func_name, error = %msg);
        } else {
            tracing::info!(name = "agent.tool.success", tool = %func_name);
        }
    }
}
