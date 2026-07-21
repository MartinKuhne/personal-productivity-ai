pub mod manager;
pub mod models;
pub mod pdf_converter;
pub mod vision_processor;

pub use manager::{BackgroundProcessManager, MAX_LOG_ENTRIES, SharedProcessManager};
pub use models::{BackgroundLogEntry, LogCategory};
pub use pdf_converter::PdfConversionJob;
