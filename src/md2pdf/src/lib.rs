//! Markdown to PDF conversion.
//!
//! Independent crate extracted from `src/app/export/pdf`. The pipeline is:
//!
//! 1. [`translator::render_markdown_to_typst`] — pure markdown to Typst markup
//! 2. [`engine::generate`] — Typst markup to PDF bytes via `typst-as-lib`
//! 3. [`compile_markdown_to_pdf`] — composition of the two for ergonomic use
//!
//! Font discovery (`typst_kit`) is cached process-wide via `OnceLock` in
//! [`engine`] so repeated compilations do not re-scan the system. This
//! preserves the original `get_cached_fonts` caching behaviour.
//!
//! Unit tests live in sibling sidecars (`translator_tests.rs`,
//! `translator_proptests.rs`, `save_tests.rs`, `save_proptests.rs`).
//! See `AGENTS.md RUST-056 / RUST-057`.

pub mod engine;
pub mod translator;

pub use engine::generate;
pub use translator::render_markdown_to_typst;

/// Typst compiler requires an 8 MiB stack to process deeply nested documents.
///
/// The default thread stack (2 MiB on Linux, 512 KiB on macOS) will reliably
/// `SIGABRT` on complex documents. Use this constant when spawning threads
/// that compile Typst. Preserved from `src/app/export/pdf/save.rs:102`.
pub const TYPST_THREAD_STACK_SIZE: usize = 8 * 1024 * 1024;

/// Compile markdown to a PDF byte vector.
///
/// This is the core, side-effect-free function. It is the right entry point
/// when the caller wants the PDF bytes directly (e.g. for in-app preview,
/// email attachment, or non-`fastmd` consumers). Mirrors the original
/// `src/app/export/pdf/save.rs:93` `compile_markdown_to_pdf`.
///
/// # Errors
///
/// Returns `Err(String)` when Typst compilation or PDF serialisation fails
/// or when the resulting PDF is empty.
pub fn compile_markdown_to_pdf(markdown: &str, title: &str) -> Result<Vec<u8>, String> {
    let body = translator::render_markdown_to_typst(markdown);
    engine::generate(title, &body)
}

#[cfg(test)]
#[path = "save_tests.rs"]
mod save_tests;

#[cfg(test)]
#[path = "save_proptests.rs"]
mod save_proptests;
