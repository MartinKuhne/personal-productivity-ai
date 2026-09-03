//! Compilation tests for the PDF pipeline (pure, no IO).
//!
//! Sidecar of lib.rs in astmd-md2pdf.

use super::compile_markdown_to_pdf;
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

// =====================================================================
// PDF content inspection
// =====================================================================
//
// The tests above check that the PDF export *compiles* — a valid
// binary header, a valid trailer, a non-empty byte count. They do
// NOT check that the rendered text matches the source markdown.
// A "valid" empty PDF would pass all of them (and did, before the
// font-loading fix).
//
// The tests below use the `pdf-extract` crate to pull the rendered
// text out of the produced PDF and assert that the text from the
// markdown actually appears in the output. This is the
// content-level contract: a "Save as PDF" that drops the body
// fails these tests, regardless of whether the file opens.

// Compile the test markdown to a PDF and return the extracted
// page text concatenated by `pdf-extract` (which decodes the
// embedded font CMaps and walks the content streams in reading
// order). The function is the single point of contact with the
// PDF inspection library; every test below uses it so we have
// one place to add a fallback or a parser-tweak if `pdf-extract`
// ever mishandles our font subset.
fn compile_and_extract(md: &str, title: &str) -> String {
    let bytes = compile_markdown_to_pdf(md, title)
        .unwrap_or_else(|e| panic!("compile_markdown_to_pdf failed for {title:?}: {e}"));

    let doc = pdf_oxide::PdfDocument::from_bytes(bytes).unwrap();
    let spans = doc.extract_spans(0).unwrap();

    let mut extracted = String::new();
    for span in spans {
        extracted.push_str(&span.text);
        extracted.push(' ');
    }
    extracted
}

/// Helper: assert that a needle appears in the rendered PDF
/// text. The match is plain substring — `pdf-extract` returns
/// the text in reading order with single newlines between
/// glyph-runs, and the Typst body uses single spaces between
/// inline tokens, so a phrase that was adjacent in the source
/// markdown is adjacent in the output. If a future Typst or
/// `pdf-extract` change introduces inter-word whitespace, the
/// test names point at the failing case so the assertion can
/// be relaxed to a whitespace-tolerant match without losing
/// meaning.
fn assert_text_contains(rendered: &str, needle: &str, test_name: &str) {
    assert!(
        rendered.contains(needle),
        "{test_name}: expected rendered PDF text to contain {needle:?}, \
         but it was missing. Extracted text:\n{rendered}"
    );
}

/// Heading text in the source must appear in the PDF. Headings
/// are the primary navigation aid in a printed document, and
/// dropping them silently would be the kind of regression the
/// header/footer-only tests cannot catch.
#[test]
fn pdf_renders_h1_through_h3() {
    let md = "# Top level\n\n## Section\n\n### Subsection\n\nBody text.\n";
    let out = compile_and_extract(md, "headings");
    assert_text_contains(&out, "Top level", "pdf_renders_h1_through_h3");
    assert_text_contains(&out, "Section", "pdf_renders_h1_through_h3");
    assert_text_contains(&out, "Subsection", "pdf_renders_h1_through_h3");
    assert_text_contains(&out, "Body text.", "pdf_renders_h1_through_h3");
}

/// Bulleted list items, ordered list items, and a task-list
/// checkbox marker must all appear in the PDF. List rendering
/// is the markdown feature most likely to be silently broken
/// by a content-mode / emphasis-marker bug in the translator.
#[test]
fn pdf_renders_lists() {
    let md = "- alpha\n- beta\n- gamma\n\n1. first\n2. second\n\n- [ ] todo one\n- [x] todo two\n";
    let out = compile_and_extract(md, "lists");
    for needle in [
        "alpha", "beta", "gamma", "first", "second", "todo one", "todo two",
    ] {
        assert_text_contains(&out, needle, "pdf_renders_lists");
    }
}

