//! Backwards-compatibility shim — [`PdfBackingTracker`] now lives in
//! [`crate::agent::session::PdfBackingTracker`]. New code should
//! import from `crate::agent::session`; this shim is kept so that
//! existing in-tree call sites continue to compile and external
//! documentation links survive the move. It will be removed
//! once every consumer migrates.
//!
//! See `doc/planning/desktop-module-boundaries-review.md` for
//! the rationale.

pub use crate::agent::session::PdfBackingTracker;
