//! Background subsystem — indexer, manager, models, PDF converter, and vision processor.
//!
//! The `notify`-based filesystem watcher lives in
//! [`crate::app::watcher::file_watcher`]. All cross-thread messaging
//! primitives (buses, event payloads, routing) live in [`crate::bus`].

pub mod indexer;
pub mod manager;
pub mod models;
pub mod pdf_converter;
pub mod vision_processor;

pub use indexer::Indexer;
pub use manager::{BackgroundProcessManager, MAX_LOG_ENTRIES, SharedProcessManager};
pub use models::{BackgroundLogEntry, LogCategory};
pub use pdf_converter::{PdfConversionJob, PdfConverterWorker};
pub use vision_processor::{ImageVisionWorker, process_image};
