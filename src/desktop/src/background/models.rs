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

#[derive(Debug, Clone)]
pub struct ImageJob {
    pub image_path: std::path::PathBuf,
    pub md_path: std::path::PathBuf,
}

impl ImageJob {
    pub fn new(image_path: std::path::PathBuf) -> Self {
        let mut md_path = image_path.clone();
        md_path.set_extension("md");
        Self { image_path, md_path }
    }

    pub fn should_process(&self) -> bool {
        if !self.md_path.exists() {
            return true;
        }
        if let (Ok(img_meta), Ok(md_meta)) = (std::fs::metadata(&self.image_path), std::fs::metadata(&self.md_path)) {
            if let (Ok(img_time), Ok(md_time)) = (img_meta.modified(), md_meta.modified()) {
                return img_time > md_time;
            }
        }
        false
    }
}

