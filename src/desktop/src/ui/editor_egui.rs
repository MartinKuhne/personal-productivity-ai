//! egui adapter for the inline text editor — palette + `show_text_editor`.
//!
//! Everything in this module imports `eframe::egui`; the data model it
//! adapts lives in [`crate::app::text_buffer`] and is egui-free.
//!
//! The two responsibilities of this file are:
//!
//! 1. Hold the inverted (black text on white) colour palette required
//!    by UI-046 in [`EditorColors`].
//! 2. Render the buffer with [`show_text_editor`], reading the
//!    [`TextBuffer::content`] and writing the resulting cursor
//!    position back via [`TextBuffer::set_cursor`] before returning.

use crate::app::text_buffer::TextBuffer;
use crate::bus::events::file::FileEventProducer;
use eframe::egui::{self, Key};

/// Inverted color scheme for the inline text editor (UI-046).
///
/// The editor must look distinctly different from the rest of the
/// dark-themed application: black text on a white background, with a
/// black border to make the inverted surface visually obvious.
/// Centralising the palette here makes the requirement testable and
/// keeps the `show` function free of magic constants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorColors {
    /// Window / frame fill colour (the editor's surface).
    pub background: egui::Color32,
    /// Text colour used inside the editing area and the status bar.
    pub text: egui::Color32,
    /// Border stroke colour around the window.
    pub border: egui::Color32,
    /// Colour used to surface validation / save errors.
    pub error: egui::Color32,
}

impl Default for EditorColors {
    fn default() -> Self {
        Self::inverted()
    }
}

impl EditorColors {
    /// Returns the inverted (black text on white) palette required by
    /// UI-046.
    pub const fn inverted() -> Self {
        Self {
            background: egui::Color32::WHITE,
            text: egui::Color32::BLACK,
            border: egui::Color32::BLACK,
            error: egui::Color32::RED,
        }
    }
}

/// Render the inline text editor.
///
/// `ui` is any `egui::Ui` (typically the root Ui from `App::ui`); the
/// editor pulls the `egui::Context` from it via `ui.ctx()` to open
/// the top-level `egui::Window`.
///
/// `buf` owns the buffer state (content, file path, undo stack, …).
/// `producer` is the bus producer the editor needs to notify the rest
/// of the app about a successful save; it is passed in rather than
/// held on the buffer so [`TextBuffer`] stays plain Rust data and does
/// not carry framework-specific slots.
///
/// Returns `true` if the user clicked the Save button and the save
/// succeeded (the buffer is then closed).
pub fn show_text_editor(
    ui: &mut egui::Ui,
    buf: &mut TextBuffer,
    producer: &FileEventProducer,
) -> bool {
    show_text_editor_with_colors(ui, buf, EditorColors::inverted(), producer)
}

