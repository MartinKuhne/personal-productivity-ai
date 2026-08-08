//! Integration test: the full CommonMark 0.31.2 spec must round-trip
//! through the markdown→Typst translator AND compile to a valid PDF.
//!
//! Vendored spec source: see `tests/fixtures/commonmark-0.31.2-spec.txt`
//! and `tests/fixtures/README.md` for provenance and license. The spec
//! is parsed in-test, not vendored as a pre-extracted fixture, so
//! refreshing the spec is a one-file drop-in.
//!
//! Per AGENTS.md RUST-058 the markdown translator and PDF pipeline are
//! both egui-free, so this test stays in `tests/` (an integration
//! test exercising only public APIs) rather than as a sidecar of an
//! implementation file.
//!
//! # Test contract
//!
//! For every numbered example in the spec the test asserts:
//!
//! 1. `render_markdown_to_typst` produces non-empty Typst markup.
//! 2. `compile_markdown_to_pdf` produces a valid PDF (correct
//!    `%PDF-` header, `%%EOF` trailer, non-zero length).
//!
//! Either failure counts as "this example didn't round-trip" and
//! fails the test. There is no allow-list. Every spec example is
//! in scope; every gap in the translator is a real bug to fix, not
//! a documented limitation to defer.

#![cfg(feature = "pdf-export")]

use fastmd::app::print_pdf::compile_markdown_to_pdf;
use fastmd::markdown::render_markdown_to_typst;

/// Source of the vendored spec. `include_str!` resolves at compile
/// time, so the test binary carries the spec as a static — the file
/// does not need to be present at test runtime.
const SPEC: &str = include_str!("fixtures/commonmark-0.31.2-spec.txt");

/// 32 backticks — the spec's example-fence delimiter. Spelled as a
/// raw string so we don't have to count backslash escapes.
const FENCE: &str = "````````````````````````````````";
const OPEN_FENCE: &str = "```````````````````````````````` example";

/// Line that separates the markdown input from the expected HTML
/// output inside a spec example block. The spec convention is a
/// line that contains *only* a period (column 0, no whitespace).
const SEPARATOR: &str = "\n.\n";

/// End of the test-suite portion of the spec; everything after this
/// marker is the prose-only parsing strategy appendix.
const END_MARKER: &str = "<!-- END TESTS -->";

/// Walk the spec source and pull out each example's markdown input
/// (the part before `\n.\n` inside the example block).
///
/// Returns `(example_number, markdown_input)` tuples in source order.
/// The HTML portion is dropped — we only need the input half. The
/// translator output is asserted to be non-empty and to compile
/// to a valid PDF; we do not assert the rendered content matches
/// the spec's expected HTML, since our target is Typst, not HTML.
fn extract_markdown_examples(spec: &str) -> Vec<(usize, String)> {
    // 1. Strip the YAML frontmatter so a leading `---` line in
    //    the spec source can't be confused with example body.
    let body = spec
        .strip_prefix("---\n")
        .and_then(|after| after.find("\n---\n").map(|idx| &after[idx + 5..]))
        .unwrap_or(spec);

    // 2. Truncate at the end-of-tests marker. Everything after
    //    is the parsing-strategy appendix, not a test case.
    let body = body
        .split(END_MARKER)
        .next()
        .expect("spec source is not empty");

    let mut out: Vec<(usize, String)> = Vec::new();
    let mut rest: &str = body;
    let mut n: usize = 0;

    while let Some(open_idx) = rest.find(OPEN_FENCE) {
        // Skip past the opening fence line itself.
        let after_open = &rest[open_idx + OPEN_FENCE.len()..];
        let after_open = after_open.strip_prefix('\n').unwrap_or(after_open);

        // Find the matching closing fence (a bare 32-backtick line).
        // If the spec is truncated mid-example we bail out — we'd
        // rather skip the rest than emit a malformed example.
        let Some(close_off) = after_open.find(FENCE) else {
            break;
        };

        // The block body sits between the fences. Trim the trailing
        // newline so the separator is anchored to the markdown, not
        // to whitespace left over from the closing fence.
        let block_body = after_open[..close_off].trim_end_matches('\n');

        // Split on the first `\n.\n` to separate markdown from
        // expected HTML. The spec convention is "line containing
        // only `.`", and `\n.\n` is the unique byte sequence for
        // that at a line boundary. If an example doesn't include
        // the separator (malformed fixture), skip it.
        if let Some((md, _html)) = block_body.split_once(SEPARATOR) {
            n += 1;
            out.push((n, md.to_string()));
        }

        // Continue scanning after the closing fence.
        rest = &after_open[close_off + FENCE.len()..];
    }

    out
}

/// Test the translator half of the round trip: every spec example
/// must produce non-empty Typst. This is the fast canary — runs
/// in seconds and catches translator-level bugs like forgotten
/// escapes, drop-everything branches, or event-routing mistakes.
#[test]
fn all_commonmark_0_31_2_examples_translate_to_non_empty_typst() {
    let examples = extract_markdown_examples(SPEC);
    assert!(
        examples.len() >= 600,
        "expected the vendored spec to contain at least 600 examples, \
         but only {} were extracted. The fixture may need to be \
         refreshed (see tests/fixtures/README.md).",
        examples.len()
    );

    let mut failures: Vec<String> = Vec::new();
    for (n, md) in &examples {
        let typst = render_markdown_to_typst(md);
        if typst.trim().is_empty() {
            failures.push(format!("example #{n}: translator produced empty Typst"));
        }
    }

    assert!(
        failures.is_empty(),
        "{} spec examples produced empty Typst:\n{}",
        failures.len(),
        failures
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Test the compile half of the round trip: every spec example
/// must compile to a valid PDF. This is the slow, deep check —
/// typst-as-lib spins up a fresh engine per compile, so this
/// takes ~30-60 seconds on a developer laptop for the 600+
/// examples. The cost of covering the entire spec; we accept it.
/// Run with `cargo nextest run -E 'test(/commonmark/)'` to time it.
#[test]
fn all_commonmark_0_31_2_examples_compile_to_valid_pdf() {
    let examples = extract_markdown_examples(SPEC);
    assert!(
        examples.len() >= 600,
        "expected at least 600 examples, got {}",
        examples.len()
    );

    let mut failures: Vec<String> = Vec::new();
    for (n, md) in &examples {
        let result = compile_markdown_to_pdf(md, "commonmark-spec");
        let bad = match &result {
            Ok(bytes) => {
                if bytes.is_empty() {
                    Some("empty PDF".to_string())
                } else if !bytes.starts_with(b"%PDF-") {
                    Some(format!(
                        "output is not a PDF (header: {:?})",
                        &bytes[..bytes.len().min(8)]
                    ))
                } else if !bytes.ends_with(b"%%EOF") {
                    Some("PDF missing %%EOF trailer".to_string())
                } else {
                    None
                }
            }
            Err(e) => Some(format!("compile failed: {e}")),
        };

        if let Some(reason) = bad {
            failures.push(format!("example #{n}: {reason}"));
        }
    }

    assert!(
        failures.is_empty(),
        "{} spec examples failed to compile:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
