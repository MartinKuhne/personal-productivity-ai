//! Tests for the background job types (`background::models`).

use super::*;
#[cfg(feature = "image-library")]
use tempfile::tempdir;

#[cfg(feature = "image-library")]
#[test]
fn test_image_job_new_swaps_extension() {
    let img = std::path::PathBuf::from("/test/photo.jpg");
    let job = ImageJob::new(img);
    assert_eq!(job.image_path.to_string_lossy(), "/test/photo.jpg");
    assert_eq!(job.md_path.to_string_lossy(), "/test/photo.md");
}

#[cfg(feature = "image-library")]
#[test]
fn test_image_job_should_process_missing_md() {
    let dir = tempdir().unwrap();
    let img = dir.path().join("photo.jpg");
    let job = ImageJob::new(img);
    assert!(job.should_process());
}

#[cfg(feature = "image-library")]
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

#[cfg(feature = "image-library")]
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

#[cfg(feature = "image-library")]
#[test]
fn test_image_job_should_not_process_metadata_read_failure() {
    let dir = tempdir().unwrap();
    // The output .md exists, but the source image does not. `metadata`
    // on the missing image fails, so `should_process` must return false.
    let img = dir.path().join("photo.jpg");
    let md = dir.path().join("photo.md");
    std::fs::write(&md, "desc").unwrap();

    let job = ImageJob::new(img);
    assert!(!job.should_process());
}

#[test]
fn test_log_category_display_still_works_via_reexport() {
    // The type lives in `bus::events::messages`, but the canonical
    // accessor `crate::background::LogCategory` must keep
    // resolving. This test guards the re-export.
    let cat: LogCategory = LogCategory::Indexer;
    assert_eq!(cat.to_string(), "Indexer");
}
