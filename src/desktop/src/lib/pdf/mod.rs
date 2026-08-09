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

/// Build a fully-formed Typst document by interpolating the body
/// into the template.
fn build_typst_document(title: &str, body: &str) -> String {
    TEMPLATE
        .replace("#title", &typst_string_literal(title))
        .replace("#body", body)
}

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

pub mod typst_translator;
