//! Single-cell rendering for markdown tables.
//!
//! When `pinned_width` is `Some(w)`, the cell Ui is clamped to exactly `w`
//! pixels (`ui.set_width`) and text is laid out with `horizontal_wrapped` +
//! `Label::wrap(true)` so that multi-word cells wrap at whitespace. This is the
//! FTWA-pinned mode (`crate::ui::table_width`). The FTWA invariant
//! `w >= min_content >= longest-token` guarantees no unbreakable token is ever
//! split or clipped — only inter-word whitespace wraps.
//!
//! When `pinned_width` is `None` (the §3.6 fallback path), the cell uses
//! `ui.horizontal` (no wrap) so the cell reports its full single-line intrinsic
//! width to the parent table layout; any overflow is handled by the wrapping
//! `ScrollArea` (current pre-FTWA behaviour).
//!
//! **Padding (TBL-032, TBL-033):** `padding` is resolved per-cell by the caller
//! via `resolve_padding(global, per_column, per_cell)` and applied as the
//! cell frame's `inner_margin`. `padding.horizontal()` is folded into the
//! column's `max_content`/`min_content` by `measure_cached`, so the FTWA-assigned
//! `pinned_width` already accounts for it; the inner layout width is therefore
//! `pinned_width - padding.horizontal()` so text fits inside the padded frame.

use crate::markdown::InlineElem;
use crate::ui::table_width::TablePadding;
use eframe::egui;
use egui::RichText;

/// Purpose: Renders a single table cell, always emitting at least one widget.
pub(super) fn render_table_cell(
    ui: &mut egui::Ui,
    cell: &[InlineElem],
    pinned_width: Option<f32>,
    pinned_height: Option<f32>,
    padding: TablePadding,
) -> f32 {
    let pad = padding.sanitised();
    let pad_h = pad.horizontal();
    let pad_v = pad.vertical();
    // `egui::Margin` stores `i8` fields, so clamp each side to the
    // representable range after rounding. Realistic paddings are small
    // (a few to a few dozen logical px), so this loses no precision in
    // practice and keeps `Margin`'s compact layout intact.
    let to_i8 = |v: f32| -> i8 { v.round().clamp(0.0, 127.0) as i8 };
    let cell_frame = egui::Frame::NONE
        .fill(egui::Color32::TRANSPARENT)
        .stroke(egui::Stroke::NONE)
        .inner_margin(egui::Margin {
            left: to_i8(pad.left),
            right: to_i8(pad.right),
            top: to_i8(pad.top),
            bottom: to_i8(pad.bottom),
        })
        .outer_margin(egui::Margin::ZERO);

    let avail_w = ui.available_width();
    let inner_w = match pinned_width {
        Some(w) => (w - pad_h).max(0.0),
        None => (avail_w - pad_h).max(0.0),
    };

    let frame_res = cell_frame.show(ui, |ui| {
        let min_h = ui.text_style_height(&egui::TextStyle::Body);

        if cell.is_empty() {
            // Use `allocate_space` rather than `set_min_size` so we reserve
            // pixels without inflating the frame's `min_rect` (which every
            // ancestor `push_id` accumulates). `set_min_size` propagates
            // upward and double-counts cell height in `row_max_h`, producing
            // a phantom ~17 px gutter per row in tables with many empty cells
            // (e.g. the sports-car comparison table). Pinned by
            // `test_table_empty_cells_no_phantom_row_height`.
            let inner_target = (pinned_height.unwrap_or(min_h) - pad_v).max(min_h);
            let _ = ui.allocate_space(egui::vec2(inner_w, inner_target));
            return min_h + pad_v;
        }

        let content = |ui: &mut egui::Ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for elem in cell {
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
                        if style.strikethrough {
                            rt = rt.strikethrough();
                        }
                        ui.add(egui::Label::new(rt).wrap());
                    }
                    InlineElem::Link(url, text) => {
                        ui.hyperlink_to(text, url);
                    }
                    InlineElem::Image(url) => {
                        ui.label(format!("[Image: {}]", url));
                    }
                    InlineElem::Html(html) => {
                        ui.label(RichText::new(html).italics().color(egui::Color32::GRAY));
                    }
                    InlineElem::SoftBreak => {
                        ui.label(" ");
                    }
                }
            }
        };

        // Render text content tightly packed at top of cell without min_height constraint.
        // `TBL-030`/`TBL-031`: `top_down(Align::Min)` anchors Y at the cursor top
        // (Align::Min is the cross-axis = horizontal → LEFT aligned rows);
        // `with_main_wrap(true)` lets individual label widgets wrap within `inner_w`.
        //
        // Previously `left_to_right(Align::Min).with_main_wrap(true)` was used.
        // In egui a `left_to_right` layout places Align::Min on the *cross* axis
        // (vertical), but the cursor is not re-anchored to the cell top when the
        // outer `allocate_ui_with_layout` reserves a height larger than the content
        // — short cells appeared vertically centred when sharing a row with a
        // taller cell. Pinned by `test_table_single_line_cell_text_at_row_top`.
        let content_res = ui.with_layout(
            egui::Layout::top_down(egui::Align::Min).with_main_wrap(true),
            |ui| {
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                ui.set_min_width(inner_w);
                ui.set_max_width(inner_w);
                content(ui);
            },
        );

        let unconstrained_text_h = content_res.response.rect.height();

        // `pinned_height` is reserved up front by the caller on the *cell* Ui
        // (see `cell_size.y` in `render_table_with_config`), so the cell frame
        // here is already inside a `pinned_height`-tall cell. No inner
        // `set_min_height`/`set_max_height` is needed: those calls would
        // cause the cell's `min_rect` to be remembered by every ancestor
        // `push_id`, which would double-count the cell height when advancing
        // the row cursor and inflate the inter-row gutter to ~17 px (the
        // "row gutter too large" bug).

        unconstrained_text_h + pad_v
    });

    frame_res.inner
}
