//! Typed background events — the per-domain replacement for the legacy
//! [`crate::bus::events::messages::BackgroundMessage`] god enum.
//!
//! ## Migration strategy
//!
//! 1. **Phase 1 (this code):** define `AgentEvent` and `BackgroundEvent`.
//!    Producers may optionally send `BackgroundEvent` alongside
//!    `BackgroundMessage`. The UI consumer handles both via a single
//!    `match` on the newtype.
//! 2. **Phase 2:** producers switch to sending only `BackgroundEvent`;
//!    `BackgroundMessage` gains `#[deprecated]`.
//! 3. **Phase 3:** `BackgroundMessage` is removed; each domain owns its
//!    own broadcast channel.

use crate::background::BackgroundLogEntry;
use crate::bus::events::messages::TokenUsageInfo;
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
    LogEntry(BackgroundLogEntry),
    FileLoaded {
        path: std::path::PathBuf,
        content: Result<String, String>,
    },
}

/// Typed replacement for [`crate::bus::events::messages::BackgroundMessage`].
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
    /// Convert a legacy [`crate::bus::events::messages::BackgroundMessage`] into
    /// the typed [`BackgroundEvent`]. Returns `None` for variants that
    /// carry a non-cloneable `RecommendedWatcher` handle (the caller
    /// must deal with those separately).
    pub fn from_legacy(msg: &crate::bus::events::messages::BackgroundMessage) -> Option<Self> {
        use crate::bus::events::messages::BackgroundMessage;
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
    use crate::bus::events::messages::TokenUsageInfo;
    use std::path::PathBuf;

    #[test]
    fn test_agent_status_from_legacy() {
        let msg = crate::bus::events::messages::BackgroundMessage::AgentStatus("Running".into());
        let ev = BackgroundEvent::from_legacy(&msg).unwrap();
        match ev {
            BackgroundEvent::Agent(AgentEvent::Status(s)) => assert_eq!(s, "Running"),
            _ => panic!("expected AgentEvent::Status"),
        }
    }

    #[test]
    fn test_agent_token_usage_from_legacy() {
        let usage = TokenUsageInfo {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
            cached_tokens: Some(5),
            reasoning_tokens: None,
        };
        let msg = crate::bus::events::messages::BackgroundMessage::AgentTokenUsage(usage);
        let ev = BackgroundEvent::from_legacy(&msg).unwrap();
        match ev {
            BackgroundEvent::Agent(AgentEvent::TokenUsage(u)) => {
                assert_eq!(u.prompt_tokens, 10);
                assert_eq!(u.completion_tokens, 20);
                assert_eq!(u.total_tokens, 30);
                assert_eq!(u.cached_tokens, Some(5));
            }
            _ => panic!("expected AgentEvent::TokenUsage"),
        }
    }

    #[test]
    fn test_fs_file_parsed_from_legacy() {
        let msg = crate::bus::events::messages::BackgroundMessage::FileParsed {
            path: PathBuf::from("/a/b.md"),
            tags: vec!["x".into()],
        };
        let ev = BackgroundEvent::from_legacy(&msg).unwrap();
        match ev {
            BackgroundEvent::Fs(FsEvent::FileParsed { path, tags }) => {
                assert_eq!(path, PathBuf::from("/a/b.md"));
                assert_eq!(tags, vec!["x".to_string()]);
            }
            _ => panic!("expected FsEvent::FileParsed"),
        }
    }

    #[test]
    fn test_process_log_entry_from_legacy() {
        let entry =
            BackgroundLogEntry::new(crate::background::LogCategory::PdfConverter, "msg".into());
        let msg = crate::bus::events::messages::BackgroundMessage::LogEntry(entry);
        let ev = BackgroundEvent::from_legacy(&msg).unwrap();
        match ev {
            BackgroundEvent::Process(ProcessEvent::LogEntry(_)) => {}
            _ => panic!("expected ProcessEvent::LogEntry"),
        }
    }
}
