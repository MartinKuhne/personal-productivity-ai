//! Unit tests for the PDF export pipeline.
//!
//! Sidecar of `print_pdf.rs`. Per AGENTS.md RUST-056 / RUST-057.
//!
//! The end-to-end test (`compile_markdown_to_pdf_produces_real_pdf`)
//! actually invokes the embedded Typst engine and writes a PDF to
//! a temp dir. This is the highest-value test in the module — it
//! catches regressions in the template, the translator, and the
//! engine wiring simultaneously.

use super::{
    SaveAsPdfJob, build_typst_document, compile_and_save_pdf, compile_markdown_to_pdf,
    typst_string_literal,
};
use std::path::PathBuf;

#[test]
fn typst_string_literal_quotes_and_escapes() {
    assert_eq!(typst_string_literal(""), "\"\"");
    assert_eq!(typst_string_literal("hello"), "\"hello\"");
    // Embedded quote gets backslash-escaped.
    assert_eq!(typst_string_literal("a\"b"), "\"a\\\"b\"");
    // Embedded backslash gets doubled.
    assert_eq!(typst_string_literal("a\\b"), "\"a\\\\b\"");
    // Newline / tab / CR get the standard escape sequences.
    assert_eq!(typst_string_literal("a\nb"), "\"a\\nb\"");
    assert_eq!(typst_string_literal("a\tb"), "\"a\\tb\"");
}

#[test]
fn build_typst_document_substitutes_title_and_body() {
    let doc = build_typst_document("My Document", "# heading\n\nbody\n");
    // Title appears as a quoted string.
    assert!(doc.contains("\"My Document\""), "got: {doc}");
    // Body appears verbatim.
    assert!(doc.contains("# heading"), "got: {doc}");
    assert!(doc.contains("body"));
    // The template preamble is still there.
    assert!(doc.contains("#set page("));
    assert!(doc.contains("#set text("));
}

#[test]
fn save_as_pdf_job_from_path_reads_markdown() {
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("hello.md");
    std::fs::write(&md, "# Title\n\nbody").unwrap();
    let job = SaveAsPdfJob::from_path(md.clone());
    assert_eq!(job.markdown_path, md);
    assert_eq!(job.title, "hello");
    assert_eq!(job.markdown_content, "# Title\n\nbody");
    assert!(job.output_path.is_none());
}

#[test]
fn save_as_pdf_job_resolved_output_path_defaults_to_neighbour() {
    let job = SaveAsPdfJob {
        markdown_path: PathBuf::from("/tmp/work/notes.md"),
        markdown_content: "x".to_string(),
        title: "notes".to_string(),
        output_path: None,
    };
    assert_eq!(
        job.resolved_output_path(),
        PathBuf::from("/tmp/work/notes.pdf")
    );
}

#[test]
fn save_as_pdf_job_resolved_output_path_honours_override() {
    let job = SaveAsPdfJob {
        markdown_path: PathBuf::from("/tmp/work/notes.md"),
        markdown_content: "x".to_string(),
        title: "notes".to_string(),
        output_path: Some(PathBuf::from("/elsewhere/out.pdf")),
    };
    assert_eq!(
        job.resolved_output_path(),
        PathBuf::from("/elsewhere/out.pdf")
    );
}

#[test]
fn save_as_pdf_job_rejects_empty_content() {
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("empty.md");
    std::fs::write(&md, "").unwrap();
    let job = SaveAsPdfJob::from_path(md);
    // Use `compile_and_save_pdf` (not the `execute_save_as_pdf_blocking`
    // wrapper) so the test path doesn't pop a PDF viewer on the
    // developer's desktop during `cargo test`.
    let result = compile_and_save_pdf(&job, None);
    assert!(result.is_err(), "expected error on empty content");
}

#[test]
fn execute_save_as_pdf_blocking_writes_pdf_file() {
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("hello.md");
    std::fs::write(&md, "# Hello\n\nA short document.\n").unwrap();
    let job = SaveAsPdfJob::from_path(md);

    // We don't pass a channel — the log path is non-essential and
    // we don't want to drag in the bus machinery for this test.
    //
    // We call `compile_and_save_pdf` (not the
    // `execute_save_as_pdf_blocking` wrapper) on purpose: the
    // wrapper also invokes `opener::open` on the resulting PDF,
    // which would pop a viewer on the developer's desktop during
    // `cargo test`. The compile-and-write half is the part we
    // actually want to exercise here.
    let output = compile_and_save_pdf(&job, None).expect("export should succeed");
    assert!(output.exists(), "output PDF should exist at {output:?}");

    let bytes = std::fs::read(&output).expect("read PDF");
    // Every PDF starts with `%PDF-` (the magic header).
    assert!(
        bytes.starts_with(b"%PDF-"),
        "file at {output:?} is not a PDF (first 8 bytes: {:?})",
        &bytes[..8.min(bytes.len())]
    );
    // And ends with `%%EOF` per the spec.
    assert!(
        bytes.ends_with(b"%%EOF"),
        "file at {output:?} does not end with %%EOF (last 8 bytes: {:?})",
        &bytes[bytes.len().saturating_sub(8)..]
    );
    // A one-page document with a heading and a paragraph should be
    // at least a kilobyte; smaller and something is wrong.
    assert!(
        bytes.len() > 1024,
        "PDF suspiciously small: {} bytes",
        bytes.len()
    );
}

