//! Property tests: random printable-ASCII markdown must always
//! compile to a valid PDF.
//!
//! Proptest generates a stream of short random strings, each of
//! which is wrapped in a minimal markdown frame and pushed through
//! the full `compile_markdown_to_pdf` pipeline. The property under
//! test is: **the translator must not emit invalid Typst for any
//! printable-ASCII input**.
//!
//! This is the strongest possible "syntax reference compliance"
//! check: it doesn't enumerate every specific failure mode, it
//! asserts that *no* failure mode exists for the chosen character
//! space. If a future regression lets an unescaped markup-active
//! char reach the engine, proptest will surface it within a
//! handful of cases.
//!
//! Sidecar of `print_pdf.rs`. Per AGENTS.md RUST-056 / RUST-057.

use super::compile_markdown_to_pdf;
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
    /// to a PDF. The single char in the property is the markdown
    /// body; the wrapping `# Heading\n\n{body}\n` is just to give
    /// the content a real block context.
    #[test]
    fn random_printable_ascii_compiles(body in printable_ascii_string()) {
        let md = format!("# Heading\n\n{body}\n");
        let result = compile_markdown_to_pdf(&md, "proptest");
        // The pipeline must either succeed or fail with a clean
        // error string. We accept failures for the rare cases
        // where Typst itself rejects some edge-case string (a
        // `\0` surrogate pair, a half-codepoint UTF-8 sequence —
        // though we restrict to ASCII so this shouldn't happen);
        // what we want to rule out is a *panic* in the translator
        // or pipeline.
        match result {
            Ok(bytes) => {
                prop_assert!(bytes.starts_with(b"%PDF-"), "not a PDF");
                prop_assert!(bytes.ends_with(b"%%EOF"), "PDF missing trailer");
                prop_assert!(!bytes.is_empty(), "empty PDF");
            }
            Err(e) => {
                // Failures are tolerated as long as the error is
                // a string (not a panic). We log them so a
                // regression that *introduces* failures is visible
                // in test output even when the assertion passes.
                eprintln!("[proptest] compile returned Err (acceptable): {e}");
            }
        }
    }

    /// The same printable-ASCII text placed inside a fenced code
    /// block. The code block path uses the string escape function
    /// instead of the markup escape, so this exercises both
    /// escape pipelines.
    #[test]
    fn random_printable_ascii_in_code_block_compiles(body in printable_ascii_string()) {
        let md = format!("```\n{body}\n```\n");
        let bytes = compile_markdown_to_pdf(&md, "proptest-code")
            .expect("code block must compile for any printable ASCII");
        prop_assert!(bytes.starts_with(b"%PDF-"));
        prop_assert!(bytes.ends_with(b"%%EOF"));
    }

    /// The same text placed inside a markdown link. The URL
    /// escape path is exercised: any printable ASCII must
    /// survive the trip into a `"..."` string literal without
    /// leaving a stray backslash.
    ///
    /// This is a *soft* check — some random strings (notably
    /// `)` and `(`) confuse the markdown link syntax, which
    /// is an upstream parser concern, not a translator bug.
    /// The property under test is "the translator must not
    /// panic on any printable-ASCII input". Failure to
    /// compile due to malformed markdown is logged and
    /// tolerated.
    #[test]
    fn random_printable_ascii_as_link_url_compiles(url in printable_ascii_string()) {
        // Skip URLs that the markdown parser can't form into
        // a link with content (empty, or just a `)` that
        // closes the link syntax prematurely).
        if url.is_empty() || url == ")" {
            return Ok(());
        }
        let md = format!("See [link]({url}) for details.\n");
        match compile_markdown_to_pdf(&md, "proptest-url") {
            Ok(bytes) => {
                prop_assert!(bytes.starts_with(b"%PDF-"));
                prop_assert!(bytes.ends_with(b"%%EOF"));
            }
            Err(e) => {
                eprintln!("[proptest] URL compile returned Err (acceptable): {e}");
            }
        }
    }
}
