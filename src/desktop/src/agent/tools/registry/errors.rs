//! Per-group error types recorded by
//! [`ToolRegistry::record_error`](super::ToolRegistry::record_error) and
//! surfaced through the UI dialog.

/// Kinds of errors the [`ToolRegistry`](super::ToolRegistry) tracks per
/// group. See TOOL-021 for the auto-clear semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolErrorKind {
    /// MCP `tools/list` failed for the group's server.
    Discovery,
    /// OAuth flow for the group's server failed (or is required but
    /// the user has not authenticated).
    Authentication,
    /// A tool call inside the group returned an `Err` (the most
    /// recent such error is recorded).
    Execution,
    /// The YAML config entry for the group is malformed.
    ConfigInvalid,
}

/// A single recorded error attached to a [`ToolGroupState`](super::groups::ToolGroupState).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolGroupError {
    /// What went wrong.
    pub kind: ToolErrorKind,
    /// Human-readable message, safe to render in the UI.
    pub message: String,
    /// Unix-epoch milliseconds (from
    /// [`std::time::SystemTime`]) when the
    /// error was recorded. Used for tooltip display only.
    pub occurred_at_unix_ms: i64,
}

impl ToolGroupError {
    /// Convenience constructor that stamps `occurred_at_unix_ms` with
    /// the current wall-clock time.
    pub fn now(kind: ToolErrorKind, message: impl Into<String>) -> Self {
        let occurred_at_unix_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        Self {
            kind,
            message: message.into(),
            occurred_at_unix_ms,
        }
    }
}
