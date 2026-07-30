//! Background subsystem — bus router, indexer, manager, models, PDF converter, and vision processor.

pub use crate::app::background::bus_router;
pub use crate::app::background::indexer;
pub use crate::app::background::models;
pub use crate::app::background::pdf_converter;
pub use crate::app::background::vision_processor;
pub use crate::app::background::{
    process_image, spawn_path_worker, BackgroundLogEntry, BackgroundProcessManager, BusRouter,
    ImageVisionWorker, Indexer, LogCategory, PdfConversionJob, PdfConverterWorker,
    SharedProcessManager,
};
