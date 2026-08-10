//! Agent-domain events and prompt type — the structured payloads that cross
//! the agent↔UI seam on `Bus<AgentEvent>` (agent→UI) and
//! `Sender<AgentPrompt>` (UI→agent).
//!
//! Requirements: AGENT-005 (no UI channel in agent), AGENT-008 (session
//! identity), AGENT-010 (structured deltas), AGENT-011 (no running buffer).
//! See `specs/003-agent-ui-seam-refactor/contracts/agent-seam.md` for the full
//! contract.

use crate::bus::events::debug::AgentDebugEntry;
use crate::bus::events::messages::TokenUsageInfo;
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;
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
}

/// Typed agent status — replaces the old `AgentEvent::Status(String)` with
/// structured states. Carried inside [`AgentEvent::Status`].
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
/// structured per-call data replacing the old `tool_call_trace: String`.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DelegateToolCall {
    /// The tool function name (e.g. `browser_navigate`).
    pub name: String,
    /// The arguments passed to the tool, as JSON.
    pub args: Value,
    /// The result returned by the tool, as JSON.
    pub result: Value,
}

/// Output message: agent → UI on `Bus<AgentEvent>` (tokio broadcast, capacity
/// 8192). Replaces the old `AgentEvent` enum in `bus/events/typed.rs`.
///
/// Every variant carries `session_id: Uuid` — the UI routes events to the
/// correct session transcript (FR-003, FR-009).
#[derive(Debug, Clone, Serialize)]
pub enum AgentEvent {
    /// Emitted when a new session starts (first event for a `session_id`).
    SessionStarted { session_id: Uuid },
    /// Emitted when a session finishes (last event for a `session_id`).
    /// Carries the final conversation history so the UI can store it for
    /// continuation in the next session (replaces `AgentEvent::Finished`).
    SessionFinished {
        session_id: Uuid,
        history: Vec<serde_json::Value>,
    },
    /// Typed status update bracketing the phases of a turn.
    Status {
        session_id: Uuid,
        status: AgentStatus,
    },
    /// Incremental thinking text from the LLM (reasoning model output).
    Thinking { session_id: Uuid, text: String },
    /// Incremental content chunk from the LLM. The UI accumulates these into
    /// its transcript view model (FR-010). Replaces the old running-buffer
    /// `AgentEvent::Response(full_response.clone())`.
    ContentDelta { session_id: Uuid, text: String },
    /// Emitted before the corresponding `ToolResult` for each tool call.
    ToolCallStarted {
        session_id: Uuid,
        id: String,
        name: String,
        args: Value,
    },
    /// Emitted after a tool completes. For `web_delegate`, `result` contains
    /// `tool_calls: Vec<DelegateToolCall>` (structured, no string trace).
    ToolResult {
        session_id: Uuid,
        id: String,
        name: String,
        result: Value,
    },
    /// Emitted once per successful side-effecting tool execution. The UI
    /// reissues this as `FsEvent::FileModified` (FR-007).
    ToolSideEffect {
        session_id: Uuid,
        effect: ToolSideEffect,
    },
    /// Emitted once per debug entry (outgoing, incoming, tool results) plus a
    /// session-boundary entry at the start of each new session.
    DebugEntry {
        session_id: Uuid,
        entry: AgentDebugEntry,
    },
    /// Emitted after every LLM turn that returns a `usage` block.
    TokenUsage {
        session_id: Uuid,
        usage: TokenUsageInfo,
    },
    /// Emitted when an error terminates the session.
    Failed { session_id: Uuid, error: String },
}

impl AgentEvent {
    /// Returns the `session_id` carried by this event (every variant has one).
    pub fn session_id(&self) -> Uuid {
        match self {
            AgentEvent::SessionStarted { session_id }
            | AgentEvent::SessionFinished { session_id, .. }
            | AgentEvent::Status { session_id, .. }
            | AgentEvent::Thinking { session_id, .. }
            | AgentEvent::ContentDelta { session_id, .. }
            | AgentEvent::ToolCallStarted { session_id, .. }
            | AgentEvent::ToolResult { session_id, .. }
            | AgentEvent::ToolSideEffect { session_id, .. }
            | AgentEvent::DebugEntry { session_id, .. }
            | AgentEvent::TokenUsage { session_id, .. }
            | AgentEvent::Failed { session_id, .. } => *session_id,
        }
    }
}
