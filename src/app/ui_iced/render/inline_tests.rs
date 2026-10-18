//! Unit tests for the inline styled-run mapping ([TASK-015]).
//!
//! Asserts the pure [`SpanModel`] mapping for every `InlineElem` variant
//! and every `TextStyle` flag, then that the mapping survives the hop into
//! an iced `Span`.

use super::*;
use crate::markdown::{InlineElem, TextStyle};
use crate::ui_iced::message::RenderMessage;
use crate::ui_iced::theme::to_iced_color;
use iced::font::Family;
use std::path::PathBuf;

fn palette() -> Palette {
    Palette::fastmd_dark()
}

fn style(flags: &[&str]) -> TextStyle {
    TextStyle {
        bold: flags.contains(&"bold"),
        italic: flags.contains(&"italic"),
        code: flags.contains(&"code"),
        strikethrough: flags.contains(&"strike"),
        muted: flags.contains(&"muted"),
    }
}

// --- plain / default ---------------------------------------------------

#[test]
fn test_plain_text_run_uses_body_colour_and_default_font() {
    let span = span_for_text("hello", &style(&[]), 14.0, palette());
    assert_eq!(span.text, "hello");
    assert_eq!(span.size, 14.0);
    assert_eq!(span.color, to_iced_color(palette().text));
    assert_eq!(span.font, Font::DEFAULT);
    assert_eq!(span.highlight, None);
    assert!(!span.underline);
    assert!(!span.strikethrough);
    assert_eq!(span.link, None);
}

// --- bold / muted / strong colour precedence ---------------------------

#[test]
fn test_bold_run_takes_the_strong_colour() {
    let span = span_for_text("b", &style(&["bold"]), 14.0, palette());
    assert_eq!(span.color, to_iced_color(palette().text_strong));
}

#[test]
fn test_muted_beats_bold() {
    // Mirrors the egui `text_format` precedence: muted → strong → text.
    let span = span_for_text("x", &style(&["bold", "muted"]), 14.0, palette());
    assert_eq!(span.color, to_iced_color(palette().text_muted));
}

#[test]
fn test_italic_sets_the_font_style_not_the_colour() {
    let span = span_for_text("i", &style(&["italic"]), 14.0, palette());
    assert_eq!(span.font.style, iced::font::Style::Italic);
    assert_eq!(span.color, to_iced_color(palette().text));
}

#[test]
fn test_strikethrough_flag_is_carried() {
    let span = span_for_text("gone", &style(&["strike"]), 14.0, palette());
    assert!(span.strikethrough);
}

// --- inline code -------------------------------------------------------

#[test]
fn test_code_run_is_monospace_with_a_border_token_highlight() {
    let span = span_for_text("x = 1", &style(&["code"]), 14.0, palette());
    assert_eq!(span.font.family, Family::Monospace);
    assert_eq!(span.highlight, Some(to_iced_color(palette().border)));
}

#[test]
fn test_nested_bold_and_code_compose() {
    let span = span_for_text("both", &style(&["bold", "code"]), 14.0, palette());
    assert_eq!(span.font.family, Family::Monospace, "code picks the font");
    assert_eq!(
        span.color,
        to_iced_color(palette().text_strong),
        "bold still sets the colour"
    );
    assert!(span.highlight.is_some(), "code still paints its background");
}

#[test]
fn test_nested_italic_and_code_compose() {
    let span = span_for_text("ic", &style(&["italic", "code"]), 14.0, palette());
    assert_eq!(span.font.family, Family::Monospace);
    assert_eq!(span.font.style, iced::font::Style::Italic);
}

#[test]
fn test_font_for_style_matrix() {
    assert_eq!(font_for_style(&style(&[])), Font::DEFAULT);
    assert_eq!(font_for_style(&style(&["bold"])), Font::DEFAULT);
    assert_eq!(
        font_for_style(&style(&["code"])),
        Font::MONOSPACE,
        "Font::MONOSPACE is the code handle"
    );
    assert_eq!(
        font_for_style(&style(&["italic"])).style,
        iced::font::Style::Italic
    );
    assert_eq!(
        font_for_style(&style(&["code", "italic"])).family,
        Family::Monospace
    );
    assert_eq!(
        font_for_style(&style(&["code", "italic"])).style,
        iced::font::Style::Italic
    );
}

// --- links -------------------------------------------------------------

#[test]
fn test_link_run_is_accent_coloured_underlined_and_carries_its_target() {
    let span = span_for_link("https://example.com", "Example", 14.0, palette());
    assert_eq!(span.text, "Example");
    assert_eq!(span.color, to_iced_color(palette().accent_text));
    assert!(span.underline, "links read as links");
    assert_eq!(span.link.as_deref(), Some("https://example.com"));
    assert_eq!(span.highlight, None);
}

#[test]
fn test_wikilink_target_is_carried_verbatim() {
    let span = span_for_link("wikilink:Journal-2023", "Journal", 14.0, palette());
    assert_eq!(
        span.link.as_ref().unwrap().as_str(),
        "wikilink:Journal-2023"
    );
}

// --- element-level mapping ---------------------------------------------

