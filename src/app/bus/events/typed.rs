//! Typed background events — the per-domain event payloads that flow
//! over the single `mpsc::Sender<BackgroundEvent>` channel owned by
//! [`crate::background::Task`].
//!
//! Each variant of [`BackgroundEvent`] wraps a domain-specific
//! sub-enum: [`FsEvent`] for the file watcher and indexer,
//! [`ProcessEvent`] for background workers (PDF converter, image-vision,
//! log entries, file-load results). The UI consumer matches on
//! [`BackgroundEvent`] and dispatches to the per-domain handler.
//! Agent events flow on a separate `Bus<AgentEvent>` (see
//! [`crate::agent::events`]).
//!
//! The non-cloneable `notify::RecommendedWatcher` handle that the
//! file-watcher owns is moved through the
//! [`crate::background::Task::finished_watcher`] slot
//! instead of the message bus.

use crate::bus::events::messages::BackgroundLogEntry;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

type RepaintCallback = Arc<dyn Fn() + Send + Sync>;

/// Sends typed background events and optionally wakes the UI after delivery.
///
/// The callback is deliberately generic so the application layer does not
/// depend on a UI framework. The UI installs its callback at startup.
#[derive(Clone)]
pub struct BackgroundEventSender {
    sender: broadcast::Sender<BackgroundEvent>,
    repaint: Arc<Mutex<Option<RepaintCallback>>>,
}

impl BackgroundEventSender {
    /// Wrap a typed background-event sender without a repaint callback.
    pub fn new(sender: broadcast::Sender<BackgroundEvent>) -> Self {
        Self {
            sender,
            repaint: Arc::new(Mutex::new(None)),
        }
    }

    /// Install the callback used to wake the UI after an event is delivered.
    pub fn set_repaint_callback(&self, callback: RepaintCallback) {
        if let Ok(mut repaint) = self.repaint.lock() {
            *repaint = Some(callback);
        }
    }

    /// Deliver an event and request a repaint only when delivery succeeds.
    pub fn send(
        &self,
        event: BackgroundEvent,
    ) -> Result<(), broadcast::error::SendError<BackgroundEvent>> {
        self.sender.send(event)?;
        if let Ok(callback) = self.repaint.lock()
            && let Some(callback) = callback.as_ref()
        {
            callback();
        }
        Ok(())
    }

    /// Create a new broadcast receiver for this sender.
    /// Each call returns a fresh receiver that sees all messages sent after this call.
    pub fn subscribe(&self) -> broadcast::Receiver<BackgroundEvent> {
        self.sender.subscribe()
    }
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
    /// A complete replacement for the UI's per-file tag index after the
    /// background subscriber detects that it missed file-bus events.
    TagsResynchronized {
        /// Current paths and extracted YAML tags in configured file libraries.
        files: Vec<(std::path::PathBuf, Vec<String>)>,
    },
    FileDeleted {
        path: std::path::PathBuf,
    },
    /// Initial scan completed and the [`notify::RecommendedWatcher`]
    /// handle is now available in
    /// [`crate::background::Task::finished_watcher`].
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
    /// Completion of a user-triggered file operation performed off the UI thread.
    UserFileOperationCompleted(crate::background::UserFileOperationResult),
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
    use crate::background::LogCategory;

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

    #[test]
    fn sender_calls_repaint_callback_after_delivery() {
        let (tx, mut rx) = tokio::sync::broadcast::channel(16);
        let sender = BackgroundEventSender::new(tx);
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let calls_for_callback = Arc::clone(&calls);
        sender.set_repaint_callback(Arc::new(move || {
            calls_for_callback.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }));

        sender.send(FsEvent::Finished.into()).unwrap();

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime for test");
        rt.block_on(async {
            assert!(matches!(
                rx.recv().await.unwrap(),
                BackgroundEvent::Fs(FsEvent::Finished)
            ));
        });
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn sender_calls_repaint_callback_even_with_no_subscribers() {
        // With broadcast, send() succeeds even with no subscribers (returns Ok(0)).
        // The repaint callback is called after a successful send.
        let (tx, _rx) = tokio::sync::broadcast::channel(16);
        // Drop the receiver so there are no subscribers
        let sender = BackgroundEventSender::new(tx);
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let calls_for_callback = Arc::clone(&calls);
        sender.set_repaint_callback(Arc::new(move || {
            calls_for_callback.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }));

        assert!(sender.send(FsEvent::Finished.into()).is_ok());
        // Callback is called because send() succeeds (broadcast doesn't fail on no subscribers)
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
