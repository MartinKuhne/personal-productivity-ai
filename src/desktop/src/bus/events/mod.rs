//! Event payload types — every type that flows over a `Bus<T>` or a
//! channel in this crate.
//!
//! | Module | Payload(s) | Channel |
//! | --- | --- | --- |
//! | [`mod@file`] | `FileEvent`, `FileEventKind`, `FileEventProducer` | `Bus<FileEvent>` |
//! | [`messages`] | `TokenUsageInfo`, `BackgroundLogEntry`, `LogCategory` | shared with [`typed`] |
//! | [`typed`] | `BackgroundEvent` (`Fs`, `Process`, `McpAuth`), `FsEvent`, `ProcessEvent` | `mpsc::Sender<BackgroundEvent>` |
//! | [`config`] | `ConfigArrived` | `Bus<ConfigArrived>` |
//!
//! The `AgentEvent` variant was removed from `BackgroundEvent` in feature 003
//! (agent-ui-seam-refactor). Agent events now flow on `Bus<AgentEvent>` from
//! `agent::events::AgentEvent`; the UI converts them into text via
//! `ui/agent/transcript.rs` → `ui/render/agent_render.rs`.
//!
//! See [`crate::bus::core`] for the transport primitive.

pub mod config;
pub mod debug;
pub mod file;
pub mod messages;
pub mod typed;

pub use config::ConfigArrived;
pub use debug::{AgentDebugEntry, DebugEntryKind, DebugEntryRow};
pub use file::{FileEvent, FileEventKind, FileEventProducer};
pub use messages::{BackgroundLogEntry, LogCategory, TokenUsageInfo};
pub use typed::{BackgroundEvent, FsEvent, McpAuthEvent, ProcessEvent};