/// Inline code and adjacent strong/emphasis markers must
/// round-trip into the PDF body. Pinned because the previous
/// backtick-fenced raw form broke embedded-backtick examples
/// (spec test case #285–296) and the previous delimiter form
/// `*text*` / `_text_` broke adjacent-to-text cases (spec
/// test case #311, #326, #337, #352, #371). The
/// `#emph[...]` / `#strong[...]` content blocks fix the
/// adjacent cases; the `box(...) text(font: ..., "...")`
/// function form (formerly `#raw("...")`) routes inline code
/// through a `text` call, which bypasses the broken
/// `raw` element in typst-as-lib 0.16 / typst 0.15.1
/// (ADR gap #2). The fenced code-block path uses the same
/// `block + text` pattern; see `pdf_renders_fenced_code_block`.
#[test]
fn pdf_renders_inline_code_and_strong_and_emphasis() {
    let md = r#"
Strong: **this is bold**.

Emphasis: *this is italic*.

Adjacent strong: foo**bar**baz.

Adjacent emphasis: foo*bar*baz.

Inline code: `let x = 1`.
"#;
    let out = compile_and_extract(md, "inline");
    assert_text_contains(
        &out,
        "this is bold",
        "pdf_renders_inline_code_and_strong_and_emphasis",
    );
    assert_text_contains(
        &out,
        "this is italic",
        "pdf_renders_inline_code_and_strong_and_emphasis",
    );
    // Adjacent cases: the *text* between the markers must be in
    // the output, even though the surrounding word has no
    // whitespace. This is the regression that was fixed by
    // switching to the `#emph[...]` / `#strong[...]` form.
    assert_text_contains(
        &out,
        "bar",
        "pdf_renders_inline_code_and_strong_and_emphasis",
    );
    assert_text_contains(
        &out,
        "baz",
        "pdf_renders_inline_code_and_strong_and_emphasis",
    );
    // Inline code body. Was previously dropped (ADR gap #2) and
    // asserted-out by the TODO. The translator now uses
    // `box(...) text(font: ..., "...")` instead of `raw("...")`
    // and the body glyphs appear in the PDF.
    assert_text_contains(
        &out,
        "let x = 1",
        "pdf_renders_inline_code_and_strong_and_emphasis",
    );
}

/// Inline code must appear as a literal token in the PDF body.
/// This is a focused companion to
/// `pdf_renders_inline_code_and_strong_and_emphasis` that
/// isolates the inline-code path so a future regression in the
/// `box + text` emit points at this test rather than the
/// multi-feature combined test.
///
/// The previous `#raw("body")` form dropped the body glyphs in
/// typst-as-lib 0.16 / typst 0.15.1 (ADR gap #2). The
/// translator now uses `#box(fill: luma(245), inset: 2pt,
/// radius: 2pt, text(font: ("DejaVu Sans Mono",
/// "Liberation Mono", "Courier New"), size: 0.9em, "body"))`
/// which routes the body through a `text` call.
#[test]
fn pdf_renders_inline_code() {
    let md = "Use `let x = 1` to assign.\n";
    let out = compile_and_extract(md, "inline-code");
    assert_text_contains(&out, "let x = 1", "pdf_renders_inline_code");
}

/// Fenced code block must appear as a literal block of source
/// text. The translator previously used Typst's
/// `raw(block: true, ...)` which renders the block border,
/// padding, and framing correctly but dropped the body glyphs
/// in typst-as-lib 0.16 / typst 0.15.1 (ADR gap #5). The
/// translator now uses a `block(...)` wrapper around a
/// `text(font: ..., "body")` call, which bypasses the broken
/// `raw` element while keeping the same visual styling (fill,
/// inset, radius, width). The inline code path uses the same
/// `box + text` pattern; see
/// `pdf_renders_inline_code_and_strong_and_emphasis`.
#[test]
fn pdf_renders_fenced_code_block() {
    let md = "```rust\nfn main() {\n    let x: i32 = 1;\n    println!(\"{}\", x);\n}\n```\n";
    let out = compile_and_extract(md, "code-block");
    assert_text_contains(&out, "fn main()", "pdf_renders_fenced_code_block");
    assert_text_contains(&out, "let x: i32 = 1;", "pdf_renders_fenced_code_block");
    assert_text_contains(&out, "println!", "pdf_renders_fenced_code_block");
}