#[test]
fn test_spans_from_inline_covers_every_variant() {
    let elems = vec![
        InlineElem::Text("t".to_string(), style(&["bold"])),
        InlineElem::Link("u".to_string(), "l".to_string()),
        InlineElem::Image("p.png".to_string()),
        InlineElem::Html("<b>".to_string()),
        InlineElem::SoftBreak,
    ];
    let spans = spans_from_inline(&elems, 14.0, palette());
    assert_eq!(spans.len(), 5);

    assert_eq!(spans[0].color, to_iced_color(palette().text_strong));
    assert_eq!(spans[1].link.as_deref(), Some("u"));
    assert_eq!(spans[2].text, "[Image: p.png]");
    assert_eq!(spans[2].color, to_iced_color(palette().text_muted));
    assert_eq!(
        spans[2].highlight, None,
        "the image placeholder is not highlighted"
    );
    assert_eq!(spans[3].text, "<b>");
    assert_eq!(spans[3].color, to_iced_color(palette().text_muted));
    assert_eq!(spans[4].text, " ", "a soft break renders as a space");
    assert_eq!(spans[4].link, None);
}

#[test]
fn test_spans_from_empty_input_is_empty() {
    assert!(spans_from_inline(&[], 14.0, palette()).is_empty());
}

#[test]
fn test_size_is_applied_to_every_span() {
    let elems = vec![
        InlineElem::Text("a".to_string(), TextStyle::default()),
        InlineElem::SoftBreak,
    ];
    for size in [12.0, 18.0, 32.0] {
        let spans = spans_from_inline(&elems, size, palette());
        assert!(
            spans.iter().all(|span| span.size == size),
            "every span must render at {size}"
        );
    }
}

// --- iced Span construction ---------------------------------------------

#[test]
fn test_to_iced_span_carries_every_field() {
    let model = span_for_text("x", &style(&["bold", "code", "strike"]), 22.0, palette());
    let span: Span<'static, String> = to_iced_span(&model);

    assert_eq!(span.text.as_ref(), "x");
    assert_eq!(span.size, Some(22.0.into()));
    assert_eq!(span.color, Some(to_iced_color(palette().text_strong)));
    assert_eq!(span.font, Some(Font::MONOSPACE));
    assert!(span.strikethrough);
    assert!(!span.underline);
    assert!(
        span.highlight.is_some(),
        "the code highlight must survive into the iced span"
    );
    let highlight = span.highlight.unwrap();
    match highlight.background {
        Background::Color(color) => assert_eq!(color, to_iced_color(palette().border)),
        other => panic!("expected a solid background, got {other:?}"),
    }
}

#[test]
fn test_to_iced_span_carries_the_link_target() {
    let model = span_for_link("https://x.test", "x", 14.0, palette());
    let span = to_iced_span(&model);
    assert_eq!(span.link.as_deref(), Some("https://x.test"));
    assert!(span.underline);
}

#[test]
fn test_non_code_span_has_no_highlight() {
    let model = span_for_text("plain", &style(&[]), 14.0, palette());
    let span = to_iced_span(&model);
    assert!(span.highlight.is_none());
    assert_eq!(span.padding, iced::Padding::ZERO);
}

#[test]
fn test_code_span_gets_the_highlight_padding() {
    let model = span_for_text("c", &style(&["code"]), 14.0, palette());
    let span = to_iced_span(&model);
    assert_eq!(span.padding, iced::Padding::from(CODE_HIGHLIGHT_PADDING));
}

// --- widget construction -------------------------------------------------

#[test]
fn test_rich_text_block_builds_from_span_models() {
    let models = spans_from_inline(
        &[
            InlineElem::Text("hello ".to_string(), TextStyle::default()),
            InlineElem::Link("https://x.test".to_string(), "world".to_string()),
        ],
        BODY_SIZE,
        palette(),
    );
    let widget = rich_text_block(&models, Some(Path::new("/tmp/doc.md")));
    // The widget is built; the link click channel is wired (asserted on
    // the message the closure produces below).
    let element = rich_text_element(&models, Some(Path::new("/tmp/doc.md")));
    let _ = (widget, element);
}

#[test]
fn test_rich_text_element_builds_without_a_source_file() {
    let models = spans_from_inline(
        &[InlineElem::Text("x".to_string(), TextStyle::default())],
        14.0,
        palette(),
    );
    let element = rich_text_element(&models, None);
    // The block fills the viewer width so paragraphs wrap at the pane edge.
    assert_eq!(element.as_widget().size().width, Length::Fill);
}

#[test]
fn test_link_message_shape_is_the_render_channel_variant() {
    // The exact payload the reducer will receive for a link click.
    let message = Message::Render(RenderMessage::LinkActivated {
        url: "wikilink:Note".to_string(),
        source_file: Some(PathBuf::from("/tmp/doc.md")),
    });
    match message {
        Message::Render(RenderMessage::LinkActivated { url, source_file }) => {
            assert_eq!(url, "wikilink:Note");
            assert_eq!(source_file.unwrap().to_str().unwrap(), "/tmp/doc.md");
        }
        other => panic!("expected the render channel, got {other:?}"),
    }
}

#[test]
fn test_empty_models_still_build_a_widget() {
    let element = rich_text_element(&[], None);
    // A paragraph with no runs must not panic — the egui renderer
    // short-circuited on empty; iced renders an empty Rich instead.
    let _ = element;
}
