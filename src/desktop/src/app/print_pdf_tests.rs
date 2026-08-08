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
    SaveAsPdfJob, build_typst_document, compile_markdown_to_pdf, execute_save_as_pdf_blocking,
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
    let result = execute_save_as_pdf_blocking(job, None);
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
    let output = execute_save_as_pdf_blocking(job, None).expect("export should succeed");
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