#[test]
fn compile_markdown_to_pdf_handles_full_gfm() {
    // A representative Markdown document: heading, paragraph,
    // list, code block, table, blockquote. If any of these
    // produce malformed Typst, compilation will fail and this test
    // will catch it.
    let md = r#"
# Title

A paragraph with **bold**, *italic*, and `inline code`.

- one
- two
- three

```rust
fn main() {
    println!("hi");
}
```

> A quotation.

| a | b |
|---|---|
| 1 | 2 |
| 3 | 4 |
"#;
    let bytes = compile_markdown_to_pdf(md, "test").expect("PDF should compile");
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(bytes.ends_with(b"%%EOF"));
    assert!(
        bytes.len() > 2048,
        "full-feature PDF should be >2KB, got {}",
        bytes.len()
    );
}

/// Regression: a PDF compiled by the print_pdf path must contain
/// actual text content, not just structural decoration.
///
/// The header/`%%EOF` trailer and the structural stream content
/// (page borders, page numbers, table grid cells) were always
/// present, even when the body was silently dropped because the
/// Typst engine had no fonts loaded. A naive assertion like
/// `bytes.starts_with(b"%PDF-")` passes for an empty-document
/// PDF. This test asserts the PDF carries a font dictionary AND
/// the page-content stream size is consistent with rendered text
/// glyphs (the empty-PDF version of the same test markdown was
/// ~3KB with only page numbers + grid cells; the fixed version
/// is ~12KB with embedded fonts + glyphs).
///
/// Triggered by the user reporting that
/// `Polaris/2008-Sportsman-500-X2-EFI-Recommissioning-and-Maintenance.md`
/// produced a "mostly empty PDF" — root cause was the default
/// `TypstEngine::builder()` not enabling `typst-kit` font
/// discovery, so the engine had an empty font book and the PDF
/// was generated with no glyphs.
#[test]
fn compiled_pdf_contains_text_content() {
    let md = "Hello world\n\n# Heading 1\n\nA paragraph of body text.\n";
    let bytes = compile_markdown_to_pdf(md, "text-content").expect("compile");
    // The PDF is binary data; scan it as bytes to avoid the
    // lossy UTF-8 conversion munging dictionary keys.
    // Font dictionary must be present. The empty-PDF regression
    // produced a PDF with zero font references; the fixed
    // version has at least one `/Type /Font` entry per embedded
    // typeface. Asserting on the *presence* of a font dict (not
    // the absence of glyphs) is the structural signal we want —
    // if the engine builds with an empty font book, the PDF
    // would not have any `/Type /Font` entries at all.
    let needle = b"/Type/Font";
    assert!(
        bytes.windows(needle.len()).any(|w| w == needle),
        "PDF has no /Type/Font entries — the Typst engine was \
         built with an empty font book, so text would not render. \
         This is the regression fixed by enabling typst-kit font \
         discovery on the engine builder; re-check \
         `compile_typst_document` in `print_pdf.rs`."
    );
    // Size sanity: an embedded-font PDF with text is meaningfully
    // larger than an empty-document PDF. The empty-PDF regression
    // produced ~3KB for this same input; the fixed version
    // embeds the Latin Modern fonts (subsetted to the doc's
    // glyphs) which alone is ~9KB, and the test markdown renders
    // to several lines of glyphs on top of that. A 6KB floor
    // leaves headroom for the empty-PDF regression but rejects
    // any future regression that drops fonts silently.
    assert!(
        bytes.len() > 6_000,
        "PDF body suspiciously small for rendered text: {} bytes. \
         The empty-PDF regression produced ~3KB for this same input.",
        bytes.len()
    );
}

// =====================================================================
// Typst syntax-reference compliance (end-to-end)
// =====================================================================
//
// These tests pin the *compiled output* to the markup-active
// character set in https://typst.app/docs/reference/syntax/.
// A passing test here is the strongest possible proof that the
// translator emits legal Typst — a string-assertion test only
// proves the call signature, not the semantic correctness.

