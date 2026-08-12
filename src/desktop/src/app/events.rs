use crate::agent::events::{AgentEventObserver, AgentStatus, ToolSideEffect};
use crate::bus::core::Bus;
use crate::bus::events::debug::AgentDebugEntry;
use crate::bus::events::messages::TokenUsageInfo;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

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

/// An observer that serializes observer calls back into `AgentEvent` variants
/// and forwards them to a `Bus<AgentEvent>`.
pub struct BusAgentEventObserver {
    session_id: Uuid,
    bus: Bus<AgentEvent>,
}

impl BusAgentEventObserver {
    /// Create a new observer scoped to `session_id` that sends events to the given `bus`.
    pub fn new(session_id: Uuid, bus: Bus<AgentEvent>) -> Self {
        Self { session_id, bus }
    }

    fn emit(&self, event: AgentEvent) {
        let _ = self.bus.publish(event);
    }
}

impl AgentEventObserver for BusAgentEventObserver {
    fn on_session_started(&self) {
        self.emit(AgentEvent::SessionStarted {
            session_id: self.session_id,
        });
    }

    fn on_session_finished(&self, history: Vec<serde_json::Value>) {
        self.emit(AgentEvent::SessionFinished {
            session_id: self.session_id,
            history,
        });
    }

    fn on_status(&self, status: AgentStatus) {
        self.emit(AgentEvent::Status {
            session_id: self.session_id,
            status,
        });
    }

    fn on_thinking(&self, text: String) {
        self.emit(AgentEvent::Thinking {
            session_id: self.session_id,
            text,
        });
    }

    fn on_content_delta(&self, text: String) {
        self.emit(AgentEvent::ContentDelta {
            session_id: self.session_id,
            text,
        });
    }

    fn on_tool_call_started(&self, id: String, name: String, args: serde_json::Value) {
        self.emit(AgentEvent::ToolCallStarted {
            session_id: self.session_id,
            id,
            name,
            args,
        });
    }

    fn on_tool_result(&self, id: String, name: String, result: serde_json::Value) {
        self.emit(AgentEvent::ToolResult {
            session_id: self.session_id,
            id,
            name,
            result,
        });
    }

    fn on_tool_side_effect(&self, effect: ToolSideEffect) {
        self.emit(AgentEvent::ToolSideEffect {
            session_id: self.session_id,
            effect,
        });
    }

    fn on_debug_entry(&self, entry: AgentDebugEntry) {
        self.emit(AgentEvent::DebugEntry {
            session_id: self.session_id,
            entry,
        });
    }

    fn on_token_usage(&self, usage: TokenUsageInfo) {
        self.emit(AgentEvent::TokenUsage {
            session_id: self.session_id,
            usage,
        });
    }

    fn on_failed(&self, error: String) {
        self.emit(AgentEvent::Failed {
            session_id: self.session_id,
            error,
        });
    }
}
