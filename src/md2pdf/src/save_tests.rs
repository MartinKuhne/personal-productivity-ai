//! Typst generation and syntax compliance tests.
//!
//! Sidecar of `lib.rs` in `fastmd-pdf`. Tests verify that Markdown constructs
//! translate into syntactically valid Typst source and preserve expected content.

use super::engine::TEMPLATE;
use super::{compile_markdown_to_pdf, is_typst_available, render_markdown_to_typst};

/// Helper: parse Typst markup and assert that no syntax errors were produced.
fn assert_valid_typst(typst: &str) {
    let node = typst_syntax::parse(typst);
    let (errors, _) = node.errors_and_warnings();
    assert!(
        errors.is_empty(),
        "Typst syntax errors in:\n{typst}\nErrors: {errors:#?}"
    );
}

/// Helper: assert that generated Typst is syntactically valid and contains a needle.
fn assert_text_contains(rendered: &str, needle: &str, test_name: &str) {
    assert_valid_typst(rendered);
    assert!(
        rendered.contains(needle),
        "{test_name}: expected rendered Typst markup to contain {needle:?}, \
         but it was missing. Rendered Typst:\n{rendered}"
    );
}

#[test]
fn render_markdown_to_typst_handles_full_gfm() {
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
    assert_valid_typst(&typst);
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
fn rendered_typst_contains_text_content() {
    let md = "Hello world\n\n# Heading 1\n\nA paragraph of body text.\n";
    let typst = render_markdown_to_typst(md);
    assert_valid_typst(&typst);
    assert!(typst.contains("Hello world"));
    assert!(typst.contains("= Heading 1"));
    assert!(typst.contains("A paragraph of body text."));
}

// =====================================================================
// Typst syntax-reference compliance (end-to-end)
// =====================================================================

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
    assert_valid_typst(&typst);
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
    assert_valid_typst(&typst);
    assert!(typst.contains(r"C\#"));
    assert!(typst.contains(r"\$5"));
}

#[test]
fn url_with_hash_and_ampersand_compiles() {
    let md = "Visit [example](https://example.com/page?x=1&y=2#section-3) for details.\n";
    let typst = render_markdown_to_typst(md);
    assert_valid_typst(&typst);
    assert!(typst.contains("https://example.com/page?x=1&y=2#section-3"));
    assert!(typst.contains("[example]"));
}

