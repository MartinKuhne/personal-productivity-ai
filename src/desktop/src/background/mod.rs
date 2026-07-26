//! Background subsystem — bus router, indexer, manager, models, PDF converter, vision processor, and watcher.

pub mod bus_router;
pub mod channel_worker;
pub mod indexer;
pub mod manager;
pub mod models;
pub mod pdf_converter;
pub mod vision_processor;
pub mod watcher;

pub use bus_router::BusRouter;
pub use channel_worker::spawn_path_worker;
pub use indexer::Indexer;
pub use manager::{BackgroundProcessManager, MAX_LOG_ENTRIES, SharedProcessManager};
pub use models::{BackgroundLogEntry, LogCategory};
pub use pdf_converter::{PdfConversionJob, PdfConverterWorker};
pub use vision_processor::{ImageVisionWorker, process_image};
pub use watcher::FileWatcher;
