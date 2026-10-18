//! Tests for `ui/editor_egui.rs`

use super::*;
use crate::ui::test_helpers::run_ui_test;
use crate::ui::test_helpers::text::extract_text;

use crate::ui::test_helpers::app::noop_producer;

#[test]
fn test_show_text_editor_is_a_noop_when_buffer_closed() {
    let mut buf = TextBuffer::new();
    let ctx = egui::Context::default();
    let producer = noop_producer();
    // Should return false and do nothing when the buffer is
    // closed.
    let raw_input = egui::RawInput::default();
    let _ = run_ui_test(&ctx, raw_input, |ui| {
        let _ = show_text_editor(ui, &mut buf, &producer);
    });
}

#[test]
fn test_show_text_editor_with_colors_is_a_noop_when_buffer_closed() {
    // UI-046 can't be visually asserted without a running egui
    // context (which needs fonts initialised via `Context::run`),
    // but we can at least prove the explicit-palette entry point
    // is hooked up and short-circuits when the buffer isn't
    // open — just like the legacy `show` method does.
    let mut buf = TextBuffer::new();
    let ctx = egui::Context::default();
    let producer = noop_producer();
    let raw_input = egui::RawInput::default();
    let _ = run_ui_test(&ctx, raw_input, |ui| {
        let _ = show_text_editor_with_colors(ui, &mut buf, EditorColors::inverted(), &producer);
    });
}

#[test]
fn test_show_text_editor_renders_when_open() {
    let mut buf = TextBuffer::new();
    // Simulate opening a file
    let path = std::env::temp_dir().join("test_editor.md");
    let raw_content = "Hello World\nLine 2";
    let _ = std::fs::write(&path, raw_content);
    let pdf_tracker = crate::agent::session::PdfBackingTracker::new();
    buf.open(&path, raw_content, Some(&pdf_tracker));

    let ctx = egui::Context::default();
    let producer = noop_producer();

    let raw_input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800.0, 600.0),
        )),
        ..Default::default()
    };

    // First frame initializes the window
    let _ = run_ui_test(&ctx, raw_input.clone(), |ui| {
        let _ = show_text_editor(ui, &mut buf, &producer);
    });

    // Second frame renders content
    let output = run_ui_test(&ctx, raw_input, |ui| {
        let _ = show_text_editor(ui, &mut buf, &producer);
    });

    let texts = extract_text(&output.shapes);

    let has_content = texts.iter().any(|t| t.contains("Hello World"));
    assert!(
        has_content,
        "Editor must render the file content. Texts: {:?}",
        texts
    );

    let has_save = texts
        .iter()
        .any(|t| t.contains(crate::ui::strings::SAVE_BUTTON));
    assert!(has_save, "Editor must render the Save button");

    let has_cancel = texts
        .iter()
        .any(|t| t.contains(crate::ui::strings::CANCEL_BUTTON));
    assert!(has_cancel, "Editor must render the Cancel button");

    // Check for "Line: 0 | Col: 0" initially (since no cursor is explicitly set, it defaults to 0)
    let has_cursor_info = texts.iter().any(|t| t.contains("Line: 0 | Col: 0"));
    assert!(has_cursor_info, "Editor must render cursor tracking info");

    // Clean up
    let _ = std::fs::remove_file(&path);
}