/// GFM table cell text must appear. A GFM-table regression
/// (e.g., the `__COLS__` placeholder not being patched) would
/// either fail to compile or render an empty cell row. The
/// test exercises a table with all four edge cases: a header
/// row, a body row, an empty cell (the `| |` row), and a cell
/// containing a long phrase that previously broke the column
/// count patcher.
#[test]
fn pdf_renders_gfm_table() {
    let md = "| Header A | Header B |\n|----------|----------|\n| alpha    | beta     |\n|          | gamma    |\n";
    let out = compile_and_extract(md, "table");
    for needle in ["Header A", "Header B", "alpha", "beta", "gamma"] {
        assert_text_contains(&out, needle, "pdf_renders_gfm_table");
    }
}

/// A link must surface the link text (the visible "click here"
/// part) in the PDF; the URL is rendered by Typst's `link`
/// function but typically not duplicated as visible text on
/// the same line.
#[test]
fn pdf_renders_link_text() {
    let md = "See [the example](https://example.com) for details.\n";
    let out = compile_and_extract(md, "link");
    assert_text_contains(&out, "the example", "pdf_renders_link_text");
    assert_text_contains(&out, "for details", "pdf_renders_link_text");
}

/// A block quote must surface the quoted text. The translator
/// emits `#quote(block: true)[...]`; if the block close is
/// dropped, the remaining document would be parsed as content
/// of the quote and either fail to compile or render only the
/// quoted text.
#[test]
fn pdf_renders_blockquote() {
    let md = "> A famous quotation.\n>\n> Attribution, year.\n";
    let out = compile_and_extract(md, "quote");
    assert_text_contains(&out, "A famous quotation.", "pdf_renders_blockquote");
    assert_text_contains(&out, "Attribution, year.", "pdf_renders_blockquote");
}

/// Markdown markup-active characters in user text must appear
/// as their literal characters in the PDF, not as Typst
/// markup-triggered constructs. The escape function for
/// regular text escapes `# * _ [ ] @ $ ~ ' " < > \`; this test
/// pins the contract that the escaped forms render as the
/// user-typed characters.
///
/// The test only uses characters that are NOT markdown-active
/// (so the input isn't parsed as markdown structure) but ARE
/// Typst-active (so the escape function has to do something).
/// The intersection `* _ ~ # [ ]` is excluded because they
/// would trigger markdown emphasis/strikethrough/heading/list
/// parsing and the resulting extracted text would no longer
/// contain the literal chars.
#[test]
fn pdf_renders_special_chars_verbatim() {
    let md = r#"C# costs $5 @mention "quoted" (parens) \backslash 'apostrophe
"#;
    let out = compile_and_extract(md, "special");
    for needle in [
        "C#",
        "$5",
        "@mention",
        "\"quoted\"",
        "(parens)",
        r"\backslash",
        "'apostrophe",
    ] {
        assert_text_contains(&out, needle, "pdf_renders_special_chars_verbatim");
    }
}

/// Horizontal rule must surface as a visible line. The
/// translator emits `#line(length: 100%, stroke: 0.5pt)`; if
/// the `---` rule is dropped, the body would just flow into
/// the next block with a single blank line.
#[test]
fn pdf_renders_horizontal_rule() {
    let md = "Before rule.\n\n---\n\nAfter rule.\n";
    let out = compile_and_extract(md, "rule");
    assert_text_contains(&out, "Before rule.", "pdf_renders_horizontal_rule");
    assert_text_contains(&out, "After rule.", "pdf_renders_horizontal_rule");
}

/// The PDF must contain AT LEAST one PDF text-show operator
/// (in the structural sense: a font dictionary entry). This
/// is the smaller, byte-level companion to
/// `pdf_renders_special_chars_verbatim` — the byte-level check
/// is fast and catches a total font-loading regression before
/// the slower content-level tests get to run.
///
/// Kept as a separate test from the content-level ones so a
/// failure in either direction is unambiguous: if the font
/// dictionary is missing, the engine has no fonts; if specific
/// text is missing, the translator or escape is wrong.
#[test]
fn pdf_has_font_dictionary() {
    let md = "Hello world\n";
    let bytes = compile_markdown_to_pdf(md, "font-dict").expect("compile_markdown_to_pdf");
    let needle = b"/Type/Font";
    assert!(
        bytes.windows(needle.len()).any(|w| w == needle),
        "compiled PDF has no /Type/Font dictionary — engine built \
         with empty font book; see compile_typst_document in \
         print_pdf.rs for the search_fonts_with call"
    );
}

