//! Unit tests for the iced markdown render mapping.
//!
//! Covers the pure `RenderEvent` → `RenderBlock` mapping — every event
//! kind, the task-index counter, the blank-heading rule and the
//! search first-match marking — plus widget construction for each block
//! kind and one narrow integration pass over the real parser.
//!
//! Related cases are grouped into one process and each case names itself in
//! its failure message, so a merged failure still reports which event kind or
//! which decoration broke.

use super::*;
use crate::markdown::model::{InlineElem, RenderEvent, TextStyle};
use crate::markdown::parse_markdown_to_events;
use crate::ui::theme::Palette;
use crate::ui_iced::table_width::TextMetrics;
use serde_norway::{Mapping, Value};

fn palette() -> Palette {
    Palette::fastmd_dark()
}

fn text(t: &str) -> InlineElem {
    InlineElem::Text(t.to_string(), TextStyle::default())
}

fn plain_ctx<'a>(source: Option<PathBuf>) -> RenderContext<'a> {
    RenderContext::new(DeficitStrategy::HybridMinPenaltyWaterFill, source)
}

fn mapped(events: &[RenderEvent], width: f32) -> Vec<RenderBlock> {
    blocks_from_events(events, width, &plain_ctx(None))
}

/// A `FlushInline` event, the shape most mapping cases need.
fn flush(
    elems: Vec<InlineElem>,
    needs_bullet: bool,
    task_checked: Option<bool>,
    indent: usize,
    list_ordinal: Option<u64>,
) -> RenderEvent {
    RenderEvent::FlushInline {
        elems,
        needs_bullet,
        task_checked,
        indent,
        list_ordinal,
    }
}

/// The indices of the blocks the search marked.
fn marked_indices(blocks: &[RenderBlock]) -> Vec<usize> {
    blocks
        .iter()
        .enumerate()
        .filter(|(_, block)| block.scroll_to_me)
        .map(|(index, _)| index)
        .collect()
}

// --- list markers ---------------------------------------------------------

#[test]
fn test_marker_text_uses_the_strings_constants() {
    assert_eq!(
        ListMarker::Bullet.text(),
        format!("{} ", strings::LIST_BULLET),
        "bullet: the glyph comes from the strings module"
    );
    assert_eq!(
        ListMarker::Ordinal(1).text(),
        format!("1{}", strings::ORDERED_LIST_MARKER_SUFFIX),
        "ordinal: the suffix comes from the strings module"
    );
    assert_eq!(
        ListMarker::Ordinal(42).text(),
        "42. ",
        "ordinal: a two-digit ordinal renders as `\"<n>. \"`"
    );
}

// --- task toggle intent ----------------------------------------------------

#[test]
fn test_task_toggle_message_carries_the_document_only_when_there_is_one() {
    let message = task_toggle_message(
        Some(&PathBuf::from("/tmp/note.md")),
        TaskCheckbox {
            task_index: 3,
            checked: false,
        },
        true,
    );
    assert_eq!(
        message,
        Some(Message::Render(RenderMessage::TaskToggled {
            document: PathBuf::from("/tmp/note.md"),
            task_index: 3,
            checked: true,
        })),
        "with a source file: the toggle carries the document and the index"
    );

    // Nothing to write the toggle back to, so the checkbox renders inert.
    let message = task_toggle_message(
        None,
        TaskCheckbox {
            task_index: 0,
            checked: true,
        },
        false,
    );
    assert_eq!(
        message, None,
        "without a source file: no intent can be emitted"
    );
}

// --- search matching --------------------------------------------------------

