//! Typed background events — domain-specific event channels.
//!
//! These exist alongside the legacy [`crate::app::messages::BackgroundMessage`]
//! enum. The long-term goal is to replace the monolithic enum with per-domain
//! typed channels (per the architecture review P1-6), but this pilot only
//! introduces the first domain (`AgentEvent`) and a wrapper that lets the
//! UI consume both representations during the migration.
//!
//! ## Migration strategy
//!
//! 1. **Phase 1 (this change):** define `AgentEvent` and `BackgroundEvent`.
//!    Producers may optionally send `BackgroundEvent` alongside
//!    `BackgroundMessage`. The UI consumer handles both via a single
//!    `match` on the newtype.
//! 2. **Phase 2:** producers switch to sending only `BackgroundEvent`;
//!    `BackgroundMessage` gains `#[deprecated]`.
//! 3. **Phase 3:** `BackgroundMessage` is removed; each domain owns its
//!    own `tokio::sync::broadcast` channel.

use crate::app::messages::TokenUsageInfo;
use serde_json::Value;

/// Agent-domain events — replaces `BackgroundMessage::Agent*` variants.
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

/// Filesystem-watcher-domain events — replaces `BackgroundMessage::{FileParsed, DirParsed, FileModified, FileDeleted, Finished, FinishedWithoutWatcher}`.
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
    Finished,
    FinishedWithoutWatcher,
}

/// Background-process-domain events — replaces `BackgroundMessage::LogEntry` and `BackgroundMessage::FileLoaded`.
#[derive(Debug, Clone)]
pub enum ProcessEvent {
    LogEntry(crate::background::BackgroundLogEntry),
    FileLoaded {
        path: std::path::PathBuf,
        content: Result<String, String>,
    },
}

/// Typed replacement for [`crate::app::messages::BackgroundMessage`].
///
/// Each variant wraps a domain-specific event enum. The UI consumer
/// matches on `BackgroundEvent` and dispatches to domain handlers,
/// replacing the flat `BackgroundMessage` `match` arms.
#[derive(Debug, Clone)]
pub enum BackgroundEvent {
    Agent(AgentEvent),
    Fs(FsEvent),
    Process(ProcessEvent),
}

impl BackgroundEvent {
    /// Convert a legacy [`crate::app::messages::BackgroundMessage`] into
    /// the typed [`BackgroundEvent`]. Returns `None` for variants that
    /// carry a non-cloneable `RecommendedWatcher` handle (the caller
    /// must deal with those separately).
    pub fn from_legacy(msg: &crate::app::messages::BackgroundMessage) -> Option<Self> {
        use crate::app::messages::BackgroundMessage;
        match msg {
            BackgroundMessage::AgentStatus(s) => Some(Self::Agent(AgentEvent::Status(s.clone()))),
            BackgroundMessage::AgentThinking(s) => {
                Some(Self::Agent(AgentEvent::Thinking(s.clone())))
            }
            BackgroundMessage::AgentResponse(s) => {
                Some(Self::Agent(AgentEvent::Response(s.clone())))
            }
            BackgroundMessage::AgentFinished(v) => {
                Some(Self::Agent(AgentEvent::Finished(v.clone())))
            }
            BackgroundMessage::AgentFailed(s) => Some(Self::Agent(AgentEvent::Failed(s.clone()))),
            BackgroundMessage::AgentTokenUsage(t) => {
                Some(Self::Agent(AgentEvent::TokenUsage(t.clone())))
            }
            BackgroundMessage::FileParsed { path, tags } => Some(Self::Fs(FsEvent::FileParsed {
                path: path.clone(),
                tags: tags.clone(),
            })),
            BackgroundMessage::DirParsed { path } => {
                Some(Self::Fs(FsEvent::DirParsed { path: path.clone() }))
            }
            BackgroundMessage::FileModified { path, tags } => {
                Some(Self::Fs(FsEvent::FileModified {
                    path: path.clone(),
                    tags: tags.clone(),
                }))
            }
            BackgroundMessage::FileDeleted { path } => {
                Some(Self::Fs(FsEvent::FileDeleted { path: path.clone() }))
            }
            BackgroundMessage::FinishedWithoutWatcher => {
                Some(Self::Fs(FsEvent::FinishedWithoutWatcher))
            }
            BackgroundMessage::Finished(_) => None,
            BackgroundMessage::LogEntry(e) => {
                Some(Self::Process(ProcessEvent::LogEntry(e.clone())))
            }
            BackgroundMessage::FileLoaded { path, content } => {
                Some(Self::Process(ProcessEvent::FileLoaded {
                    path: path.clone(),
                    content: content.clone(),
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::messages::TokenUsageInfo;
    use std::path::PathBuf;

    #[test]
    fn test_agent_status_from_legacy() {
        let msg = crate::app::messages::BackgroundMessage::AgentStatus("Running".into());
        let event = BackgroundEvent::from_legacy(&msg).unwrap();
        assert!(matches!(event, BackgroundEvent::Agent(AgentEvent::Status(s)) if s == "Running"));
    }

    #[test]
    fn test_agent_token_usage_from_legacy() {
        let msg = crate::app::messages::BackgroundMessage::AgentTokenUsage(TokenUsageInfo {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            cached_tokens: None,
            reasoning_tokens: None,
        });
        let event = BackgroundEvent::from_legacy(&msg).unwrap();
        match event {
            BackgroundEvent::Agent(AgentEvent::TokenUsage(t)) => {
                assert_eq!(t.prompt_tokens, 100);
                assert_eq!(t.completion_tokens, 50);
                assert_eq!(t.total_tokens, 150);
            }
            _ => panic!("expected AgentEvent::TokenUsage"),
        }
    }

    #[test]
    fn test_fs_file_parsed_from_legacy() {
        let msg = crate::app::messages::BackgroundMessage::FileParsed {
            path: PathBuf::from("test.md"),
            tags: vec!["tag1".into()],
        };
        let event = BackgroundEvent::from_legacy(&msg).unwrap();
        match event {
            BackgroundEvent::Fs(FsEvent::FileParsed { path, tags }) => {
                assert_eq!(path, PathBuf::from("test.md"));
                assert_eq!(tags, vec!["tag1".to_string()]);
            }
            _ => panic!("expected FsEvent::FileParsed"),
        }
    }

    #[test]
    fn test_process_log_entry_from_legacy() {
        let entry = crate::background::models::BackgroundLogEntry::new(
            crate::background::models::LogCategory::Indexer,
            "progress".to_string(),
        );
        let msg = crate::app::messages::BackgroundMessage::LogEntry(entry);
        let event = BackgroundEvent::from_legacy(&msg).unwrap();
        assert!(matches!(
            event,
            BackgroundEvent::Process(ProcessEvent::LogEntry(_))
        ));
    }
}
