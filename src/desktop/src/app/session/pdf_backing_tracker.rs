//! PDF-backing tracker — maintains the set of Markdown files that have a
//! same-stem `.pdf` sibling, built from bus events on the UI thread and
//! shared read-only with tool executors.
//!
//! The tracker stores the set in an `Arc<RwLock<HashSet>>` so it can be
//! cheaply cloned and shared across threads. The UI thread drains
//! `FileEvent` bus events and updates the set via `process_discovered` /
//! `process_removed`. Tool executors read the set via `is_pdf_backed`
//! under a read lock.
//!
//! Spec: REQ-450 — PDF derivation (PDF-backed rendering / write blocking).
//!
//! **Home:** this type lives in `crate::app::session` so that
//! the LLM agent and the application orchestrator can share it
//! without either having to reach into the file-watcher plumbing
//! (`crate::app::watcher`) directly. The old path still
//! re-exports this module for backwards compatibility.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// Tracks which Markdown files have a same-stem `.pdf` sibling.
///
/// The tracker owns the set of PDF-backed paths in an `Arc<RwLock>`.
/// The UI thread calls `process_discovered` / `process_removed` to
/// update the set. Tool executors call `is_pdf_backed` to check
/// membership under a read lock.
#[derive(Clone)]
pub struct PdfBackingTracker {
    pdf_backed_md: Arc<RwLock<HashSet<PathBuf>>>,
}

impl PdfBackingTracker {
    /// Create a new empty tracker.
    pub fn new() -> Self {
        Self {
            pdf_backed_md: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Process `FileEvent::Discovered` paths and update the internal set.
    ///
    /// This should be called on the UI thread after draining the bus.
    /// Returns `true` if any PDF-backing membership changed.
    pub fn process_discovered(&self, paths: &[PathBuf]) -> bool {
        let mut changed = false;
        let mut set = self.pdf_backed_md.write().unwrap();
        for p in paths {
            if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("md"))
                && p.with_extension("pdf").exists()
                && set.insert(p.clone())
            {
                changed = true;
            }
        }
        changed
    }

    /// Process `FileEvent::Removed` paths.
    ///
    /// Call on the UI thread after draining the bus for `Removed` events.
    pub fn process_removed(&self, paths: &[PathBuf]) -> bool {
        let mut changed = false;
        let mut set = self.pdf_backed_md.write().unwrap();
        for p in paths {
            if set.remove(p) {
                changed = true;
            }
        }
        changed
    }

    /// Check whether the given path is a Markdown file with a PDF sibling.
    ///
    /// Returns `false` for non-`.md` files and paths not yet discovered
    /// (the tracker only tracks files that have appeared on the bus).
    pub fn is_pdf_backed(&self, path: &Path) -> bool {
        let set = self.pdf_backed_md.read().unwrap();
        set.contains(path)
    }

    /// Number of tracked PDF-backed Markdown files.
    pub fn len(&self) -> usize {
        let set = self.pdf_backed_md.read().unwrap();
        set.len()
    }

    /// Check whether the tracker has no entries.
    pub fn is_empty(&self) -> bool {
        let set = self.pdf_backed_md.read().unwrap();
        set.is_empty()
    }
}

impl Default for PdfBackingTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
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
}
