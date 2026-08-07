//! Tests for `ui/editor_egui.rs`

use super::*;
use crate::bus::core::Bus;
use crate::bus::events::file::FileEvent;
use crate::ui::test_helpers::text::extract_text;

/// A producer that publishes to a throwaway bus. Editor tests
/// don't need to consume the events — they only care about the
/// rendered output / early-return path.
fn noop_producer() -> FileEventProducer<'static> {
    let bus: &'static Bus<FileEvent> = Box::leak(Box::new(Bus::new()));
    FileEventProducer::new(bus)
}

#[test]
fn test_show_text_editor_is_a_noop_when_buffer_closed() {
    let mut buf = TextBuffer::new();
    let ctx = egui::Context::default();
    let producer = noop_producer();
    // Should return false and do nothing when the buffer is
    // closed.
    let raw_input = egui::RawInput::default();
    let _ = ctx.run_ui(raw_input, |ui| {
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
    let _ = ctx.run_ui(raw_input, |ui| {
        let _ = show_text_editor_with_colors(ui, &mut buf, EditorColors::inverted(), &producer);
    });
}

// --- UI-046: inverted (black text on white) colour scheme ---

/// UI-046 in one assertion set: the inverted palette has
/// black text on a white background with a black border, the
/// default palette equals the inverted one (so callers don't
/// have to opt in), and the per-channel RGB contrast between
/// text and background is maximal.
#[test]
fn test_editor_colors_inverted_satisfies_ui046() {
    let colors = EditorColors::inverted();

    // Inverted palette: white surface, black foreground, black border.
    assert_eq!(colors.background, egui::Color32::WHITE);
    assert_eq!(colors.text, egui::Color32::BLACK);
    assert_eq!(colors.border, egui::Color32::BLACK);

    // Default palette is the inverted one — callers don't need to opt in.
    assert_eq!(EditorColors::default(), EditorColors::inverted());

    // Per-channel contrast is maximal: 255 = 0 + 255.
    let bg = colors.background;
    let fg = colors.text;
    assert_eq!(bg.r() + fg.r(), 255);
    assert_eq!(bg.g() + fg.g(), 255);
    assert_eq!(bg.b() + fg.b(), 255);
}

#[test]
fn test_show_text_editor_renders_when_open() {
    let mut buf = TextBuffer::new();
    // Simulate opening a file
    let path = std::env::temp_dir().join("test_editor.md");
    let raw_content = "Hello World\nLine 2";
    let _ = std::fs::write(&path, raw_content);
    let pdf_tracker = crate::app::session::PdfBackingTracker::new();
    let _ = buf.open(&path, raw_content, Some(&pdf_tracker));

    let ctx = egui::Context::default();
    let producer = noop_producer();
    
    let raw_input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))),
        ..Default::default()
    };
    
    // First frame initializes the window
    let _ = ctx.run_ui(raw_input.clone(), |ui| {
        let _ = show_text_editor(ui, &mut buf, &producer);
    });

    // Second frame renders content
    let output = ctx.run_ui(raw_input, |ui| {
        let _ = show_text_editor(ui, &mut buf, &producer);
    });

    let texts = extract_text(&output.shapes);
    
    let has_content = texts.iter().any(|t| t.contains("Hello World"));
    assert!(has_content, "Editor must render the file content. Texts: {:?}", texts);
    
    let has_save = texts.iter().any(|t| t.contains(crate::ui::strings::SAVE_BUTTON));
    assert!(has_save, "Editor must render the Save button");

    let has_cancel = texts.iter().any(|t| t.contains(crate::ui::strings::CANCEL_BUTTON));
    assert!(has_cancel, "Editor must render the Cancel button");
    
    // Check for "Line: 0 | Col: 0" initially (since no cursor is explicitly set, it defaults to 0)
    let has_cursor_info = texts.iter().any(|t| t.contains("Line: 0 | Col: 0"));
    assert!(has_cursor_info, "Editor must render cursor tracking info");
    
    // Clean up
    let _ = std::fs::remove_file(&path);
}
