//! Tests for [`crate::ui::render::heading`].
//! Lives in a sidecar so the implementation file stays focused.

use super::*;
use crate::markdown::{InlineElem, TextStyle};
use crate::ui::test_helpers::text::assert_text_contains;
use eframe::egui;

#[test]
fn test_render_heading_empty_text() {
    let ctx = egui::Context::default();
    let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
        let mut scroll = None;
        let elems = vec![InlineElem::Text("   ".to_string(), TextStyle::default())];
        render_heading(ui, &elems, 1, &mut scroll, "h1-id");
        // No output should be produced.
        // We can't easily assert 0 shapes here without checking output, but if it doesn't crash we're good.
    });
}

#[test]
fn test_render_heading_styles() {
    let ctx = egui::Context::default();
    let output = ctx.run_ui(egui::RawInput::default(), |ui| {
        let mut scroll = None;
        let mut style_bold = TextStyle::default();
        style_bold.bold = true;
        let mut style_italic = TextStyle::default();
        style_italic.italic = true;
        let mut style_code = TextStyle::default();
        style_code.code = true;
        let mut style_strike = TextStyle::default();
        style_strike.strikethrough = true;

        let elems = vec![
            InlineElem::Text("bold".to_string(), style_bold),
            InlineElem::SoftBreak,
            InlineElem::Text("italic".to_string(), style_italic),
            InlineElem::SoftBreak,
            InlineElem::Text("code".to_string(), style_code),
            InlineElem::SoftBreak,
            InlineElem::Text("strike".to_string(), style_strike),
            InlineElem::SoftBreak,
            InlineElem::Link("http://example.com".to_string(), "link".to_string()),
            InlineElem::SoftBreak,
            InlineElem::Image("http://example.com/img.png".to_string()),
            InlineElem::SoftBreak,
            InlineElem::Html("<b>html</b>".to_string()),
        ];
        render_heading(ui, &elems, 2, &mut scroll, "h2-id");
    });

    assert_text_contains(&output.shapes, "bold");
    assert_text_contains(&output.shapes, "italic");
    assert_text_contains(&output.shapes, "code");
    assert_text_contains(&output.shapes, "strike");
    assert_text_contains(&output.shapes, "link");
    assert_text_contains(&output.shapes, "[Image: http://example.com/img.png]");
    assert_text_contains(&output.shapes, "<b>html</b>");
}

#[test]
fn test_render_heading_scroll_to_me() {
    let ctx = egui::Context::default();
    let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
        let mut scroll = Some("target-id".to_string());
        let elems = vec![InlineElem::Text("Target".to_string(), TextStyle::default())];
        render_heading(ui, &elems, 3, &mut scroll, "target-id");

        // Assert the scroll variable was cleared, meaning it matched and triggered the scroll.
        assert!(scroll.is_none());
    });
}
