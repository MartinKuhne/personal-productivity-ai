//! Conversation logger — logs user prompts, chat model responses, and write tool calls to timestamped markdown files in the system library's Conversations folder.
//!
//! Requirements: VFS-110, VFS-111, VFS-112, VFS-113, VFS-114.
//!
//! Unit tests live in the sibling `conversation_logger_tests.rs` sidecar.

use crate::tool_executor::ToolCallRecord;
use chrono::{DateTime, Local};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use uuid::Uuid;

/// Generates the standard timestamped filename for a conversation log: `YYYY-MM-DD HH-MM-SS.md` (VFS-112).
pub fn generate_conversation_filename(now: DateTime<Local>) -> String {
    now.format("%Y-%m-%d %H-%M-%S.md").to_string()
}

/// Formats the prompt heading: `## Prompt (nnn)` where nnn is a 1-based index (VFS-113).
pub fn format_prompt_heading(turn: usize) -> String {
    format!("## Prompt ({})", turn)
}

/// Formats the response heading: `## Response (nnn)` where nnn is a 1-based index (VFS-113).
pub fn format_response_heading(turn: usize) -> String {
    format!("## Response ({})", turn)
}

/// Formats a list of write tool call records to be appended at the end of the response section (VFS-114).
pub fn format_write_tool_calls(records: &[ToolCallRecord]) -> String {
    if records.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for r in records {
        out.push_str(&format!("\n\n> **Executing tool `{}`**\n", r.name));
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&r.arguments) {
            if let Some(obj) = val.as_object() {
                for (k, v) in obj {
                    if let Some(s) = v.as_str() {
                        out.push_str(&format!("> {}: `{}`\n", k, s));
                    } else {
                        out.push_str(&format!("> {}: {}\n", k, v));
                    }
                }
            } else {
                out.push_str(&format!("> Arguments: {}\n", r.arguments));
            }
        } else if !r.arguments.is_empty() {
            out.push_str(&format!("> Arguments: {}\n", r.arguments));
        }
        out.push_str(&format!("> **Result (`{}`):** {}", r.name, r.result.trim()));
    }
    out
}

/// Formats a complete turn entry (prompt, response, write tools) for a conversation log file.
pub fn format_turn_entry(
    turn: usize,
    prompt: &str,
    response: &str,
    write_tools: &[ToolCallRecord],
) -> String {
    let mut entry = String::new();
    if turn > 1 {
        entry.push_str("\n\n");
    }
    entry.push_str(&format_prompt_heading(turn));
    entry.push_str("\n\n");
    entry.push_str(prompt.trim());
    entry.push_str("\n\n");
    entry.push_str(&format_response_heading(turn));
    entry.push_str("\n\n");
    entry.push_str(response.trim());
    let tool_str = format_write_tool_calls(write_tools);
    if !tool_str.is_empty() {
        entry.push_str(&tool_str);
    }
    entry.push('\n');
    entry
}

/// Thread-safe manager for tracking active conversation log files and their turn counts across sessions.
#[derive(Default)]
pub struct ConversationLogger {
    sessions: Mutex<HashMap<Uuid, (PathBuf, usize)>>,
}

impl ConversationLogger {
    /// Creates a new, empty `ConversationLogger`.
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    /// Logs a prompt and response turn for the given session ID into a conversation markdown file (VFS-111..114).
    /// Returns the path to the log file written.
    pub fn log_turn(
        &self,
        session_id: Uuid,
        prompt: &str,
        response: &str,
        write_tools: &[ToolCallRecord],
        conversations_dir: &Path,
    ) -> Result<PathBuf, io::Error> {
        let mut sessions = self.sessions.lock().unwrap();
        let (path, turn) = if let Some((path, turn)) = sessions.get_mut(&session_id) {
            let current_turn = *turn;
            *turn += 1;
            (path.clone(), current_turn)
        } else {
            std::fs::create_dir_all(conversations_dir)?;
            let filename = generate_conversation_filename(Local::now());
            let path = conversations_dir.join(filename);
            sessions.insert(session_id, (path.clone(), 2));
            (path, 1)
        };

        let entry = format_turn_entry(turn, prompt, response, write_tools);
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        file.write_all(entry.as_bytes())?;
        file.flush()?;

        Ok(path)
    }

    /// Returns the log file path for a session if one has been initiated.
    pub fn get_session_log_path(&self, session_id: &Uuid) -> Option<PathBuf> {
        self.sessions
            .lock()
            .unwrap()
            .get(session_id)
            .map(|(p, _)| p.clone())
    }

    /// Clears all tracked active sessions (primarily for test cleanup).
    pub fn clear(&self) {
        self.sessions.lock().unwrap().clear();
    }
}

/// Returns a reference to the global `ConversationLogger` singleton.
pub fn global_logger() -> &'static ConversationLogger {
    static LOGGER: OnceLock<ConversationLogger> = OnceLock::new();
    LOGGER.get_or_init(ConversationLogger::new)
}

#[cfg(test)]
#[path = "conversation_logger_tests.rs"]
mod tests;
