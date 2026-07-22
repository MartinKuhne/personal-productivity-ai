//! Background subsystem data types — `LogCategory`, `BackgroundLogEntry`, `ImageJob`, `PdfConversionJob`.

use chrono::{DateTime, Local};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LogCategory {
    Indexer,
    Watcher,
    PdfConverter,
    ImageVision,
    LlmTools,
    Print,
    Batch,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_image_job_new_swaps_extension() {
        let img = std::path::PathBuf::from("/test/photo.jpg");
        let job = ImageJob::new(img);
        assert_eq!(job.image_path.to_string_lossy(), "/test/photo.jpg");
        assert_eq!(job.md_path.to_string_lossy(), "/test/photo.md");
    }

    #[test]
    fn test_image_job_should_process_missing_md() {
        let dir = tempdir().unwrap();
        let img = dir.path().join("photo.jpg");
        std::fs::write(&img, "image data").unwrap();
        let job = ImageJob::new(img);
        assert!(job.should_process());
    }

    #[test]
    fn test_image_job_should_process_md_older_than_image() {
        let dir = tempdir().unwrap();
        let img = dir.path().join("photo.jpg");
        let md = dir.path().join("photo.md");
        std::fs::write(&img, "image data").unwrap();
        std::fs::write(&md, "desc").unwrap();
        let past = filetime::FileTime::from_unix_time(1000, 0);
        filetime::set_file_mtime(&md, past).unwrap();
        let job = ImageJob::new(img);
        assert!(job.should_process());
    }

    #[test]
    fn test_image_job_should_not_process_md_newer() {
        let dir = tempdir().unwrap();
        let img = dir.path().join("photo.jpg");
        let md = dir.path().join("photo.md");
        std::fs::write(&img, "image data").unwrap();
        std::fs::write(&md, "desc").unwrap();
        let now = filetime::FileTime::now();
        filetime::set_file_mtime(&md, now).unwrap();
        let job = ImageJob::new(img);
        assert!(!job.should_process());
    }

    #[test]
    fn test_log_category_display() {
        assert_eq!(LogCategory::Indexer.to_string(), "Indexer");
        assert_eq!(LogCategory::Watcher.to_string(), "Watcher");
        assert_eq!(LogCategory::PdfConverter.to_string(), "PDF Converter");
        assert_eq!(LogCategory::ImageVision.to_string(), "Image Vision");
        assert_eq!(LogCategory::LlmTools.to_string(), "LLM Tools");
    }

    #[test]
    fn test_background_log_entry_new() {
        let entry = BackgroundLogEntry::new(LogCategory::Indexer, "test message".to_string());
        assert_eq!(entry.category, LogCategory::Indexer);
        assert_eq!(entry.message, "test message");
    }
}

impl std::fmt::Display for LogCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            LogCategory::Indexer => "Indexer",
            LogCategory::Watcher => "Watcher",
            LogCategory::PdfConverter => "PDF Converter",
            LogCategory::ImageVision => "Image Vision",
            LogCategory::LlmTools => "LLM Tools",
            LogCategory::Print => "Print",
            LogCategory::Batch => "Batch",
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
        Self {
            image_path,
            md_path,
        }
    }

    pub fn should_process(&self) -> bool {
        if !self.md_path.exists() {
            return true;
        }
        if let (Ok(img_meta), Ok(md_meta)) = (
            std::fs::metadata(&self.image_path),
            std::fs::metadata(&self.md_path),
        ) {
            if let (Ok(img_time), Ok(md_time)) = (img_meta.modified(), md_meta.modified()) {
                return img_time > md_time;
            }
        }
        false
    }
}
