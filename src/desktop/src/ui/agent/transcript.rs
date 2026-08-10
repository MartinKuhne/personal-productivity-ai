//! UI-owned view model accumulating `AgentEvent` deltas into a displayable,
//! interactable transcript (FR-010).
//!
//! Unit tests live in the sibling `transcript_tests.rs` sidecar.

use crate::agent::events::AgentEvent;
use crate::ui::render::agent_render::{format_tool_call_message, format_tool_result_message};
use uuid::Uuid;

/// Ordered transcript entry — a `Content` block, a `ToolCall` pair, or a
/// `Thinking` block. Accumulated from [`AgentEvent`] deltas by
/// [`AgentTranscript::apply_event`].
#[derive(Debug, Clone)]
pub enum TranscriptBlock {
    /// Accumulated `ContentDelta` text.
    Content { text: String },
    /// A tool call paired with its optional result (matched by `id`).
    ToolCall {
        id: String,
        name: String,
        args: serde_json::Value,
        result: Option<serde_json::Value>,
    },
    /// Accumulated `Thinking` text.
    Thinking { text: String },
}

/// UI-owned view model for one agent session. Accumulates structured
/// [`AgentEvent`] deltas into a flat `content` buffer (for
/// `render_markdown` and `apply_task_toggle`) and ordered `blocks`
/// (for structured rendering in US3).
#[derive(Debug, Clone)]
pub struct AgentTranscript {
    pub session_id: Uuid,
    pub blocks: Vec<TranscriptBlock>,
    pub thinking: String,
    /// Flat content buffer — the markdown string the UI renders.
    /// Matches the pre-refactor `AgentState::response` / `full_response`
    /// buffer for no-regression rendering (US1, SC-002).
    pub content: String,
}

impl AgentTranscript {
    /// Create a new empty transcript for the given session.
    pub fn new(session_id: Uuid) -> Self {
        Self {
            session_id,
            blocks: Vec::new(),
            thinking: String::new(),
            content: String::new(),
        }
    }

    /// Apply an [`AgentEvent`] to the transcript, updating `content`,
    /// `thinking`, and `blocks`.
    ///
    /// Events for a different `session_id` are ignored (UI-side invariant).
    pub fn apply_event(&mut self, event: &AgentEvent) {
        if event.session_id() != self.session_id {
            return;
        }
        match event {
            AgentEvent::ContentDelta { text, .. } => {
                self.content.push_str(text);
                match self.blocks.last_mut() {
                    Some(TranscriptBlock::Content { text: buf }) => buf.push_str(text),
                    _ => self
                        .blocks
                        .push(TranscriptBlock::Content { text: text.clone() }),
                }
            }
            AgentEvent::Thinking { text, .. } => {
                self.thinking.push_str(text);
                match self.blocks.last_mut() {
                    Some(TranscriptBlock::Thinking { text: buf }) => buf.push_str(text),
                    _ => self
                        .blocks
                        .push(TranscriptBlock::Thinking { text: text.clone() }),
                }
            }
            AgentEvent::ToolCallStarted { id, name, args, .. } => {
                // If the agent wrapped a raw JSON string in Value::String,
                // extract the inner string directly (matches the old path
                // which passed the raw `tool_call.function.arguments`
                // string to `format_tool_call_message`). Otherwise, serialize
                // the structured value back to a JSON string.
                let args_str = match args {
                    serde_json::Value::String(s) => s.clone(),
                    _ => serde_json::to_string(args).unwrap_or_default(),
                };
                let formatted = format_tool_call_message(name, &args_str);
                self.content.push_str(&formatted);
                self.blocks.push(TranscriptBlock::ToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    args: args.clone(),
                    result: None,
                });
            }
            AgentEvent::ToolResult {
                id, name, result, ..
            } => {
                let result_str = match result {
                    serde_json::Value::String(s) => s.clone(),
                    _ => serde_json::to_string(result).unwrap_or_default(),
                };
                let formatted = format_tool_result_message(name, &result_str);
                self.content.push_str(&formatted);
                // Update the matching ToolCall block's result.
                for block in self.blocks.iter_mut() {
                    if let TranscriptBlock::ToolCall {
                        id: block_id,
                        result: block_result,
                        ..
                    } = block
                        && block_id == id
                    {
                        *block_result = Some(result.clone());
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    /// Reset the transcript to empty (new session or re-run).
    pub fn reset(&mut self) {
        self.blocks.clear();
        self.thinking.clear();
        self.content.clear();
    }
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `transcript_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "transcript_tests.rs"]
mod tests;
