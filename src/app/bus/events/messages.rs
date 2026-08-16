//! Cross-cutting value types that do not belong to a single event
//! channel.
//!
//! The per-domain event payloads (`BackgroundEvent`, `FsEvent`,
//! `ProcessEvent`, `FileEvent`, `ConfigArrived`) live in their
//! respective modules under [`super`]. Agent events live in
//! [`crate::agent::events`]. This module is reserved for types
//! shared across channels or independent of any single event domain —
//! currently:
//!
//! - [`TokenUsageInfo`] — the LLM token-usage record attached to
//!   `crate::app::events::AgentEvent::TokenUsage`.
//! - [`BackgroundLogEntry`] / [`LogCategory`] — the structured log
//!   line carried inside [`super::ProcessEvent::LogEntry`]. They used
//!   to live in `app::background::models`; moving them here removes
//!   the `bus → app` layering inversion that resulted from having the
//!   transport define a payload type from a downstream subsystem.
//!
//! See [`crate::bus::core`] for the transport primitive.

use chrono::{DateTime, Local};

pub use crate::agent::events::TokenUsageInfo;

/// Categorises a single [`BackgroundLogEntry`].
///
/// Used by the UI log panel for filter chips and by the per-log
/// row colour-coding. New variants go at the end; the order is the
/// on-disk / on-wire order so appending is a non-breaking change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LogCategory {
    Indexer,
    Watcher,
    PdfConverter,
    ImageVision,
    LlmTools,
    Print,
    Batch,
}

impl std::fmt::Display for LogCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            LogCategory::Indexer => "Indexer",
            LogCategory::Watcher => "Watcher",
            LogCategory::PdfConverter => "PDF Converter",
            LogCategory::ImageVision => "Image Vision",
            LogCategory::LlmTools => "LLM Tools",
            LogCategory::Print => "Print",
            LogCategory::Batch => "Batch",
        };
        write!(f, "{}", s)
    }
}

/// A single structured log line emitted by a background producer
/// and consumed by the UI log panel.
///
/// Carried inside [`crate::bus::events::ProcessEvent::LogEntry`]
/// and produced by every background worker via the
/// `BackgroundEvent::From<BackgroundLogEntry>` impl in
/// [`crate::bus::events::typed`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BackgroundLogEntry {
    pub timestamp: DateTime<Local>,
    pub category: LogCategory,
    pub message: String,
}

impl BackgroundLogEntry {
    /// Build a new entry stamped with the current local time.
    pub fn new(category: LogCategory, message: String) -> Self {
        Self {
            timestamp: Local::now(),
            category,
            message,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_category_display() {
        assert_eq!(LogCategory::Indexer.to_string(), "Indexer");
        assert_eq!(LogCategory::Watcher.to_string(), "Watcher");
        assert_eq!(LogCategory::PdfConverter.to_string(), "PDF Converter");
        assert_eq!(LogCategory::ImageVision.to_string(), "Image Vision");
        assert_eq!(LogCategory::LlmTools.to_string(), "LLM Tools");
        assert_eq!(LogCategory::Print.to_string(), "Print");
        assert_eq!(LogCategory::Batch.to_string(), "Batch");
    }

    #[test]
    fn test_background_log_entry_new() {
        let entry = BackgroundLogEntry::new(LogCategory::Indexer, "test message".to_string());
        assert_eq!(entry.category, LogCategory::Indexer);
        assert_eq!(entry.message, "test message");
    }
}
