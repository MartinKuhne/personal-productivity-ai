//! Conversation logger observer — logs user prompts, chat model responses, and mutating tool calls to timestamped markdown files in the system library's Conversations folder via AgentEventObserver methods.
//!
//! Requirements: VFS-110, VFS-111, VFS-112, VFS-113, VFS-114.
//!
//! Unit tests live in the sibling `conversation_logger_tests.rs` sidecar.

use crate::agent::events::{
    AgentDebugEntry, AgentEventObserver, AgentStatus, TokenUsageInfo, ToolSideEffect,
};
use chrono::{DateTime, Local};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

/// Record of an executed mutating/write tool call for conversation logging (VFS-114).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutatingToolCallRecord {
    /// Name of the executed tool (e.g. `create_note`, `patch_note`).
    pub name: String,
    /// Arguments string passed to the tool.
    pub arguments: String,
    /// Execution result string produced by the tool.
    pub result: String,
}

/// Checks if a tool name corresponds to a mutating/write operation (VFS-114).
pub fn is_mutating_tool(name: &str) -> bool {
    matches!(
        name,
        "create_note" | "patch_note" | "insert_into_note" | "move_note" | "delete_note"
    )
}

/// Generates a timestamped markdown filename `YYYY-MM-DD HH-MM-SS.md` (VFS-112).
pub fn generate_conversation_filename(now: DateTime<Local>) -> String {
    format!("{}.md", now.format("%Y-%m-%d %H-%M-%S"))
}

/// Formats a prompt section heading `## Prompt (nnn)` (VFS-113).
pub fn format_prompt_heading(turn_number: usize) -> String {
    format!("## Prompt ({})", turn_number)
}

/// Formats a response section heading `## Response (nnn)` (VFS-113).
pub fn format_response_heading(turn_number: usize) -> String {
    format!("## Response ({})", turn_number)
}

