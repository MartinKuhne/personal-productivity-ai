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
//! Either failure counts as "this example didn't round-trip". The
//! allow-list in [`KNOWN_ROUND_TRIP_FAILURES`] covers examples we
//! *expect* to fail. The test fails on any example that fails and
//! is NOT in the allow-list — that catches regressions while
//! letting the documented gaps stand.
//!
//! # Known limitations (covered by the allow-list)
//!
//! ## Setext heading + raw HTML interaction
//!
//! The CommonMark spec explicitly tests that a `---` setext
//! underline appearing inside what *should* be a raw HTML block
//! does not turn the HTML into a heading. The test URLs `<a ...>`
//! trigger pulldown-cmark 0.13's inline-HTML fall-through, which
//! then *does* see the `---` as a heading underline. The translator
//! faithfully emits the resulting events as Typst markup, and Typst
//! rightly rejects the malformed output.
//!
//! This is a pulldown-cmark-level limitation, not a translator bug.
//! Workaround would require a pre-processor that detects type-1 HTML
//! blocks by hand and routes them around the setext check.
//!
//! ## Code spans with embedded backticks
//!
//! `` ``foo`bar`` `` produces a `Code("`foo`bar")` event. The
//! surrounding `\`\`` markers in the source markdown are part of the
//! span delimiter, not the content. The translator's `\`-escape for
//! a literal backtick inside the body is correct, but Typst's
//! raw-block parser also wants the closing backtick to match
//! the opening run, and the embedded one trips it up.
//!
//! ## Emphasis / strong adjacent to text
//!
//! `foo*bar*` (no space before `*`), `*foo**bar***` — the CommonMark
//! emphasis algorithm resolves these into `<em>` and `<strong>` in
//! ways the translator currently does not surface. The resulting
//! Typst has unbalanced `*` markers.
//!
//! ## Lazy / nested list continuation
//!
//! A list item that continues with an indented paragraph (or
//! another list) is the dominant remaining failure class. The
//! translator emits `#list(...)` with one `+ ` per item, but Typst
//! requires every paragraph after the marker to either start with
//! `+ ` (a new item) or be wrapped in its own content block. Mixed
//! ordered/unordered markers (`1.` then `-`) and `1)` start markers
//! are similarly not handled.
//!
//! ## Reference link with malformed text / URLs containing `<`
//!
//! `[a](<b>c` exercises an autolink-in-link URL that the markdown
//! parser passes through to our link handler. The translator emits
//! a Typst string that contains a `<`, which Typst treats as the
//! start of a label, leading to "unclosed label".

#![cfg(feature = "pdf-export")]

use std::collections::HashSet;

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

/// Spec examples that the translator does not yet round-trip to a
/// valid PDF, with a short reason for each category. The test
/// fails on any example that fails to round-trip and is NOT in
/// this list — that catches regressions while letting the
/// documented gaps stand. When a gap is fixed, remove its number
/// from the list and the test starts asserting the case passes.
///
/// Numbers are 1-based and match the spec's "Example N" labels.
/// They are sorted within each category for easy review.
///
/// The previous version of this list contained 77 entries. After
/// fixing the ordered-list syntax bug (the old
/// `#list(marker: ([_],), + Item)` shape is invalid Typst; the
/// new shape is `#enum(numbering: "1.")[ + Item]`), 24 list
/// entries now pass and have been removed. The remaining 51
/// entries are the real translator gaps the spec corpus still
/// surfaces: 29 HTML-only examples (translator drops the HTML
/// per v1 scope), 6 setext-heading-plus-raw-HTML cases
/// (pulldown-cmark limitation), 2 code-span-with-embedded-
/// backtick cases, 4 emphasis-adjacent-to-text cases, and 10
/// lazy / nested-list continuation cases.
const KNOWN_ROUND_TRIP_FAILURES: &[usize] = &[
    // -- HTML-only content (29) --
    // Body is entirely raw HTML, which the v1 translator drops
    // (see `crate::markdown::typst`'s "Out of scope for v1" section).
    // The PDF compiles to a valid (but blank) page for these.
    107, 108, 110, 111, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 128, 130, 135, 138,
    143, 146, 147, 569, 570, 571, 572, 579, 598, 599,
    // -- Setext heading + raw HTML (6) --
    // pulldown-cmark 0.13 sometimes treats `<a ...>`-prefixed lines
    // as paragraphs instead of type-1 HTML blocks, then sees a
    // `---` setext underline on the next line and emits a spurious
    // heading. Translator faithfully reflects the events;
    // pulldown-cmark-level limitation.
    48, 575, 576, 577, 578, 588,
    // -- Code spans with embedded backticks (2) --
    // `` ``foo`bar`` `` produces a `Code("`foo`bar")` event whose
    // body contains a backtick. Typst's raw-block parser wants
    // matching backtick runs; the embedded one trips it up.
    295, 296,
    // -- Emphasis / strong adjacent to text (4) --
    // `foo*bar*` (no space before `*`), `*foo**bar***` and friends
    // — the CommonMark emphasis algorithm resolves these into
    // `<em>` / `<strong>` in ways the translator currently does
    // not surface. The resulting Typst has unbalanced `*` markers.
    311, 326, 352, 371,
    // -- Lazy / nested list continuation (10) --
    // List items that continue with an indented paragraph or a
    // sub-list, or mix ordered/unordered markers in a way the
    // translator's flat `#enum` / bullet-list emission does not
    // surface. The previous allow-list had 34 of these; the 24
    // that are now passing went through once we stopped emitting
    // the invalid `#list(marker: ([_],), + Item)` shape.
    285, 286, 287, 337, 450, 524, 552, 555, 558, 562,
];

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

    let known: HashSet<usize> = KNOWN_ROUND_TRIP_FAILURES.iter().copied().collect();

    let mut unexpected_empty: Vec<String> = Vec::new();
    for (n, md) in &examples {
        let typst = render_markdown_to_typst(md);
        if typst.trim().is_empty() && !known.contains(n) {
            unexpected_empty.push(format!("example #{n}: translator produced empty Typst"));
        }
    }

    assert!(
        unexpected_empty.is_empty(),
        "{} spec examples produced empty Typst unexpectedly. \
         The {} entries in `KNOWN_ROUND_TRIP_FAILURES` are expected \
         to be empty; these are not in the allow-list. New failures:\n{}",
        unexpected_empty.len(),
        known.len(),
        unexpected_empty.join("\n")
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

    let known: HashSet<usize> = KNOWN_ROUND_TRIP_FAILURES.iter().copied().collect();

    let mut unexpected_failures: Vec<String> = Vec::new();
    let mut known_failures_seen: usize = 0;

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
            if known.contains(n) {
                known_failures_seen += 1;
                eprintln!("  known-fail example #{n}: {reason}");
            } else {
                unexpected_failures.push(format!("example #{n}: {reason}"));
            }
        }
    }

    eprintln!(
        "compile_to_pdf: {} of {} spec examples failed ({} in allow-list, {} unexpected)",
        known_failures_seen + unexpected_failures.len(),
        examples.len(),
        known_failures_seen,
        unexpected_failures.len(),
    );

    assert!(
        unexpected_failures.is_empty(),
        "{} UNEXPECTED spec-example failures (the {} known \
         translator gaps in `KNOWN_ROUND_TRIP_FAILURES` are still \
         expected to fail and are not listed here). \
         These new failures are regressions — please either fix the \
         translator or add the new example to the allow-list with a \
         comment explaining the gap. Unexpected failures:\n{}",
        unexpected_failures.len(),
        known_failures_seen,
        unexpected_failures.join("\n")
    );
}
