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
//! 3. The PDF carries *content from the source* — at least one
//!    word-token from the markdown input appears in the extracted
//!    PDF text. This is the content-fidelity check that the
//!    header/EOF assertions above cannot make: a translator
//!    that silently drops body text would still be 652/652
//!    green for the structural checks but fail the content check
//!    immediately.
//!
//! Test (3) is currently `#[ignore]`'d because it is ~5x slower
//! than the structural checks (the spec test spins up a fresh
//! Typst engine per example either way, but the `pdf_oxide`
//! text extraction adds a per-example cost on top). The rollout
//! plan is per-section: start with one section, verify the
//! needles are right, expand. The `#[ignore]` attribute
//! prevents this from running in CI by default; remove it once
//! the runtime is acceptable. See `doc/adr/pdf-export-test-gaps.md`
//! gaps #1, #4, #10 for the contract being verified.

#![cfg(feature = "pdf-export")]

use fastmd::app::print_pdf::compile_markdown_to_pdf;
use fastmd::pdf::typst_translator::render_markdown_to_typst;

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
    // 0. Normalize line endings. `include_str!` embeds the file
    //    bytes verbatim, which on Windows means the spec is
    //    CRLF-terminated. The line-anchored searches below
    //    (SEPARATOR = "\n.\n", OPEN_FENCE stripped of "\n",
    //    etc.) would never match a spec with "\r\n" terminators
    //    and the function would return zero examples. The
    //    spec is committed with LF endings; the normalisation
    //    is a no-op on Linux CI and a one-pass swap on
    //    Windows. Drive-by fix for the previously-broken
    //    Windows run.
    let normalised = spec.replace("\r\n", "\n");

    // 1. Strip the YAML frontmatter so a leading `---` line in
    //    the spec source can't be confused with example body.
    let body = normalised
        .strip_prefix("---\n")
        .and_then(|after| after.find("\n---\n").map(|idx| &after[idx + 5..]))
        .unwrap_or(&normalised);

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

/// Extract a small set of representative word-tokens from a
/// markdown source string. The contract is "at least one needle
/// appears in the rendered PDF text"; the heuristic picks the
/// first N non-trivial alphanumeric tokens (length ≥ 4) to
/// avoid false positives from short common words (the, and, of,
/// …) that might appear in unrelated content.
///
/// Markdown markers and punctuation are collapsed to whitespace
/// before tokenisation, so `**bold**` and `\`code\`` both
/// surface the bare word. Code-block fences and HTML tags are
/// dropped the same way; the goal is "did the user's *content*
/// make it into the PDF", not "did the markup survive".
///
/// Case is lowercased for both the needle and the extracted PDF
/// text; `pdf_oxide` preserves original case in spans, so
/// lowercasing both sides is a no-op for the matching but
/// avoids case-sensitivity false negatives.
fn extract_content_needles(md: &str) -> Vec<String> {
    md.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .filter(|w| w.len() >= 4)
        .take(3)
        .map(String::from)
        .collect()
}

/// Test the *content* half of the round trip: every spec example
/// whose source contains at least one non-trivial word must
/// render that word into the PDF. Closes ADR gaps #1, #4, #10
/// (the structural-only spec test was 652/652 green for a
/// translator that dropped all body content; this content
/// fidelity check would fail that same translator immediately).
///
/// Marked `#[ignore]` because the test is ~5x slower than the
/// compile-only spec test: each of the 652 examples pays the
/// `pdf_oxide` extraction cost on top of the Typst engine
/// compile. Rollout plan (per the ADR):
///
/// 1. `cargo nextest run -p fastmd --features pdf-export --run-ignored all_commonmark_examples_render_content_into_pdf`
///    to time it locally (~2-5 minutes expected).
/// 2. If the runtime is acceptable, remove the `#[ignore]`.
/// 3. If specific sections fail, those are real translator bugs
///    to fix (likely gaps in `escape_typst`, `escape_typst_string`,
///    or the spec-corpus edge cases like type-1 HTML blocks).
///
/// The needle count per example is capped at 3 to keep the test
/// fast and to avoid pinning a test pass on a single rare
/// occurrence of a word; "at least one" is the contract.
#[test]
#[ignore = "per-example content fidelity; ~5x slower than the \
          structural-only spec test (adds pdf_oxide text \
          extraction on top of the Typst engine compile). \
          Promote to default-on once the runtime is acceptable. \
          See doc/adr/pdf-export-test-gaps.md gaps #1, #4, #10."]
fn all_commonmark_examples_render_content_into_pdf() {
    let examples = extract_markdown_examples(SPEC);
    assert!(
        examples.len() >= 600,
        "expected at least 600 examples, got {}",
        examples.len()
    );

    let mut failures: Vec<String> = Vec::new();
    let mut no_needles: Vec<String> = Vec::new();
    for (n, md) in &examples {
        let needles = extract_content_needles(md);
        if needles.is_empty() {
            // No tokens of length ≥ 4 (e.g. an example that is
            // just punctuation or a single character). Nothing
            // meaningful to assert; record and move on.
            no_needles.push(format!("#{n}"));
            continue;
        }
        let result = compile_markdown_to_pdf(md, "commonmark-content");
        let bytes = match result {
            Ok(b) => b,
            Err(e) => {
                failures.push(format!(
                    "example #{n}: compile failed: {e} (needles: {needles:?})"
                ));
                continue;
            }
        };
        let doc = match pdf_oxide::PdfDocument::from_bytes(bytes) {
            Ok(d) => d,
            Err(e) => {
                failures.push(format!(
                    "example #{n}: pdf_oxide parse failed: {e} (needles: {needles:?})"
                ));
                continue;
            }
        };
        let spans = match doc.extract_spans(0) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!(
                    "example #{n}: extract_spans failed: {e} (needles: {needles:?})"
                ));
                continue;
            }
        };
        let extracted: String = spans
            .iter()
            .map(|s| s.text.to_ascii_lowercase())
            .collect::<Vec<_>>()
            .join(" ");
        if !needles.iter().any(|needle| extracted.contains(needle)) {
            failures.push(format!(
                "example #{n}: no needle from source found in PDF text. \
                 Needles: {needles:?}. Source: {md:?}"
            ));
        }
    }

    // The no-needles case is informational, not a failure —
    // some spec examples are pure punctuation or single chars
    // and have nothing to assert on. We log it via eprintln so
    // it's visible in the test output without failing the
    // assertion.
    if !no_needles.is_empty() {
        eprintln!(
            "[commonmark-content] {} examples had no needles of length >= 4 \
             (skipped): {}",
            no_needles.len(),
            no_needles
                .iter()
                .take(20)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    assert!(
        failures.is_empty(),
        "{} spec examples failed content fidelity check:\n{}",
        failures.len(),
        failures
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