/// Formats mutating tool call records as blockquotes at the end of a response section (VFS-114).
pub fn format_write_tool_calls(records: &[MutatingToolCallRecord]) -> String {
    if records.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for rec in records {
        out.push_str("\n\n> **Executing tool `");
        out.push_str(&rec.name);
        out.push_str("`**\n");
        if !rec.arguments.is_empty() {
            for line in rec.arguments.lines() {
                out.push_str("> ");
                out.push_str(line);
                out.push('\n');
            }
        }
        if !rec.result.is_empty() {
            out.push_str("> **Result (`");
            out.push_str(&rec.name);
            out.push_str("`):**\n");
            for line in rec.result.lines() {
                out.push_str("> ");
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    out
}

/// Formats a turn entry with prompt, response, and mutating tool calls (VFS-111, VFS-113, VFS-114).
pub fn format_turn_entry(
    turn_number: usize,
    prompt: &str,
    response: &str,
    write_tools: &[MutatingToolCallRecord],
) -> String {
    let mut entry = String::new();
    entry.push_str(&format_prompt_heading(turn_number));
    entry.push_str("\n\n");
    entry.push_str(prompt);
    entry.push_str("\n\n");
    entry.push_str(&format_response_heading(turn_number));
    entry.push_str("\n\n");
    entry.push_str(response);
    entry.push_str(&format_write_tool_calls(write_tools));
    entry.push_str("\n\n");
    entry
}

#[derive(Default)]
struct SessionLogState {
    file_path: Option<PathBuf>,
    turn_number: usize,
    started_tool_calls: HashMap<String, (String, String)>,
    mutating_tool_records: Vec<MutatingToolCallRecord>,
}

/// Observer that handles automated conversation logging for agent prompts to the system library (VFS-110..114).
pub struct ConversationLoggerObserver {
    session_id: Uuid,
    conversations_dir: PathBuf,
    state: Mutex<SessionLogState>,
}

impl ConversationLoggerObserver {
    /// Creates a new conversation logger observer for a specific session.
    pub fn new(session_id: Uuid, conversations_dir: PathBuf) -> Self {
        Self {
            session_id,
            conversations_dir,
            state: Mutex::new(SessionLogState::default()),
        }
    }

    /// Returns the session ID being observed.
    pub fn session_id(&self) -> Uuid {
        self.session_id
    }

    /// Returns the active log file path for this session, if created.
    pub fn get_log_path(&self) -> Option<PathBuf> {
        self.state.lock().ok().and_then(|s| s.file_path.clone())
    }

    /// Returns the current turn number for this session.
    pub fn get_turn_number(&self) -> usize {
        self.state.lock().ok().map(|s| s.turn_number).unwrap_or(0)
    }

    fn extract_user_prompt(history: &[serde_json::Value]) -> Option<String> {
        history.iter().rev().find_map(|m| {
            if m.get("role").and_then(|r| r.as_str()) == Some("user") {
                m.get("content")
                    .and_then(|c| c.as_str())
                    .map(|s| s.to_string())
            } else {
                None
            }
        })
    }

    fn extract_assistant_response(history: &[serde_json::Value]) -> String {
        history
            .iter()
            .rev()
            .find_map(|m| {
                if m.get("role").and_then(|r| r.as_str()) == Some("assistant") {
                    m.get("content")
                        .and_then(|c| c.as_str())
                        .map(|s| s.to_string())
                } else {
                    None
                }
            })
            .unwrap_or_default()
    }
}

impl AgentEventObserver for ConversationLoggerObserver {
    fn on_session_started(&self) {}

    fn on_session_finished(&self, history: Vec<serde_json::Value>) {
        let Some(user_prompt) = Self::extract_user_prompt(&history) else {
            return;
        };
        let assistant_response = Self::extract_assistant_response(&history);

        let Ok(mut state) = self.state.lock() else {
            return;
        };

        if let Err(e) = std::fs::create_dir_all(&self.conversations_dir) {
            tracing::error!(
                name = "ui.conversation_logger.dir_create_failed",
                path = %self.conversations_dir.display(),
                error = %e,
                "Failed to create conversations directory."
            );
            return;
        }

        let path = match state.file_path.clone() {
            Some(p) => p,
            None => {
                let filename = generate_conversation_filename(Local::now());
                let p = self.conversations_dir.join(filename);
                state.file_path = Some(p.clone());
                p
            }
        };

        state.turn_number += 1;
        let turn = state.turn_number;
        let entry = format_turn_entry(
            turn,
            &user_prompt,
            &assistant_response,
            &state.mutating_tool_records,
        );

        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
            let _ = file.write_all(entry.as_bytes());
            let _ = file.flush();
        } else {
            tracing::error!(
                name = "ui.conversation_logger.write_failed",
                path = %path.display(),
                "Failed to write to conversation log file."
            );
        }

        state.mutating_tool_records.clear();
        state.started_tool_calls.clear();
    }

    fn on_status(&self, _status: AgentStatus) {}

    fn on_thinking(&self, _text: String) {}

    fn on_content_delta(&self, _text: String) {}

    fn on_tool_call_started(&self, id: String, name: String, args: serde_json::Value) {
        if let Ok(mut state) = self.state.lock() {
            let args_str = match args {
                serde_json::Value::String(s) => s,
                other => serde_json::to_string(&other).unwrap_or_default(),
            };
            state.started_tool_calls.insert(id, (name, args_str));
        }
    }

    fn on_tool_result(&self, id: String, name: String, result: serde_json::Value) {
        if !is_mutating_tool(&name) {
            return;
        }
        if let Ok(mut state) = self.state.lock() {
            let arguments = state
                .started_tool_calls
                .remove(&id)
                .map(|(_, a)| a)
                .unwrap_or_default();
            let result_str = match result {
                serde_json::Value::String(s) => s,
                other => serde_json::to_string(&other).unwrap_or_default(),
            };
            state.mutating_tool_records.push(MutatingToolCallRecord {
                name,
                arguments,
                result: result_str,
            });
        }
    }

    fn on_tool_side_effect(&self, _effect: ToolSideEffect) {}

    fn on_debug_entry(&self, _entry: AgentDebugEntry) {}

    fn on_token_usage(&self, _usage: TokenUsageInfo) {}

    fn on_failed(&self, _error: String) {}
}

#[cfg(test)]
#[path = "conversation_logger_tests.rs"]
mod tests;
