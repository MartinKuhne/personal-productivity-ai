//! Unit tests for [`super`] (the shared inline-run styling helpers).
use super::*;
use crate::markdown::TextStyle;
use crate::ui::theme::Palette;
use egui::{Color32, FontId, Stroke};

fn colors() -> RunColors {
    RunColors {
        text: Color32::from_gray(210),
        strong: Color32::WHITE,
        code_bg: Color32::from_gray(40),
        muted: Color32::from_gray(160),
    }
}

#[test]
fn styled_text_applies_every_attribute() {
    let palette = Palette::fastmd_dark();
    let style = TextStyle {
        bold: true,
        italic: true,
        code: true,
        muted: true,
        strikethrough: true,
    };
    let expected = RichText::new("hi")
        .strong()
        .italics()
        .monospace()
        .background_color(palette.border)
        .color(palette.text_muted)
        .strikethrough();
    assert_eq!(styled_text("hi", &style, None, palette), expected);
}

#[test]
fn styled_text_default_has_no_attributes() {
    let palette = Palette::fastmd_dark();
    assert_eq!(
        styled_text("plain", &TextStyle::default(), None, palette),
        RichText::new("plain")
    );
}

#[test]
fn styled_text_applies_size_when_provided() {
    let palette = Palette::fastmd_dark();
    assert_eq!(
        styled_text("x", &TextStyle::default(), Some(18.0), palette),
        RichText::new("x").size(18.0)
    );
}

#[test]
fn text_format_default_run() {
    let body = FontId::proportional(14.0);
    let mono = FontId::monospace(13.0);
    let c = colors();
    let fmt = text_format(&TextStyle::default(), &body, &mono, c);
    assert_eq!(fmt.font_id, body);
    assert_eq!(fmt.color, c.text);
    assert_eq!(fmt.background, Color32::TRANSPARENT);
    assert!(!fmt.italics);
    assert_eq!(fmt.strikethrough, Stroke::NONE);
}

#[test]
fn text_format_code_uses_mono_and_background() {
    let body = FontId::proportional(14.0);
    let mono = FontId::monospace(13.0);
    let c = colors();
    let style = TextStyle {
        code: true,
        ..Default::default()
    };
    let fmt = text_format(&style, &body, &mono, c);
    assert_eq!(fmt.font_id, mono);
    assert_eq!(fmt.background, c.code_bg);
}

#[test]
fn text_format_bold_uses_strong_colour() {
    let body = FontId::proportional(14.0);
    let mono = FontId::monospace(13.0);
    let c = colors();
    let style = TextStyle {
        bold: true,
        ..Default::default()
    };
    assert_eq!(text_format(&style, &body, &mono, c).color, c.strong);
}

#[test]
fn text_format_muted_overrides_bold_colour() {
    let body = FontId::proportional(14.0);
    let mono = FontId::monospace(13.0);
    let c = colors();
    let style = TextStyle {
        bold: true,
        muted: true,
        ..Default::default()
    };
    assert_eq!(text_format(&style, &body, &mono, c).color, c.muted);
}

#[test]
fn text_format_italic_and_strikethrough() {
    let body = FontId::proportional(14.0);
    let mono = FontId::monospace(13.0);
    let c = colors();
    let italic = TextStyle {
        italic: true,
        ..Default::default()
    };
    assert!(text_format(&italic, &body, &mono, c).italics);

    let strike = TextStyle {
        strikethrough: true,
        ..Default::default()
    };
    assert_eq!(
        text_format(&strike, &body, &mono, c).strikethrough,
        Stroke::new(1.0, c.text)
    );
}

#[test]
fn link_hover_tip_reveals_wikilink_target() {
    assert_eq!(link_hover_tip("wikilink:My Note"), "Note: My Note");
    assert_eq!(link_hover_tip("wikilink:Goals#Q1"), "Note: Goals#Q1");
}

#[test]
fn link_hover_tip_passes_through_other_urls() {
    assert_eq!(link_hover_tip("https://example.com"), "https://example.com");
    assert_eq!(link_hover_tip("doc.md"), "doc.md");
}
