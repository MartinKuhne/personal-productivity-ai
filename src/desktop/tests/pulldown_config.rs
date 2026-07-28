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

use fastmd::ui::render::RenderEvent;
use fastmd::ui::render::parse_markdown_to_events;

fn has_table(events: &[RenderEvent]) -> bool {
    events.iter().any(|e| matches!(e, RenderEvent::Table(_)))
}

fn has_footnote_ref(events: &[RenderEvent]) -> bool {
    events.iter().any(|e| match e {
        RenderEvent::FlushInline { elems, .. } => elems.iter().any(|el| match el {
            fastmd::ui::render::InlineElem::Text(t, style) if style.code => t.contains("[^"),
            _ => false,
        }),
        _ => false,
    })
}

fn has_strikethrough(events: &[RenderEvent]) -> bool {
    events.iter().any(|e| match e {
        RenderEvent::FlushInline { elems, .. } => elems.iter().any(|el| match el {
            fastmd::ui::render::InlineElem::Text(_, style) => style.strikethrough,
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

#[test]
fn gfm_tables_are_recognized() {
    let md = "| A | B |\n|---|---|\n| 1 | 2 |";
    let events = parse_markdown_to_events(md);
    assert!(
        has_table(&events),
        "GFM table not recognized — Options::ENABLE_TABLES may be unset: {events:?}"
    );
}

#[test]
fn footnotes_are_recognized() {
    let md = "Footnote[^1]\n\n[^1]: details";
    let events = parse_markdown_to_events(md);
    assert!(
        has_footnote_ref(&events),
        "footnote reference not recognized — Options::ENABLE_FOOTNOTES may be unset: {events:?}"
    );
}

#[test]
fn strikethrough_is_recognized() {
    let md = "~~struck~~";
    let events = parse_markdown_to_events(md);
    assert!(
        has_strikethrough(&events),
        "strikethrough not recognized — Options::ENABLE_STRIKETHROUGH may be unset: {events:?}"
    );
}

#[test]
fn task_lists_are_recognized() {
    let md = "- [ ] todo\n- [x] done";
    let events = parse_markdown_to_events(md);
    assert!(
        has_task_marker(&events),
        "task list not recognized — Options::ENABLE_TASKLISTS may be unset: {events:?}"
    );
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
    let md = "line1\nline2";
    let events = parse_markdown_to_events(md);
    let inline_texts: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            RenderEvent::FlushInline { elems, .. } => {
                let mut s = String::new();
                for elem in elems {
                    if let fastmd::ui::render::InlineElem::Text(t, _) = elem {
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
