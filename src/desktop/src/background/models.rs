use chrono::{DateTime, Local};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LogCategory {
    Indexer,
    Watcher,
    PdfConverter,
    ImageVision,
    LlmTools,
}

impl std::fmt::Display for LogCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            LogCategory::Indexer => "Indexer",
            LogCategory::Watcher => "Watcher",
            LogCategory::PdfConverter => "PDF Converter",
            LogCategory::ImageVision => "Image Vision",
            LogCategory::LlmTools => "LLM Tools",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BackgroundLogEntry {
    pub timestamp: DateTime<Local>,
    pub category: LogCategory,
    pub message: String,
}

impl BackgroundLogEntry {
    pub fn new(category: LogCategory, message: String) -> Self {
        Self {
            timestamp: Local::now(),
            category,
            message,
        }
    }
}
