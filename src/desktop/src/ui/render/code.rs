//! Code-block rendering — paints a fenced markdown code block as a
//! monospaced, dark-background, scrollable panel with a copy-to-clipboard
//! button. Two functions:
//!
//! - `render_code_block` paints the panel and the copy button.
//! - `copy_code_to_output` is the side-effecting helper invoked by the
//!   button. Extracted so it can be unit-tested without driving a click
//!   event (the button's Tier 4 click test is `#[ignore]`d until
//!   `egui_kittest` is available — see `doc/planning/egui-testing.md`).
//!
//! The file is intentionally leaf-level: no table layout, no heading
//! ids, no inline-style composition. It depends only on `egui` and the
//! `RichText` helper.

use eframe::egui;
use egui::RichText;

/// Purpose: Renders a code block.
///
/// Inputs: `ui` (mut), `content`
///
/// Purity: Impure (modifies UI state). Thin adapter.
pub(crate) fn render_code_block(ui: &mut egui::Ui, language: Option<&str>, content: &str) {
    egui::Frame::NONE
        .fill(egui::Color32::from_rgb(20, 20, 22))
        .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_gray(40)))
        .inner_margin(8.0)
        .corner_radius(4.0)
        .show(ui, |ui| {
            if let Some(lang) = language
                && !lang.is_empty()
            {
                ui.label(RichText::new(lang).weak().small());
            }
            // Constrain the wrapping label's width so the copy button
            // always has room, while computing content height dynamically.
            ui.horizontal_top(|ui| {
                let button_width = 30.0;
                let label_width = (ui.available_width() - button_width).max(0.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(label_width, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.add(egui::Label::new(RichText::new(content).monospace()).wrap());
                    },
                );
                if ui.button("📋").on_hover_text("Copy code").clicked() {
                    copy_code_to_output(ui, content);
                }
            });
        });
}

/// Copy the supplied content to the UI's `copied_text` output.
///
/// Extracted from the copy-code button's click handler so the side
/// effect is testable without driving a click (the button's
/// Tier 4 click test is `#[ignore]`d until `egui_kittest` is
/// available; see the open question in `doc/planning/egui-testing.md`).
pub(crate) fn copy_code_to_output(ui: &mut egui::Ui, content: &str) {
    // egui 0.35: `PlatformOutput::copied_text` was removed. Use the
    // dedicated `Ui::copy_text` helper, which routes through the
    // context's `PlatformOutput` for us.
    ui.copy_text(content.to_string());
}