#[test]
fn test_the_search_predicates_match_rendered_text_only() {
    // Inline elements: text, link labels and raw HTML are searchable; image
    // placeholders and breaks are not.
    assert!(
        inline_elems_contain(&[text("Hello World")], "world"),
        "inline text is searchable"
    );
    assert!(
        inline_elems_contain(&[InlineElem::Link("u".into(), "Label".into())], "label"),
        "a link label is searchable"
    );
    assert!(
        inline_elems_contain(&[InlineElem::Html("<em>note</em>".into())], "note"),
        "raw HTML is searchable"
    );
    assert!(
        !inline_elems_contain(&[InlineElem::Image("needle.png".into())], "needle"),
        "image placeholders are not searchable"
    );
    assert!(
        !inline_elems_contain(&[InlineElem::SoftBreak], "x"),
        "a soft break carries no text"
    );
    assert!(
        !inline_elems_contain(&[], "x"),
        "an empty run carries no text"
    );

    // Events: each content-bearing kind is searched through its own text.
    let table = RenderEvent::Table(vec![vec![vec![text("cellvalue")]]]);
    assert!(
        event_contains(&flush(vec![text("find me")], false, None, 0, None), "find"),
        "a flushed inline run is searched through its spans"
    );
    assert!(
        event_contains(
            &RenderEvent::CodeBlock {
                language: None,
                content: "SECRET = 1".to_string(),
            },
            "secret"
        ),
        "a code block is searched through its content"
    );
    assert!(
        event_contains(
            &RenderEvent::Heading {
                level: 1,
                elems: vec![text("Chapter")],
            },
            "chapter"
        ),
        "a heading is searched through its runs"
    );
    assert!(
        event_contains(&table, "cellvalue"),
        "a table is searched through its cells"
    );
    assert!(
        !event_contains(&RenderEvent::Space(10.0), "x"),
        "a spacer carries no text"
    );
    assert!(
        !event_contains(&RenderEvent::Separator, "x"),
        "a separator carries no text"
    );
}

#[test]
fn test_only_the_first_matching_block_is_marked() {
    let events = vec![
        flush(vec![text("nothing here")], false, None, 0, None),
        flush(vec![text("first needle")], false, None, 0, None),
        flush(vec![text("second needle")], false, None, 0, None),
    ];
    let ctx = plain_ctx(None).with_search("needle");
    let blocks = blocks_from_events(&events, 800.0, &ctx);
    assert_eq!(
        marked_indices(&blocks),
        vec![1],
        "with a search term: the search clears after the first hit"
    );

    let events = vec![flush(vec![text("needle")], false, None, 0, None)];
    let blocks = mapped(&events, 800.0);
    assert!(
        blocks.iter().all(|block| !block.scroll_to_me),
        "without a search term: nothing is marked"
    );
}

// --- event mapping -----------------------------------------------------------

#[test]
fn test_flush_inline_events_map_to_paragraphs_with_their_decoration() {
    let cases: Vec<(&str, RenderEvent, &str, Option<ListMarker>, usize)> = vec![
        (
            "plain paragraph",
            flush(vec![text("body text")], false, None, 0, None),
            "body text",
            None,
            0,
        ),
        (
            "unordered list item",
            flush(vec![text("item")], true, None, 0, None),
            "item",
            Some(ListMarker::Bullet),
            0,
        ),
        (
            "ordered list item",
            flush(vec![text("step")], true, None, 1, Some(7)),
            "step",
            Some(ListMarker::Ordinal(7)),
            1,
        ),
    ];

    for (label, event, expected_text, expected_marker, expected_indent) in &cases {
        let blocks = mapped(std::slice::from_ref(event), 800.0);
        assert_eq!(blocks.len(), 1, "{label}: one event maps to one block");
        match &blocks[0].kind {
            BlockKind::Paragraph {
                spans,
                indent,
                marker,
                task,
            } => {
                assert_eq!(spans.len(), 1, "{label}: one span per inline run");
                assert_eq!(
                    spans[0].text, *expected_text,
                    "{label}: the span carries the event's text"
                );
                assert_eq!(
                    *indent, *expected_indent,
                    "{label}: the list indent maps through"
                );
                assert_eq!(
                    *marker, *expected_marker,
                    "{label}: the marker maps from the event's bullet or ordinal"
                );
                assert!(task.is_none(), "{label}: a non-task item has no checkbox");
            }
            other => panic!("{label}: expected a paragraph, got {other:?}"),
        }
    }
}

