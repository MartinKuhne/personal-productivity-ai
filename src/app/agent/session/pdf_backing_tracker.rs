//! PDF-backing tracker — maintains the set of Markdown files that have a
//! same-stem `.pdf` sibling, built from bus events on the UI thread and
//! shared read-only with tool executors.
//!
//! The tracker stores the set in an `Arc<RwLock<HashSet>>` so it can be
//! cheaply cloned and shared across threads. The UI thread drains
//! `FileChanged` bus events and updates the set via `process_discovered` /
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
//!
//! Unit tests live in the sibling `pdf_backing_tracker_tests.rs` sidecar.

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

    /// Process `[FileChanged][Added]` paths and update the internal set.
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

    /// Process `[FileChanged][Deleted]` paths.
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

impl crate::agent::tools::policy::ToolCallPolicy for PdfBackingTracker {
    fn check_write_allowed(&self, path: &std::path::Path) -> Result<(), String> {
        if self.is_pdf_backed(path) {
            Err(crate::ui::strings::PDF_BACKED_ERROR.to_string())
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "pdf_backing_tracker_tests.rs"]
mod tests;
