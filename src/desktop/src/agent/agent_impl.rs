//! Top-level agent orchestration — builds the system prompt, sends requests, executes tool calls, and streams results back to the UI.

use crate::agent::context::AgentContext;
use crate::agent::datamark;
use crate::agent::events::{AgentEvent as SeamAgentEvent, AgentStatus};
use crate::agent::llm_client::{LLMClient, parse_usage_block};
use crate::agent::prompt_builder::SystemPromptBuilder;
use crate::agent::tool_executor::ToolExecutor;
use crate::ui::render::agent_render::{
    format_tool_call_message, format_tool_result_message, split_thinking_and_content,
};

use crate::bus::core::Bus;
use crate::bus::events::debug::{AgentDebugEntry, DebugEntryKind, DebugEntryRow};
use crate::bus::events::typed::{AgentEvent, BackgroundEvent};
use crate::config::get_config_path;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;
use uuid::Uuid;

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

    let session_boundary = AgentDebugEntry {
        turn: 0,
        session: ctx.session_number,
        timestamp: chrono::Local::now(),
        kind: DebugEntryKind::Outgoing,
        summary: format!("Session {}", ctx.session_number),
        content: None,
        row_type: DebugEntryRow::SessionBoundary,
    };
    dual_publish_debug(
        &ctx.tx_gui,
        &ctx.agent_event_bus,
        ctx.session_id,
        session_boundary.clone(),
    );
    // New seam: SessionStarted lifecycle event
    let _ = ctx.agent_event_bus.publish(SeamAgentEvent::SessionStarted {
        session_id: ctx.session_id,
    });

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
            &mut full_response,
            &executor,
            &mut turn_number,
            &mut prev_messages_len,
        ) {
            Turn::Continue => {}
            Turn::Done => break,
            Turn::Failed => {
                let _ = ctx
                    .agent_event_bus
                    .publish(SeamAgentEvent::SessionFinished {
                        session_id: ctx.session_id,
                    });
                return;
            }
        }
    }
    if !ctx.cancel_flag.load(Ordering::SeqCst) {
        let _ = ctx
            .tx_gui
            .send(BackgroundEvent::from(AgentEvent::Status("Done".into())));
        let _ = ctx.agent_event_bus.publish(SeamAgentEvent::Status {
            session_id: ctx.session_id,
            status: AgentStatus::Done,
        });
    }
    let _ = ctx
        .tx_gui
        .send(BackgroundEvent::from(AgentEvent::Finished(messages)));
    let _ = ctx
        .agent_event_bus
        .publish(SeamAgentEvent::SessionFinished {
            session_id: ctx.session_id,
        });
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
) -> Turn {
    *turn_number += 1;
    let turn = *turn_number;

    let _ = ctx.tx_gui.send(BackgroundEvent::from(AgentEvent::Status(
        "Waiting for LLM completions...".into(),
    )));
    let _ = ctx.agent_event_bus.publish(SeamAgentEvent::Status {
        session_id: ctx.session_id,
        status: AgentStatus::AwaitingLlm,
    });

    let tool_count = tools_json.as_array().map(|a| a.len()).unwrap_or(0);
    let delta: Vec<serde_json::Value> = messages[*prev_messages_len..].to_vec();
    let tx = &ctx.tx_gui;
    let outgoing_entry = AgentDebugEntry {
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
        content: Some(serde_json::json!({
            "model": llm.model_name(),
            "max_tokens": llm.max_tokens(),
            "tools": tools_json,
            "new_messages": delta,
        })),
        row_type: DebugEntryRow::Entry,
    };
    dual_publish_debug(tx, &ctx.agent_event_bus, ctx.session_id, outgoing_entry);
    *prev_messages_len = messages.len();

    let resp_val = match llm.chat_completion(messages, tools_json) {
        Ok(v) => v,
        Err(e) => {
            let _ = ctx
                .tx_gui
                .send(BackgroundEvent::from(AgentEvent::Failed(e.user_message())));
            let _ = ctx.agent_event_bus.publish(SeamAgentEvent::Failed {
                session_id: ctx.session_id,
                error: e.user_message(),
            });
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
        content: Some(resp_val.clone()),
        row_type: DebugEntryRow::Entry,
    };
    dual_publish_debug(tx, &ctx.agent_event_bus, ctx.session_id, incoming_entry);

    emit_usage(&resp_val, &ctx.tx_gui, &ctx.agent_event_bus, ctx.session_id);
    let message = match extract_message(&resp_val) {
        Some(m) => m,
        None => {
            let _ = ctx.tx_gui.send(BackgroundEvent::from(AgentEvent::Failed(
                "Invalid response schema".into(),
            )));
            let _ = ctx.agent_event_bus.publish(SeamAgentEvent::Failed {
                session_id: ctx.session_id,
                error: "Invalid response schema".into(),
            });
            return Turn::Failed;
        }
    };
    handle_reasoning(&message, &ctx.tx_gui, &ctx.agent_event_bus, ctx.session_id);
    handle_content(
        &message,
        full_response,
        &ctx.tx_gui,
        &ctx.agent_event_bus,
        ctx.session_id,
    );
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
                full_response.push_str(&format_tool_call_message(fn_name, args));
                full_response.push_str("\n\n");
                let _ = ctx.tx_gui.send(BackgroundEvent::from(AgentEvent::Response(
                    full_response.clone(),
                )));
                let _ = ctx
                    .agent_event_bus
                    .publish(SeamAgentEvent::ToolCallStarted {
                        session_id: ctx.session_id,
                        id: call_id,
                        name: fn_name.to_string(),
                        args: serde_json::Value::String(args.to_string()),
                    });
            }
            let _ = ctx.tx_gui.send(BackgroundEvent::from(AgentEvent::Status(
                "Executing tools...".into(),
            )));
            let _ = ctx.agent_event_bus.publish(SeamAgentEvent::Status {
                session_id: ctx.session_id,
                status: AgentStatus::ExecutingTools,
            });
            let results = executor.execute_all(tc, &ctx.tx_gui);
            emit_tool_results_debug(
                turn,
                ctx.session_number,
                tx,
                &ctx.agent_event_bus,
                ctx.session_id,
                &results,
            );
            process_tool_results(
                &results,
                tc,
                messages,
                full_response,
                &ctx.tx_gui,
                &ctx.agent_event_bus,
                ctx.session_id,
            );
            Turn::Continue
        }
        _ => Turn::Done,
    }
}
fn resolve_ll_client(ctx: &AgentContext) -> Option<LLMClient> {
    let client = LLMClient::from_config(&ctx.config, ctx.model_name.as_deref())?;
    if !client.api_key_valid() {
        tracing::warn!(name = "agent.api_key.missing", "Agent run skipped.");
        let err = format!(
            "API key not set. Configure in {} or use `/models`.",
            get_config_path().display()
        );
        let _ = ctx
            .tx_gui
            .send(BackgroundEvent::from(AgentEvent::Failed(err.clone())));
        let _ = ctx.agent_event_bus.publish(SeamAgentEvent::Failed {
            session_id: ctx.session_id,
            error: err,
        });
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
fn emit_usage(
    resp: &serde_json::Value,
    tx: &Sender<BackgroundEvent>,
    event_bus: &Bus<SeamAgentEvent>,
    session_id: Uuid,
) {
    if let Some(info) = resp.get("usage").and_then(parse_usage_block) {
        tracing::info!(
            name = "agent.usage",
            prompt_tokens = info.prompt_tokens,
            completion_tokens = info.completion_tokens,
            total_tokens = info.total_tokens,
            "LLM usage."
        );
        let _ = tx.send(BackgroundEvent::from(AgentEvent::TokenUsage(info.clone())));
        let _ = event_bus.publish(SeamAgentEvent::TokenUsage {
            session_id,
            usage: info,
        });
    }
}
fn extract_message(resp: &serde_json::Value) -> Option<serde_json::Value> {
    resp.get("choices")?.get(0)?.get("message").cloned()
}
fn handle_reasoning(
    message: &serde_json::Value,
    tx: &Sender<BackgroundEvent>,
    event_bus: &Bus<SeamAgentEvent>,
    session_id: Uuid,
) {
    if let Some(r) = message.get("reasoning_content").and_then(|r| r.as_str()) {
        let _ = tx.send(BackgroundEvent::from(AgentEvent::Thinking(r.to_string())));
        let _ = event_bus.publish(SeamAgentEvent::Thinking {
            session_id,
            text: r.to_string(),
        });
    }
}
fn handle_content(
    message: &serde_json::Value,
    full_response: &mut String,
    tx: &Sender<BackgroundEvent>,
    event_bus: &Bus<SeamAgentEvent>,
    session_id: Uuid,
) {
    let content_str = message
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("");
    let (thinking, content) = split_thinking_and_content(content_str);
    if !thinking.is_empty() {
        let _ = tx.send(BackgroundEvent::from(AgentEvent::Thinking(
            thinking.clone(),
        )));
        let _ = event_bus.publish(SeamAgentEvent::Thinking {
            session_id,
            text: thinking,
        });
    }
    if !content.is_empty() {
        full_response.push_str(&content);
        full_response.push_str("\n\n");
        let _ = tx.send(BackgroundEvent::from(AgentEvent::Response(
            full_response.clone(),
        )));
        let _ = event_bus.publish(SeamAgentEvent::ContentDelta {
            session_id,
            text: format!("{}\n\n", content),
        });
    }
}
fn process_tool_results(
    results: &[(String, String, String, String)],
    tool_calls: &[serde_json::Value],
    messages: &mut Vec<serde_json::Value>,
    full_response: &mut String,
    tx: &Sender<BackgroundEvent>,
    event_bus: &Bus<SeamAgentEvent>,
    session_id: Uuid,
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
                && let Some(trace) = parsed.get("tool_call_trace").and_then(|t| t.as_str())
                && !trace.is_empty()
            {
                full_response.push_str(trace);
            }
            full_response.push_str(&format_tool_result_message(&fn_name, &result));
            let _ = tx.send(BackgroundEvent::from(AgentEvent::Response(
                full_response.clone(),
            )));
            let result_value = serde_json::from_str::<serde_json::Value>(&result)
                .unwrap_or(serde_json::Value::String(result.clone()));
            let _ = event_bus.publish(SeamAgentEvent::ToolResult {
                session_id,
                id: cid.clone(),
                name: fn_name.clone(),
                result: result_value,
            });
            // R1 (Spotlighting): wrap the tool result in a
            // datamark envelope so the LLM treats it as data, not
            // instructions. The user-facing response above is built
            // from the raw `result` (so the chat panel still shows
            // the real content); only the message we push into the
            // conversation history is wrapped.
            let wrapped = datamark::wrap_tool_result(&fn_name, &result);
            messages.push(serde_json::json!({
                "role": "tool",
                "tool_call_id": cid,
                "content": wrapped
            }));
        }
    }
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
    event_bus: &Bus<SeamAgentEvent>,
    session_id: Uuid,
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
    let entry = AgentDebugEntry {
        turn,
        session,
        timestamp: chrono::Local::now(),
        kind: DebugEntryKind::ToolResults,
        summary: format!("Turn {} — Tool results ({} tools)", turn, entries.len()),
        content: Some(serde_json::Value::Array(entries)),
        row_type: DebugEntryRow::Entry,
    };
    dual_publish_debug(tx, event_bus, session_id, entry);
}

/// Publish a debug entry on both the legacy `tx_gui` mpsc channel (wrapped as
/// `BackgroundEvent::Agent(AgentEvent::DebugEntry)`) and the new
/// `Bus<AgentEvent>` (as `SeamAgentEvent::DebugEntry`). Dual-published during
/// migration steps 2-4; the `tx_gui` path is removed at step 5 (T016).
fn dual_publish_debug(
    tx: &Sender<BackgroundEvent>,
    event_bus: &Bus<SeamAgentEvent>,
    session_id: Uuid,
    entry: AgentDebugEntry,
) {
    let _ = tx.send(entry.clone().into());
    let _ = event_bus.publish(SeamAgentEvent::DebugEntry { session_id, entry });
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
}
