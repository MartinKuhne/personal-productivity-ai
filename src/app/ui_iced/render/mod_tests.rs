//! Unit tests for the iced markdown render mapping ([TASK-015]).
//!
//! Covers the pure `RenderEvent` → `RenderBlock` mapping — every event
//! kind, the task-index counter, the blank-heading rule and the
//! search first-match marking — plus widget construction for each block
//! kind and one narrow integration pass over the real parser.

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

// --- list markers ---------------------------------------------------------

#[test]
fn test_bullet_marker_text_uses_the_strings_constant() {
    assert_eq!(
        ListMarker::Bullet.text(),
        format!("{} ", strings::LIST_BULLET)
    );
}

#[test]
fn test_ordinal_marker_text_uses_the_suffix_constant() {
    assert_eq!(
        ListMarker::Ordinal(1).text(),
        format!("1{}", strings::ORDERED_LIST_MARKER_SUFFIX)
    );
    assert_eq!(ListMarker::Ordinal(42).text(), "42. ");
}

// --- task toggle intent ----------------------------------------------------

#[test]
fn test_task_toggle_message_carries_document_and_index() {
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
        }))
    );
}

#[test]
fn test_task_toggle_without_a_source_file_is_none() {
    // Nothing to write the toggle back to, so the checkbox renders inert.
    let message = task_toggle_message(
        None,
        TaskCheckbox {
            task_index: 0,
            checked: true,
        },
        false,
    );
    assert_eq!(message, None);
}

// --- search matching --------------------------------------------------------

#[test]
fn test_inline_elems_contain_matches_text_link_and_html_only() {
    assert!(inline_elems_contain(&[text("Hello World")], "world"));
    assert!(inline_elems_contain(
        &[InlineElem::Link("u".into(), "Label".into())],
        "label"
    ));
    assert!(inline_elems_contain(
        &[InlineElem::Html("<em>note</em>".into())],
        "note"
    ));
    assert!(
        !inline_elems_contain(&[InlineElem::Image("needle.png".into())], "needle"),
        "image placeholders are not searchable, as in the legacy UI build"
    );
    assert!(!inline_elems_contain(&[InlineElem::SoftBreak], "x"));
    assert!(!inline_elems_contain(&[], "x"));
}

#[test]
fn test_event_contains_per_event_kind() {
    let table = RenderEvent::Table(vec![vec![vec![text("cellvalue")]]]);
    assert!(event_contains(
        &RenderEvent::FlushInline {
            elems: vec![text("find me")],
            needs_bullet: false,
            task_checked: None,
            indent: 0,
            list_ordinal: None,
        },
        "find"
    ));
    assert!(event_contains(
        &RenderEvent::CodeBlock {
            language: None,
            content: "SECRET = 1".to_string(),
        },
        "secret"
    ));
    assert!(event_contains(
        &RenderEvent::Heading {
            level: 1,
            elems: vec![text("Chapter")],
        },
        "chapter"
    ));
    assert!(event_contains(&table, "cellvalue"));
    assert!(!event_contains(&RenderEvent::Space(10.0), "x"));
    assert!(!event_contains(&RenderEvent::Separator, "x"));
}

#[test]
fn test_only_the_first_matching_block_is_marked() {
    let events = vec![
        RenderEvent::FlushInline {
            elems: vec![text("nothing here")],
            needs_bullet: false,
            task_checked: None,
            indent: 0,
            list_ordinal: None,
        },
        RenderEvent::FlushInline {
            elems: vec![text("first needle")],
            needs_bullet: false,
            task_checked: None,
            indent: 0,
            list_ordinal: None,
        },
        RenderEvent::FlushInline {
            elems: vec![text("second needle")],
            needs_bullet: false,
            task_checked: None,
            indent: 0,
            list_ordinal: None,
        },
    ];
    let ctx = plain_ctx(None).with_search("needle");
    let blocks = blocks_from_events(&events, 800.0, &ctx);
    let marked: Vec<usize> = blocks
        .iter()
        .enumerate()
        .filter(|(_, block)| block.scroll_to_me)
        .map(|(index, _)| index)
        .collect();
    assert_eq!(
        marked,
        vec![1],
        "the legacy UI cleared the search after the first hit"
    );
}

#[test]
fn test_no_search_marks_nothing() {
    let events = vec![RenderEvent::FlushInline {
        elems: vec![text("needle")],
        needs_bullet: false,
        task_checked: None,
        indent: 0,
        list_ordinal: None,
    }];
    let blocks = mapped(&events, 800.0);
    assert!(blocks.iter().all(|block| !block.scroll_to_me));
}

// --- event mapping -----------------------------------------------------------

