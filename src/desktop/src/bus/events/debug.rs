//! Agent debug entry types — captures raw LLM API traffic (outgoing messages, incoming responses, tool results) for the agent debug window.

use chrono::{DateTime, Local};
use serde::Serialize;

/// Direction of a debug entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DebugEntryKind {
    /// Net-new messages sent to the LLM this turn (delta vs previous outgoing).
    Outgoing,
    /// Full JSON response received from the LLM.
    Incoming,
    /// Tool results returned after execution.
    ToolResults,
}

/// Row type — distinguishes normal collapsible entries from session dividers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DebugEntryRow {
    /// A normal debug entry (collapsible, with content).
    Entry,
    /// A non-interactive session-divider row.
    SessionBoundary,
}

/// A single debug log entry capturing the raw content of one part of one turn.
#[derive(Debug, Clone, Serialize)]
pub struct AgentDebugEntry {
    /// Monotonic turn number within the session (1-based; 0 for session boundaries).
    pub turn: usize,
    /// Monotonic session number (1-based), incremented on each new prompt.
    pub session: usize,
    /// When the entry was created.
    pub timestamp: DateTime<Local>,
    /// Kind of the entry.
    pub kind: DebugEntryKind,
    /// One-line summary shown in the collapsed row.
    pub summary: String,
    /// Full JSON content shown when expanded. `None` for SessionBoundary.
    pub content: Option<serde_json::Value>,
    /// Whether this row is a session boundary.
    pub row_type: DebugEntryRow,
}
