//! Top-level agent orchestration — sends requests, executes tool calls, and streams results back to the UI.

use crate::context::AgentContext;
use crate::datamark;
use crate::events::{
    AgentDebugEntry, AgentEventObserver, AgentStatus, DebugEntryKind, DebugEntryRow,
};
use crate::llm_client::{LLMClient, parse_usage_block};
use crate::tool_executor::ToolExecutor;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

/// Run an agent session to completion on the current thread.
///
/// Called by the long-lived driver thread (see `agent/manager.rs`) with a
/// per-session `AgentContext` built from an `AgentPrompt`. The driver owns
/// the shared resources; this function runs `run_agent_inner` inline —
/// no inner `std::thread::spawn` (research.md §3, migration step 10).
pub fn run_agent(ctx: AgentContext) {
    run_agent_inner(ctx);
}
fn run_agent_inner(ctx: AgentContext) {
    let llm = match resolve_ll_client(&ctx) {
        Some(c) => c,
        None => return,
    };
    let system_prompts = ctx.system_prompts.clone();
    log_prompt_context(&ctx.active_file, &ctx.active_dir, &ctx.selected_files);
    let mut messages = build_messages(system_prompts, &ctx.prompt, ctx.history.clone());
    ctx.tool_context.rcu(|bundle| {
        let mut new_bundle = (**bundle).clone();
        new_bundle.registry.update_and_refresh(&ctx.agent_config);
        new_bundle
    });
    let tools_json = ctx
        .tool_context
        .load()
        .registry
        .get_schema(&ctx.agent_config, &ctx.prompt);
    let executor = crate::tool_executor::ToolExecutorBuilder::new(
        std::sync::Arc::new(ctx.agent_config.clone()),
        ctx.file_observer.clone(),
        ctx.cache.clone(),
        ctx.tool_context.clone(),
    )
    .with_tool_call_policy(ctx.tool_call_policy.clone())
    .with_uuid_gen(ctx.uuid_gen.clone())
    .build();

    let session_boundary = AgentDebugEntry {
        turn: 0,
        timestamp: chrono::Local::now(),
        kind: DebugEntryKind::Outgoing,
        summary: format!("Session {:8}", ctx.session_id),
        content: None,
        row_type: DebugEntryRow::SessionBoundary,
    };
    publish_debug(&*ctx.observer, session_boundary.clone());
    // New seam: SessionStarted lifecycle event
    ctx.observer.on_session_started();

    let mut turn_number: usize = 0;
    let mut prev_messages_len: usize = 0;
    loop {
        if ctx.cancel_flag.load(Ordering::SeqCst) {
            break;
        }
        match process_turn(
            &llm,
            &ctx,
            &mut messages,
            &tools_json,
            &executor,
            &mut turn_number,
            &mut prev_messages_len,
        ) {
            Turn::Continue => {}
            Turn::Done => break,
            Turn::Failed => {
                ctx.observer.on_session_finished(Vec::new());
                return;
            }
        }
    }
    if !ctx.cancel_flag.load(Ordering::SeqCst) {
        ctx.observer.on_status(AgentStatus::Done);
    }
    ctx.observer.on_session_finished(messages);
}
enum Turn {
    Continue,
    Done,
    Failed,
}
#[allow(clippy::too_many_arguments)]
fn process_turn(
    llm: &LLMClient,
    ctx: &AgentContext,
    messages: &mut Vec<serde_json::Value>,
    tools_json: &serde_json::Value,
    executor: &ToolExecutor,
    turn_number: &mut usize,
    prev_messages_len: &mut usize,
) -> Turn {
    *turn_number += 1;
    let turn = *turn_number;

    ctx.observer.on_status(AgentStatus::AwaitingLlm);

    let tool_count = tools_json.as_array().map(|a| a.len()).unwrap_or(0);
    let delta: Vec<serde_json::Value> = messages[*prev_messages_len..].to_vec();
    let outgoing_entry = AgentDebugEntry {
        turn,
        timestamp: chrono::Local::now(),
        kind: DebugEntryKind::Outgoing,
        summary: format!(
            "Turn {} — Outgoing (+{} messages, {} tools)",
            turn,
            delta.len(),
            tool_count
        ),
        content: Some(serde_json::json!({
            "model": llm.model_name(),
            "max_tokens": llm.max_tokens(),
            "tools": tools_json,
            "new_messages": delta,
        })),
        row_type: DebugEntryRow::Entry,
    };
    publish_debug(&*ctx.observer, outgoing_entry);
    *prev_messages_len = messages.len();

    let resp_val = match llm.chat_completion(messages, tools_json) {
        Ok(v) => v,
        Err(e) => {
            ctx.observer.on_failed(e.user_message());
            return Turn::Failed;
        }
    };

    let incoming_tool_call_count = resp_val
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("tool_calls"))
        .and_then(|tc| tc.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let incoming_entry = AgentDebugEntry {
        turn,
        timestamp: chrono::Local::now(),
        kind: DebugEntryKind::Incoming,
        summary: format!(
            "Turn {} — Incoming (assistant{} {})",
            turn,
            if incoming_tool_call_count > 0 {
                format!(" + {} tool call(s)", incoming_tool_call_count)
            } else {
                String::new()
            },
            if resp_val.get("choices").is_some() {
                "OK"
            } else {
                "no choices"
            },
        ),
        content: Some(resp_val.clone()),
        row_type: DebugEntryRow::Entry,
    };
    publish_debug(&*ctx.observer, incoming_entry);

    emit_usage(&resp_val, &*ctx.observer);
    let message = match extract_message(&resp_val) {
        Some(m) => m,
        None => {
            ctx.observer
                .on_failed("Invalid response schema".to_string());
            return Turn::Failed;
        }
    };
    handle_reasoning(&message, &*ctx.observer);
    handle_content(&message, &*ctx.observer);
    messages.push(message.clone());
    match message.get("tool_calls").and_then(|t| t.as_array()) {
        Some(tc) if !tc.is_empty() => {
            for tool_call in tc {
                let fn_name = tool_call
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("");
                let args = tool_call
                    .get("function")
                    .and_then(|f| f.get("arguments"))
                    .and_then(|a| a.as_str())
                    .unwrap_or("");
                let call_id = tool_call
                    .get("id")
                    .and_then(|i| i.as_str())
                    .unwrap_or("")
                    .to_string();
                let args_value = serde_json::from_str::<serde_json::Value>(args)
                    .unwrap_or(serde_json::Value::String(args.to_string()));
                ctx.observer
                    .on_tool_call_started(call_id, fn_name.to_string(), args_value);
            }
            ctx.observer.on_status(AgentStatus::ExecutingTools);
            let (results, side_effects) = executor.execute_all(tc);
            for effect in side_effects {
                ctx.observer.on_tool_side_effect(effect.clone());
            }
            emit_tool_results_debug(turn, &*ctx.observer, &results);
            process_tool_results(&results, tc, messages, &*ctx.observer);
            Turn::Continue
        }
        _ => Turn::Done,
    }
}
fn resolve_ll_client(ctx: &AgentContext) -> Option<LLMClient> {
    let client = match LLMClient::from_agent_config(&ctx.agent_config, ctx.model_name.as_deref()) {
        Some(c) => c,
        None => {
            ctx.observer
                .on_failed("Model configuration not found".to_string());
            return None;
        }
    };
    if !client.api_key_valid() {
        tracing::warn!(name = "agent.api_key.missing", "Agent run skipped.");
        let err = format!(
            "API key not set. Configure in {} or use `/models`.",
            ctx.agent_config.config_path().display()
        );
        ctx.observer.on_failed(err);
        return None;
    }
    Some(client)
}
fn build_messages(
    system_prompts: Vec<String>,
    prompt: &str,
    history: Option<Vec<serde_json::Value>>,
) -> Vec<serde_json::Value> {
    if let Some(mut existing) = history {
        existing.push(serde_json::json!({"role": "user", "content": prompt}));
        existing
    } else {
        let mut messages: Vec<serde_json::Value> = system_prompts
            .into_iter()
            .map(|sp| serde_json::json!({"role": "system", "content": sp}))
            .collect();
        messages.push(serde_json::json!({"role": "user", "content": prompt}));
        messages
    }
}
fn emit_usage(resp: &serde_json::Value, observer: &dyn AgentEventObserver) {
    if let Some(info) = resp.get("usage").and_then(parse_usage_block) {
        tracing::info!(
            name = "agent.usage",
            prompt_tokens = info.prompt_tokens,
            completion_tokens = info.completion_tokens,
            total_tokens = info.total_tokens,
            "LLM usage."
        );
        observer.on_token_usage(info);
    }
}
fn extract_message(resp: &serde_json::Value) -> Option<serde_json::Value> {
    resp.get("choices")?.get(0)?.get("message").cloned()
}
fn handle_reasoning(message: &serde_json::Value, observer: &dyn AgentEventObserver) {
    if let Some(r) = message.get("reasoning_content").and_then(|r| r.as_str()) {
        observer.on_thinking(r.to_string());
    }
}
fn handle_content(message: &serde_json::Value, observer: &dyn AgentEventObserver) {
    let content_str = message
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("");
    if !content_str.is_empty() {
        observer.on_content_delta(format!("{}\n\n", content_str));
    }
}
fn process_tool_results(
    results: &[(String, String, String, String)],
    tool_calls: &[serde_json::Value],
    messages: &mut Vec<serde_json::Value>,
    observer: &dyn AgentEventObserver,
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
        if let Some((fn_name, _args, result)) = map.remove(&cid) {
            log_tool_result(&fn_name, &result);
            let result_value = serde_json::from_str::<serde_json::Value>(&result)
                .unwrap_or(serde_json::Value::String(result.clone()));
            observer.on_tool_result(cid.clone(), fn_name.clone(), result_value);
            // R1 (Spotlighting): wrap the tool result in a
            // datamark envelope so the LLM treats it as data, not
            // instructions. The user-facing response above is built
            // from the raw `result` (so the chat panel still shows
            // the real content); only the message we push into the
            // conversation history is wrapped.
            //
            // `web_delegate` carries a `tool_call_trace` field that is
            // purely a UI artefact (already pushed into `full_response`
            // above). Strip it from the LLM-bound payload so the model
            // only sees the `status` and `result` fields, avoiding
            // redundant context bloat.
            let llm_result = strip_web_delegate_trace(&fn_name, &result);
            let wrapped = datamark::wrap_tool_result(&fn_name, &llm_result);
            messages.push(serde_json::json!({
                "role": "tool",
                "tool_call_id": cid,
                "content": wrapped
            }));
        }
    }
}
/// Strip the `tool_call_trace` field from a `web_delegate` tool result
/// before the result is handed to the LLM.
///
/// `tool_call_trace` is a UI-only artefact (sub-agent tool-call log
/// formatted for the response window). Returning it to the LLM as part
/// of the tool result bloats the conversation with redundant content
/// that the `result` field already summarises. This helper parses the
/// `ToolResponse::Success { data }` envelope, removes `tool_call_trace`
/// from `data` when present, and re-serialises. On any parse failure the
/// original string is returned unchanged so the LLM still gets a result.
fn strip_web_delegate_trace(fn_name: &str, result: &str) -> String {
    if fn_name != "web_delegate" {
        return result.to_string();
    }
    let mut parsed = match serde_json::from_str::<serde_json::Value>(result) {
        Ok(v) => v,
        Err(_) => return result.to_string(),
    };
    if let Some(data) = parsed.get_mut("data").and_then(|d| d.as_object_mut()) {
        data.remove("tool_call_trace");
    }
    serde_json::to_string(&parsed).unwrap_or_else(|_| result.to_string())
}

