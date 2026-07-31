//! Event payload types — every type that flows over a `Bus<T>` or a
//! channel in this crate.
//!
//! | Module | Payload(s) | Channel |
//! | --- | --- | --- |
//! | [`mod@file`] | `FileEvent`, `FileEventKind`, `FileEventProducer` | `Bus<FileEvent>` |
//! | [`messages`] | `TokenUsageInfo` | shared with [`typed`] |
//! | [`typed`] | `BackgroundEvent`, `AgentEvent`, `FsEvent`, `ProcessEvent` | `mpsc::Sender<BackgroundEvent>` (one channel, three sub-enums) |
//! | [`config`] | `ConfigArrived` | `Bus<ConfigArrived>` |
//!
//! The legacy `BackgroundMessage` god-enum and its `from_legacy`
//! compatibility shim were removed in Phase 3 of the P1-6
//! architecture review. New code constructs the typed
//! [`BackgroundEvent`] variants in [`typed`] directly.
//!
//! See [`crate::bus::core`] for the transport primitive.

pub mod config;
pub mod file;
pub mod messages;
pub mod typed;

pub use config::ConfigArrived;
pub use file::{FileEvent, FileEventKind, FileEventProducer};
pub use messages::TokenUsageInfo;
pub use typed::{AgentEvent, BackgroundEvent, FsEvent, ProcessEvent};