#[test]
fn test_the_task_counter_counts_task_items_in_document_order() {
    // Two task items get consecutive indices in document order.
    let events = vec![
        flush(vec![text("first")], true, Some(false), 0, None),
        flush(vec![text("second")], true, Some(true), 0, None),
    ];
    let blocks = mapped(&events, 800.0);
    let tasks: Vec<TaskCheckbox> = blocks
        .iter()
        .filter_map(|block| match &block.kind {
            BlockKind::Paragraph { task, .. } => *task,
            _ => None,
        })
        .collect();
    assert_eq!(
        tasks,
        vec![
            TaskCheckbox {
                task_index: 0,
                checked: false
            },
            TaskCheckbox {
                task_index: 1,
                checked: true
            }
        ],
        "task items: task indices count in document order"
    );

    // A plain item in front of a task item must not consume an index.
    let events = vec![
        flush(vec![text("plain")], true, None, 0, None),
        flush(vec![text("task")], true, Some(false), 0, None),
    ];
    let blocks = mapped(&events, 800.0);
    match &blocks[1].kind {
        BlockKind::Paragraph {
            task: Some(task), ..
        } => assert_eq!(task.task_index, 0, "plain items: only task items count"),
        other => panic!("plain items: expected a task paragraph, got {other:?}"),
    }
}

#[test]
fn test_non_paragraph_events_map_to_their_own_block_kinds() {
    // A fenced code block becomes a code model carrying its own copy intent.
    let events = vec![RenderEvent::CodeBlock {
        language: Some("rust".to_string()),
        content: "fn main() {}".to_string(),
    }];
    let blocks = mapped(&events, 800.0);
    match &blocks[0].kind {
        BlockKind::Code(model) => {
            assert_eq!(
                model.language.as_deref(),
                Some("rust"),
                "code block: the info string carries the language"
            );
            assert_eq!(
                model.content, "fn main() {}",
                "code block: the body maps verbatim"
            );
            assert_eq!(
                model.copy_message,
                Message::Render(RenderMessage::CopyCodeRequested {
                    content: "fn main() {}".to_string()
                }),
                "code block: the copy button carries the block's own content"
            );
        }
        other => panic!("code block: expected a code block, got {other:?}"),
    }

    // A heading maps at its level's rung size.
    let events = vec![RenderEvent::Heading {
        level: 2,
        elems: vec![text("Title")],
    }];
    let blocks = mapped(&events, 800.0);
    match &blocks[0].kind {
        BlockKind::Heading { level, spans } => {
            assert_eq!(*level, 2, "heading: the level maps through");
            assert_eq!(
                spans[0].size,
                heading::H2_SIZE,
                "heading: the run takes the level's own size"
            );
        }
        other => panic!("heading: expected a heading, got {other:?}"),
    }

    // A heading with no visible text produces no block at all.
    let events = vec![
        RenderEvent::Heading {
            level: 1,
            elems: vec![text("   ")],
        },
        flush(vec![text("body")], false, None, 0, None),
    ];
    let blocks = mapped(&events, 800.0);
    assert_eq!(
        blocks.len(),
        1,
        "blank heading: the empty heading produces no block"
    );
    assert!(
        matches!(blocks[0].kind, BlockKind::Paragraph { .. }),
        "blank heading: only the body survives"
    );

    // Spacers and rules map straight through.
    let events = vec![RenderEvent::Space(12.0), RenderEvent::Separator];
    let blocks = mapped(&events, 800.0);
    assert_eq!(blocks.len(), 2, "spacer and separator: both map through");
    assert!(
        matches!(blocks[0].kind, BlockKind::Spacer(12.0)),
        "spacer: the pixel extent maps through"
    );
    assert!(
        matches!(blocks[1].kind, BlockKind::Separator),
        "separator: the rule maps through"
    );
}

