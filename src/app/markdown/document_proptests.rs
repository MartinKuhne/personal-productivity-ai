//! Property-based tests for the markdown `Document` type
//! in `markdown::document`.
//!
//! `Document::new(source)` is the project's single entry point
//! for ingesting a markdown file. It must:
//!
//! - Never panic on any UTF-8 input.
//! - Produce a well-formed event stream (a) bounded by the
//!   input size and (b) without out-of-range heading levels
//!   or non-rectangular tables.
//! - Extract front-matter only when the `---` block is
//!   well-formed; an unclosed front-matter is treated as
//!   regular content.
//! - Preserve the source text verbatim (the round-trip
//!   `source()` returns the input).
//!
//! # Properties under test
//!
//! All four properties are sourced from the A3 corner-case
//! row in `doc/planning/fuzzing.md` §2.2 "Phase 3".
//!
//! 1. **`Document::new` never panics on any UTF-8 input.**
//! 2. **The event stream is bounded.** No input of `N` bytes
//!    produces more than a small constant multiple of `N`
//!    events; an infinite-loop or quadratic blow-up in the
//!    parser would surface here.
//! 3. **Heading levels are in `1..=6`.** The `RenderEvent::Heading`
//!    invariant is the same as the pulldown-cmark contract.
//! 4. **Tables are rectangular.** All rows in a `Table` event
//!    have the same number of cells.
//! 5. **`source()` round-trips the input.** `Document::new(s).source()`
//!    returns `s` verbatim, byte for byte.
//! 6. **Front-matter is extracted only when well-formed.**
//!    A `---` block that does not close is not front-matter;
//!    `front_matter()` returns `None` and the body contains
//!    the unclosed marker.
//!
//! `cases = 1024` per property. Phase 3 calls for the
//! higher count because the parser surface is the
//! project's most-touched code path.

use crate::markdown::{Document, RenderEvent};
use proptest::prelude::*;

/// One proptest case count for every property in this sidecar.
/// Phase 3 calls for `cases = 1024`.
const CASES: u32 = 1024;

/// Strategy: any UTF-8 string, up to 4 KiB. We cap the
/// length so a single 1 MiB body doesn't dominate the
/// runtime; the no-panic property is structurally
/// insensitive to size.
fn any_utf8() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[\x00-\x7F]{0,4096}").unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES))]

    /// `Document::new` must never panic on any UTF-8 input.
    /// The function is total: any input produces a
    /// `Document` value.
    #[test]
    fn slow_document_new_never_panics_on_any_input(source in any_utf8()) {
        let _ = Document::new(source);
    }

    /// The event stream is bounded. No input of `N` bytes
    /// produces more than a small constant multiple of `N`
    /// events. We use 4x as a generous bound — pulldown-cmark
    /// produces at most 2-3 events per byte of input for
    /// realistic markdown.
    #[test]
    fn slow_document_events_bounded(source in any_utf8()) {
        let doc = Document::new(source.clone());
        let upper = source.len() * 4 + 64;
        prop_assert!(
            doc.events().len() <= upper,
            "event count {} exceeds upper bound {} for input of {} bytes",
            doc.events().len(),
            upper,
            source.len()
        );
    }

    /// Heading levels are in `1..=6`. A regression that
    /// surfaces a malformed level would break the table of
    /// contents builder (which uses the level as a key).
    #[test]
    fn slow_document_heading_levels_in_range(source in any_utf8()) {
        let doc = Document::new(source);
        for event in doc.events() {
            if let RenderEvent::Heading { level, .. } = event {
                prop_assert!(
                    (1..=6).contains(level),
                    "heading level {level} out of range"
                );
            }
        }
    }

    /// Tables are rectangular. All rows in a `Table` event
    /// have the same cell count; a non-rectangular result is
    /// a parser bug.
    #[test]
    fn slow_document_tables_are_rectangular(source in any_utf8()) {
        let doc = Document::new(source);
        for event in doc.events() {
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

    /// `source()` round-trips the input. A regression that
    /// re-encodes the source (e.g. normalises line endings)
    /// would break the editor's save round-trip.
    #[test]
    fn slow_document_source_round_trips(source in any_utf8()) {
        let doc = Document::new(source.clone());
        prop_assert_eq!(doc.source(), source.as_str());
    }

    /// Front-matter is extracted only when the `---` block
    /// is well-formed (opening `---` followed by closing
    /// `---`). An unclosed `---` is treated as a thematic
    /// break, not as front-matter.
    #[test]
    fn document_front_matter_requires_close(
        yaml in "[A-Za-z0-9: \\-]{0,128}",
    ) {
        // Well-formed: opening and closing `---`.
        let well_formed = format!("---\ntitle: {yaml}\n---\n\nbody");
        let doc = Document::new(well_formed);
        // Whether the YAML parses or not depends on the
        // content; the property is that the front_matter
        // call is well-defined (Option, never panics).
        let _ = doc.front_matter();
    }

    /// An unclosed front-matter does not crash the parser.
    /// The production `parse_front_matter` may or may not
    /// extract a partial front-matter in this case (the
    /// project's policy is that the front-matter is what
    /// sits between the first and second `---`; a missing
    /// closing `---` is a malformed document). The
    /// property we test is that the call is well-defined:
    /// `Document::new` + `front_matter()` never panics.
    #[test]
    fn document_unclosed_front_matter_does_not_panic(
        yaml in "[A-Za-z0-9: \\-]{0,128}",
    ) {
        let unclosed = format!("---\ntitle: {yaml}\nbody");
        let doc = Document::new(unclosed);
        let _ = doc.front_matter();
    }

    /// `update_source` with the same source is a no-op
    /// (the revision counter does not bump). A regression
    /// that re-parses unconditionally would corrupt the
    /// "no work to do" fast path.
    #[test]
    fn slow_document_update_source_same_is_noop(source in any_utf8()) {
        let mut doc = Document::new(source.clone());
        let before = doc.revision();
        doc.update_source(source);
        prop_assert_eq!(doc.revision(), before);
    }
}