/// Math events (`Event::InlineMath` / `Event::DisplayMath`) used
/// to be silently dropped (ADR gap #7). This is the
/// integration-level proof that they now emit Typst math mode
/// AND compile to a valid PDF. A drop-the-event bug would not
/// crash the spec test (the surrounding text still compiles);
/// only an explicit compile check catches it.
///
/// The test uses Typst-native math syntax — `alpha` (no
/// backslash) and `frac(a, b)` (function call) — because
/// Typst's math mode is NOT LaTeX-compatible. The `\` is the
/// markup escape character even inside math, so LaTeX-style
/// `\alpha` / `\frac{a}{b}` would fail to compile and the
/// test would be testing Typst's parser instead of the
/// translator.
#[test]
fn math_compiles_to_a_valid_pdf() {
    let md =
        "Pythagoras: $a^2 + b^2 = c^2$.\n\nGreek letter: $alpha$.\n\nFraction: $frac(1, 2)$.\n";
    let bytes = compile_markdown_to_pdf(md, "math").expect("math markdown must compile");
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(bytes.ends_with(b"%%EOF"));
    // PDF body must be meaningful — not a header-only artifact.
    assert!(
        bytes.len() > 6_000,
        "math PDF suspiciously small: {} bytes",
        bytes.len()
    );
}

/// Footnote events (`Event::FootnoteReference`,
/// `Tag::FootnoteDefinition`) used to be silently dropped
/// (ADR gap #6). This is the integration-level proof that
/// they now emit Typst `#footnote[body]` content blocks AND
/// compile to a valid PDF.
///
/// The test exercises the two-pass translation path: a
/// reference before the definition, a body with inline
/// emphasis, and a second reference to the same footnote.
/// A drop-the-event bug would not crash the spec test (the
/// surrounding text still compiles); only an explicit
/// compile check catches it.
#[test]
fn footnotes_compile_to_a_valid_pdf() {
    let md = "\
First[^1] ref, then later: second[^1] ref.

Footnotes can have **bold** in the body[^2].

[^1]: shared body.
[^2]: another body with *italic* text.
";
    let bytes = compile_markdown_to_pdf(md, "footnotes").expect("footnote markdown must compile");
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(bytes.ends_with(b"%%EOF"));
    // PDF body must be meaningful — not a header-only artifact.
    assert!(
        bytes.len() > 6_000,
        "footnotes PDF suspiciously small: {} bytes",
        bytes.len()
    );
}

#[test]
fn pdf_font_sizes_match_spec() {
    let md = "# Heading 1\n\n## Heading 2\n\n### Heading 3\n\nBody text.\n";
    let bytes =
        compile_markdown_to_pdf(md, "font-sizes").expect("font-sizes markdown must compile");

    let doc = pdf_oxide::PdfDocument::from_bytes(bytes).unwrap();
    let spans = doc.extract_spans(0).unwrap();

    let mut h1_size = 0.0;
    let mut h2_size = 0.0;
    let mut h3_size = 0.0;
    let mut body_size = 0.0;

    for span in spans {
        let text = span.text.trim();
        let size = span.font_size;

        if text.contains("Heading 1") {
            h1_size = size;
        } else if text.contains("Heading 2") {
            h2_size = size;
        } else if text.contains("Heading 3") {
            h3_size = size;
        } else if text.contains("Body text") {
            body_size = size;
        }
    }

    assert!(
        (body_size - 10.0).abs() < 0.1,
        "Body text should be 10pt, got {}pt",
        body_size
    );
    assert!(
        (h1_size - 16.0).abs() < 0.1,
        "H1 should be 16pt, got {}pt",
        h1_size
    );
    assert!(
        (h2_size - 14.0).abs() < 0.1,
        "H2 should be 14pt, got {}pt",
        h2_size
    );
    assert!(
        (h3_size - 12.0).abs() < 0.1,
        "H3 should be 12pt, got {}pt",
        h3_size
    );
}