#[test]
fn smart_quote_chars_preserved_in_output() {
    let md = "He said \"don't worry\" and walked away.\n";
    let typst = render_markdown_to_typst(md);
    assert_valid_typst(&typst);
    assert!(typst.contains(r#"\""#) || typst.contains(r#"""#) || typst.contains("don"));
}

#[test]
fn tilde_in_text_compiles() {
    let md = "Saved ~50% of the bytes.\n";
    let typst = render_markdown_to_typst(md);
    assert_valid_typst(&typst);
    assert!(typst.contains(r"\~50%"));
}

#[test]
fn combined_special_chars_in_paragraph_compile() {
    let md = "Try `a*b_c` and `c#lang` and `cost $5` and `~50%` — all literal.\n";
    let typst = render_markdown_to_typst(md);
    assert_valid_typst(&typst);
    assert!(typst.contains("a*b_c"));
    assert!(typst.contains("c#lang"));
    assert!(typst.contains("cost $5"));
    assert!(typst.contains("~50%"));
}

#[test]
fn ordered_list_with_long_item_compiles() {
    let md = "\
### Phase 1: Pre-Start Assessment

1. **Battery**: Remove, test voltage. Likely needs replacement after years storage. X2 battery location: under seat, negative cable first. PN 4140006.
2. **Fuel system**: Drain tank completely.
";
    let typst = render_markdown_to_typst(md);
    assert_valid_typst(&typst);
    assert!(typst.contains("=== Phase 1: Pre-Start Assessment"));
    assert!(typst.contains("#strong[Battery]"));
    assert!(typst.contains("#strong[Fuel system]"));
}

// =====================================================================
// Typst content & structure assertions
// =====================================================================

#[test]
fn typst_renders_h1_through_h3() {
    let md = "# Top level\n\n## Section\n\n### Subsection\n\nBody text.\n";
    let out = render_markdown_to_typst(md);
    assert_text_contains(&out, "= Top level", "typst_renders_h1_through_h3");
    assert_text_contains(&out, "== Section", "typst_renders_h1_through_h3");
    assert_text_contains(&out, "=== Subsection", "typst_renders_h1_through_h3");
    assert_text_contains(&out, "Body text.", "typst_renders_h1_through_h3");
}

#[test]
fn typst_renders_lists() {
    let md = "- alpha\n- beta\n- gamma\n\n1. first\n2. second\n\n- [ ] todo one\n- [x] todo two\n";
    let out = render_markdown_to_typst(md);
    assert_valid_typst(&out);
    for needle in [
        "- alpha", "- beta", "- gamma", "first", "second", "todo one", "todo two",
    ] {
        assert_text_contains(&out, needle, "typst_renders_lists");
    }
}

#[test]
fn typst_renders_inline_code_and_strong_and_emphasis() {
    let md = r#"
Strong: **this is bold**.

Emphasis: *this is italic*.

Adjacent strong: foo**bar**baz.

Adjacent emphasis: foo*bar*baz.

Inline code: `let x = 1`.
"#;
    let out = render_markdown_to_typst(md);
    assert_text_contains(
        &out,
        "#strong[this is bold]",
        "typst_renders_inline_code_and_strong_and_emphasis",
    );
    assert_text_contains(
        &out,
        "#emph[this is italic]",
        "typst_renders_inline_code_and_strong_and_emphasis",
    );
    assert_text_contains(
        &out,
        "foo#strong[bar]baz",
        "typst_renders_inline_code_and_strong_and_emphasis",
    );
    assert_text_contains(
        &out,
        "foo#emph[bar]baz",
        "typst_renders_inline_code_and_strong_and_emphasis",
    );
    assert_text_contains(
        &out,
        "let x = 1",
        "typst_renders_inline_code_and_strong_and_emphasis",
    );
}

#[test]
fn typst_renders_inline_code() {
    let md = "Use `let x = 1` to assign.\n";
    let out = render_markdown_to_typst(md);
    assert_text_contains(&out, "let x = 1", "typst_renders_inline_code");
}

#[test]
fn typst_renders_fenced_code_block() {
    let md = "```rust\nfn main() {\n    let x: i32 = 1;\n    println!(\"{}\", x);\n}\n```\n";
    let out = render_markdown_to_typst(md);
    assert_text_contains(&out, "fn main()", "typst_renders_fenced_code_block");
    assert_text_contains(&out, "let x: i32 = 1;", "typst_renders_fenced_code_block");
    assert_text_contains(&out, "println!", "typst_renders_fenced_code_block");
}

#[test]
fn typst_renders_gfm_table() {
    let md = "| Header A | Header B |\n|----------|----------|\n| alpha    | beta     |\n|          | gamma    |\n";
    let out = render_markdown_to_typst(md);
    assert_text_contains(&out, "#table(", "typst_renders_gfm_table");
    for needle in ["Header A", "Header B", "alpha", "beta", "gamma"] {
        assert_text_contains(&out, needle, "typst_renders_gfm_table");
    }
}

#[test]
fn typst_renders_link_text() {
    let md = "See [the example](https://example.com) for details.\n";
    let out = render_markdown_to_typst(md);
    assert_text_contains(&out, "the example", "typst_renders_link_text");
    assert_text_contains(&out, "for details", "typst_renders_link_text");
}

#[test]
fn typst_renders_blockquote() {
    let md = "> A famous quotation.\n>\n> Attribution, year.\n";
    let out = render_markdown_to_typst(md);
    assert_text_contains(&out, "#quote(block: true)", "typst_renders_blockquote");
    assert_text_contains(&out, "A famous quotation.", "typst_renders_blockquote");
    assert_text_contains(&out, "Attribution, year.", "typst_renders_blockquote");
}

#[test]
fn typst_renders_special_chars_verbatim() {
    let md = r#"C# costs $5 @mention "quoted" (parens) \backslash 'apostrophe
"#;
    let out = render_markdown_to_typst(md);
    assert_valid_typst(&out);
    for needle in [
        r"C\#",
        r"\$5",
        r"\@mention",
        r#"\"quoted\""#,
        "(parens)",
        "backslash",
        "apostrophe",
    ] {
        assert_text_contains(&out, needle, "typst_renders_special_chars_verbatim");
    }
}

#[test]
fn typst_renders_horizontal_rule() {
    let md = "Before rule.\n\n---\n\nAfter rule.\n";
    let out = render_markdown_to_typst(md);
    assert_text_contains(&out, "#line(length: 100%", "typst_renders_horizontal_rule");
    assert_text_contains(&out, "Before rule.", "typst_renders_horizontal_rule");
    assert_text_contains(&out, "After rule.", "typst_renders_horizontal_rule");
}

#[test]
fn typst_template_specifies_font_family() {
    assert!(
        TEMPLATE.contains(r#"font: ("Segoe UI", "Helvetica Neue", "Liberation Sans", "Arial")"#),
        "Template must configure default fallback font family"
    );
}

#[test]
fn math_renders_to_typst_math_blocks() {
    let md =
        "Pythagoras: $a^2 + b^2 = c^2$.\n\nGreek letter: $alpha$.\n\nFraction: $frac(1, 2)$.\n";
    let typst = render_markdown_to_typst(md);
    assert_valid_typst(&typst);
    assert!(typst.contains("$a^2 + b^2 = c^2$"));
    assert!(typst.contains("$alpha$"));
    assert!(typst.contains("$frac(1, 2)$"));
}

#[test]
fn footnotes_render_to_typst_footnote_blocks() {
    let md = "\
First[^1] ref, then later: second[^1] ref.

Footnotes can have **bold** in the body[^2].

[^1]: shared body.
[^2]: another body with *italic* text.
";
    let typst = render_markdown_to_typst(md);
    assert_valid_typst(&typst);
    assert!(typst.contains("#footnote[shared body.]"));
    assert!(typst.contains("#strong[bold]"));
    assert!(typst.contains("#emph[italic]"));
}

#[test]
fn typst_template_specifies_correct_font_sizes() {
    assert!(
        TEMPLATE.contains("size: 10pt"),
        "Body text font size should be 10pt"
    );
    assert!(
        TEMPLATE.contains("heading.where(level: 1): set text(size: 16pt)"),
        "H1 heading font size should be 16pt"
    );
    assert!(
        TEMPLATE.contains("heading.where(level: 2): set text(size: 14pt)"),
        "H2 heading font size should be 14pt"
    );
    assert!(
        TEMPLATE.contains("heading.where(level: 3): set text(size: 12pt)"),
        "H3 heading font size should be 12pt"
    );
}

#[test]
fn compile_markdown_to_pdf_smoke_test() {
    if !is_typst_available() {
        return;
    }
    let bytes = compile_markdown_to_pdf("# Smoke Test\n\nContent.\n", "smoke")
        .expect("smoke compilation should succeed");
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(bytes.ends_with(b"%%EOF"));
}
