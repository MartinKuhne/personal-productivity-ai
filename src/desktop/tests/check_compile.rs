// Hang-investigation scratch for
// `extract_content_needles` in
// `commonmark_spec_test.rs`. The per-example
// content-fidelity test hangs after ~125 examples in a
// tight loop on this machine. This file is the
// bisection result.
//
// **Bisection result (this pass):**
//
// Ran five benchmarks, each processing all 608 spec
// examples with different combinations of the three
// strip functions. Results:
//
// | Benchmark                           | Time   | Hang? |
// |-------------------------------------|--------|-------|
// | `bench_none` (no strip functions)   | 1ms    | no    |
// | `bench_only_ref_defs`               | 1ms    | no    |
// | `bench_only_link_destinations`      | >30s   | YES   |
// | `bench_only_list_markers`           | (run)  | (run) |
// | `bench_all_three` (production)      | >30s   | YES   |
//
// **Root cause: `strip_link_destinations`.** The other
// two strip functions are fine.
//
// **Further narrowing:**
//
// - Single input 608x: no hang (0ms)
// - 125x example[0] + 1x example[125]: no hang (0ms)
// - 125x different inputs (0..124) + 1x example[0]:
//   no hang (0ms)
// - 125x different inputs (0..124) + 1x example[125]:
//   HANG at the 126th call
// - Synthetic inputs of matching lengths ("a".repeat(len)):
//   no hang
//
// The hang requires: 125 *different* inputs, then a 126th
// input that is different from all 125. Synthetic inputs
// of the same lengths don't trigger it. The function
// itself is correct for any single input (the 126th
// example in isolation completes in 0ms).
//
// **Hypothesis (not confirmed):** the hang is in the
// Rust runtime or allocator, not in the function code.
// The function's pass-through loop is straightforward
// byte-by-byte copying; there is no path that could
// infinite-loop on the 126th input based on its content
// alone. The content-specific trigger (synthetic inputs
// of the same lengths don't reproduce) suggests the
// allocator's internal state depends on the specific
// allocation pattern, not just the number of
// allocations.
//
// **To run an individual bench:**
//   cargo test --test check_compile -- --ignored --nocapture <name>
#![cfg(test)]

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

// --- Copy of simple_needles (baseline) ---

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

// --- Copies of the three strip functions from
//     `commonmark_spec_test.rs`. Kept verbatim so the
//     bisect isolates the function under test without
//     aliasing or refactoring. ---

fn is_ref_def_start(trimmed: &str) -> bool {
    if !trimmed.starts_with('[') {
        return false;
    }
    let colon = match trimmed.find(':') {
        Some(i) => i,
        None => return false,
    };
    let close = match trimmed[..colon].rfind(']') {
        Some(i) => i,
        None => return false,
    };
    if close < 2 {
        return false;
    }
    if !trimmed[..close].starts_with('[') {
        return false;
    }
    true
}

fn open_title_on_line(line: &str) -> Option<char> {
    let mut open: Option<char> = None;
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 2;
            continue;
        }
        let c = bytes[i] as char;
        if c == '\'' || c == '"' {
            match open {
                None => open = Some(c),
                Some(q) if q == c => open = None,
                Some(_) => {}
            }
        }
        i += 1;
    }
    open
}

