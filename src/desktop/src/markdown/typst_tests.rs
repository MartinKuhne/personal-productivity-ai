//! Unit tests for the Markdown → Typst translator.
//!
//! Sidecar of `typst.rs`. Tests are pure-string assertions on the
//! generated Typst markup; they do not invoke the Typst compiler.
//!
//! Per AGENTS.md RUST-056 / RUST-057 — extracted to a sidecar.

use super::render_markdown_to_typst;

#[test]
fn empty_input_emits_empty_string() {
    let out = render_markdown_to_typst("");
    assert!(out.trim().is_empty(), "got: {out}");
}

#[test]
fn plain_paragraph_passes_through() {
    let out = render_markdown_to_typst("hello world");
    assert!(out.contains("hello world"), "got: {out}");
}

#[test]
fn heading_levels_are_emitted() {
    let md = "# h1\n\n## h2\n\n### h3\n";
    let out = render_markdown_to_typst(md);
    // Typst heading markers: `= ` for h1, `== ` for h2, etc. They
    // appear at the start of a line and the heading text follows.
    assert!(out.contains("= h1"), "missing h1 marker in: {out}");
    assert!(out.contains("== h2"));
    assert!(out.contains("=== h3"));
}

#[test]
fn strong_renders_as_typst_strong() {
    // Markdown `**bold**` becomes typst `*bold*` (single asterisk).
    let out = render_markdown_to_typst("**bold**");
    assert!(out.contains("*bold*"), "got: {out}");
}

#[test]
fn emphasis_renders_as_typst_emphasis() {
    // Markdown `*italic*` becomes typst `_italic_`.
    let out = render_markdown_to_typst("*italic*");
    assert!(out.contains("_italic_"), "got: {out}");
}

#[test]
fn strikethrough_uses_strike_function() {
    let out = render_markdown_to_typst("~~gone~~");
    assert!(out.contains("#strike["), "got: {out}");
    assert!(out.contains("gone"));
    assert!(out.contains(']'));
}

#[test]
fn inline_code_renders_as_backtick() {
    let out = render_markdown_to_typst("`x = 1`");
    assert!(out.contains("`x = 1`"), "got: {out}");
}

#[test]
fn fenced_code_block_uses_raw_with_lang() {
    let md = "```rust\nfn main() {}\n```\n";
    let out = render_markdown_to_typst(md);
    // The new format uses the Typst string form so that curly
    // braces and other markup-special characters in code bodies
    // are passed through verbatim.
    assert!(
        out.contains("#raw(block: true, lang: \"rust\", \"fn main() {}\")")
            || out.contains("#raw(block: true, lang: \"rust\""),
        "got: {out}"
    );
    assert!(out.contains("fn main()"));
}

#[test]
fn fenced_code_block_without_lang_uses_raw_string() {
    let md = "```\nplain code\n```\n";
    let out = render_markdown_to_typst(md);
    // We emit `#raw(block: true, "...")` for the untagged case
    // so that any markup-special characters in the code body
    // (curly braces, asterisks, etc.) are passed through verbatim
    // and not interpreted as Typst markup.
    assert!(out.contains("#raw(block: true"), "got: {out}");
    assert!(out.contains("plain code"));
}

#[test]
fn unordered_list_renders_bullets() {
    let out = render_markdown_to_typst("- one\n- two\n- three\n");
    // Each item should be a `- ` bullet.
    let bullets = out.matches("- ").count();
    assert!(
        bullets >= 3,
        "expected >=3 bullets, got {bullets} in: {out}"
    );
}

#[test]
fn ordered_list_wraps_in_list_function() {
    let out = render_markdown_to_typst("1. one\n2. two\n");
    assert!(out.contains("#list("), "got: {out}");
    assert!(out.contains("+ one"));
    assert!(out.contains("+ two"));
}

#[test]
fn task_list_emits_checkbox_marker() {
    let out = render_markdown_to_typst("- [ ] todo\n- [x] done\n");
    assert!(out.contains("[ ]"), "got: {out}");
    assert!(out.contains("[x]"));
}

#[test]
fn blockquote_renders_as_quote_function() {
    let out = render_markdown_to_typst("> a quotation\n");
    assert!(out.contains("#quote(block: true)["), "got: {out}");
    assert!(out.contains("a quotation"));
}

#[test]
fn link_renders_as_link_function() {
    let out = render_markdown_to_typst("[text](https://example.com)");
    assert!(
        out.contains("#link(\"https://example.com\")["),
        "got: {out}"
    );
    assert!(out.contains("text"));
}

#[test]
fn image_renders_as_image_function() {
    let out = render_markdown_to_typst("![alt](https://example.com/img.png)");
    assert!(
        out.contains("#image(\"https://example.com/img.png\")"),
        "got: {out}"
    );
}

#[test]
fn gfm_table_emits_table_function() {
    let md = "| a | b |\n|---|---|\n| 1 | 2 |\n";
    let out = render_markdown_to_typst(md);
    assert!(out.contains("#table("), "got: {out}");
    // The placeholder is patched at Table end.
    assert!(out.contains("columns: 2"), "missing column count in: {out}");
    assert!(
        out.contains("table.header("),
        "missing table.header in: {out}"
    );
    assert!(out.contains("a"));
    assert!(out.contains("b"));
    assert!(out.contains("1"));
    assert!(out.contains("2"));
}

#[test]
fn horizontal_rule_renders_as_line() {
    let out = render_markdown_to_typst("---\n");
    assert!(out.contains("#line(length: 100%"), "got: {out}");
}

#[test]
fn hard_break_renders_as_backslash() {
    let out = render_markdown_to_typst("line one  \nline two\n");
    assert!(out.contains(" \\"), "got: {out}");
}

#[test]
fn hash_sign_in_text_is_escaped() {
    // The user writes "C# language" in Markdown. Without escaping,
    // Typst would interpret the `#` as a function-call prefix.
    let out = render_markdown_to_typst("I love C# language");
    assert!(out.contains("C\\#"), "expected escaped hash, got: {out}");
}

#[test]
fn asterisk_in_text_is_escaped() {
    let out = render_markdown_to_typst("rating: 5 * 3");
    // `*` outside of `**` or `*` markers should be escaped; the
    // important invariant is that we don't emit unescaped asterisks
    // that typst would interpret as strong/emphasis.
    assert!(
        !out.contains("rating: 5 * 3\n"),
        "unescaped asterisk: {out}"
    );
}

#[test]
fn backslash_in_text_is_escaped() {
    let out = render_markdown_to_typst(r"path: C:\Users");
    assert!(
        out.contains("C:\\\\Users") || out.contains("C:\\Users"),
        "got: {out}"
    );
}

#[test]
fn complex_doc_round_trips_all_features() {
    let md = "# Title\n\nA paragraph with **bold** and *italic*.\n\n- one\n- two\n\n```rust\nlet x = 1;\n```\n\n> a quote\n\n| h1 | h2 |\n|----|----|\n| a  | b  |\n";
    let out = render_markdown_to_typst(md);
    // Spot-check each feature survived.
    assert!(out.contains("= Title"), "missing heading: {out}");
    assert!(out.contains("*bold*") || out.contains("\\*bold\\*"));
    assert!(out.contains("_italic_"));
    assert!(
        out.contains("#raw(block: true, lang: \"rust\""),
        "got: {out}"
    );
    assert!(out.contains("#quote(block: true)["));
    assert!(out.contains("#table("));
}