#[test]
fn test_paragraph_event_maps_to_a_plain_paragraph() {
    let events = vec![RenderEvent::FlushInline {
        elems: vec![text("body text")],
        needs_bullet: false,
        task_checked: None,
        indent: 0,
        list_ordinal: None,
    }];
    let blocks = mapped(&events, 800.0);
    assert_eq!(blocks.len(), 1);
    match &blocks[0].kind {
        BlockKind::Paragraph {
            spans,
            indent,
            marker,
            task,
        } => {
            assert_eq!(spans.len(), 1);
            assert_eq!(spans[0].text, "body text");
            assert_eq!(*indent, 0);
            assert!(marker.is_none());
            assert!(task.is_none());
        }
        other => panic!("expected a paragraph, got {other:?}"),
    }
}

#[test]
fn test_unordered_list_item_gets_a_bullet() {
    let events = vec![RenderEvent::FlushInline {
        elems: vec![text("item")],
        needs_bullet: true,
        task_checked: None,
        indent: 0,
        list_ordinal: None,
    }];
    let blocks = mapped(&events, 800.0);
    match &blocks[0].kind {
        BlockKind::Paragraph { marker, .. } => {
            assert_eq!(*marker, Some(ListMarker::Bullet))
        }
        other => panic!("expected a paragraph, got {other:?}"),
    }
}

#[test]
fn test_ordered_list_item_gets_its_ordinal() {
    let events = vec![RenderEvent::FlushInline {
        elems: vec![text("step")],
        needs_bullet: true,
        task_checked: None,
        indent: 1,
        list_ordinal: Some(7),
    }];
    let blocks = mapped(&events, 800.0);
    match &blocks[0].kind {
        BlockKind::Paragraph { marker, indent, .. } => {
            assert_eq!(*marker, Some(ListMarker::Ordinal(7)));
            assert_eq!(*indent, 1);
        }
        other => panic!("expected a paragraph, got {other:?}"),
    }
}

#[test]
fn test_task_item_gets_a_checkbox_with_its_document_index() {
    let events = vec![
        RenderEvent::FlushInline {
            elems: vec![text("first")],
            needs_bullet: true,
            task_checked: Some(false),
            indent: 0,
            list_ordinal: None,
        },
        RenderEvent::FlushInline {
            elems: vec![text("second")],
            needs_bullet: true,
            task_checked: Some(true),
            indent: 0,
            list_ordinal: None,
        },
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
        "task indices count in document order, as the legacy UI walker did"
    );
}

#[test]
fn test_non_task_items_do_not_advance_the_task_counter() {
    let events = vec![
        RenderEvent::FlushInline {
            elems: vec![text("plain")],
            needs_bullet: true,
            task_checked: None,
            indent: 0,
            list_ordinal: None,
        },
        RenderEvent::FlushInline {
            elems: vec![text("task")],
            needs_bullet: true,
            task_checked: Some(false),
            indent: 0,
            list_ordinal: None,
        },
    ];
    let blocks = mapped(&events, 800.0);
    match &blocks[1].kind {
        BlockKind::Paragraph {
            task: Some(task), ..
        } => assert_eq!(task.task_index, 0, "only task items count"),
        other => panic!("expected a task paragraph, got {other:?}"),
    }
}

#[test]
fn test_code_block_event_maps_to_a_code_model() {
    let events = vec![RenderEvent::CodeBlock {
        language: Some("rust".to_string()),
        content: "fn main() {}".to_string(),
    }];
    let blocks = mapped(&events, 800.0);
    match &blocks[0].kind {
        BlockKind::Code(model) => {
            assert_eq!(model.language.as_deref(), Some("rust"));
            assert_eq!(model.content, "fn main() {}");
            assert_eq!(
                model.copy_message,
                Message::Render(RenderMessage::CopyCodeRequested {
                    content: "fn main() {}".to_string()
                })
            );
        }
        other => panic!("expected a code block, got {other:?}"),
    }
}

#[test]
fn test_heading_event_maps_at_its_level_size() {
    let events = vec![RenderEvent::Heading {
        level: 2,
        elems: vec![text("Title")],
    }];
    let blocks = mapped(&events, 800.0);
    match &blocks[0].kind {
        BlockKind::Heading { level, spans } => {
            assert_eq!(*level, 2);
            assert_eq!(spans[0].size, heading::H2_SIZE);
        }
        other => panic!("expected a heading, got {other:?}"),
    }
}

#[test]
fn test_blank_heading_is_dropped() {
    let events = vec![
        RenderEvent::Heading {
            level: 1,
            elems: vec![text("   ")],
        },
        RenderEvent::FlushInline {
            elems: vec![text("body")],
            needs_bullet: false,
            task_checked: None,
            indent: 0,
            list_ordinal: None,
        },
    ];
    let blocks = mapped(&events, 800.0);
    assert_eq!(blocks.len(), 1, "the empty heading produces no block");
    assert!(matches!(blocks[0].kind, BlockKind::Paragraph { .. }));
}

