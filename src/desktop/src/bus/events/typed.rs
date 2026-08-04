//! Typed background events — the per-domain event payloads that flow
//! over the single `mpsc::Sender<BackgroundEvent>` channel owned by
//! [`crate::app::background_task::Task`].
//!
//! Each variant of [`BackgroundEvent`] wraps a domain-specific
//! sub-enum: [`AgentEvent`] for the LLM agent loop, [`FsEvent`] for
//! the file watcher and indexer, [`ProcessEvent`] for background
//! workers (PDF converter, image-vision, log entries, file-load
//! results). The UI consumer matches on [`BackgroundEvent`] and
//! dispatches to the per-domain handler.
//!
//! The non-cloneable `notify::RecommendedWatcher` handle that the
//! file-watcher owns is moved through the
//! [`crate::app::background_task::Task::finished_watcher`] slot
//! instead of the message bus.

use crate::bus::events::messages::{BackgroundLogEntry, TokenUsageInfo};
use serde_json::Value;

/// Agent-domain events emitted by the LLM agent loop.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    Status(String),
    Thinking(String),
    Response(String),
    Finished(Vec<Value>),
    Failed(String),
    /// Emitted after every LLM turn that returns a `usage` block.
    TokenUsage(TokenUsageInfo),
}

/// Filesystem-watcher-domain events emitted by the file watcher,
/// indexer, and tool executor.
#[derive(Debug, Clone)]
pub enum FsEvent {
    FileParsed {
        path: std::path::PathBuf,
        tags: Vec<String>,
    },
    DirParsed {
        path: std::path::PathBuf,
    },
    FileModified {
        path: std::path::PathBuf,
        tags: Vec<String>,
    },
    FileDeleted {
        path: std::path::PathBuf,
    },
    /// Initial scan completed and the [`notify::RecommendedWatcher`]
    /// handle is now available in
    /// [`crate::app::background_task::Task::finished_watcher`].
    Finished,
    /// Initial scan completed but no watcher was created (e.g. the
    /// library root is empty).
    FinishedWithoutWatcher,
}

/// Background-process-domain events emitted by the PDF converter,
/// image-vision worker, file loader, and ad-hoc log producers.
#[derive(Debug, Clone)]
pub enum ProcessEvent {
    LogEntry(BackgroundLogEntry),
    FileLoaded {
        path: std::path::PathBuf,
        content: Result<String, String>,
    },
}

/// MCP OAuth authorization events emitted by the background thread
/// that runs the authorization flow triggered by the Tools dialog
/// "Authenticate" button.
#[derive(Debug, Clone)]
pub enum McpAuthEvent {
    /// The OAuth flow for a server completed (success or failure).
    /// `error` is `None` on success, `Some(message)` on failure.
    Completed {
        server_name: String,
        error: Option<String>,
    },
}

/// Typed event that the UI drains on every frame. Each variant
/// wraps a per-domain sub-enum; the consumer matches here and
/// forwards to the appropriate handler.
#[derive(Debug, Clone)]
pub enum BackgroundEvent {
    Agent(AgentEvent),
    Fs(FsEvent),
    Process(ProcessEvent),
    /// OAuth authorization-flow completion for a single MCP server.
    McpAuth(McpAuthEvent),
}

// ---------------------------------------------------------------------------
// Conversion impls — make the producer side ergonomic.
//
// The most common producer pattern is:
//
// ```ignore
// tx.send(BackgroundEvent::Process(ProcessEvent::LogEntry(
//     BackgroundLogEntry::new(category, message),
// )));
// ```
//
// With the `From` impls below that becomes:
//
// ```ignore
// tx.send(BackgroundLogEntry::new(category, message).into());
// ```
//
// All four domain sub-enums have a `From` impl, plus a `From` for the
// common `BackgroundLogEntry` so `tx.send(entry.into())` is the
// shortest path for log-only producers.
// ---------------------------------------------------------------------------

impl From<AgentEvent> for BackgroundEvent {
    fn from(event: AgentEvent) -> Self {
        Self::Agent(event)
    }
}

impl From<FsEvent> for BackgroundEvent {
    fn from(event: FsEvent) -> Self {
        Self::Fs(event)
    }
}

impl From<ProcessEvent> for BackgroundEvent {
    fn from(event: ProcessEvent) -> Self {
        Self::Process(event)
    }
}

impl From<McpAuthEvent> for BackgroundEvent {
    fn from(event: McpAuthEvent) -> Self {
        Self::McpAuth(event)
    }
}

impl From<BackgroundLogEntry> for BackgroundEvent {
    fn from(entry: BackgroundLogEntry) -> Self {
        Self::Process(ProcessEvent::LogEntry(entry))
    }
}

#[cfg(test)]
mod tests {
    //! Smoke tests for the blanket `From` conversions on
    //! [`BackgroundEvent`]. Producer ergonomics are covered by the
    //! call-site tests in `app/background/*` and `agent/*`.

    use super::*;
    use crate::app::background::LogCategory;

    #[test]
    fn from_log_entry_wraps_process_variant() {
        let entry = BackgroundLogEntry::new(LogCategory::PdfConverter, "msg".into());
        let ev: BackgroundEvent = entry.into();
        match ev {
            BackgroundEvent::Process(ProcessEvent::LogEntry(e)) => {
                assert_eq!(e.message, "msg");
            }
            other => panic!("expected ProcessEvent::LogEntry, got {:?}", other),
        }
    }

    #[test]
    fn from_agent_event_wraps_agent_variant() {
        let ev: BackgroundEvent = AgentEvent::Failed("boom".into()).into();
        assert!(matches!(ev, BackgroundEvent::Agent(AgentEvent::Failed(_))));
    }

    #[test]
    fn from_mcp_auth_event_wraps_variant() {
        let ev: BackgroundEvent = McpAuthEvent::Completed {
            server_name: "srv".to_owned(),
            error: None,
        }
        .into();
        assert!(matches!(
            ev,
            BackgroundEvent::McpAuth(McpAuthEvent::Completed { error: None, .. })
        ));
    }
}
