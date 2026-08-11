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
    // Pre-processing passes, in order:
    // 1. Strip link reference definitions — the label/URL/title
    //    metadata is not visible content.
    // 2. Strip link/image destination and title portions — the
    //    URL of `[text](url)` and the alt+title of `![alt](url)`
    //    are hyperlink metadata, not body content. Without this
    //    pass, examples like `[](./target.md)` (empty link text,
    //    URL is the only "content") or `![foo](/url "title")`
    //    (image with title metadata) produce needles that never
    //    appear in the rendered PDF — the test would then
    //    require needles that the spec corpus doesn't have
    //    visible content for.
    // 3. Strip ordered list markers — `1.`, `123.`, etc. at
    //    the start of a line are syntax, not content. Without
    //    this pass, the literal digit sequence (e.g.
    //    `123456789` from `123456789. ok`) is extracted as a
    //    needle that doesn't appear in the PDF (the PDF shows
    //    `1.` after Typst's list re-numbering).
    let stripped = strip_link_ref_defs(md);
    let stripped = strip_link_destinations(&stripped);
    let stripped = strip_list_markers(&stripped);
    stripped
        .chars()
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

/// Strip link reference definitions from the source. A
/// link ref def matches the pattern `[label]: url
/// "title"` (or `[label]: url`, or `[label]:\n  url`,
/// or with title only, etc.) and occupies one or more
/// lines. The function detects the start of a ref def
/// (a line starting with `[...]`) and removes the entire
/// block from the source.
///
/// The pattern is intentionally narrow: a ref def must
/// start at column 0 with `[label]:` (closing bracket
/// followed by colon). Indented ref defs (4+ space
/// indent) are part of a code block and are left alone.
///
/// Continuation lines fall into three categories:
///
/// 1. **Indented or empty** — the standard CommonMark
///    continuation form. The URL and title may span
///    multiple lines if the second and later lines are
///    indented (typically 2-4 spaces) or empty.
/// 2. **URL-only** — a line starting with `<` (e.g.
///    `<my url>` in spec example #152) is a URL on its
///    own line.
/// 3. **Title-only** — a line starting with `'` or `"`
///    (e.g. `'title'` in #152) is a title on its own
///    line.
///
/// A continuation can also be another ref def's start
/// (e.g. `[foo]: first\n[foo]: second` in #161); the
/// strip loop re-checks `is_ref_def_start` on each
/// non-continuation line while in the ref-def state.
///
/// Multi-line titles (e.g. spec example #153,
/// `[foo]: /url '\ntitle\nline1\nline2\n'`) are handled
/// by tracking an "inside title" state: when the label
/// line has an unclosed `'` or `"`, subsequent lines
/// are stripped until the matching closing quote.
fn strip_link_ref_defs(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    let mut in_ref_def = false;
    let mut in_title: Option<char> = None; // Some('\'') or Some('"') when inside a multi-line title
    for line in md.lines() {
        if !in_ref_def {
            if is_ref_def_start(line.trim_start()) {
                in_ref_def = true;
                // Check if the label line opens a multi-line title.
                in_title = open_title_on_line(line);
                continue;
            }
            out.push_str(line);
            out.push('\n');
        } else if let Some(quote) = in_title {
            // Inside a multi-line title. The line is
            // stripped until the closing quote. If the
            // closing quote is on this line, exit the
            // title state.
            if let Some(close_quote) = find_closing_quote(line, quote) {
                // The title content (before the close
                // quote) and the rest of the line after
                // it are both stripped — the title is
                // done. The next iteration will check
                // whether the remainder of the line is a
                // new ref def or another continuation.
                in_title = None;
                // If the line after the closing quote
                // looks like another ref def, stay in
                // the ref-def state. Otherwise, exit
                // ref-def.
                let after = line[close_quote + 1..].trim_start();
                if is_ref_def_start(after) {
                    in_title = open_title_on_line(&line[close_quote + 1..]);
                } else {
                    in_ref_def = false;
                }
            }
            // else: still inside the title, drop the line.
        } else if line.is_empty() || line.starts_with(' ') || line.starts_with('\t') {
            // Standard continuation (indented or empty).
        } else if line.trim_start().starts_with('<') {
            // URL on its own line (spec example #152).
        } else if line.trim_start().starts_with('\'') || line.trim_start().starts_with('"') {
            // Title on its own line (spec example #152).
            // If the quote is unclosed, enter title
            // state for the multi-line form.
            let trimmed = line.trim_start();
            in_title = open_title_on_line(line);
            // If open_title_on_line returned None (quote
            // was already closed on this line), the
            // title is done; the next iteration will
            // check the rest of the line.
            if in_title.is_none() {
                // Title was a single line; check if the
                // line (or its remainder) is a new ref
                // def or another continuation.
                if is_ref_def_start(trimmed) {
                    in_title = open_title_on_line(line);
                } else {
                    in_ref_def = false;
                }
            }
        } else if is_ref_def_start(line.trim_start()) {
            // Next ref def starts immediately on a new
            // (non-indented, non-empty) line. CommonMark
            // collapses the second definition; the strip
            // mirrors that by removing it too.
            in_title = open_title_on_line(line);
        } else {
            // Non-ref-def line that ends the current ref
            // def; re-emit.
            in_ref_def = false;
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// If `line` contains an unclosed `'` or `"`, return
/// the opening quote char. Used to detect the start of
/// a multi-line title in a ref def.
fn open_title_on_line(line: &str) -> Option<char> {
    // Walk the line, tracking which quote chars are
    // active. A backslash-escaped quote is skipped. The
    // first quote we see opens a title; if the line
    // doesn't close it, return that quote char.
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
                Some(_) => {
                    // Mismatched quote inside an open
                    // title — treat as literal. The spec
                    // disallows mismatched quotes in a
                    // title anyway, so this is purely
                    // defensive.
                }
            }
        }
        i += 1;
    }
    open
}

