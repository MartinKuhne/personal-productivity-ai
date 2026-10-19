//! Unit tests for the heading mapping ([TASK-015]).

use super::*;
use crate::markdown::{InlineElem, TextStyle};
use crate::ui::theme::Palette;

fn palette() -> Palette {
    Palette::fastmd_dark()
}

fn text(t: &str) -> InlineElem {
    InlineElem::Text(t.to_string(), TextStyle::default())
}

#[test]
fn test_heading_size_ladder_matches_the_legacy_renderer() {
    assert_eq!(heading_size(1), H1_SIZE);
    assert_eq!(heading_size(2), H2_SIZE);
    assert_eq!(heading_size(3), H3_SIZE);
    assert_eq!(heading_size(4), H4_SIZE);
    assert_eq!(heading_size(5), H5_SIZE);
    assert_eq!(heading_size(6), H5_SIZE);
}

#[test]
fn test_heading_size_numeric_order_is_monotonic_down() {
    let sizes = [
        heading_size(1),
        heading_size(2),
        heading_size(3),
        heading_size(4),
        heading_size(5),
    ];
    for pair in sizes.windows(2) {
        assert!(
            pair[0] > pair[1],
            "H(n) must be larger than H(n+1): {:?} vs {:?}",
            pair[0],
            pair[1]
        );
    }
}

#[test]
fn test_out_of_range_levels_collapse_to_the_floor() {
    // The legacy UI ladder's `_ => 12.0` arm.
    assert_eq!(heading_size(0), H5_SIZE);
    assert_eq!(heading_size(7), H5_SIZE);
    assert_eq!(heading_size(u32::MAX), H5_SIZE);
}

#[test]
fn test_blank_heading_is_detected() {
    assert!(is_blank_heading(&[]));
    assert!(is_blank_heading(&[text("   ")]));
    assert!(is_blank_heading(&[InlineElem::SoftBreak]));
    assert!(is_blank_heading(&[text("\t\n  ")]));
}

#[test]
fn test_non_blank_heading_is_not_skipped() {
    assert!(!is_blank_heading(&[text("Title")]));
    assert!(!is_blank_heading(&[InlineElem::Link(
        "u".into(),
        "l".into()
    )]));
    assert!(!is_blank_heading(&[InlineElem::Html("<b>x</b>".into())]));
}

#[test]
fn test_heading_spans_render_at_the_level_size() {
    for level in 1..=6 {
        let spans = heading_spans(&[text("Heading")], level, palette());
        assert_eq!(spans.len(), 1);
        assert_eq!(
            spans[0].size,
            heading_size(level),
            "H{level} must render at its ladder size"
        );
    }
}

#[test]
fn test_heading_spans_keep_inline_styling() {
    let elems = vec![
        InlineElem::Text(
            "bold ".to_string(),
            TextStyle {
                bold: true,
                ..Default::default()
            },
        ),
        text("plain"),
        InlineElem::Link("https://x.test".into(), "link".into()),
    ];
    let spans = heading_spans(&elems, 2, palette());
    assert_eq!(spans.len(), 3);
    assert_eq!(spans[0].color, palette().text_strong);
    assert_eq!(spans[1].color, palette().text);
    assert_eq!(spans[2].link.as_deref(), Some("https://x.test"));
    assert!(
        spans.iter().all(|span| span.size == H2_SIZE),
        "every run in a heading shares the heading size"
    );
}

#[test]
fn test_heading_code_run_is_monospace_at_the_heading_size() {
    let elems = vec![InlineElem::Text(
        "code".to_string(),
        TextStyle {
            code: true,
            ..Default::default()
        },
    )];
    let spans = heading_spans(&elems, 1, palette());
    assert_eq!(spans[0].font, iced::Font::MONOSPACE);
    assert_eq!(spans[0].size, H1_SIZE);
    assert!(spans[0].highlight.is_some());
}

#[test]
fn test_heading_block_builds() {
    let spans = heading_spans(&[text("Title")], 1, palette());
    let element = heading_block(&spans, None);
    assert_eq!(element.as_widget().size().width, iced::Length::Fill);
}

#[test]
fn test_heading_block_with_source_file_builds() {
    let spans = heading_spans(
        &[InlineElem::Link("wikilink:A".into(), "A".into())],
        3,
        palette(),
    );
    let element = heading_block(&spans, Some(std::path::Path::new("/tmp/a.md")));
    let _ = element;
}