#[test]
fn test_table_events_are_solved_during_mapping() {
    let events = vec![RenderEvent::Table(vec![
        vec![vec![text("a")], vec![text("b")]],
        vec![vec![text("c")], vec![text("d")]],
    ])];
    let blocks = mapped(&events, 1000.0);
    match &blocks[0].kind {
        BlockKind::Table(model) => {
            assert_eq!(model.column_count(), 2, "roomy table: two columns");
            assert_eq!(model.rows.len(), 2, "roomy table: header plus one data row");
            assert!(
                !model.needs_horizontal_scroll,
                "roomy table: 1000 px holds the content"
            );
        }
        other => panic!("roomy table: expected a table, got {other:?}"),
    }

    let events = vec![RenderEvent::Table(vec![vec![
        vec![text("unbreakabletokenone")],
        vec![text("unbreakabletokentwo")],
    ]])];
    let blocks = mapped(&events, 20.0);
    match &blocks[0].kind {
        BlockKind::Table(model) => assert!(
            model.needs_horizontal_scroll,
            "narrow table: 20 px cannot hold two long tokens"
        ),
        other => panic!("narrow table: expected a table, got {other:?}"),
    }
}

#[test]
fn test_context_measurer_is_used_by_the_table_solve() {
    // A renderer-backed measurer (1 px per char) must produce narrower
    // columns than the default heuristic at the same available width.
    let events = vec![RenderEvent::Table(vec![vec![vec![text("hello world")]]])];
    let heuristic = mapped(&events, 1000.0);
    let tight_ctx = plain_ctx(None).with_measurer(IcedTextMeasurer::new(
        &|text, _font, _size| text.chars().count() as f32,
        TextMetrics {
            body_size: 14.0,
            mono_size: 13.0,
            line_height: 18.0,
        },
    ));
    let tight = blocks_from_events(&events, 1000.0, &tight_ctx);

    let width_of = |blocks: &[RenderBlock]| match &blocks[0].kind {
        BlockKind::Table(model) => model.column_widths[0],
        other => panic!("expected a table, got {other:?}"),
    };
    assert!(
        width_of(&tight) < width_of(&heuristic),
        "the injected measurer must drive the solve"
    );
}

// --- widget construction -------------------------------------------------------

#[test]
fn test_every_block_kind_renders() {
    let events = vec![
        RenderEvent::Heading {
            level: 1,
            elems: vec![text("H")],
        },
        flush(vec![text("p")], false, None, 2, None),
        flush(vec![text("t")], true, Some(true), 0, Some(3)),
        RenderEvent::CodeBlock {
            language: Some("md".to_string()),
            content: "x".to_string(),
        },
        RenderEvent::Table(vec![vec![vec![text("a")]]]),
        RenderEvent::Space(4.0),
        RenderEvent::Separator,
    ];
    let ctx = plain_ctx(Some(PathBuf::from("/tmp/doc.md")));
    let blocks = blocks_from_events(&events, 900.0, &ctx);
    assert_eq!(blocks.len(), 7);
    for block in &blocks {
        let element = render_block(block, &ctx);
        let _ = element;
    }
}

#[test]
fn test_paragraph_block_renders_without_a_source_file() {
    let ctx = plain_ctx(None);
    let spans = inline::spans_from_inline(&[text("x")], inline::BODY_SIZE, palette());
    let element = paragraph_block(
        &spans,
        1,
        Some(&ListMarker::Bullet),
        Some(TaskCheckbox {
            task_index: 0,
            checked: false,
        }),
        &ctx,
    );
    let _ = element;
}

#[test]
fn test_render_markdown_builds_a_fill_width_column() {
    let element = render_markdown(
        &parse_markdown_to_events("# H\n\npara\n\n- item\n"),
        800.0,
        &plain_ctx(None),
    );
    assert_eq!(
        element.as_widget().size().width,
        Length::Fill,
        "a populated document fills the available width"
    );

    // An empty event stream still builds a column rather than panicking.
    let empty = render_markdown(&[], 800.0, &plain_ctx(None));
    let _ = empty;
}

