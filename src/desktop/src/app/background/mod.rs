//! Background subsystem — indexer, manager, models, PDF converter, and vision processor.
//!
//! The `notify`-based filesystem watcher lives in
//! [`crate::app::watcher::file_watcher`]. All cross-thread messaging
//! primitives (buses, event payloads, routing) live in [`crate::bus`].
//!
//! ## `image-library` feature
//!
//! The `vision_processor` module and its re-exports
//! (`ImageVisionWorker`, `process_image`) are gated behind the
//! `image-library` Cargo feature. The `ImageJob`
//! data type lives in `models` under the same feature; the rest of
//! `models` (the `BackgroundLogEntry` / `LogCategory` re-exports
//! and `PdfConversionJob`) stay always compiled.

pub mod indexer;
pub mod logs;
pub mod models;
pub mod pdf_converter;
#[cfg(feature = "vector-search")]
pub mod vector_search;
#[cfg(feature = "image-library")]
pub mod vision_processor;

pub use indexer::Indexer;
pub use logs::{BackgroundLogs, MAX_LOG_ENTRIES, SharedBackgroundLogs};
#[cfg(feature = "image-library")]
pub use models::ImageJob;
pub use models::{BackgroundLogEntry, LogCategory};
pub use pdf_converter::{PdfConversionJob, PdfConverterWorker};
#[cfg(feature = "vector-search")]
pub use vector_search::{
    VectorSearchService, is_markdown, markdown_chunks, start as start_vector_search,
};
#[cfg(feature = "image-library")]
pub use vision_processor::{ImageVisionWorker, process_image};
