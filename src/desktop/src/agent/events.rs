//! Agent-domain events and prompt type — the structured payloads that cross
//! the agent↔UI seam on `Bus<AgentEvent>` (agent→UI) and
//! `Sender<AgentPrompt>` (UI→agent).
//!
//! Requirements: AGENT-005 (no UI channel in agent), AGENT-008 (session
//! identity), AGENT-010 (structured deltas), AGENT-011 (no running buffer).
//! See `specs/003-agent-ui-seam-refactor/contracts/agent-seam.md` for the full
//! contract.
//!
//! Unit tests live in the sibling `events_tests.rs` sidecar.

use crate::bus::events::debug::AgentDebugEntry;
use crate::bus::events::messages::TokenUsageInfo;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use uuid::Uuid;

/// Input message: UI → agent on `std::sync::mpsc::Sender<AgentPrompt>`.
///
/// The UI mints `session_id: Uuid::new_v4()` for a new session and reuses it
/// for continuation prompts in the same session (FR-008).
pub struct AgentPrompt {
    /// Identity of the session this prompt belongs to.
    pub session_id: Uuid,
    /// The user's prompt text. MUST be non-empty after trim.
    pub text: String,
    /// UI selection context passed through to tools.
    pub active_file: Option<PathBuf>,
    /// UI selection context passed through to tools.
    pub active_dir: Option<PathBuf>,
    /// UI selection context passed through to tools.
    pub selected_files: HashSet<PathBuf>,
    /// Shared cancel flag for the session. The UI sets this to `true`
    /// to abort the in-progress turn (FR-015). The driver passes it
    /// into the `AgentContext` so the agent loop can poll it.
    pub cancel_flag: Arc<AtomicBool>,
}

/// Typed agent status — replaces the old `AgentEvent::Status(String)` with
/// structured states. Carried inside [`crate::app::events::AgentEvent::Status`].
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum AgentStatus {
    /// Waiting for the LLM to return a response.
    AwaitingLlm,
    /// Executing tool calls returned by the LLM.
    ExecutingTools,
    /// The turn loop ended normally (LLM returned a final response).
    Done,
}

impl AgentStatus {
    /// Human-readable status string for the UI status label (replaces
    /// the old `AgentEvent::Status(String)` free-form strings).
    pub fn display_string(&self) -> &'static str {
        match self {
            AgentStatus::AwaitingLlm => "Waiting for LLM completions...",
            AgentStatus::ExecutingTools => "Executing tools...",
            AgentStatus::Done => "Done",
        }
    }
}

/// A side effect produced by a tool execution that the UI must reissue as a
/// filesystem event. Returned by `ToolExecutor::execute_all` and republished
/// by the agent as `AgentEvent::ToolSideEffect`.
#[derive(Debug, Clone, Serialize)]
pub enum ToolSideEffect {
    /// A file was created by a tool (e.g. `create_note`).
    FileCreated {
        /// Absolute path of the created file.
        path: PathBuf,
        /// Tags extracted from the note's front matter.
        tags: Vec<String>,
    },
}

/// A single sub-agent tool call captured inside a `WebDelegateResponse` —
/// structured per-call data replacing the old string-based trace field.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DelegateToolCall {
    /// The tool function name (e.g. `browser_navigate`).
    pub name: String,
    /// The arguments passed to the tool, as JSON.
    pub args: Value,
    /// The result returned by the tool, as JSON.
    pub result: Value,
}

// ---------------------------------------------------------------------------
// Observer Pattern for AgentEvent
// ---------------------------------------------------------------------------

/// Trait for receiving `AgentEvent`s from the agent loop. Decouples the
/// agent from the concrete transport (like `tokio::sync::broadcast::Sender`).
/// The observer is scoped to a specific session.
pub trait AgentEventObserver: Send + Sync {
    fn on_session_started(&self);
    fn on_session_finished(&self, history: Vec<serde_json::Value>);
    fn on_status(&self, status: AgentStatus);
    fn on_thinking(&self, text: String);
    fn on_content_delta(&self, text: String);
    fn on_tool_call_started(&self, id: String, name: String, args: serde_json::Value);
    fn on_tool_result(&self, id: String, name: String, result: serde_json::Value);
    fn on_tool_side_effect(&self, effect: ToolSideEffect);
    fn on_debug_entry(&self, entry: AgentDebugEntry);
    fn on_token_usage(&self, usage: TokenUsageInfo);
    fn on_failed(&self, error: String);
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `events_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "events_tests.rs"]
mod tests;
