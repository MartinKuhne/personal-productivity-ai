//! Tests for [`crate::ui::test_helpers::text`].
//! Lives in a sidecar so the implementation file stays focused.

use super::*;
use eframe::egui;

fn create_text_clipped_shape(text: &str) -> egui::epaint::ClippedShape {
    let ctx = egui::Context::default();
    let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
    output.textures_delta.clear();
    let galley = ctx.fonts_mut(|f| {
        f.layout_no_wrap(
            text.to_string(),
            egui::FontId::proportional(14.0),
            egui::Color32::WHITE,
        )
    });
    let text_shape = egui::epaint::TextShape::new(egui::Pos2::ZERO, galley, egui::Color32::WHITE);
    egui::epaint::ClippedShape {
        clip_rect: egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::ZERO),
        shape: egui::Shape::Text(text_shape),
    }
}

#[test]
fn test_extract_text() {
    let shape1 = create_text_clipped_shape("Hello");
    let shape2 = create_text_clipped_shape("World");
    let shapes = vec![shape1, shape2];
    let extracted = extract_text(&shapes);
    assert_eq!(extracted.len(), 2);
    assert_eq!(extracted[0], "Hello");
    assert_eq!(extracted[1], "World");
}

#[test]
fn test_assert_text_contains_pass() {
    let shape = create_text_clipped_shape("Hello World");
    assert_text_contains(&[shape], "World");
}

#[test]
#[should_panic(expected = "expected rendered output to contain \"Missing\"")]
fn test_assert_text_contains_fail() {
    let shape = create_text_clipped_shape("Hello World");
    assert_text_contains(&[shape], "Missing");
}

#[test]
fn test_assert_text_contains_all_pass() {
    let shape1 = create_text_clipped_shape("Hello World");
    let shape2 = create_text_clipped_shape("Foo Bar");
    assert_text_contains_all(&[shape1, shape2], &["World", "Foo"]);
}

#[test]
#[should_panic(expected = "expected rendered output to contain \"Missing\"")]
fn test_assert_text_contains_all_fail() {
    let shape1 = create_text_clipped_shape("Hello World");
    let shape2 = create_text_clipped_shape("Foo Bar");
    assert_text_contains_all(&[shape1, shape2], &["World", "Missing"]);
}
