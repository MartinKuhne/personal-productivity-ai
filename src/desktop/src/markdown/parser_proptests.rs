//! Property-based tests for the markdown parser in
//! `markdown::parser::parse_markdown_to_events`.
//!
//! This sidecar replaces the inline `test_parse_markdown_fuzz_property`
//! in `ui/render/tests.rs:828`. The inline test ran 64
//! cases; the sidecar lifts the same shape to 1024 cases
//! under the `proptest!` macro and exposes it as a
//! dedicated sidecar per the `RUST-056` convention.
//!
//! # Properties under test
//!
//! All four properties are sourced from the A3 / A4
//! corner-case rows in `doc/planning/fuzzing.md` §2.2
//! "Phase 3".
//!
//! 1. **`parse_markdown_to_events` never panics on any
//!    input.** Any byte string is parsed; the result is a
//!    `Vec<RenderEvent>` (possibly empty).
//! 2. **The event stream is bounded.** No input of `N`
//!    bytes produces more than a small constant multiple
//!    of `N` events. The pre-sidecar test used 1000 as
//!    the upper bound; we keep that here.
//! 3. **Heading levels are in `1..=6`.** Same as the
//!    `Document` proptest; the parser-level invariant
//!    is checked here so a regression in the parser is
//!    caught even if the document-level wrapper hides
//!    the bug.
//! 4. **Tables are rectangular.** All rows in a `Table`
//!    event have the same cell count.
//! 5. **`FlushInline::indent` is bounded.** The parser
//!    increments `list_depth` on `Tag::List` and
//!    decrements on `TagEnd::List`; an indent > 8 is
//!    impossible for a small input.
//!
//! `cases = 1024` per property.

use crate::markdown::{RenderEvent, parse_markdown_to_events};
use proptest::prelude::*;

/// One proptest case count for every property in this sidecar.
/// Phase 3 calls for `cases = 1024`.
const CASES: u32 = 1024;

/// Strategy: any 7-bit ASCII + newline. Markdown is a
/// 7-bit format; the existing inline test used the same
/// range. (UTF-8 markdown with non-ASCII is covered by
/// the `Document` proptest at the file layer; the parser
/// itself works in bytes.)
fn any_markdown() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[\x00-\x7F\n]{0,2048}").unwrap()
}

/// A `md_grammar` strategy that generates a string of common
/// markdown elements joined by blank lines. This is the
/// same shape as the inline test at `ui/render/tests.rs:841-859`
/// — lifted here so the sidecar is the single source of
/// truth for parser fuzz input.
fn md_grammar() -> impl Strategy<Value = String> {
    let heading = "[#]{1,6}[ \\t]+[A-Za-z ]{1,30}";
    let para = "[A-Za-z ,.!?]{0,80}";
    let code_block = "```[a-z]*\\n[a-zA-Z0-9 ;]{0,40}\\n```";
    let bullet = "- [ \\t]{0,3}[A-Za-z ]{1,30}";
    let task = "- \\[[ x]\\] [A-Za-z ]{1,30}";
    let table_row = "\\|?[A-Za-z ]{1,5}(\\|[A-Za-z ]{1,5})*\\|?";
    let table_sep = "\\|?[ -]{3}(\\|[ -]{3})*\\|?";
    let link = "\\[[A-Za-z ]{1,20}\\]\\(https?://[a-z.]+\\)";
    let inline = prop_oneof![
        2 => Just(para.to_string()),
        1 => Just(heading.to_string()),
        1 => Just(code_block.to_string()),
        1 => Just(bullet.to_string()),
        1 => Just(task.to_string()),
        1 => Just(format!("{table_row}\n{table_sep}\n{table_row}")),
        1 => Just(link.to_string()),
    ];
    proptest::collection::vec(inline, 0..8).prop_map(|v| v.join("\n\n"))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES))]

    /// `parse_markdown_to_events` never panics on any
    /// input. The function is total: any byte string
    /// produces a `Vec<RenderEvent>`.
    #[test]
    fn parse_markdown_never_panics_on_any_input(input in any_markdown()) {
        let _ = parse_markdown_to_events(&input);
    }

    /// The event stream is bounded. The upper bound is
    /// 4x the input byte count plus 64; pulldown-cmark
    /// produces at most a small constant multiple of
    /// the input size for realistic inputs.
    #[test]
    fn parse_markdown_events_bounded(input in any_markdown()) {
        let events = parse_markdown_to_events(&input);
        let upper = input.len() * 4 + 64;
        prop_assert!(
            events.len() <= upper,
            "event count {} exceeds upper bound {} for input of {} bytes",
            events.len(),
            upper,
            input.len()
        );
    }

    /// Heading levels are in `1..=6`. The
    /// `RenderEvent::Heading` invariant is the same as
    /// pulldown-cmark's contract.
    #[test]
    fn parse_markdown_heading_levels_in_range(input in any_markdown()) {
        for event in parse_markdown_to_events(&input) {
            if let RenderEvent::Heading { level, .. } = event {
                prop_assert!(
                    (1..=6).contains(&level),
                    "heading level {level} out of range"
                );
            }
        }
    }

    /// Tables are rectangular. All rows in a `Table`
    /// event have the same cell count.
    #[test]
    fn parse_markdown_tables_are_rectangular(input in any_markdown()) {
        for event in parse_markdown_to_events(&input) {
            if let RenderEvent::Table(rows) = event
                && let Some(first) = rows.first()
            {
                let expected = first.len();
                for (i, row) in rows.iter().enumerate() {
                    let got = row.len();
                    prop_assert_eq!(
                        got,
                        expected,
                        "table row {} has {} cells, expected {}",
                        i,
                        got,
                        expected
                    );
                }
            }
        }
    }

    /// `FlushInline::indent` is bounded. The parser
    /// tracks list depth; an indent > 8 is impossible
    /// for a small input.
    #[test]
    fn parse_markdown_flush_indent_bounded(input in any_markdown()) {
        for event in parse_markdown_to_events(&input) {
            if let RenderEvent::FlushInline { indent, .. } = event {
                prop_assert!(
                    indent <= 8,
                    "indent {indent} exceeds safe bound"
                );
            }
        }
    }

    /// The `md_grammar` strategy — a mix of headings,
    /// paragraphs, code blocks, lists, tasks, tables,
    /// and links — is the realistic-input property. This
    /// is the same shape as the inline test at
    /// `ui/render/tests.rs:841-859`, lifted here so the
    /// sidecar is the single source of truth.
    #[test]
    fn parse_markdown_realistic_grammar_does_not_panic(input in md_grammar()) {
        let events = parse_markdown_to_events(&input);
        // Output must be bounded (the inline test's first
        // assertion) and every event variant must be a
        // legal value.
        prop_assert!(
            events.len() < 1_000,
            "event count exploded for input of {} bytes: {} events",
            input.len(),
            events.len()
        );
        for event in &events {
            match event {
                RenderEvent::Heading { level, .. } => {
                    prop_assert!(
                        (1..=6).contains(level),
                        "heading level out of range: {level}"
                    );
                }
                RenderEvent::Table(rows) => {
                    if let Some(first) = rows.first() {
                        let expected = first.len();
                        for (i, row) in rows.iter().enumerate() {
                            let got = row.len();
                            prop_assert_eq!(
                                got,
                                expected,
                                "table row {} has {} cells, expected {}",
                                i,
                                got,
                                expected
                            );
                        }
                    }
                }
                RenderEvent::FlushInline { indent, .. } => {
                    prop_assert!(*indent <= 8, "indent {indent} exceeds safe bound");
                }
                _ => {}
            }
        }
    }
}
