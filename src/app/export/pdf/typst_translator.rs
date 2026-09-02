//! Markdown → Typst translator — facade over `fastmd-pdf::translator`.
//!
//! The implementation now lives in `src/md2pdf/src/translator.rs`. This
//! shim re-exports it so `fastmd::export::pdf::typst_translator::render_markdown_to_typst`
//! and the associated `escape_*` helpers keep their original path (used by
//! `tests/commonmark_spec_test.rs` and in-crate callers). The cache and
//! behaviour are preserved verbatim by delegating to the independent crate.
//!
//! Unit tests live in `src/md2pdf/src/translator_tests.rs` (moved from
//! `src/app/export/pdf/typst_translator_tests.rs`) — no tests lost.

#[cfg(feature = "pdf-export")]
pub use fastmd_pdf::translator::{
    escape_typst, escape_typst_autolink, escape_typst_string, render_markdown_to_typst,
};
