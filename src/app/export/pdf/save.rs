//! PDF export via embedded Typst.
//!
//! Compiles the Markdown document to a Typst source string, runs it
//! through the in-process Typst compiler, and writes the resulting
//! PDF next to the source `.md` file. Sibling of [`crate::export::print`]
//! which still drives the browser-based "open in browser for printing"
//! flow.
//!
//! # Pipeline
//!
//! 1. [`super::typst_translator::render_markdown_to_typst`] — pure markdown
//!    to Typst markup, see [`super::typst_translator`] for the
//!    translator.
//! 2. `TypstEngine::compile` — in-process Typst engine from
//!    `typst-as-lib`, fed the translated body inside a fixed
//!    `TEMPLATE` that sets page geometry, fonts, and a small style
//!    set (code-block shading, table cell padding, justified body
//!    text, sized headings).
//! 3. `std::fs::write` — the compiled PDF bytes are dropped next to
//!    the source `.md`. We then `opener::open` it (already a
//!    dependency) so the user lands in their default PDF viewer, the
//!    same UX the existing `print.rs` provides.
//!
//! # Spec traceability
//!
//! The `pdf-export` Cargo feature gates this module off entirely
//! (the optional dep `typst-as-lib` is also gated on the same
//! feature). When the feature is disabled, this file is not compiled
//! and `crate::export::pdf` simply does not exist. The UI wires
//! the "Save as PDF" menu entry only when the feature is on — see
//! `src/ui/tree/render.rs`.
//!
//! Unit tests live in the sibling `save_tests.rs` sidecar.

use crate::background::{BackgroundLogEntry, LogCategory};
use std::path::{Path, PathBuf};

/// Job description for the "Save as PDF" background action.
///
/// Mirrors [`crate::export::print::PrintJob`] but produces a file rather
/// than opening a browser tab. The `output_path` is filled in by
/// [`execute_save_as_pdf_blocking`]; callers pass `None` and accept
/// the default of "next to the source markdown with `.pdf`
/// extension", or `Some(path)` to override.
#[derive(Debug, Clone)]
pub struct SaveAsPdfJob {
    pub markdown_path: PathBuf,
    pub markdown_content: String,
    pub title: String,
    /// Optional override; when `None`, defaults to
    /// `<stem>.pdf` next to the source file.
    pub output_path: Option<PathBuf>,
}

impl SaveAsPdfJob {
    /// Build a job from an on-disk Markdown file. Reads the file
    /// eagerly; if the file is missing or unreadable, the markdown
    /// content is left empty and the call will fail later in
    /// `execute_save_as_pdf_blocking`.
    pub fn from_path(markdown_path: PathBuf) -> Self {
        let markdown_content = std::fs::read_to_string(&markdown_path).unwrap_or_default();
        let title = markdown_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Document")
            .to_string();
        Self {
            markdown_path,
            markdown_content,
            title,
            output_path: None,
        }
    }

    /// Resolve the final output path, falling back to
    /// `<stem>.pdf` next to the source.
    pub fn resolved_output_path(&self) -> PathBuf {
        if let Some(p) = &self.output_path {
            return p.clone();
        }
        let mut p = self.markdown_path.clone();
        p.set_extension("pdf");
        p
    }
}

/// Compile markdown to a PDF byte vector.
///
/// Delegates to `fastmd-pdf::compile_markdown_to_pdf`. This is the
/// core, side-effect-free function, preserved at the original path so
/// `fastmd::export::pdf::compile_markdown_to_pdf` stays stable for the
/// integration test `tests/commonmark_spec_test.rs`.
pub use fastmd_pdf::compile_markdown_to_pdf;

/// Typst compiler requires an 8MB stack — re-exported from the
/// independent crate so callers do not need to depend on it directly.
pub use fastmd_pdf::TYPST_THREAD_STACK_SIZE;

