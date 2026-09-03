//! Typst engine and PDF generation.
//!
//! Wraps the in-process `typst-as-lib` engine with a fixed template and
//! cached font book. Font discovery is expensive, so the font list is
//! cached process-wide via a `OnceLock` — this is the preservation of
//! the original `get_cached_fonts` caching behaviour.
//!
//! Unit tests live in the sibling `save_tests.rs` sidecar via `lib.rs`.

const TEMPLATE: &str = r##"
#set page(
  paper: "a4",
  margin: (x: 2.4cm, y: 2.6cm),
  header: align(right, text(size: 9pt, fill: luma(120))[#title]),
  numbering: "1 / 1",
)
#set text(
  size: 10pt,
  lang: "en",
  font: ("Segoe UI", "Helvetica Neue", "Liberation Sans", "Arial"),
)
#set par(justify: true, leading: 0.65em)
#show heading: set text(font: ("Segoe UI", "Helvetica Neue", "Liberation Sans", "Arial"))
#show heading.where(level: 1): set text(size: 16pt)
#show heading.where(level: 2): set text(size: 14pt)
#show heading.where(level: 3): set text(size: 12pt)
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

/// Encode a string as a Typst string literal.
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

/// Build a fully-formed Typst document by interpolating the body into the template.
fn build_typst_document(title: &str, body: &str) -> String {
    TEMPLATE
        .replace("#title", &typst_string_literal(title))
        .replace("#body", body)
}

/// Cached font book — embedded plus system fonts, initialized once.
///
/// This preserves the original `get_cached_fonts` behaviour from
/// `src/app/export/pdf/mod.rs:82` which used a `static OnceLock` to
/// avoid re-discovering fonts on every PDF compilation.
fn get_cached_fonts() -> &'static [typst::text::Font] {
    static CACHED_FONTS: std::sync::OnceLock<Vec<typst::text::Font>> = std::sync::OnceLock::new();
    CACHED_FONTS.get_or_init(|| {
        let mut fonts = Vec::new();
        for (font, _) in typst_kit::fonts::embedded() {
            fonts.push(font);
        }
        for (font_path, _) in typst_kit::fonts::system() {
            use typst_kit::fonts::FontSource;
            if let Some(font) = font_path.load() {
                fonts.push(font);
            }
        }
        fonts
    })
}

/// Generate a PDF byte vector from a Typst body and title.
///
/// The `typst_body` is the output of [`crate::translator::render_markdown_to_typst`],
/// already escaped and shaped as Typst markup. The `title` is interpolated
/// as the page header.
pub fn generate(title: &str, typst_body: &str) -> Result<Vec<u8>, String> {
    let document = build_typst_document(title, typst_body);
    use typst_as_lib::TypstEngine;

    let engine = TypstEngine::builder()
        .main_file(document.to_string())
        .fonts(get_cached_fonts().iter().cloned())
        .build();

    let result = engine.compile();
    let doc: typst_layout::PagedDocument = result
        .output
        .map_err(|e| format!("Typst compilation failed: {e}"))?;

    let options = typst_pdf::PdfOptions::default();
    let pdf_bytes = typst_pdf::pdf(&doc, &options)
        .map_err(|e| format!("Typst PDF serialisation failed: {e:?}"))?;

    if pdf_bytes.is_empty() {
        return Err("Typst PDF serialisation produced an empty PDF".to_string());
    }

    Ok(pdf_bytes)
}