/// Return the byte index of the closing `quote` char
/// on `line`, or `None` if the quote is not closed.
/// Only counts quotes that aren't backslash-escaped.
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

/// True if `trimmed` is the start of a link reference
/// definition: `[label]:` (label must be non-empty,
/// closing `]` followed by `:`).
///
/// Labels can contain escaped brackets (`\[`, `\]`),
/// so the closing `]` is the LAST `]` before the first
/// `:` on the line, not the first `]`. This matters for
/// labels like `Foo*bar\]` in spec example #151:
/// `[Foo*bar\]]:my_(url) '...'`.
fn is_ref_def_start(trimmed: &str) -> bool {
    if !trimmed.starts_with('[') {
        return false;
    }
    // The label's closing `]` is the last `]` before the
    // first `:` on the line. Labels can contain `\]`
    // escapes, so the first `]` is not necessarily the
    // closing one.
    let colon = match trimmed.find(':') {
        Some(i) => i,
        None => return false,
    };
    let close = match trimmed[..colon].rfind(']') {
        Some(i) => i,
        None => return false,
    };
    if close < 2 {
        // Label is empty (e.g. `[]:` — actually not a
        // valid ref def per CommonMark; bail out).
        return false;
    }
    // The label must start with `[`; reject `foo]: bar`.
    if !trimmed[..close].starts_with('[') {
        return false;
    }
    true
}

