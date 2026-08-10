//! Top-level agent orchestration — builds the system prompt, sends requests, executes tool calls, and streams results back to the UI.

use crate::agent::context::AgentContext;
use crate::agent::datamark;
use crate::agent::llm_client::{LLMClient, parse_usage_block};
use crate::agent::prompt_builder::SystemPromptBuilder;
use crate::agent::response_formatter::{
    format_tool_call_message, format_tool_result_message, split_thinking_and_content,
};
use crate::agent::tool_executor::ToolExecutor;

use crate::bus::events::debug::{AgentDebugEntry, DebugEntryKind, DebugEntryRow};
use crate::bus::events::typed::{AgentEvent, BackgroundEvent};
use crate::config::get_config_path;
use std::collections::HashSet;
use std::path::PathBuf;
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
    log_prompt_context(&ctx.active_file, &ctx.active_dir, &ctx.selected_files);
    let mut messages = build_messages(system_prompt, &ctx.prompt, ctx.history.clone());
    let tools_json = ctx
        .tool_manager
        .write()
        .unwrap()
        .get_tools_schema(&ctx.config, &ctx.prompt);
    let mut full_response = ctx.current_response.clone();
    let executor = ToolExecutor::new(
        ctx.config.clone(),
        ctx.file_event_bus.clone(),
        ctx.browser_session.clone(),
        ctx.pdf_backing.clone(),
        ctx.tool_manager.clone(),
        ctx.uuid_gen.clone(),
    );

    let _ = ctx.tx_gui.send(
        AgentDebugEntry {
            turn: 0,
            session: ctx.session_number,
            timestamp: chrono::Local::now(),
            kind: DebugEntryKind::Outgoing,
            summary: format!("Session {}", ctx.session_number),
            content: None,
            row_type: DebugEntryRow::SessionBoundary,
        }
        .into(),
    );

    let mut turn_number: usize = 0;
    let mut prev_messages_len: usize = 0;
    let mut prev_tools_json: Option<serde_json::Value> = None;
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
            &mut turn_number,
            &mut prev_messages_len,
            &mut prev_tools_json,
        ) {
            Turn::Continue => {}
            Turn::Done => break,
            Turn::Failed => return,
        }
    }
    if !ctx.cancel_flag.load(Ordering::SeqCst) {
        let _ = ctx
            .tx_gui
            .send(BackgroundEvent::from(AgentEvent::Status("Done".into())));
    }
    let _ = ctx
        .tx_gui
        .send(BackgroundEvent::from(AgentEvent::Finished(messages)));
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
    full_response: &mut String,
    executor: &ToolExecutor,
    turn_number: &mut usize,
    prev_messages_len: &mut usize,
    prev_tools_json: &mut Option<serde_json::Value>,
) -> Turn {
    *turn_number += 1;
    let turn = *turn_number;

    let _ = ctx.tx_gui.send(BackgroundEvent::from(AgentEvent::Status(
        "Waiting for LLM completions...".into(),
    )));

    let tool_count = tools_json.as_array().map(|a| a.len()).unwrap_or(0);
    let delta: Vec<serde_json::Value> = messages[*prev_messages_len..].to_vec();
    let tx = &ctx.tx_gui;
    let tools_unchanged = prev_tools_json
        .as_ref()
        .is_some_and(|prev| prev == tools_json);
    let mut content = serde_json::json!({
        "model": llm.model_name(),
        "max_tokens": llm.max_tokens(),
        "tools": tools_json,
        "new_messages": delta,
    });
    if tools_unchanged && let serde_json::Value::Object(ref mut map) = content {
        map.remove("tools");
    }
    *prev_tools_json = Some(tools_json.clone());
    let _ = tx.send(
        AgentDebugEntry {
            turn,
            session: ctx.session_number,
            timestamp: chrono::Local::now(),
            kind: DebugEntryKind::Outgoing,
            summary: format!(
                "Turn {} — Outgoing (+{} messages, {} tools)",
                turn,
                delta.len(),
                tool_count
            ),
            content: Some(unescape_json_strings(content)),
            row_type: DebugEntryRow::Entry,
        }
        .into(),
    );
    *prev_messages_len = messages.len();

    let resp_val = match llm.chat_completion(messages, tools_json) {
        Ok(v) => v,
        Err(e) => {
            let _ = ctx
                .tx_gui
                .send(BackgroundEvent::from(AgentEvent::Failed(e.user_message())));
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
    let _ = tx.send(
        AgentDebugEntry {
            turn,
            session: ctx.session_number,
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
            content: Some(unescape_json_strings(resp_val.clone())),
            row_type: DebugEntryRow::Entry,
        }
        .into(),
    );

    emit_usage(&resp_val, &ctx.tx_gui);
    let message = match extract_message(&resp_val) {
        Some(m) => m,
        None => {
            let _ = ctx.tx_gui.send(BackgroundEvent::from(AgentEvent::Failed(
                "Invalid response schema".into(),
            )));
            return Turn::Failed;
        }
    };
    handle_reasoning(&message, &ctx.tx_gui);
    handle_content(&message, full_response, &ctx.tx_gui);
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
                full_response.push_str(&format_tool_call_message(fn_name, args));
                full_response.push_str("\n\n");
                let _ = ctx.tx_gui.send(BackgroundEvent::from(AgentEvent::Response(
                    full_response.clone(),
                )));
            }
            let _ = ctx.tx_gui.send(BackgroundEvent::from(AgentEvent::Status(
                "Executing tools...".into(),
            )));
            let results = executor.execute_all(tc, &ctx.tx_gui);
            emit_tool_results_debug(turn, ctx.session_number, tx, &results);
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
        let _ = ctx
            .tx_gui
            .send(BackgroundEvent::from(AgentEvent::Failed(format!(
                "API key not set. Configure in {} or use `/models`.",
                get_config_path().display()
            ))));
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
fn emit_usage(resp: &serde_json::Value, tx: &Sender<BackgroundEvent>) {
    if let Some(info) = resp.get("usage").and_then(parse_usage_block) {
        tracing::info!(
            name = "agent.usage",
            prompt_tokens = info.prompt_tokens,
            completion_tokens = info.completion_tokens,
            total_tokens = info.total_tokens,
            "LLM usage."
        );
        let _ = tx.send(BackgroundEvent::from(AgentEvent::TokenUsage(info)));
    }
}
fn extract_message(resp: &serde_json::Value) -> Option<serde_json::Value> {
    resp.get("choices")?.get(0)?.get("message").cloned()
}
fn handle_reasoning(message: &serde_json::Value, tx: &Sender<BackgroundEvent>) {
    if let Some(r) = message.get("reasoning_content").and_then(|r| r.as_str()) {
        let _ = tx.send(BackgroundEvent::from(AgentEvent::Thinking(r.to_string())));
    }
}
fn handle_content(
    message: &serde_json::Value,
    full_response: &mut String,
    tx: &Sender<BackgroundEvent>,
) {
    let content_str = message
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("");
    let (thinking, content) = split_thinking_and_content(content_str);
    if !thinking.is_empty() {
        let _ = tx.send(BackgroundEvent::from(AgentEvent::Thinking(thinking)));
    }
    if !content.is_empty() {
        full_response.push_str(&content);
        full_response.push_str("\n\n");
        let _ = tx.send(BackgroundEvent::from(AgentEvent::Response(
            full_response.clone(),
        )));
    }
}
fn process_tool_results(
    results: &[(String, String, String, String)],
    tool_calls: &[serde_json::Value],
    messages: &mut Vec<serde_json::Value>,
    full_response: &mut String,
    tx: &Sender<BackgroundEvent>,
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
            if fn_name == "web_delegate"
                && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&result)
                && let Some(trace) = parsed
                    .get("data")
                    .and_then(|d| d.get("tool_call_trace"))
                    .and_then(|t| t.as_str())
                && !trace.is_empty()
            {
                full_response.push_str(trace);
            }
            full_response.push_str(&format_tool_result_message(&fn_name, &result));
            let _ = tx.send(BackgroundEvent::from(AgentEvent::Response(
                full_response.clone(),
            )));
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
    session: usize,
    tx: &Sender<BackgroundEvent>,
    results: &[(String, String, String, String)],
) {
    let entries: Vec<serde_json::Value> = results
        .iter()
        .map(|(call_id, fn_name, args, result)| {
            serde_json::json!({
                "call_id": call_id,
                "name": fn_name,
                "arguments": args,
                "result": result,
            })
        })
        .collect();
    let _ = tx.send(
        AgentDebugEntry {
            turn,
            session,
            timestamp: chrono::Local::now(),
            kind: DebugEntryKind::ToolResults,
            summary: format!("Turn {} — Tool results ({} tools)", turn, entries.len()),
            content: Some(unescape_json_strings(serde_json::Value::Array(entries))),
            row_type: DebugEntryRow::Entry,
        }
        .into(),
    );
}

/// Recursively walk a JSON value and, for any string that itself contains
/// valid JSON (an object or array), replace it with the parsed value.
///
/// The OpenAI tool-call wire format encodes `function.arguments` as a
/// JSON-encoded *string* rather than a nested object, and tool results in
/// this crate return `args`/`result` as JSON-encoded strings. Without
/// unescaping, `serde_json::to_string_pretty` renders every inner quote as
/// `\"`, making the debug window's JSON nearly unreadable. This helper
/// flattens those string-encoded JSON payloads so they pretty-print as
/// nested objects/arrays. Strings that do not parse as JSON, or that parse
/// to a scalar, are left untouched.
fn unescape_json_strings(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(mut map) => {
            for v in map.values_mut() {
                *v = unescape_json_strings(std::mem::replace(v, serde_json::Value::Null));
            }
            serde_json::Value::Object(map)
        }
        serde_json::Value::Array(mut arr) => {
            for v in arr.iter_mut() {
                *v = unescape_json_strings(std::mem::replace(v, serde_json::Value::Null));
            }
            serde_json::Value::Array(arr)
        }
        serde_json::Value::String(s) => {
            let trimmed = s.trim_start();
            if (trimmed.starts_with('{') || trimmed.starts_with('['))
                && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&s)
                && (parsed.is_object() || parsed.is_array())
            {
                return unescape_json_strings(parsed);
            }
            serde_json::Value::String(s)
        }
        other => other,
    }
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

    #[test]
    fn test_process_tool_results_injects_web_delegate_trace() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut messages: Vec<serde_json::Value> = Vec::new();
        let mut full_response = String::new();

        let trace = "**Executing tool `web_fetch`**\n{\n  \"url\": \"https://example.com\"\n}\n";
        let result = serde_json::json!({
            "status": "success",
            "data": {
                "result": "Fetched content",
                "tool_call_trace": trace
            }
        })
        .to_string();

        let results = vec![(
            "call_1".to_string(),
            "web_delegate".to_string(),
            r#"{"instruction":"search for foo"}"#.to_string(),
            result,
        )];

        let tool_calls = vec![serde_json::json!({
            "id": "call_1",
            "type": "function",
            "function": {
                "name": "web_delegate",
                "arguments": r#"{"instruction":"search for foo"}"#
            }
        })];

        process_tool_results(
            &results,
            &tool_calls,
            &mut messages,
            &mut full_response,
            &tx,
        );

        let mut responses: Vec<String> = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            if let BackgroundEvent::Agent(AgentEvent::Response(r)) = ev {
                responses.push(r);
            }
        }

        assert!(
            responses
                .iter()
                .any(|r| r.contains("**Executing tool `web_fetch`**")),
            "Expected web_delegate trace in responses. Got: {:?}",
            responses
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

    /// `unescape_json_strings` parses a string-valued `function.arguments`
    /// (OpenAI wire format) into a nested object so it pretty-prints
    /// without escaped quotes.
    #[test]
    fn test_unescape_json_strings_parses_function_arguments() {
        let input = serde_json::json!({
            "id": "call_1",
            "type": "function",
            "function": {
                "name": "run_command",
                "arguments": "{\"command\":\"ls\",\"flags\":[\"-l\",\"-a\"]}",
            }
        });
        let out = unescape_json_strings(input);
        let args = out
            .get("function")
            .and_then(|f| f.get("arguments"))
            .expect("arguments present");
        assert!(
            args.is_object(),
            "arguments should be a parsed object, got: {args}"
        );
        assert_eq!(args.get("command").and_then(|c| c.as_str()), Some("ls"));
        assert_eq!(
            args.get("flags")
                .and_then(|f| f.as_array())
                .map(|a| a.len()),
            Some(2)
        );
    }

    /// Tool-results entries store `args` and `result` as JSON-encoded
    /// strings; both must be parsed so the debug window shows nested JSON.
    #[test]
    fn test_unescape_json_strings_parses_tool_result_args_and_result() {
        let input = serde_json::json!({
            "call_id": "call_1",
            "name": "read_file",
            "arguments": "{\"path\":\"doc.md\"}",
            "result": "{\"status\":\"success\",\"data\":{\"bytes\":42}}",
        });
        let out = unescape_json_strings(input);
        assert!(out.get("arguments").unwrap().is_object());
        assert!(out.get("result").unwrap().is_object());
        assert_eq!(
            out.get("result")
                .and_then(|r| r.get("data"))
                .and_then(|d| d.get("bytes"))
                .and_then(|b| b.as_i64()),
            Some(42)
        );
    }

    /// Strings that do not contain JSON (or that parse to a scalar) must
    /// be left untouched — otherwise we would corrupt normal string fields.
    #[test]
    fn test_unescape_json_strings_leaves_non_json_strings_untouched() {
        let input = serde_json::json!({
            "name": "read_file",
            "summary": "Turn 1 — Outgoing (+2 messages)",
            "leading_whitespace": "   {\"a\":1}",
            "scalar_json": "42",
            "text_starting_with_brace": "{not actually json",
            "nested": {
                "deep": "{\"x\":[1,2]}",
                "plain": "hello",
            },
        });
        let out = unescape_json_strings(input);
        assert_eq!(out.get("name").and_then(|n| n.as_str()), Some("read_file"));
        assert_eq!(
            out.get("summary").and_then(|s| s.as_str()),
            Some("Turn 1 — Outgoing (+2 messages)")
        );
        // Leading whitespace before the JSON object is still parsed.
        assert!(out.get("leading_whitespace").unwrap().is_object());
        // A bare-number string is NOT promoted to a number — we only
        // unescape object/array payloads, leaving scalar JSON strings as
        // strings so callers don't lose the original type information.
        assert!(out.get("scalar_json").unwrap().is_string());
        // A string that starts with `{` but is not valid JSON stays a string.
        assert!(out.get("text_starting_with_brace").unwrap().is_string());
        assert!(out.get("nested").unwrap().get("deep").unwrap().is_object());
        assert!(
            out.get("nested")
                .and_then(|n| n.get("deep"))
                .and_then(|d| d.get("x"))
                .unwrap()
                .is_array()
        );
        assert_eq!(
            out.get("nested")
                .and_then(|n| n.get("plain"))
                .and_then(|p| p.as_str()),
            Some("hello")
        );
    }

    /// Round-trip: a pretty-printed unescaped value must not contain a
    /// backslash-escaped quote inside the `arguments` payload. This is the
    /// user-visible behaviour the fix targets.
    #[test]
    fn test_unescape_json_strings_pretty_output_has_no_escaped_quotes() {
        let input = serde_json::json!({
            "function": {
                "name": "run_command",
                "arguments": "{\"command\":\"ls\"}",
            }
        });
        let pretty = serde_json::to_string_pretty(&unescape_json_strings(input)).unwrap();
        assert!(
            !pretty.contains("\\\""),
            "escaped quotes still present in pretty output: {pretty}"
        );
        assert!(pretty.contains("\"command\": \"ls\""));
    }
}
