//! Unit tests for [`super::text_width`].
use super::*;
use crate::ui::test_helpers::run_ui_test;

#[test]
fn text_width_grows_with_text_length() {
    let ctx = egui::Context::default();
    let _ = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
        let font = egui::FontId::proportional(14.0);
        let short = text_width(ui.ctx(), "a", font.clone(), egui::Color32::WHITE);
        let long = text_width(ui.ctx(), "aaaaaaaaaa", font.clone(), egui::Color32::WHITE);
        assert!(short > 0.0, "single glyph must have positive width");
        assert!(long > short, "longer text must measure wider");
    });
}

#[test]
fn text_width_of_empty_string_is_zero() {
    let ctx = egui::Context::default();
    let _ = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
        let w = text_width(
            ui.ctx(),
            "",
            egui::FontId::proportional(14.0),
            egui::Color32::WHITE,
        );
        assert_eq!(w, 0.0);
    });
}