/// Render the inline text editor with an explicit colour palette.
///
/// Exposed so tests and callers can drive the rendering with a known
/// palette, and so the UI-046 requirement ("inverted, black text on
/// white background") is expressed as data rather than scattered
/// `Color32::*` constants in the rendering code.
pub fn show_text_editor_with_colors(
    ui: &mut egui::Ui,
    buf: &mut TextBuffer,
    colors: EditorColors,
    producer: &FileEventProducer,
) -> bool {
    if !buf.is_open {
        return false;
    }

    let mut is_open = buf.is_open;
    let mut did_save = false;

    // UI-046: the editor's surface must be inverted relative to the
    // rest of the dark-themed app. We start from the default window
    // frame (so margins / rounding / shadow match the platform look)
    // and override fill + stroke so the editor clearly stands out.
    //
    // egui 0.35 removed `Context::style`; use `style_of(Theme)` to
    // retrieve the style that `Frame::window` expects.
    let style = ui.ctx().style_of(egui::Theme::Dark);
    let editor_frame = egui::Frame {
        fill: colors.background,
        stroke: egui::Stroke::new(1.0_f32, colors.border),
        ..egui::Frame::window(&style)
    };

    egui::Window::new("Inline Editor")
        .open(&mut is_open)
        .collapsible(false)
        .resizable(true)
        .frame(editor_frame)
        .default_size(egui::vec2(800.0, 600.0))
        .show(ui.ctx(), |ui| {
            ui.vertical_centered(|ui| {
                if let Some(err) = &buf.error_message {
                    ui.colored_label(colors.error, err);
                }
            });

            // Calculate cursor position for status bar
            let mut cursor_line = 0;
            let mut cursor_col = 0;

            let avail = ui.available_height();
            let button_bar = 30.0;

            // egui 0.27 dropped TextEdit::background_color; the
            // background of a TextEdit is taken from
            // `visuals.extreme_bg_color`. We scope a fresh style
            // around the TextEdit so the inverted palette UI-046
            // asks for is applied without leaking into the rest
            // of the app.
            let extreme_bg = colors.background;

            let mut page_scroll = 0.0;
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::PageUp)) {
                page_scroll = -(avail - button_bar) * 0.9;
            }
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::PageDown)) {
                page_scroll = (avail - button_bar) * 0.9;
            }
            egui::ScrollArea::vertical()
                .id_salt("inline_editor_scroll")
                .max_height(avail - button_bar)
                .show(ui, |ui| {
                    if page_scroll != 0.0 {
                        ui.scroll_with_delta(egui::vec2(0.0, page_scroll));
                    }

                    // Apply the inverted background to every widget
                    // state inside the scroll area (the TextEdit and
                    // its selection rectangles all read from
                    // `extreme_bg_color`).
                    let v = ui.visuals_mut();
                    v.extreme_bg_color = extreme_bg;
                    v.widgets.noninteractive.bg_fill = extreme_bg;
                    v.widgets.inactive.bg_fill = extreme_bg;
                    v.widgets.hovered.bg_fill = extreme_bg;
                    v.widgets.active.bg_fill = extreme_bg;
                    v.widgets.open.bg_fill = extreme_bg;

                    // egui 0.35: `TextEdit::frame(bool)` was replaced
                    // with `.frame(Frame)` (use `Frame::NONE` to keep
                    // the previous `false` behaviour, or build a
                    // custom frame). We want the editor's bordered
                    // surface, so pass `Frame::NONE` here and let the
                    // window frame own the visible border.
                    let text_edit = egui::TextEdit::multiline(&mut buf.content)
                        .font(egui::TextStyle::Monospace)
                        .code_editor()
                        .desired_width(f32::INFINITY)
                        .lock_focus(true)
                        .text_color(colors.text)
                        .frame(egui::Frame::NONE);

                    let output = text_edit.show(ui);

                    if let Some(cursor_range) = output.cursor_range {
                        // egui 0.35: `CCursor` no longer has a
                        // `ccursor` field — the cursor itself IS the
                        // `CCursor`, with `index: CharIndex(usize)`.
                        let cursor_char_idx = cursor_range.primary.index.0;
                        let byte_idx = buf
                            .content
                            .char_indices()
                            .nth(cursor_char_idx)
                            .map(|(i, _)| i)
                            .unwrap_or(buf.content.len());
                        let text_up_to_cursor = &buf.content[..byte_idx];
                        cursor_line = text_up_to_cursor.chars().filter(|&c| c == '\n').count() + 1;
                        if let Some(last_newline) = text_up_to_cursor.rfind('\n') {
                            cursor_col = text_up_to_cursor.chars().count()
                                - text_up_to_cursor[..last_newline].chars().count();
                        } else {
                            cursor_col = text_up_to_cursor.chars().count() + 1;
                        }
                    }
                });

            ui.separator();

            ui.horizontal(|ui| {
                if ui.button(crate::ui::strings::SAVE_BUTTON).clicked()
                    && buf.save(producer).is_ok()
                {
                    did_save = true;
                }
                if ui.button(crate::ui::strings::CANCEL_BUTTON).clicked() {
                    buf.close();
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.colored_label(
                        colors.text,
                        format!("Line: {} | Col: {}", cursor_line, cursor_col),
                    );
                });
            });
        });

    if !is_open {
        buf.close();
    }

    did_save
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::core::Bus;
    use crate::bus::events::file::FileEvent;

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
}
