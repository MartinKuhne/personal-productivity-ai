//! Event payload types — every type that flows over a `Bus<T>` or a
//! channel in this crate.
//!
//! | Module | Payload(s) | Channel |
//! | --- | --- | --- |
//! | [`mod@file`] | `FileEvent`, `FileEventKind`, `FileEventProducer` | `Bus<FileEvent>` |
//! | [`messages`] | `BackgroundMessage` (legacy), `TokenUsageInfo` | `mpsc::Sender<BackgroundMessage>` |
//! | [`typed`] | `BackgroundEvent`, `AgentEvent`, `FsEvent`, `ProcessEvent` | future `Bus<…>` channels |
//! | [`config`] | `ConfigArrived` | `Bus<ConfigArrived>` |
//!
//! See [`crate::bus::core`] for the transport primitive.

pub mod config;
pub mod file;
pub mod messages;
pub mod typed;

pub use config::ConfigArrived;
pub use file::{FileEvent, FileEventKind, FileEventProducer};
pub use messages::{BackgroundMessage, TokenUsageInfo};
pub use typed::{AgentEvent, BackgroundEvent, FsEvent, ProcessEvent};