/// Log the file and directory context handed to the LLM when a new
/// prompt starts. Emitted once per prompt, from the point where the
/// system prompt is assembled, so the log always reflects what the
/// LLM actually received (AGENT-026).
fn log_prompt_context(
    active_file: &Option<PathBuf>,
    active_dir: &Option<PathBuf>,
    selected_files: &HashSet<PathBuf>,
) {
    tracing::info!(
        name = "agent.prompt.started",
        active_file = ?active_file,
        active_dir = ?active_dir,
        selected_files = ?selected_files,
        "Starting new agent prompt; file and directory context handed to the LLM"
    );
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

fn emit_tool_results_debug(
    turn: usize,
    observer: &dyn AgentEventObserver,
    results: &[(String, String, String, String)],
) {
    let entries: Vec<serde_json::Value> = results
        .iter()
        .map(|(call_id, fn_name, args, result)| {
            let args_value = serde_json::from_str::<serde_json::Value>(args)
                .unwrap_or_else(|_| serde_json::Value::String(args.clone()));
            let result_value = serde_json::from_str::<serde_json::Value>(result)
                .unwrap_or_else(|_| serde_json::Value::String(result.clone()));
            serde_json::json!({
                "call_id": call_id,
                "name": fn_name,
                "arguments": args_value,
                "result": result_value,
            })
        })
        .collect();
    let entry = AgentDebugEntry {
        turn,
        timestamp: chrono::Local::now(),
        kind: DebugEntryKind::ToolResults,
        summary: format!("Turn {} — Tool results ({} tools)", turn, entries.len()),
        content: Some(serde_json::Value::Array(entries)),
        row_type: DebugEntryRow::Entry,
    };
    publish_debug(observer, entry);
}

/// Publish a debug entry on the `Bus<AgentEvent>` as
/// `SeamAgentEvent::DebugEntry`. The legacy `tx_gui` mpsc path was
/// removed at step 5 (T016).
fn publish_debug(observer: &dyn AgentEventObserver, entry: AgentDebugEntry) {
    observer.on_debug_entry(entry);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    /// `MakeWriter` that appends formatted tracing output into a
    /// shared byte buffer so tests can assert on it.
    #[derive(Clone)]
    struct BufferWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for BufferWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'writer> tracing_subscriber::fmt::MakeWriter<'writer> for BufferWriter {
        type Writer = BufferWriter;

        fn make_writer(&'writer self) -> Self::Writer {
            self.clone()
        }
    }

    /// Run `body` under a `fmt` subscriber whose output is captured
    /// into a `String`. Returns everything logged at `INFO` or above
    /// during the call.
    fn capture_log(body: impl FnOnce()) -> String {
        let buf = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::fmt()
            .with_writer(BufferWriter(buf.clone()))
            .with_ansi(false)
            .with_max_level(tracing::level_filters::LevelFilter::INFO)
            .finish();
        tracing::subscriber::with_default(subscriber, body);
        String::from_utf8(buf.lock().unwrap().clone()).unwrap_or_default()
    }

    /// AGENT-026: starting a new prompt must log the file and
    /// directory context handed to the LLM, so operators can see what
    /// context each prompt carried.
    #[test]
    fn test_log_prompt_context_emits_file_and_directory_context() {
        let mut files = HashSet::new();
        files.insert(PathBuf::from("notes.md"));
        let out = capture_log(|| {
            log_prompt_context(
                &Some(PathBuf::from("doc.md")),
                &Some(PathBuf::from("lib")),
                &files,
            );
        });
        assert!(out.contains("agent.prompt.started"), "log: {out}");
        assert!(out.contains("doc.md"), "active file must be logged: {out}");
        assert!(
            out.contains("lib"),
            "active directory must be logged: {out}"
        );
        assert!(
            out.contains("notes.md"),
            "selected files must be logged: {out}"
        );
    }

    /// AGENT-026: with no file or directory context, the log must
    /// still fire and record that the context was empty.
    #[test]
    fn test_log_prompt_context_emits_empty_context() {
        let out = capture_log(|| {
            log_prompt_context(&None, &None, &HashSet::new());
        });
        assert!(out.contains("agent.prompt.started"), "log: {out}");
        assert!(
            out.contains("active_file=None") && out.contains("active_dir=None"),
            "empty context must be logged as None: {out}"
        );
    }

    /// `strip_web_delegate_trace` must remove the `tool_call_trace` field
    /// from a `web_delegate` tool result so it is never sent to the LLM,
    /// while preserving `status` and `result`.
    #[test]
    fn test_strip_web_delegate_trace_removes_trace_field() {
        let result = serde_json::json!({
            "status": "success",
            "data": {
                "result": "Fetched content",
                "tool_call_trace": "**Executing tool `web_fetch`**\n"
            }
        })
        .to_string();
        let stripped = strip_web_delegate_trace("web_delegate", &result);
        let parsed: serde_json::Value =
            serde_json::from_str(&stripped).expect("stripped result is valid JSON");
        assert!(
            parsed
                .get("data")
                .and_then(|d| d.get("tool_call_trace"))
                .is_none(),
            "tool_call_trace must be removed from LLM-bound payload"
        );
        assert_eq!(
            parsed
                .get("data")
                .and_then(|d| d.get("result"))
                .and_then(|r| r.as_str()),
            Some("Fetched content")
        );
        assert_eq!(
            parsed.get("status").and_then(|s| s.as_str()),
            Some("success")
        );
    }

    /// Non-`web_delegate` tools must pass through unchanged — the strip
    /// is scoped to the one tool that carries a `tool_call_trace` field.
    #[test]
    fn test_strip_web_delegate_trace_leaves_other_tools_unchanged() {
        let result = r#"{"status":"success","data":{"bytes":42}}"#.to_string();
        let stripped = strip_web_delegate_trace("read_file", &result);
        assert_eq!(stripped, result);
    }

    /// Malformed JSON must round-trip unchanged rather than panic — the
    /// LLM still receives the original tool result on a parse failure.
    #[test]
    fn test_strip_web_delegate_trace_passthrough_on_parse_error() {
        let result = "not json at all".to_string();
        let stripped = strip_web_delegate_trace("web_delegate", &result);
        assert_eq!(stripped, result);
    }

    /// `web_delegate` result with no `tool_call_trace` field must round-trip
    /// unchanged (e.g. when the delegate produced no sub-agent tool calls).
    #[test]
    fn test_strip_web_delegate_trace_no_trace_field_unchanged() {
        let result = serde_json::json!({
            "status": "success",
            "data": {"result": "answer"}
        })
        .to_string();
        let stripped = strip_web_delegate_trace("web_delegate", &result);
        let parsed: serde_json::Value = serde_json::from_str(&stripped).unwrap();
        assert_eq!(
            parsed
                .get("data")
                .and_then(|d| d.get("result"))
                .and_then(|r| r.as_str()),
            Some("answer")
        );
    }
}