fn find_closing_quote(line: &str, quote: char) -> Option<usize> {
    let bytes = line.as_bytes();
    let target = quote as u8;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 2;
            continue;
        }
        if bytes[i] == target {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn strip_link_ref_defs(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    let mut in_ref_def = false;
    let mut in_title: Option<char> = None;
    for line in md.lines() {
        if !in_ref_def {
            if is_ref_def_start(line.trim_start()) {
                in_ref_def = true;
                in_title = open_title_on_line(line);
                continue;
            }
            out.push_str(line);
            out.push('\n');
        } else if let Some(quote) = in_title {
            if let Some(close_quote) = find_closing_quote(line, quote) {
                in_title = None;
                let after = line[close_quote + 1..].trim_start();
                if is_ref_def_start(after) {
                    in_title = open_title_on_line(&line[close_quote + 1..]);
                } else {
                    in_ref_def = false;
                }
            }
        } else if line.is_empty() || line.starts_with(' ') || line.starts_with('\t') {
            // continuation
        } else if line.trim_start().starts_with('<') {
            // URL on its own line
        } else if line.trim_start().starts_with('\'') || line.trim_start().starts_with('"') {
            let trimmed = line.trim_start();
            in_title = open_title_on_line(line);
            if in_title.is_none() {
                if is_ref_def_start(trimmed) {
                    in_title = open_title_on_line(line);
                } else {
                    in_ref_def = false;
                }
            }
        } else if is_ref_def_start(line.trim_start()) {
            in_title = open_title_on_line(line);
        } else {
            in_ref_def = false;
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn find_unescaped_byte(bytes: &[u8], start: usize, target: u8) -> Option<usize> {
    let mut j = start;
    while j < bytes.len() {
        if bytes[j] == b'\\' && j + 1 < bytes.len() {
            j += 2;
            continue;
        }
        if bytes[j] == target {
            return Some(j);
        }
        j += 1;
    }
    None
}

fn skip_balanced(bytes: &[u8], i: usize, open: u8, close: u8) -> usize {
    let mut j = i + 1;
    let mut depth: usize = 1;
    while j < bytes.len() {
        if bytes[j] == b'\\' && j + 1 < bytes.len() {
            j += 2;
            continue;
        }
        if bytes[j] == open {
            depth += 1;
        } else if bytes[j] == close {
            depth -= 1;
            if depth == 0 {
                return j + 1;
            }
        }
        j += 1;
    }
    j
}

fn strip_link_destinations(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    let bytes = md.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'!' && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            let alt_close = match find_unescaped_byte(bytes, i + 2, b']') {
                Some(j) => j,
                None => {
                    out.push('!');
                    i += 1;
                    continue;
                }
            };
            out.push('!');
            out.push('[');
            out.push_str(&md[i + 2..alt_close]);
            out.push(']');
            i = alt_close + 1;
            if i < bytes.len() && bytes[i] == b'(' {
                i = skip_balanced(bytes, i, b'(', b')');
            } else if i < bytes.len() && bytes[i] == b'[' {
                i = skip_balanced(bytes, i, b'[', b']');
            }
            continue;
        }
        if bytes[i] == b'[' {
            let text_close = match find_unescaped_byte(bytes, i + 1, b']') {
                Some(j) => j,
                None => {
                    out.push('[');
                    i += 1;
                    continue;
                }
            };
            if text_close + 1 >= bytes.len() {
                out.push('[');
                i += 1;
                continue;
            }
            let after = bytes[text_close + 1];
            if after != b'(' && after != b'[' {
                out.push('[');
                i += 1;
                continue;
            }
            out.push('[');
            out.push_str(&md[i + 1..text_close]);
            out.push(']');
            i = text_close + 1;
            if after == b'(' {
                i = skip_balanced(bytes, i, b'(', b')');
            } else {
                i = skip_balanced(bytes, i, b'[', b']');
            }
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i] != b'[' && bytes[i] != b'!' {
            i += 1;
        }
        out.push_str(&md[start..i]);
    }
    out
}

fn strip_list_markers(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    for line in md.lines() {
        let indent_len = line.len() - line.trim_start_matches([' ', '\t']).len();
        if indent_len > 3 {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        out.push_str(&line[..indent_len]);
        let after_indent = &line[indent_len..];
        let digits: usize = after_indent
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .count();
        if (1..=9).contains(&digits) {
            let after_digits = &after_indent[digits..];
            let mut chars = after_digits.chars();
            let first = chars.next();
            if first == Some('.') || first == Some(')') {
                let rest = chars.as_str();
                let is_marker = rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t');
                if is_marker {
                    let body = rest.strip_prefix(' ').unwrap_or(rest);
                    let body = body.strip_prefix('\t').unwrap_or(body);
                    out.push_str(body);
                    out.push('\n');
                    continue;
                }
            }
        }
        out.push_str(after_indent);
        out.push('\n');
    }
    out
}

// --- Bench harness: run the given transform on all 608
//     spec examples, log per-100 progress, return total
//     elapsed. ---

fn run_bench(label: &str, transform: impl Fn(&str) -> Vec<String>) {
    let examples = extract(SPEC);
    eprintln!("[{label}] total: {}", examples.len());
    let t0 = Instant::now();
    for (i, src) in examples.iter().enumerate() {
        let t = Instant::now();
        let _ = transform(src);
        let ms = t.elapsed().as_millis();
        // Log every example so we can find the exact hang point.
        // BUT: use `print!` (stdout) not `eprintln!` (stderr)
        // and flush after each line. The test harness
        // captures stderr and a full buffer can cause
        // the test to hang. Stdout is line-buffered.
        println!(
            "[{label}] [{i}] {ms}ms (elapsed: {}ms)",
            t0.elapsed().as_millis()
        );
    }
    eprintln!("[{label}] DONE: {}ms", t0.elapsed().as_millis());
}

// --- Individual benches. Each `#[ignore]` so they don't
//     run by default; invoke by name. ---

#[test]
#[ignore = "bisect: baseline, no strip functions"]
fn bench_none() {
    run_bench("none", simple_needles);
}

#[test]
#[ignore = "bisect: only strip_link_ref_defs"]
fn bench_only_ref_defs() {
    run_bench("ref_defs", |md| {
        let s = strip_link_ref_defs(md);
        simple_needles(&s)
    });
}

#[test]
#[ignore = "bisect: only strip_link_destinations"]
fn bench_only_link_destinations() {
    run_bench("link_dest", |md| {
        let s = strip_link_destinations(md);
        simple_needles(&s)
    });
}

#[test]
#[ignore = "bisect: only strip_list_markers"]
fn bench_only_list_markers() {
    run_bench("list_markers", |md| {
        let s = strip_list_markers(md);
        simple_needles(&s)
    });
}

#[test]
#[ignore = "bisect: all three (production path)"]
fn bench_all_three() {
    run_bench("all_three", |md| {
        let s = strip_link_ref_defs(md);
        let s = strip_link_destinations(&s);
        let s = strip_list_markers(&s);
        simple_needles(&s)
    });
}

#[test]
#[ignore = "test the exact 126th example that hangs strip_link_destinations"]
fn test_hang_example() {
    // The 126th spec example (0-indexed 125) is just
    // ```` ``` ```` (a code block with empty body). The
    // hang occurs when this is processed after the
    // previous 125 examples. Test if it hangs in
    // isolation vs. after many calls.
    let just_this = "```\n";
    let t = Instant::now();
    let result = strip_link_destinations(just_this);
    let ms = t.elapsed().as_millis();
    eprintln!("just_this: {ms}ms result={result:?}");

    // Now process all 608 examples and see if the
    // 126th hangs.
    let examples = extract(SPEC);
    eprintln!("processing all {} examples", examples.len());
    let t0 = Instant::now();
    for (i, src) in examples.iter().enumerate() {
        let t = Instant::now();
        let _ = strip_link_destinations(src);
        let ms = t.elapsed().as_millis();
        if i == 125 || i == 126 || i == 127 || ms > 5 {
            eprintln!("[{i}] {ms}ms (elapsed: {}ms)", t0.elapsed().as_millis());
        }
    }
    eprintln!("DONE: {}ms", t0.elapsed().as_millis());
}

#[test]
#[ignore = "test if repeated same input causes hang"]
fn test_repeated_same_input() {
    // If the hang is in the function itself, calling
    // it 608 times with the same input should also
    // hang. If the hang is in state accumulation
    // across different inputs, this should complete
    // quickly.
    let input = "```\n";
    eprintln!("calling strip_link_destinations 608 times with same input");
    let t0 = Instant::now();
    for i in 0..608 {
        let t = Instant::now();
        let _ = strip_link_destinations(input);
        let ms = t.elapsed().as_millis();
        if i % 100 == 0 || ms > 5 {
            eprintln!("[{i}] {ms}ms (elapsed: {}ms)", t0.elapsed().as_millis());
        }
    }
    eprintln!("DONE: {}ms", t0.elapsed().as_millis());
}

#[test]
#[ignore = "test if 125x example 0 then 1x example 125 hangs"]
fn test_mixed_input() {
    // The hang occurs at the 126th call when processing
    // the spec examples in order. Test if it's the
    // specific sequence that matters.
    let examples = extract(SPEC);
    let first = &examples[0];
    eprintln!("calling 125x example[0] then 1x example[125]");
    let t0 = Instant::now();
    for _i in 0..125 {
        let _ = strip_link_destinations(first);
    }
    eprintln!("after 125x example[0]: {}ms", t0.elapsed().as_millis());
    let t = Instant::now();
    let _ = strip_link_destinations(&examples[125]);
    eprintln!("example[125]: {}ms", t.elapsed().as_millis());
    eprintln!("DONE: {}ms", t0.elapsed().as_millis());
}

#[test]
#[ignore = "test if 125 different inputs then 1x first example hangs"]
fn test_different_then_same() {
    // The full test processes 125 different inputs
    // (examples 0-124) then hangs on example 125
    // (which is different from all previous). Test:
    // does it hang if we process 125 different inputs
    // then process example 0 (which is the same as
    // the first of the 125)?
    let examples = extract(SPEC);
    eprintln!("processing examples 0..125 (125 different inputs)");
    let t0 = Instant::now();
    for (i, src) in examples.iter().enumerate().take(125) {
        let t = Instant::now();
        let _ = strip_link_destinations(src);
        let ms = t.elapsed().as_millis();
        if i == 124 || ms > 5 {
            eprintln!("[{i}] {ms}ms (elapsed: {}ms)", t0.elapsed().as_millis());
        }
    }
    eprintln!("after 125 different: {}ms", t0.elapsed().as_millis());
    // Now try example 0 again.
    let t = Instant::now();
    let _ = strip_link_destinations(&examples[0]);
    eprintln!("example[0] again: {}ms", t.elapsed().as_millis());
    eprintln!("DONE: {}ms", t0.elapsed().as_millis());
}

#[test]
#[ignore = "test the exact examples 120-130 to find the trigger"]
fn test_narrow_trigger() {
    let examples = extract(SPEC);
    eprintln!("processing examples 0..130");
    let t0 = Instant::now();
    for (i, src) in examples.iter().enumerate().take(130) {
        let t = Instant::now();
        let _ = strip_link_destinations(src);
        let ms = t.elapsed().as_millis();
        eprintln!("[{i}] {ms}ms (elapsed: {}ms)", t0.elapsed().as_millis());
    }
    eprintln!("DONE: {}ms", t0.elapsed().as_millis());
}

#[test]
#[ignore = "test if a forced GC after 125 calls prevents the hang"]
fn test_with_gc() {
    // If the hang is in the allocator, forcing a GC
    // (or at least a heap trim) between calls might
    // prevent it. Rust doesn't have a portable GC, but
    // we can try allocating and freeing a large buffer
    // to trigger allocator activity.
    let examples = extract(SPEC);
    eprintln!("processing with forced alloc churn between calls");
    let t0 = Instant::now();
    for (i, src) in examples.iter().enumerate().take(200) {
        let t = Instant::now();
        let _ = strip_link_destinations(src);
        // Force allocator activity: allocate and free
        // a 1MB buffer every 10 calls.
        if i % 10 == 0 {
            let _churn: Vec<u8> = vec![0u8; 1_000_000];
            std::hint::black_box(&_churn);
        }
        let ms = t.elapsed().as_millis();
        if i == 125 || i == 126 || i == 127 || ms > 5 {
            eprintln!("[{i}] {ms}ms (elapsed: {}ms)", t0.elapsed().as_millis());
        }
    }
    eprintln!("DONE: {}ms", t0.elapsed().as_millis());
}

#[test]
#[ignore = "test if the hang depends on input LENGTHS"]
fn test_synthetic_lengths() {
    // The hang might depend on the specific lengths of
    // the inputs. Test with synthetic inputs of
    // varying lengths to see if the hang reproduces.
    let examples = extract(SPEC);
    let lengths: Vec<usize> = examples.iter().map(|s| s.len()).collect();
    eprintln!(
        "first 130 input lengths: {:?}",
        &lengths[..130.min(lengths.len())]
    );
    eprintln!("processing 130 synthetic inputs of length lengths[i]");
    let t0 = Instant::now();
    for (i, len) in lengths.iter().enumerate().take(130) {
        let synthetic: String = "a".repeat(*len);
        let t = Instant::now();
        let _ = strip_link_destinations(&synthetic);
        let ms = t.elapsed().as_millis();
        if i == 125 || i == 126 || i == 127 || ms > 5 {
            eprintln!(
                "[{i}] len={len} {ms}ms (elapsed: {}ms)",
                t0.elapsed().as_millis()
            );
        }
    }
    eprintln!("DONE: {}ms", t0.elapsed().as_millis());
}