#[test]
fn test_render_document_tolerates_every_front_matter_shape() {
    let mut map = Mapping::new();
    map.insert(
        Value::String("title".to_string()),
        Value::String("Doc".to_string()),
    );

    let cases: Vec<(&str, Option<Value>, Option<PathBuf>)> = vec![
        (
            "a mapping prepends the table",
            Some(Value::Mapping(map)),
            Some(PathBuf::from("/tmp/doc.md")),
        ),
        ("no front matter", None, None),
        (
            "a non-mapping scalar is ignored",
            Some(Value::String("not a mapping".to_string())),
            None,
        ),
    ];

    let events = parse_markdown_to_events("body");
    for (label, front_matter, source) in &cases {
        let element = render_document(
            &events,
            front_matter.as_ref(),
            800.0,
            &plain_ctx(source.clone()),
        );
        // Reaching here at all is the assertion: none of the three shapes
        // may panic or reject the document.
        let _ = (element, label);
    }
}

// --- narrow integration over the real parser --------------------------------

#[test]
fn test_real_documents_map_to_the_expected_blocks() {
    let markdown = concat!(
        "# Title\n",
        "\n",
        "A paragraph with **bold** and a [link](https://x.test).\n",
        "\n",
        "- bullet one\n",
        "- [x] checked task\n",
        "\n",
        "```rust\n",
        "fn main() {}\n",
        "```\n",
        "\n",
        "| a | b |\n",
        "| --- | --- |\n",
        "| 1 | 2 |\n",
    );
    let events = parse_markdown_to_events(markdown);
    let blocks = mapped(&events, 1200.0);

    let kinds: Vec<&str> = blocks
        .iter()
        .map(|block| match &block.kind {
            BlockKind::Heading { .. } => "heading",
            BlockKind::Paragraph { .. } => "paragraph",
            BlockKind::Code(_) => "code",
            BlockKind::Table(_) => "table",
            BlockKind::Spacer(_) => "spacer",
            BlockKind::Separator => "separator",
        })
        .collect();

    assert_eq!(
        kinds.first().copied(),
        Some("heading"),
        "real document: the heading opens the page"
    );
    assert!(
        kinds.contains(&"paragraph"),
        "real document: the paragraph is rendered"
    );
    assert!(
        kinds.contains(&"code"),
        "real document: the fence is rendered as code"
    );
    assert!(
        kinds.contains(&"table"),
        "real document: the table is rendered"
    );

    let link = blocks
        .iter()
        .filter_map(|block| match &block.kind {
            BlockKind::Paragraph { spans, .. } => Some(spans),
            _ => None,
        })
        .flatten()
        .find(|span| span.link.is_some());
    assert_eq!(
        link.unwrap().link.as_deref(),
        Some("https://x.test"),
        "real document: the inline link carries its target"
    );

    let task = blocks
        .iter()
        .filter_map(|block| match &block.kind {
            BlockKind::Paragraph { task, .. } => *task,
            _ => None,
        })
        .next();
    assert_eq!(
        task,
        Some(TaskCheckbox {
            task_index: 0,
            checked: true
        }),
        "real document: the checked task is index 0"
    );

    // The parser consumes `| --- | --- |`; the model never carries it.
    let events = parse_markdown_to_events("| a | b |\n|:---|---:|\n| 1 | 2 |\n");
    let blocks = mapped(&events, 1200.0);
    match &blocks[0].kind {
        BlockKind::Table(model) => {
            assert_eq!(
                model.rows.len(),
                2,
                "alignment row: header + 1 data row only"
            );
            assert_eq!(
                model.rows[0][0].spans[0].text, "a",
                "alignment row: the header cell is the first row"
            );
            assert_eq!(
                model.rows[1][0].spans[0].text, "1",
                "alignment row: the data cell follows it"
            );
        }
        other => panic!("alignment row: expected a table, got {other:?}"),
    }
}

#[test]
fn test_context_defaults_are_the_fastmd_palette_and_zero_padding() {
    let ctx = plain_ctx(None);
    assert_eq!(ctx.palette, palette());
    assert_eq!(ctx.table_padding, TablePadding::ZERO);
    assert!(ctx.search_term.is_none());
    assert_eq!(ctx.palette.text, Palette::fastmd_dark().text);
}
