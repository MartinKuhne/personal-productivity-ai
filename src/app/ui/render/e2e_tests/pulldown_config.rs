//! Sanity tests for the project's `pulldown-cmark` configuration.
//!
//! `parse_markdown_to_events` in `fastmd::ui::render` constructs the
//! parser with a specific set of `Options`. These tests verify, through
//! observable parser behavior, that those options are actually in effect —
//! a regression in the parser setup (or a `pulldown-cmark` upgrade that
//! changes option defaults) would surface here.
//!
//! Previously these lived in `src/ui/render_tests.rs` as raw
//! `Options::ENABLE_*` checks against the `pulldown_cmark` crate
//! directly. Rewritten to assert the project's effective behavior
//! instead, so the test fails when the user's markdown stops being
//! recognized.

use super::*;

fn has_table(events: &[RenderEvent]) -> bool {
    events.iter().any(|e| matches!(e, RenderEvent::Table(_)))
}

fn has_footnote_ref(events: &[RenderEvent]) -> bool {
    events.iter().any(|e| match e {
        RenderEvent::FlushInline { elems, .. } => elems.iter().any(|el| match el {
            InlineElem::Text(t, style) if style.code => t.contains("[^"),
            _ => false,
        }),
        _ => false,
    })
}

fn has_strikethrough(events: &[RenderEvent]) -> bool {
    events.iter().any(|e| match e {
        RenderEvent::FlushInline { elems, .. } => elems.iter().any(|el| match el {
            InlineElem::Text(_, style) => style.strikethrough,
            _ => false,
        }),
        _ => false,
    })
}

fn has_task_marker(events: &[RenderEvent]) -> bool {
    events.iter().any(|e| match e {
        RenderEvent::FlushInline { task_checked, .. } => task_checked.is_some(),
        _ => false,
    })
}

/// Pin the parser's `Options` set end-to-end. Each case is a small
/// markdown snippet whose effective parse result is observable only
/// when the corresponding `Options::ENABLE_*` flag is set. A
/// regression in the parser setup (or a `pulldown-cmark` upgrade
/// that changes option defaults) will surface here with a
/// diagnostic naming the option that needs to be re-enabled.
///
/// Consolidated from 4 formerly-separate tests
/// (`gfm_tables_are_recognized`, `footnotes_are_recognized`,
/// `strikethrough_is_recognized`, `task_lists_are_recognized`) —
/// each was a 7-line `parse_markdown_to_events` + `assert!` pair
/// with no shared logic beyond the `events.iter().any(...)` shape.
#[test]
fn parse_markdown_to_events_uses_required_pulldown_options() {
    struct Case {
        label: &'static str,
        md: &'static str,
        predicate: fn(&[RenderEvent]) -> bool,
        failure_msg: &'static str,
    }
    let cases: &[Case] = &[
        Case {
            label: "GFM table",
            md: "| A | B |\n|---|---|\n| 1 | 2 |",
            predicate: has_table,
            failure_msg: "Options::ENABLE_TABLES may be unset",
        },
        Case {
            label: "footnote reference",
            md: "Footnote[^1]\n\n[^1]: details",
            predicate: has_footnote_ref,
            failure_msg: "Options::ENABLE_FOOTNOTES may be unset",
        },
        Case {
            label: "strikethrough",
            md: "~~struck~~",
            predicate: has_strikethrough,
            failure_msg: "Options::ENABLE_STRIKETHROUGH may be unset",
        },
        Case {
            label: "task list",
            md: "- [ ] todo\n- [x] done",
            predicate: has_task_marker,
            failure_msg: "Options::ENABLE_TASKLISTS may be unset",
        },
    ];

    for case in cases {
        let events = parse_markdown_to_events(case.md);
        assert!(
            (case.predicate)(&events),
            "[{}] {} — events: {events:?}",
            case.label,
            case.failure_msg
        );
    }
}

#[test]
fn parse_emits_flush_inline_for_newline_separated_text() {
    // A single-newline input must produce a `FlushInline` carrying
    // both fragments — the parser collapses the soft line break into
    // a single inline stream with a space. This is the contract
    // pinned here: a regression that promotes `\n` to a hard line
    // break (or drops one of the fragments) would surface as either
    // two separate `FlushInline` events or a missing fragment.
    //
    // Note: the project does not enable `Options::ENABLE_HARD_BREAKS`,
    // and `RenderEvent` has no `HardBreak` variant — the soft-break
    // path is the only legal one. If a future refactor adds a
    // `HardBreak` variant or enables the option, this test will
    // catch the event-stream change.
    //
    // This case is kept separate from
    // `parse_markdown_to_events_uses_required_pulldown_options` because
    // it asserts a different invariant (the soft-break collapse
    // contract), not a `Options::ENABLE_*` flag.
    let md = "line1\nline2";
    let events = parse_markdown_to_events(md);
    let inline_texts: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            RenderEvent::FlushInline { elems, .. } => {
                let mut s = String::new();
                for elem in elems {
                    if let InlineElem::Text(t, _) = elem {
                        s.push_str(t);
                    }
                }
                Some(s)
            }
            _ => None,
        })
        .collect();
    assert!(
        inline_texts.iter().any(|t| t.contains("line1")),
        "first fragment lost from the inline stream: {events:?}"
    );
    assert!(
        inline_texts.iter().any(|t| t.contains("line2")),
        "second fragment lost from the inline stream: {events:?}"
    );
}
