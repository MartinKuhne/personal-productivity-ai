//! Property tests: random printable-ASCII markdown must always
//! compile to valid Typst markup without panicking.
//!
//! Proptest generates a stream of short random strings, each of
//! which is wrapped in a minimal markdown frame and pushed through
//! `render_markdown_to_typst`. The property under test is: **the
//! translator must not panic or emit empty output for any
//! printable-ASCII input**.
//!
//! Sidecar of `save.rs`. Per AGENTS.md RUST-056 / RUST-057.

use fastmd_pdf::render_markdown_to_typst;
use proptest::prelude::*;

/// Strategy: short printable-ASCII strings (no newlines, no
/// nulls, no control chars). Catches every character that the
/// Typst syntax reference marks as markup-active, plus the
/// surrounding safe chars that should pass through unchanged.
fn printable_ascii_string() -> impl Strategy<Value = String> {
    prop::string::string_regex("[\\x20-\\x7E]{0,40}")
        .expect("valid regex")
        .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Any printable-ASCII text inside a paragraph must compile
    /// to Typst markup without panicking.
    #[test]
    fn random_printable_ascii_compiles(body in printable_ascii_string()) {
        let md = format!("# Heading\n\n{body}\n");
        let typst = render_markdown_to_typst(&md);
        prop_assert!(!typst.is_empty(), "typst output was empty");
        prop_assert!(typst.contains("= Heading"), "heading was not emitted");
    }

    /// The same printable-ASCII text placed inside a fenced code
    /// block must compile to Typst without panicking.
    #[test]
    fn random_printable_ascii_in_code_block_compiles(body in printable_ascii_string()) {
        let md = format!("```\n{body}\n```\n");
        let typst = render_markdown_to_typst(&md);
        prop_assert!(!typst.is_empty(), "typst output was empty");
    }

    /// The same text placed inside a markdown link must compile
    /// to Typst without panicking.
    #[test]
    fn random_printable_ascii_as_link_url_compiles(url in printable_ascii_string()) {
        if url.is_empty() || url == ")" {
            return Ok(());
        }
        let md = format!("See [link]({url}) for details.\n");
        let typst = render_markdown_to_typst(&md);
        prop_assert!(!typst.is_empty(), "typst output was empty");
    }
}
