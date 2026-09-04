//! Markdown to PDF conversion.
//!
//! Independent crate extracted from `src/app/export/pdf`. The pipeline is:
//!
//! 1. [`translator::render_markdown_to_typst`] — pure markdown to Typst markup
//! 2. [`engine::generate`] — Typst markup to PDF bytes via the official `typst` CLI
//! 3. [`compile_markdown_to_pdf`] — composition of the two for ergonomic use
//!
//! Unit tests live in sibling sidecars (`translator_tests.rs`,
//! `translator_proptests.rs`, `save_tests.rs`, `save_proptests.rs`).
//! See `AGENTS.md RUST-056 / RUST-057`.

pub mod engine;
pub mod translator;

pub use engine::{find_typst_binary, generate, generate_to_file, is_typst_available};
pub use translator::render_markdown_to_typst;

/// Thread stack size recommendation when running background jobs.
pub const TYPST_THREAD_STACK_SIZE: usize = 8 * 1024 * 1024;

/// Compile markdown to a PDF byte vector.
///
/// Invokes the official `typst` CLI binary found in system PATH.
///
/// # Errors
///
/// Returns `Err(String)` when Typst is not found, compilation fails,
/// or the resulting PDF is empty.
pub fn compile_markdown_to_pdf(markdown: &str, title: &str) -> Result<Vec<u8>, String> {
    let body = translator::render_markdown_to_typst(markdown);
    engine::generate(title, &body)
}

/// Compile markdown and write the PDF directly to `output_path`.
///
/// # Errors
///
/// Returns `Err(String)` when Typst is not found, compilation fails,
/// or writing to the target file fails.
pub fn compile_markdown_to_pdf_file(
    markdown: &str,
    title: &str,
    output_path: &std::path::Path,
) -> Result<(), String> {
    let body = translator::render_markdown_to_typst(markdown);
    engine::generate_to_file(title, &body, output_path)
}

#[cfg(test)]
#[path = "save_tests.rs"]
mod save_tests;

#[cfg(test)]
#[path = "save_proptests.rs"]
mod save_proptests;
