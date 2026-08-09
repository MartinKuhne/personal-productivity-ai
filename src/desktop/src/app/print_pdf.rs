//! PDF export via embedded Typst.
//!
//! Compiles the Markdown document to a Typst source string, runs it
//! through the in-process Typst compiler, and writes the resulting
//! PDF next to the source `.md` file. Sibling of [`super::print`]
//! which still drives the browser-based "open in browser for printing"
//! flow.
//!
//! # Pipeline
//!
//! 1. [`crate::markdown::render_markdown_to_typst`] — pure markdown
//!    to Typst markup, see `crate::markdown::typst` for the
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
//! and `crate::app::print_pdf` simply does not exist. The UI wires
//! the "Save as PDF" menu entry only when the feature is on — see
//! `src/ui/tree/render.rs`.
//!
//! Unit tests live in the sibling `print_pdf_tests.rs` sidecar.

use crate::app::background::{BackgroundLogEntry, LogCategory};
use crate::bus::events::typed::BackgroundEvent;
use crate::markdown::render_markdown_to_typst;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

/// Job description for the "Save as PDF" background action.
///
/// Mirrors [`crate::app::print::PrintJob`] but produces a file rather
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

/// Typst source template used as the entry point for every PDF
/// export. The user's translated markdown body is interpolated as
/// the `body` argument so the template can set up page geometry
/// and styling once.
///
/// The default page is A4. Heading sizes scale with their level
/// and the body is justified. Code blocks are lightly shaded;
/// table cells are padded; the table header is bold.
///
/// `title` is interpolated into the running header (right-aligned)
/// and the body.
const TEMPLATE: &str = r##"
#set page(
  paper: "a4",
  margin: (x: 2.4cm, y: 2.6cm),
  header: align(right, text(size: 9pt, fill: luma(120))[#title]),
  numbering: "1 / 1",
)
#set text(
  size: 11pt,
  lang: "en",
  // Modern sans-serif font chain. The default body font. Primary
  // is `Segoe UI` (the Windows system UI font — modern, designed
  // for screen reading, ships on every Windows install since
  // Vista). Fallbacks cover macOS (`Helvetica Neue`, the macOS
  // UI staple) and Linux (`Liberation Sans`, the most widely
  // available modern sans on Linux distros). The final fallback
  // `Arial` is universal but dated; it's the last resort only if
  // none of the platform-specific faces are present.
  font: ("Segoe UI", "Helvetica Neue", "Liberation Sans", "Arial"),
)
#set par(justify: true, leading: 0.65em)
// Headings inherit the same modern sans chain. The earlier
// template used `New Computer Modern` (Latin Modern), which is
// a classic scholarly serif — readable but dated, and a poor
// match for a modern sans body. Keeping the family in sync
// avoids the visual disconnect of a sans body with serif
// headings.
#show heading: set text(font: ("Segoe UI", "Helvetica Neue", "Liberation Sans", "Arial"))
#show heading.where(level: 1): set text(size: 1.8em)
#show heading.where(level: 2): set text(size: 1.4em)
#show heading.where(level: 3): set text(size: 1.15em)
#show raw.where(block: true): block(
  fill: luma(245),
  inset: 8pt,
  radius: 4pt,
  width: 100%,
)
#show raw.where(block: false): box(
  fill: luma(245),
  inset: 2pt,
  radius: 2pt,
)
#show table.cell: cell => pad(cell, x: 4pt, y: 3pt)
#show table.cell.where(y: 0): strong

#body
"##;

/// Build a fully-formed Typst document by interpolating the body
/// into the template. Exposed for unit tests so they can verify
/// the produced source without going through the full engine.
fn build_typst_document(title: &str, body: &str) -> String {
    TEMPLATE
        .replace("#title", &typst_string_literal(title))
        .replace("#body", body)
}

/// Encode a string as a Typst string literal. The translator
/// already escapes content for inclusion in markup; this helper
/// quotes it for use as a value (e.g. inside a `header: ...`
/// function call).
fn typst_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// Compile markdown to a PDF byte vector.
///
/// This is the core, side-effect-free function. It is exercised by
/// the unit tests and is the right entry point when the caller
/// wants the PDF bytes directly (e.g. for in-app preview, email
/// attachment, or non-`fastmd` consumers).
pub fn compile_markdown_to_pdf(markdown: &str, title: &str) -> Result<Vec<u8>, String> {
    let body = render_markdown_to_typst(markdown);
    let document = build_typst_document(title, &body);
    compile_typst_document(&document)
}

