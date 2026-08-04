//! Background subsystem data types — `ImageJob` and `PdfConversionJob`.
//!
//! `LogCategory` and `BackgroundLogEntry` moved to
//! [`crate::bus::events::messages`] in the layering-inversion fix so the
//! `bus` module no longer depends on `app`. They are re-exported here
//! for backwards compatibility with existing in-tree call sites.

pub use crate::bus::events::messages::{BackgroundLogEntry, LogCategory};

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
    fn test_log_category_display_still_works_via_reexport() {
        // After the move, the type lives in `bus::events::messages` but the
        // canonical accessor `crate::app::background::LogCategory` must keep
        // resolving. This test guards the re-export.
        let cat: LogCategory = LogCategory::Indexer;
        assert_eq!(cat.to_string(), "Indexer");
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
        ) && let (Ok(img_time), Ok(md_time)) = (img_meta.modified(), md_meta.modified())
        {
            return img_time > md_time;
        }
        false
    }
}
