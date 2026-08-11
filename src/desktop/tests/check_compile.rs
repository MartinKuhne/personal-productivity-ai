// Perf-investigation scratch. Used to find the hang in
// `extract_content_needles` during the
// `chore/test-investments` pass — the per-example
// content-fidelity test hangs after ~125 examples in a
// tight loop. The investigation was inconclusive (the
// strip functions are pure string manipulation with no
// obvious infinite loop; the hang is likely a
// state-accumulation or allocator-heap issue). Left in
// the tree as a starting point for the next investigator.
// To run: `cargo test --test check_compile -- --ignored
// --nocapture`.
#![cfg(test)]

use fastmd::app::print_pdf::compile_markdown_to_pdf;
use std::time::Instant;

const SPEC: &str = include_str!("fixtures/commonmark-0.31.2-spec.txt");
const FENCE: &str = "````````````````````````````````";
const OPEN_FENCE: &str = "```````````````````````````````` example";
const SEPARATOR: &str = "\n.\n";
const END_MARKER: &str = "<!-- END TESTS -->";

fn extract(spec: &str) -> Vec<String> {
    let normalised = spec.replace("\r\n", "\n");
    let body = normalised
        .strip_prefix("---\n")
        .and_then(|after| after.find("\n---\n").map(|idx| &after[idx + 5..]))
        .unwrap_or(&normalised);
    let body = body.split(END_MARKER).next().unwrap();
    let mut out = Vec::new();
    let mut rest = body;
    while let Some(open_idx) = rest.find(OPEN_FENCE) {
        let after_open = &rest[open_idx + OPEN_FENCE.len()..];
        let after_open = after_open.strip_prefix('\n').unwrap_or(after_open);
        let Some(close_off) = after_open.find(FENCE) else {
            break;
        };
        let block_body = after_open[..close_off].trim_end_matches('\n');
        if let Some((md, _html)) = block_body.split_once(SEPARATOR) {
            out.push(md.to_string());
        }
        rest = &after_open[close_off + FENCE.len()..];
    }
    out
}

fn simple_needles(md: &str) -> Vec<String> {
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

#[test]
#[ignore = "perf investigation"]
fn bench_compile_all_spec() {
    // Compile all 608 spec examples (after filtering by
    // `simple_needles`). On this machine: ~5s total, no
    // hang. Proves the hang in the real per-example test
    // is NOT in `compile_markdown_to_pdf` or `pdf_oxide`.
    let examples = extract(SPEC);
    eprintln!("total: {}", examples.len());
    let t0 = Instant::now();
    let mut compiled = 0;
    for (i, src) in examples.iter().enumerate() {
        let needles = simple_needles(src);
        if needles.is_empty() {
            continue;
        }
        let t = Instant::now();
        if let Ok(bytes) = compile_markdown_to_pdf(src, "perf")
            && let Ok(doc) = pdf_oxide::PdfDocument::from_bytes(bytes)
        {
            let _ = doc.extract_spans(0);
        }
        let ms = t.elapsed().as_millis();
        compiled += 1;
        if ms > 50 || i % 50 == 0 {
            eprintln!(
                "[{i}] {ms}ms (elapsed: {}ms compiled: {})",
                t0.elapsed().as_millis(),
                compiled
            );
        }
    }
    eprintln!(
        "DONE: total={}ms compiled={}",
        t0.elapsed().as_millis(),
        compiled
    );
}