/// Build a Typst engine with the embedded fonts and compile the
/// document. Pulled out so tests can verify compilation succeeds
/// against a small fixture without the surrounding save-to-file
/// machinery.
///
/// Type note: `engine.compile()` is generic over the `Doc` output
/// type. We pin `Doc` to `PagedDocument` by binding the result of
/// `output.expect(...)` to a typed local — Rust's inference flows
/// back through `.output` to pick the right `Doc` for us, even
/// though the outer `Warned<...>` wrapper type is unnameable from
/// this crate.
fn get_cached_fonts() -> &'static [typst::text::Font] {
    static CACHED_FONTS: std::sync::OnceLock<Vec<typst::text::Font>> = std::sync::OnceLock::new();
    CACHED_FONTS.get_or_init(|| {
        let mut fonts = Vec::new();
        for (font, _) in typst_kit::fonts::embedded() {
            fonts.push(font);
        }
        for (font_path, _) in typst_kit::fonts::system() {
            use typst_kit::fonts::FontSource; // wait, is this needed for .load()? typst-as-lib uses it.
            if let Some(font) = font_path.load() {
                fonts.push(font);
            }
        }
        fonts
    })
}

fn compile_typst_document(document: &str) -> Result<Vec<u8>, String> {
    use typst_as_lib::TypstEngine;
    // The `TypstEngine::builder()` default leaves font discovery
    // off — `typst_kit_font_options: None` — so the engine builds
    // with an empty font book. Typst then accepts the document
    // (the markup is well-formed) but cannot shape any text, and
    // the produced PDF contains page numbers, layout lines, and
    // table grid cells but no actual text content. Discovered
    // against the user's Polaris wiki file: every spec example
    // compiled to a "valid" PDF (correct header, correct trailer,
    // correct page count) but the file opened to a near-empty
    // document. The default options of `TypstKitFontOptions`
    // enable both system font discovery and the embedded
    // `typst-assets` fonts ("New Computer Modern" + "Liberation
    // Serif" in the user's template, the former from the
    // embedded set).
    //
    // We now cache these fonts globally to avoid scanning the
    // filesystem on every engine build (a ~300ms overhead).
    let engine = TypstEngine::builder()
        .main_file(document.to_string())
        .fonts(get_cached_fonts().iter().cloned())
        .build();
    let result = engine.compile();
    // Bind the document to a typed local so `Doc = PagedDocument`
    // propagates through inference. The warnings field is a
    // `Vec<SourceDiagnostic>` — we log the count and let the
    // user see them in the Background Process Log if they care.
    let warnings_count = result.warnings.len();
    if warnings_count > 0 {
        tracing::warn!(
            name = "print_pdf.typst_warnings",
            count = warnings_count,
            "Typst compilation emitted warnings. The PDF was still produced."
        );
    }
    let doc: typst_layout::PagedDocument = result
        .output
        .map_err(|e| format!("Typst compilation failed: {e}"))?;
    // PagedDocument carries the laid-out document; we then ask
    // typst-pdf to serialise it to PDF bytes. The options struct
    // is `#[non_exhaustive]`; `Default::default()` is the
    // well-supported way to get the standard PDF. The error
    // type from typst-pdf doesn't implement Display, only Debug,
    // so we use `{:?}`.
    let options = typst_pdf::PdfOptions::default();
    let pdf_bytes = typst_pdf::pdf(&doc, &options)
        .map_err(|e| format!("Typst PDF serialisation failed: {e:?}"))?;
    if pdf_bytes.is_empty() {
        return Err("Typst PDF serialisation produced an empty PDF".to_string());
    }
    Ok(pdf_bytes)
}

/// Compile the markdown content to a PDF file at
/// `job.resolved_output_path()`. Suitable to be called from a
/// `std::thread::spawn` background worker (matches the
/// `execute_print_blocking` pattern in [`super::print`]).
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
    tx: Option<&Sender<BackgroundEvent>>,
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
/// `execute_print_blocking` pattern in [`super::print`]).
///
/// On the test path, call [`compile_and_save_pdf`] directly
/// instead — that function does not pop a viewer, so the
/// developer's `cargo test` run is silent.
pub fn execute_save_as_pdf_blocking(
    job: SaveAsPdfJob,
    tx: Option<Sender<BackgroundEvent>>,
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

// Unit tests live in the sibling `print_pdf_tests.rs` sidecar
// (AGENTS.md RUST-056 / RUST-057).
//
// see: `src/app/print_pdf_tests.rs`
//
// Proptest-based property tests live in
// `src/app/print_pdf_proptests.rs`. They run the same compile
// pipeline against random printable-ASCII inputs as the strongest
// possible "Typst syntax reference compliance" check: any escape
// the translator is missing will surface as a compile failure
// somewhere in the random sample.

#[cfg(test)]
#[path = "print_pdf_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "print_pdf_proptests.rs"]
mod proptests;
