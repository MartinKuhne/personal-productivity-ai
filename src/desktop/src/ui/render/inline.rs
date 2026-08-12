//! Inline markdown rendering — paints a list of `InlineElem` runs into the
//! supplied `egui::Ui`. Two layers:
//!
//! - `render_inline` is the entry point; it wraps the body in
//!   `horizontal_wrapped` and short-circuits when the cell is empty.
//! - `render_inline_inner` does the actual painting (bullet/checkbox/runs).
//!
//! The file is intentionally leaf-level: it shares no mutable state with
//! `render_markdown` beyond the `pending_toggles` sink the caller passes
//! in, and it depends only on the `crate::markdown` re-exports and
//! `egui::RichText`. No table layout, no heading id bookkeeping, no
//! viewport math.

use crate::markdown::InlineElem;
use eframe::egui;
use egui::RichText;

/// Purpose: Renders inline markdown elements.
///
/// Inputs: `ui` (mut), `elems`, `needs_bullet`, `task_checked`, `indent`, `wrap`
/// Outputs: None
/// Purity: Impure (modifies UI state). Thin adapter for rendering text.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_inline(
    ui: &mut egui::Ui,
    elems: &[InlineElem],
    needs_bullet: bool,
    task_checked: Option<bool>,
    indent: usize,
    list_ordinal: Option<u64>,
    task_index: usize,
    pending_toggles: &mut Vec<(usize, bool)>,
) {
    if elems.is_empty() && !needs_bullet && task_checked.is_none() {
        return;
    }

    // P1-2: Pin `main_align: Min` so wrapped continuation lines are
    // left-aligned, not centered. egui 0.35's `ui.horizontal_wrapped`
    // is implemented as
    //   `allocate_ui_with_layout_dyn(
    //      vec2(available_width, interact_size.y),
    //      Layout::left_to_right(Align::Center).with_main_wrap(true),
    //      ...,
    //   )`
    // where `Align::Center` is the `cross_align` (vertical) and
    // `main_align` is hardcoded to `Align::Center` (horizontal). The
    // hardcoded `main_align: Center` centers children along the main
    // axis within each line — for a wrapped continuation line narrower
    // than the line width, the centered placement can land the text at
    // a positive x offset, making the leftmost characters fall outside
    // the visible left edge of the scroll viewport (the "text cut off
    // on the left on subsequent lines" symptom in the agent response
    // window). This block reproduces the exact `ui.horizontal_wrapped`
    // allocation (same `initial_size`, same layout fields) but with
    // `main_align: Min` via `.with_main_align(Align::Min)`. The
    // `initial_size` must match `horizontal_wrapped`'s exactly — a
    // smaller allocation causes the test
    // `render_markdown_no_offscreen_text_across_viewports` to
    // regress by 2 px (the off-viewport text moves down by `interact_size.y`).
    // Pinned by `test_render_inline_wrapped_rows_left_aligned` and
    // `test_render_markdown_wrapped_paragraph_left_aligned` in
    // `render/e2e_tests/render_smoke.rs`.
    let initial_size = egui::vec2(
        ui.available_size_before_wrap().x,
        ui.spacing().interact_size.y,
    );
    ui.allocate_ui_with_layout(
        initial_size,
        egui::Layout::left_to_right(egui::Align::Center)
            .with_main_align(egui::Align::Min)
            .with_main_wrap(true),
        |ui| {
            render_inline_inner(
                ui,
                elems,
                needs_bullet,
                task_checked,
                indent,
                list_ordinal,
                task_index,
                pending_toggles,
            );
        },
    );
}

/// Inner inline rendering — actually paints the styled `InlineElem` runs.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_inline_inner(
    ui: &mut egui::Ui,
    elems: &[InlineElem],
    needs_bullet: bool,
    task_checked: Option<bool>,
    indent: usize,
    list_ordinal: Option<u64>,
    task_index: usize,
    pending_toggles: &mut Vec<(usize, bool)>,
) {
    ui.spacing_mut().item_spacing.x = 0.0;

    if indent > 0 {
        ui.add_space(indent as f32 * 20.0);
    }
    // P0-3: Render ordered list ordinals instead of bullets.
    if needs_bullet {
        if let Some(n) = list_ordinal {
            ui.label(RichText::new(format!("{}. ", n)).size(14.0));
        } else {
            ui.label(RichText::new("• ").size(14.0));
        }
    }
    if let Some(checked) = task_checked {
        ui.add_space(4.0);
        let mut c = checked;
        let resp = ui.checkbox(&mut c, "");
        // P0-2: Write back the toggle result instead of discarding it.
        // The caller drains `pending_toggles` after rendering and applies
        // them to the markdown source.
        if resp.changed() {
            pending_toggles.push((task_index, c));
        }
        ui.add_space(4.0);
    }

    for elem in elems {
        match elem {
            InlineElem::Text(t, style) => {
                let mut rt = RichText::new(t);
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
                if style.muted {
                    rt = rt.color(egui::Color32::from_rgb(160, 160, 160));
                }
                if style.strikethrough {
                    rt = rt.strikethrough();
                }
                // P1: egui 0.35 `Label::new` defaults `wrap_mode` to
                // `None`; wrap is only inherited if the parent layout
                // is vertical or horizontal+main_wrap AND the available
                // width is finite — a fragile contract. The other text
                // paths in this file (`render_code_block`,
                // `render_table_cell`, `render_yaml_table`) already pin
                // `.wrap()` explicitly for the same reason. Pinned by
                // `test_render_markdown_long_paragraph_wraps_in_preview`.
                ui.add(egui::Label::new(rt).wrap());
            }
            InlineElem::Link(url, text) => {
                ui.hyperlink_to(text, url);
            }
            InlineElem::Image(url) => {
                // Same egui 0.35 wrap-mode default-off hazard as
                // `InlineElem::Text`; pin the wrap explicitly.
                ui.add(egui::Label::new(format!("[Image: {}]", url)).wrap());
            }
            InlineElem::Html(html) => {
                // Same egui 0.35 wrap-mode default-off hazard as
                // `InlineElem::Text`; pin the wrap explicitly.
                ui.add(
                    egui::Label::new(RichText::new(html).italics().color(egui::Color32::GRAY))
                        .wrap(),
                );
            }
            InlineElem::SoftBreak => {
                ui.label(" ");
            }
        }
    }
}
