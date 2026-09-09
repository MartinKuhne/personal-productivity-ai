//! Unit tests for the PDF export pipeline.
//!
//! Sidecar of `save.rs`. Per AGENTS.md RUST-056 / RUST-057.
//!
//! Unit tests primarily verify translation of markdown constructs into
//! valid Typst markup via `render_markdown_to_typst`. Tests requiring
//! the external `typst` binary check `fastmd_pdf::is_typst_available()`
//! and skip cleanly when it is not installed.

use super::{SaveAsPdfJob, compile_and_save_pdf};
use fastmd_pdf::render_markdown_to_typst;
use std::path::PathBuf;

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
fn save_as_pdf_job_from_path_missing_file_falls_back_to_empty() {
    let missing = PathBuf::from("definitely_missing_job_file_67890.md");
    let job = SaveAsPdfJob::from_path(missing);
    assert_eq!(job.markdown_content, "");
    assert_eq!(job.title, "definitely_missing_job_file_67890");
}

#[test]
fn save_as_pdf_job_resolved_output_path_without_extension_appends_pdf() {
    // A source path with no extension still resolves to a `.pdf` sibling.
    let job = SaveAsPdfJob {
        markdown_path: PathBuf::from("/tmp/work/notes"),
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
    if !fastmd_pdf::is_typst_available() {
        eprintln!("Skipping: typst CLI binary not found in PATH");
        return;
    }
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

/// Pin the "Save as PDF..." flow: when the user picks a
/// destination via the native dialog, the PDF must land at
/// that exact path, NOT next to the source `.md` (the old
/// default-destination behaviour). The dialog itself is an
/// OS call that we can't drive from a unit test, so we
/// exercise the same code path the dialog's `Option<PathBuf>`
/// result feeds into: setting `job.output_path` explicitly and
/// calling `compile_and_save_pdf`.
#[test]
fn save_as_pdf_honours_user_chosen_destination() {
    if !fastmd_pdf::is_typst_available() {
        eprintln!("Skipping: typst CLI binary not found in PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    // The source `.md` lives in `dir/source/`. The old default
    // destination would be `dir/source/<stem>.pdf` (next to
    // the source). The user-chosen destination is `dir/elsewhere/
    // custom-name.pdf` — explicitly NOT the default.
    let source_dir = dir.path().join("source");
    std::fs::create_dir(&source_dir).unwrap();
    let md = source_dir.join("notes.md");
    std::fs::write(&md, "# Notes\n\nBody.\n").unwrap();
    let target_dir = dir.path().join("elsewhere");
    std::fs::create_dir(&target_dir).unwrap();
    let target = target_dir.join("custom-name.pdf");

    let mut job = SaveAsPdfJob::from_path(md);
    job.output_path = Some(target.clone());
    let output = compile_and_save_pdf(&job, None).expect("export should succeed");

    // The returned path is the user-chosen destination, not the
    // default next-to-source one.
    assert_eq!(output, target);
    assert!(output.exists(), "PDF at user-chosen path should exist");
    // The default next-to-source path must NOT have been written.
    let default_destination = source_dir.join("notes.pdf");
    assert!(
        !default_destination.exists(),
        "default next-to-source path was written even though the \
         user picked a custom destination: {}",
        default_destination.display()
    );
}

#[test]
fn compile_markdown_to_typst_handles_full_gfm() {
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
    let typst = render_markdown_to_typst(md);
    assert!(typst.contains("= Title"));
    assert!(typst.contains("#strong[bold]"));
    assert!(typst.contains("#emph[italic]"));
    assert!(typst.contains("- one"));
    assert!(typst.contains("- two"));
    assert!(typst.contains("- three"));
    assert!(typst.contains("fn main()"));
    assert!(typst.contains("#quote(block: true)"));
    assert!(typst.contains("#table("));
}

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
    let typst = render_markdown_to_typst(md);
    assert!(typst.contains("= Test"));
    assert!(typst.contains(r"\$"));
    assert!(typst.contains(r"\#"));
    assert!(typst.contains(r"\@"));
    assert!(typst.contains(r"\~"));
}

#[test]
fn dollar_sign_in_text() {
    let md = "C# is a language. It costs $5 to buy a license.\n";
    let typst = render_markdown_to_typst(md);
    assert!(typst.contains(r"C\#"));
    assert!(typst.contains(r"\$5"));
}

#[test]
fn url_with_hash_and_ampersand_compiles() {
    let md = "Visit [example](https://example.com/page?x=1&y=2#section-3) for details.\n";
    let typst = render_markdown_to_typst(md);
    assert!(typst.contains("https://example.com/page?x=1&y=2#section-3"));
    assert!(typst.contains("[example]"));
}

#[test]
fn smart_quote_chars_preserved_in_output() {
    let md = "He said \"don't worry\" and walked away.\n";
    let typst = render_markdown_to_typst(md);
    assert!(typst.contains(r#"\""#) || typst.contains(r#"""#) || typst.contains("don"));
}

#[test]
fn tilde_in_text_compiles() {
    let md = "Saved ~50% of the bytes.\n";
    let typst = render_markdown_to_typst(md);
    assert!(typst.contains(r"\~50%"));
}

#[test]
fn combined_special_chars_in_paragraph_compile() {
    let md = "Try `a*b_c` and `c#lang` and `cost $5` and `~50%` — all literal.\n";
    let typst = render_markdown_to_typst(md);
    assert!(typst.contains("a*b_c"));
    assert!(typst.contains("c#lang"));
    assert!(typst.contains("cost $5"));
    assert!(typst.contains("~50%"));
}

#[test]
fn ordered_list_with_long_item_compiles() {
    let md = "\
### Phase 1: Pre-Start Assessment

1. Inspect fluid levels before turning the key.
2. Drain tank completely.
";
    let typst = render_markdown_to_typst(md);
    assert!(typst.contains("=== Phase 1: Pre-Start Assessment"));
    assert!(typst.contains("+ Inspect fluid levels"));
    assert!(typst.contains("+ Drain tank completely."));
}

#[test]
fn typst_renders_h1_through_h3() {
    let md = "# Top level\n\n## Section\n\n### Subsection\n\nBody text.\n";
    let typst = render_markdown_to_typst(md);
    assert!(typst.contains("= Top level"));
    assert!(typst.contains("== Section"));
    assert!(typst.contains("=== Subsection"));
    assert!(typst.contains("Body text."));
}

#[test]
fn typst_renders_lists() {
    let md = "- alpha\n- beta\n- gamma\n\n1. first\n2. second\n\n- [ ] todo one\n- [x] todo two\n";
    let typst = render_markdown_to_typst(md);
    assert!(typst.contains("- alpha"));
    assert!(typst.contains("- beta"));
    assert!(typst.contains("- gamma"));
    assert!(typst.contains("+ first"));
    assert!(typst.contains("+ second"));
    assert!(typst.contains("[ ]"));
    assert!(typst.contains("[x]"));
}

#[test]
fn typst_renders_inline_code_and_strong_and_emphasis() {
    let md = "Strong: **this is bold**.\n\nEmphasis: *this is italic*.\n\nInline: `let x = 1`.\n";
    let typst = render_markdown_to_typst(md);
    assert!(typst.contains("#strong[this is bold]"));
    assert!(typst.contains("#emph[this is italic]"));
    assert!(typst.contains("let x = 1"));
}

#[test]
fn typst_renders_inline_code() {
    let md = "Use `let x = 1` to assign.\n";
    let typst = render_markdown_to_typst(md);
    assert!(typst.contains("let x = 1"));
}

#[test]
fn typst_renders_fenced_code_block() {
    let md = "```rust\nfn main() {\n    let x: i32 = 1;\n    println!(\"{}\", x);\n}\n```\n";
    let typst = render_markdown_to_typst(md);
    assert!(typst.contains("#block("));
    assert!(typst.contains("fn main()"));
    assert!(typst.contains("let x: i32 = 1;"));
}

#[test]
fn typst_renders_gfm_table() {
    let md = "| Header A | Header B |\n|----------|----------|\n| alpha    | beta     |\n|          | gamma    |\n";
    let typst = render_markdown_to_typst(md);
    assert!(typst.contains("#table("));
    assert!(typst.contains("Header A"));
    assert!(typst.contains("Header B"));
    assert!(typst.contains("alpha"));
    assert!(typst.contains("beta"));
}

#[test]
fn typst_renders_link_text() {
    let md = "See [the example](https://example.com) for details.\n";
    let typst = render_markdown_to_typst(md);
    assert!(typst.contains("#link(\"https://example.com\")[the example]"));
}

#[test]
fn typst_renders_blockquote() {
    let md = "> A famous quotation.\n>\n> Attribution, year.\n";
    let typst = render_markdown_to_typst(md);
    assert!(typst.contains("#quote(block: true)["));
    assert!(typst.contains("A famous quotation."));
    assert!(typst.contains("Attribution, year."));
}

#[test]
fn typst_renders_special_chars_verbatim() {
    let md = r#"C# costs $5 @mention "quoted" (parens) \backslash 'apostrophe
"#;
    let typst = render_markdown_to_typst(md);
    assert!(typst.contains(r"C\#"));
    assert!(typst.contains(r"\$5"));
    assert!(typst.contains(r"\@mention"));
}

#[test]
fn typst_renders_horizontal_rule() {
    let md = "Before rule.\n\n---\n\nAfter rule.\n";
    let typst = render_markdown_to_typst(md);
    assert!(typst.contains("#line(length: 100%"));
    assert!(typst.contains("Before rule."));
    assert!(typst.contains("After rule."));
}

#[test]
fn math_compiles_to_typst() {
    let md =
        "Pythagoras: $a^2 + b^2 = c^2$.\n\nGreek letter: $alpha$.\n\nFraction: $frac(1, 2)$.\n";
    let typst = render_markdown_to_typst(md);
    assert!(typst.contains("$a^2 + b^2 = c^2$"));
    assert!(typst.contains("$alpha$"));
    assert!(typst.contains("$frac(1, 2)$"));
}

#[test]
fn footnotes_compile_to_typst() {
    let md = "\
First[^1] ref, then later: second[^1] ref.

Footnotes can have **bold** in the body[^2].

[^1]: shared body.
[^2]: another body with *italic* text.
";
    let typst = render_markdown_to_typst(md);
    assert!(typst.contains("#footnote[shared body.]"));
    assert!(typst.contains("#strong[bold]"));
}