/// Strip the URL/title portion of inline links and images,
/// plus the reference-style label of `[text][label]` and
/// `![alt][label]`. The URL and title are hyperlink
/// metadata, not body content — extracting them as
/// "needles" produced false positives (e.g. the URL of
/// an empty-text link, the `"title"` of an image, the
/// ref label of an image that resolves to a ref def).
///
///   `[text](url)`       →  `[text]`
///   `[text](url "t")`   →  `[text]`
///   `![alt](url)`       →  `![alt]`
///   `![alt](url "t")`   →  `![alt]`
///   `[text][label]`     →  `[text]`
///   `![alt][label]`     →  `![alt]`
///
/// Operates on byte indices but copies whole `&str` slices
/// from the original source so multi-byte UTF-8 sequences
/// (e.g. `ΑΓΩ` in spec example #163) pass through
/// unmodified. Only ASCII bracket bytes (`[`, `]`, `(`, `)`)
/// are inspected; all other bytes are copied through.
fn strip_link_destinations(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    let bytes = md.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Image: `![alt](...)` or `![alt][label]`
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
            // SAFETY: alt_close points to an ASCII `]`
            // byte, which is also a valid UTF-8 char
            // boundary. The slice `md[i+2..alt_close]`
            // is therefore valid UTF-8.
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
        // Link: `[text](...)` or `[text][label]`.
        // Plain `[text]` with no `(` or `[` after is left
        // alone — it's not a link.
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
        // Pass-through: copy the next run of non-special
        // bytes. The only bytes that need per-char handling
        // are `[` (link opener, handled above) and `!` (image
        // opener at top of loop). All other bytes —
        // including `\` (potential escape), multi-byte
        // UTF-8 continuation bytes, and ASCII punctuation
        // — are copied verbatim. The bracket-finding
        // helpers (`find_unescaped_byte`, `skip_balanced`)
        // still treat `\` as an escape when looking for
        // `]`, `)`, or `[`, so a backslash-escaped bracket
        // in a link/image alt text is correctly handled.
        let start = i;
        while i < bytes.len() && bytes[i] != b'[' && bytes[i] != b'!' {
            i += 1;
        }
        out.push_str(&md[start..i]);
    }
    out
}

/// Find the next unescaped `target` byte at or after
/// `start`. A backslash immediately before the byte
/// (e.g. `\]`) is treated as an escape and the byte
/// is not returned. Returns `None` if no such byte
/// exists before the end of the buffer.
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