/// A doc that uses every markup-active char per the syntax
/// reference. Each of these lines, taken individually, used to
/// fail the compiler before the missing escape chars were
/// added (`$` triggered math mode, `~` triggered symbol
/// shorthand, `` ` `` triggered raw mode, `'` / `"` triggered
/// smart-quote conversion, `#` triggered code mode, `*` and `_`
/// triggered strong/emphasis, `@` triggered a reference).
#[test]
fn typst_syntax_reference_compliance_every_markup_char() {
    let md = r#"
# Test

Plain prose with *literal* asterisks, _literal_ underscores,
`literal` backticks, # literal hash, @ literal at-sign,
$ literal dollar, ~ literal tilde, ' apostrophe,
" straight quote, [open bracket, ]close bracket,
\ backslash, and a https://example.com/#anchor link.
"#;
    let bytes = compile_markdown_to_pdf(md, "compliance")
        .expect("every markup-active char must compile to a valid Typst document");
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(bytes.ends_with(b"%%EOF"));
    // A compliance doc should still be small but non-trivial —
    // 1 KB is the floor.
    assert!(
        bytes.len() > 1024,
        "compliance PDF too small: {} bytes",
        bytes.len()
    );
}

/// C# in body text — the `#` would normally be code-mode
/// entry, and used to break compilation before the fix.
#[test]
fn dollar_sign_in_text() {
    let md = "C# is a language. It costs $5 to buy a license.\n";
    compile_markdown_to_pdf(md, "cs").expect("$ and # in body must compile");
}

/// URLs that contain `#` (anchor) and `&` (query separator)
/// used to render a stray backslash because we ran the markup
/// escape on the URL. The string escape now runs instead.
#[test]
fn url_with_hash_and_ampersand_compiles() {
    let md = "Visit [example](https://example.com/page?x=1&y=2#section-3) for details.\n";
    let bytes =
        compile_markdown_to_pdf(md, "urls").expect("URLs with # and & must compile to a valid PDF");
    assert!(bytes.starts_with(b"%PDF-"));
}

/// Smart-quote chars in user content (apostrophe, quote) used
/// to be passed through unescaped, which caused Typst to
/// convert them to typographic curly variants in the PDF. The
/// escape now preserves the literal character.
#[test]
fn smart_quote_chars_preserved_in_output() {
    let md = "He said \"don't worry\" and walked away.\n";
    let bytes = compile_markdown_to_pdf(md, "smartquote")
        .expect("smart-quote chars in user content must compile");
    assert!(bytes.starts_with(b"%PDF-"));
}

/// The `~` character used to trigger Typst's symbol shorthand
/// for a non-breaking space. Now escaped.
#[test]
fn tilde_in_text_compiles() {
    let md = "Saved ~50% of the bytes.\n";
    compile_markdown_to_pdf(md, "tilde").expect("~ in body must compile");
}

/// Combining the high-risk chars inside a single phrase is
/// the most realistic failure mode — a partial fix that
/// handles one char but not another would still fail here.
#[test]
fn combined_special_chars_in_paragraph_compile() {
    let md = "Try `a*b_c` and `c#lang` and `cost $5` and `~50%` — all literal.\n";
    let bytes =
        compile_markdown_to_pdf(md, "combined").expect("combined special chars must compile");
    assert!(bytes.starts_with(b"%PDF-"));
}

/// Real-world document that triggered the original bug report
/// (`2008-Sportsman-500-X2-EFI-Recommissioning-and-Maintenance.md`):
/// a heading followed by a single-item ordered list used to
/// emit `#list(marker: ([_],),\n+ Item\n)`, which Typst rejects
/// because `+ Item` is a list-item expression, not a valid
/// function-call arg. The fix routes the items through a
/// `#enum(numbering: "1.")[ ... ]` content block. This test
/// pins that fix with a representative fragment of the
/// original failing document.
#[test]
fn ordered_list_with_long_item_compiles() {
    let md = "\
### Phase 1: Pre-Start Assessment

1. **Battery**: Remove, test voltage. Likely needs replacement after years storage. X2 battery location: under seat, negative cable first. PN 4140006.
2. **Fuel system**: Drain tank completely.
";
    let bytes = compile_markdown_to_pdf(md, "ordered-list-regression")
        .expect("ordered list after heading with long item must compile to a valid PDF");
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(bytes.ends_with(b"%%EOF"));
    assert!(
        bytes.len() > 1024,
        "ordered-list regression PDF too small: {} bytes",
        bytes.len()
    );
}
