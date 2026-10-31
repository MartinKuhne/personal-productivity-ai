//! Tests for [`PdfBackingTracker`] (`agent::session::pdf_backing_tracker`).

use super::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_initial_state_is_empty() {
    let tracker = PdfBackingTracker::new();
    assert!(tracker.is_empty());
    assert_eq!(tracker.len(), 0);
}

#[test]
fn test_discovered_md_with_pdf_sibling_is_tracked() {
    let dir = tempdir().unwrap();
    let md_path = dir.path().join("doc.md");
    let pdf_path = dir.path().join("doc.pdf");
    fs::write(&md_path, "# Hello").unwrap();
    fs::write(&pdf_path, "%PDF-1.4").unwrap();

    let tracker = PdfBackingTracker::new();
    assert!(tracker.process_discovered(std::slice::from_ref(&md_path)));
    assert!(tracker.is_pdf_backed(&md_path));
    assert_eq!(tracker.len(), 1);
}

#[test]
fn test_discovered_md_without_pdf_sibling_is_not_tracked() {
    let dir = tempdir().unwrap();
    let md_path = dir.path().join("doc.md");
    fs::write(&md_path, "# Hello").unwrap();

    let tracker = PdfBackingTracker::new();
    assert!(!tracker.process_discovered(std::slice::from_ref(&md_path)));
    assert!(!tracker.is_pdf_backed(&md_path));
    assert!(tracker.is_empty());
}

#[test]
fn test_discovered_pdf_file_does_not_add_md() {
    let dir = tempdir().unwrap();
    let pdf_path = dir.path().join("doc.pdf");
    fs::write(&pdf_path, "%PDF-1.4").unwrap();

    let tracker = PdfBackingTracker::new();
    assert!(!tracker.process_discovered(std::slice::from_ref(&pdf_path)));
    assert!(tracker.is_empty());
}

#[test]
fn test_removed_md_is_untracked() {
    let dir = tempdir().unwrap();
    let md_path = dir.path().join("doc.md");
    let pdf_path = dir.path().join("doc.pdf");
    fs::write(&md_path, "# Hello").unwrap();
    fs::write(&pdf_path, "%PDF-1.4").unwrap();

    let tracker = PdfBackingTracker::new();
    tracker.process_discovered(std::slice::from_ref(&md_path));
    assert!(tracker.is_pdf_backed(&md_path));

    tracker.process_removed(std::slice::from_ref(&md_path));
    assert!(!tracker.is_pdf_backed(&md_path));
    assert!(tracker.is_empty());
}

#[test]
fn test_removed_nonexistent_md_is_noop() {
    let tracker = PdfBackingTracker::new();
    let path = PathBuf::from("nonexistent.md");
    assert!(!tracker.process_removed(&[path]));
}

#[test]
fn test_discovered_batch_mixed_files() {
    let dir = tempdir().unwrap();
    let md_pdf = dir.path().join("with_pdf.md");
    let pdf = dir.path().join("with_pdf.pdf");
    let md_no_pdf = dir.path().join("without_pdf.md");
    fs::write(&md_pdf, "# Hello").unwrap();
    fs::write(&pdf, "%PDF-1.4").unwrap();
    fs::write(&md_no_pdf, "# World").unwrap();

    let tracker = PdfBackingTracker::new();
    let paths = vec![md_pdf.clone(), pdf.clone(), md_no_pdf.clone()];
    assert!(tracker.process_discovered(&paths));
    assert!(tracker.is_pdf_backed(&md_pdf));
    assert!(!tracker.is_pdf_backed(&md_no_pdf));
    assert_eq!(tracker.len(), 1);
}

#[test]
fn test_discovered_is_idempotent() {
    let dir = tempdir().unwrap();
    let md_path = dir.path().join("doc.md");
    let pdf_path = dir.path().join("doc.pdf");
    fs::write(&md_path, "# Hello").unwrap();
    fs::write(&pdf_path, "%PDF-1.4").unwrap();

    let tracker = PdfBackingTracker::new();
    tracker.process_discovered(std::slice::from_ref(&md_path));
    let changed2 = tracker.process_discovered(std::slice::from_ref(&md_path));
    assert!(
        !changed2,
        "second process_discovered should return false for no changes"
    );
    assert_eq!(tracker.len(), 1);
}

#[test]
fn test_is_pdf_backed_returns_false_for_non_md() {
    let dir = tempdir().unwrap();
    let pdf_path = dir.path().join("doc.pdf");
    fs::write(&pdf_path, "%PDF-1.4").unwrap();

    let tracker = PdfBackingTracker::new();
    tracker.process_discovered(std::slice::from_ref(&pdf_path));
    assert!(!tracker.is_pdf_backed(&pdf_path));
}

#[test]
fn test_case_insensitive_md_extension() {
    let dir = tempdir().unwrap();
    let md_path = dir.path().join("doc.MD");
    let pdf_path = dir.path().join("doc.pdf");
    fs::write(&md_path, "# Hello").unwrap();
    fs::write(&pdf_path, "%PDF-1.4").unwrap();

    let tracker = PdfBackingTracker::new();
    tracker.process_discovered(std::slice::from_ref(&md_path));
    assert!(tracker.is_pdf_backed(&md_path));
}