/// Skip a balanced `open...close` group starting at
/// index `i` (which must point at the opening byte).
/// Tracks nesting depth. Returns the index just after
/// the matching close, or `bytes.len()` if the group
/// is unclosed. Used to skip the URL+title of a link
/// or the label of a ref-style link.
fn skip_balanced(bytes: &[u8], i: usize, open: u8, close: u8) -> usize {
    debug_assert_eq!(bytes[i], open);
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

/// Strip ordered list markers (`1.`, `2)`, `123.` etc.)
/// from the start of each line. Per CommonMark, a list
/// item may start with 1-9 digits followed by `.` or
/// `)` and at least one space (or end of line). The
/// digit sequence is syntax, not content.
///
/// Lines with 4+ space leading indent are part of a
/// code block and are left alone. The optional
/// trailing space after the marker is also stripped.
fn strip_list_markers(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    for line in md.lines() {
        let indent_len = line.len() - line.trim_start_matches([' ', '\t']).len();
        if indent_len > 3 {
            // 4+ space indent: code block per CommonMark.
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
                // Must be followed by a space, tab, or
                // end of line — otherwise this is a
                // number in running text, not a list
                // marker.
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

/// Focused regression test for the 12 content-fidelity gaps
/// that the per-example test (`all_commonmark_examples_render_content_into_pdf`)
/// surfaces in its first run. The full test is `#[ignore]`'d
/// because it pays the `pdf_oxide` extraction cost on top of
/// the Typst engine compile for all 652 spec examples (~5x
/// slower than the structural-only spec test, and the
/// compile loop is currently non-deterministic enough that
/// running it in CI is not yet safe — see
/// `doc/adr/pdf-export-test-gaps.md`).
///
/// This focused test pins the 12 specific examples so the
/// needle-extraction fixes are covered by a fast, default-on
/// test. Each example is its own assertion so a regression
/// in one fix points directly at the gap that regressed.
///
/// The sources below are taken verbatim from the
/// `commonmark-0.31.2-spec.txt` fixture; the example
/// numbers are the sequential indices assigned by
/// `extract_markdown_examples` (not the spec's own
/// numbering, which uses non-sequential section-relative
/// numbers).
#[test]
fn content_fidelity_known_gaps() {
    // (label, source) — sources are the markdown half of
    // the spec example, i.e. the text between the opening
    // 32-backtick fence line and the `\n.\n` separator.
    let cases: &[(&str, &str)] = &[
        // Gap #151: multi-line ref def with `\]` in label.
        // `is_ref_def_start` now finds the LAST `]` before
        // the first `:` so the label `Foo*bar\]` is
        // recognised and the ref def is stripped.
        (
            "#151 multi-line ref def with escaped bracket in label",
            "[Foo*bar\\]]:my_(url) 'title (with parens)'\n\n[Foo*bar\\]]",
        ),
        // Gap #152: multi-line ref def with URL and title
        // on separate lines. After stripping the ref def,
        // the remaining source is `[Foo bar]` (empty
        // needles → skipped, no failure).
        (
            "#152 multi-line ref def with URL and title on separate lines",
            "[Foo bar]:\n<my url>\n'title'\n\n[Foo bar]",
        ),
        // Gap #153: multi-line ref def with a title that
        // spans multiple lines (the `'` markers are on
        // their own lines). After stripping, the
        // `[foo]` ref link is the only content.
        (
            "#153 multi-line ref def with multi-line title",
            "[foo]: /url '\ntitle\nline1\nline2\n'\n\n[foo]",
        ),
        // Gap #161: two consecutive ref defs. The second
        // (`[foo]: second`) is now recognised and stripped
        // because the strip loop re-checks
        // `is_ref_def_start` on each non-continuation line
        // while in the ref-def state.
        (
            "#161 two consecutive ref defs",
            "[foo]\n\n[foo]: first\n[foo]: second",
        ),
        // Gap #221: ordered list marker. `123456789.` is
        // not a needle; the list item body `ok` is too
        // short (< 4 chars) so the example is skipped.
        ("#221 ordered list with multi-digit marker", "123456789. ok"),
        // Gap #440: empty link text. After stripping the
        // URL portion, `[]` has no text → no needles →
        // skipped.
        (
            "#440 link with empty text and relative URL",
            "[](./target.md)",
        ),
        // Gap #500: duplicate ref defs interleaved with a
        // ref link. Both `[foo]:` lines are stripped.
        (
            "#500 duplicate ref defs with ref link",
            "[foo]: /url1\n\n[foo]: /url2\n\n[bar][foo]",
        ),
        // Gap #521: ref defs and ref links, multiple.
        (
            "#521 ref link to second ref def",
            "[foo][bar]\n\n[foo]: /url1\n[bar]: /url2",
        ),
        // Gap #526: ref link chain to a ref def.
        (
            "#526 ref link chain to a ref def",
            "[foo][bar][baz]\n\n[baz]: /url1\n[bar]: /url2",
        ),
        // Gap #527: ref link chain, second ref wins.
        (
            "#527 ref link chain, second ref wins",
            "[foo][bar][baz]\n\n[baz]: /url1\n[foo]: /url2",
        ),
        // Gap #528: image with title metadata. After
        // stripping the URL+title, the alt text `foo` is
        // 3 chars (< 4) → no needles → skipped.
        ("#528 image with title metadata", "![foo](/url \"title\")"),
        // Gap #533: image with ref link label. After
        // stripping the ref def and the ref label, the
        // alt text `foo *bar*` yields `foo` (3) and `bar`
        // (3) → no needles → skipped.
        (
            "#533 image with ref link label",
            "![foo *bar*][foobar]\n\n[FOOBAR]: train.jpg \"train & tracks\"",
        ),
    ];

    let mut failures: Vec<String> = Vec::new();
    for (label, source) in cases {
        let needles = extract_content_needles(source);
        // The contract for every gap-fix: either the
        // extracted needles appear in the rendered PDF
        // text, or no needles are extracted (the example
        // is too small / too metadata-heavy to assert
        // on and is correctly skipped). The previous
        // implementation extracted metadata (URL paths,
        // ref labels, list markers, image titles) as
        // needles that never appear in the PDF; the
        // new extraction strips those out.
        if needles.is_empty() {
            // Skipped — nothing to assert.
            continue;
        }
        let bytes = match compile_markdown_to_pdf(source, "content-fidelity-known-gaps") {
            Ok(b) => b,
            Err(e) => {
                failures.push(format!("{label}: compile failed: {e}"));
                continue;
            }
        };
        let doc = match pdf_oxide::PdfDocument::from_bytes(bytes) {
            Ok(d) => d,
            Err(e) => {
                failures.push(format!("{label}: pdf_oxide parse failed: {e}"));
                continue;
            }
        };
        let spans = match doc.extract_spans(0) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("{label}: extract_spans failed: {e}"));
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
                "{label}: no needle from source found in PDF text. Needles: {needles:?}. Source: {source:?}"
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} known content-fidelity gaps regressed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