/// Compile the markdown content to a PDF file at
/// `job.resolved_output_path()`. Suitable to be called from a
/// `std::thread::spawn` background worker (matches the
/// `execute_print_blocking` pattern in [`crate::export::print`]).
///
/// The `tx` channel is used to push log entries into the Background
/// Process Log so the user can see "Exported `notes.md` →
/// `notes.pdf` (12 KB)" without having to check the file system.
///
/// **Does not open the resulting PDF in a viewer.** Callers that
/// want that behaviour (i.e. the UI) should chain a call to
/// [`open_pdf_in_viewer`] on the returned path. Splitting compile
/// from open means the test suite can exercise the compile+write
/// path without popping a PDF viewer on the developer's
/// desktop during `cargo test`.
pub fn compile_and_save_pdf(
    job: &SaveAsPdfJob,
    tx: Option<&crate::bus::events::typed::BackgroundEventSender>,
) -> Result<PathBuf, String> {
    if job.markdown_content.is_empty() {
        return Err(format!(
            "Markdown content is empty for {}",
            job.markdown_path.display()
        ));
    }
    let output_path = job.resolved_output_path();
    let _ = tx.as_ref().map(|sender| {
        let _ = sender.send(
            BackgroundLogEntry::new(
                LogCategory::Print,
                format!(
                    "Compiling PDF for {} -> {}",
                    job.markdown_path.display(),
                    output_path.display()
                ),
            )
            .into(),
        );
    });

    let pdf_bytes = match compile_markdown_to_pdf(&job.markdown_content, &job.title) {
        Ok(b) => b,
        Err(e) => {
            let _ = tx.as_ref().map(|sender| {
                let _ = sender.send(
                    BackgroundLogEntry::new(
                        LogCategory::Print,
                        format!("PDF compile failed for {}: {}", job.title, e),
                    )
                    .into(),
                );
            });
            return Err(e);
        }
    };

    write_pdf(&output_path, &pdf_bytes).map_err(|e| {
        let _ = tx.as_ref().map(|sender| {
            let _ = sender.send(
                BackgroundLogEntry::new(
                    LogCategory::Print,
                    format!("PDF write failed for {}: {}", output_path.display(), e),
                )
                .into(),
            );
        });
        e
    })?;

    let _ = tx.as_ref().map(|sender| {
        let _ = sender.send(
            BackgroundLogEntry::new(
                LogCategory::Print,
                format!(
                    "Exported {} -> {} ({} bytes)",
                    job.markdown_path.display(),
                    output_path.display(),
                    pdf_bytes.len()
                ),
            )
            .into(),
        );
    });

    Ok(output_path)
}

/// Open an already-saved PDF in the user's default viewer. Best-
/// effort: a failure here is logged but does not propagate. Kept
/// separate from [`compile_and_save_pdf`] so the test suite
/// can exercise the compile+write path without popping a
/// viewer on the developer's desktop during `cargo test`.
///
/// Public so the UI layer can call it independently of
/// [`execute_save_as_pdf_blocking`] (e.g. a "Reveal in folder"
/// action that just opens the existing file).
pub fn open_pdf_in_viewer(path: &Path) -> Result<(), String> {
    opener::open(path).map_err(|e| {
        tracing::warn!(
            name = "print_pdf.open_failed",
            path = %path.display(),
            error = %e,
            "Could not open PDF in the default viewer."
        );
        format!("opener::open({}): {}", path.display(), e)
    })
}

/// UI-layer composition: compile + save, then open the result
/// in the user's default PDF viewer. Suitable to be called from
/// a `std::thread::spawn` background worker (matches the
/// `execute_print_blocking` pattern in [`crate::export::print`]).
///
/// On the test path, call [`compile_and_save_pdf`] directly
/// instead — that function does not pop a viewer, so the
/// developer's `cargo test` run is silent.
pub fn execute_save_as_pdf_blocking(
    job: SaveAsPdfJob,
    tx: Option<crate::bus::events::typed::BackgroundEventSender>,
) -> Result<PathBuf, String> {
    let output_path = compile_and_save_pdf(&job, tx.as_ref())?;
    // Open the resulting PDF in the user's default viewer. Best-effort:
    // the export is already complete and successful, so a failure to
    // open the viewer is logged but does not propagate.
    if let Err(e) = open_pdf_in_viewer(&output_path) {
        let _ = tx.as_ref().map(|sender| {
            let _ = sender.send(
                BackgroundLogEntry::new(
                    LogCategory::Print,
                    format!(
                        "Saved {} but could not open viewer: {}",
                        output_path.display(),
                        e
                    ),
                )
                .into(),
            );
        });
    }
    Ok(output_path)
}

/// Write the PDF bytes to `path`, creating parent directories if
/// needed. Returns the I/O error verbatim on failure.
fn write_pdf(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create_dir_all({}): {}", parent.display(), e))?;
    }
    std::fs::write(path, bytes).map_err(|e| format!("write({}): {}", path.display(), e))
}

// Unit tests live in the sibling `save_tests.rs` sidecar
// (AGENTS.md RUST-056 / RUST-057).
//
// see: `src/export/pdf/save_tests.rs`
//
// Proptest-based property tests live in
// `src/export/pdf/save_proptests.rs`. They run the same compile
// pipeline against random printable-ASCII inputs as the strongest
// possible "Typst syntax reference compliance" check: any escape
// the translator is missing will surface as a compile failure
// somewhere in the random sample.

#[cfg(test)]
#[path = "save_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "save_proptests.rs"]
mod proptests;