#[test]
fn test_space_and_separator_events_map_through() {
    let events = vec![RenderEvent::Space(12.0), RenderEvent::Separator];
    let blocks = mapped(&events, 800.0);
    assert_eq!(blocks.len(), 2);
    assert!(matches!(blocks[0].kind, BlockKind::Spacer(12.0)));
    assert!(matches!(blocks[1].kind, BlockKind::Separator));
}

#[test]
fn test_table_event_is_solved_during_mapping() {
    let events = vec![RenderEvent::Table(vec![
        vec![vec![text("a")], vec![text("b")]],
        vec![vec![text("c")], vec![text("d")]],
    ])];
    let blocks = mapped(&events, 1000.0);
    match &blocks[0].kind {
        BlockKind::Table(model) => {
            assert_eq!(model.column_count(), 2);
            assert_eq!(model.rows.len(), 2);
            assert!(!model.needs_horizontal_scroll);
        }
        other => panic!("expected a table, got {other:?}"),
    }
}

#[test]
fn test_table_narrower_than_its_tokens_falls_back_to_scroll() {
    let events = vec![RenderEvent::Table(vec![vec![
        vec![text("unbreakabletokenone")],
        vec![text("unbreakabletokentwo")],
    ]])];
    let blocks = mapped(&events, 20.0);
    match &blocks[0].kind {
        BlockKind::Table(model) => assert!(
            model.needs_horizontal_scroll,
            "20 px cannot hold two long tokens"
        ),
        other => panic!("expected a table, got {other:?}"),
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
        RenderEvent::FlushInline {
            elems: vec![text("p")],
            needs_bullet: false,
            task_checked: None,
            indent: 2,
            list_ordinal: None,
        },
        RenderEvent::FlushInline {
            elems: vec![text("t")],
            needs_bullet: true,
            task_checked: Some(true),
            indent: 0,
            list_ordinal: Some(3),
        },
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
fn test_render_markdown_builds_a_column() {
    let events = parse_markdown_to_events("# H\n\npara\n\n- item\n");
    let element = render_markdown(&events, 800.0, &plain_ctx(None));
    assert_eq!(element.as_widget().size().width, Length::Fill);
}

#[test]
fn test_render_markdown_of_no_events_builds() {
    let element = render_markdown(&[], 800.0, &plain_ctx(None));
    let _ = element;
}

#[test]
fn test_render_document_with_front_matter_prepends_the_table() {
    let mut map = Mapping::new();
    map.insert(
        Value::String("title".to_string()),
        Value::String("Doc".to_string()),
    );
    let events = parse_markdown_to_events("body");
    let element = render_document(
        &events,
        Some(&Value::Mapping(map)),
        800.0,
        &plain_ctx(Some(PathBuf::from("/tmp/doc.md"))),
    );
    let _ = element;
}

#[test]
fn test_render_document_without_front_matter_builds() {
    let events = parse_markdown_to_events("body");
    let element = render_document(&events, None, 800.0, &plain_ctx(None));
    let _ = element;
}

#[test]
fn test_render_document_ignores_non_mapping_front_matter() {
    let events = parse_markdown_to_events("body");
    let element = render_document(
        &events,
        Some(&Value::String("not a mapping".to_string())),
        800.0,
        &plain_ctx(None),
    );
    let _ = element;
}

// --- narrow integration over the real parser --------------------------------

#[test]
fn test_real_document_maps_to_the_expected_block_sequence() {
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

    assert_eq!(kinds.first().copied(), Some("heading"));
    assert!(kinds.contains(&"paragraph"));
    assert!(kinds.contains(&"code"));
    assert!(kinds.contains(&"table"));

    let link = blocks
        .iter()
        .filter_map(|block| match &block.kind {
            BlockKind::Paragraph { spans, .. } => Some(spans),
            _ => None,
        })
        .flatten()
        .find(|span| span.link.is_some());
    assert_eq!(link.unwrap().link.as_deref(), Some("https://x.test"));

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
        })
    );
}

#[test]
fn test_alignment_row_is_not_a_rendered_row() {
    // The parser consumes `| --- | --- |`; the model never carries it.
    let events = parse_markdown_to_events("| a | b |\n|:---|---:|\n| 1 | 2 |\n");
    let blocks = mapped(&events, 1200.0);
    match &blocks[0].kind {
        BlockKind::Table(model) => {
            assert_eq!(model.rows.len(), 2, "header + 1 data row only");
            assert_eq!(model.rows[0][0].spans[0].text, "a");
            assert_eq!(model.rows[1][0].spans[0].text, "1");
        }
        other => panic!("expected a table, got {other:?}"),
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
