//! Heading rendering — paints a heading at the level-appropriate font
//! size and handles the "scroll-to-me" side effect when the caller
//! passes a `scroll_to_id_str` matching this heading's stable id.
//!
//! The function is leaf-level: it only renders the heading, it does
//! not compute the heading id (the caller pre-computes it and passes
//! the `heading_id_str` argument). The id computation lives in
//! `render_markdown` (in `mod.rs`) because it needs the
//! `HashMap<String, usize>` occurrence counter that is also used by
//! `build_toc`.

use crate::markdown::{InlineElem, heading_plain_text};
use eframe::egui;
use egui::RichText;

/// Purpose: Renders a heading.
///
/// Inputs: `ui` (mut), `elems` (heading inline elements), `level`,
/// `scroll_to_id_str` (mut, the stable string id of the heading
/// the user wants to scroll to), `heading_id_str` (pre-computed
/// stable string id for this heading).
///
/// Purity: Impure (modifies UI state). Thin adapter.
pub(crate) fn render_heading(
    ui: &mut egui::Ui,
    elems: &[InlineElem],
    level: u32,
    scroll_to_id_str: &mut Option<String>,
    heading_id_str: &str,
) {
    let plain = heading_plain_text(elems);
    let trimmed = plain.trim().to_string();
    if trimmed.is_empty() {
        return;
    }
    let size = match level {
        1 => 32.0,
        2 => 24.0,
        3 => 18.0,
        4 => 14.0,
        _ => 12.0,
    };
    // Render the styled inline elements with the heading's size.
    // Use `horizontal_wrapped` so long headings wrap instead of
    // overflowing horizontally, and zero `item_spacing.x` to avoid
    // spurious gaps between styled spans (matching `render_inline`).
    let response = ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for elem in elems {
            match elem {
                InlineElem::Text(t, style) => {
                    let mut rt = RichText::new(t).size(size);
                    // Respect the heading's TextStyle.bold instead of
                    // unconditionally applying .strong().
                    if style.bold {
                        rt = rt.strong();
                    }
                    if style.italic {
                        rt = rt.italics();
                    }
                    if style.code {
                        rt = rt
                            .monospace()
                            .background_color(egui::Color32::from_gray(40));
                    }
                    if style.strikethrough {
                        rt = rt.strikethrough();
                    }
                    ui.label(rt);
                }
                InlineElem::Link(url, text) => {
                    ui.hyperlink_to(egui::RichText::new(text).size(size), url);
                }
                InlineElem::Image(url) => {
                    ui.label(RichText::new(format!("[Image: {}]", url)).size(size));
                }
                InlineElem::Html(h) => {
                    ui.label(
                        RichText::new(h)
                            .size(size)
                            .italics()
                            .color(egui::Color32::GRAY),
                    );
                }
                InlineElem::SoftBreak => {
                    ui.label(RichText::new(" ").size(size));
                }
            }
        }
    });
    if scroll_to_id_str.as_deref() == Some(heading_id_str) {
        response.response.scroll_to_me(Some(egui::Align::Center));
        *scroll_to_id_str = None;
    }
    ui.add_space(4.0);
}
