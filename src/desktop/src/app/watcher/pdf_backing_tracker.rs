//! Backwards-compatibility shim — [`PdfBackingTracker`] now lives in
//! [`crate::app::session::pdf_backing_tracker`]. New code should
//! import from `crate::app::session`; this shim is kept so that
//! existing in-tree call sites continue to compile and external
//! documentation links survive the move. It will be removed
//! once every consumer migrates.
//!
//! See `doc/planning/desktop-module-boundaries-review.md` for
//! the rationale.

pub use crate::app::session::pdf_backing_tracker::PdfBackingTracker;
