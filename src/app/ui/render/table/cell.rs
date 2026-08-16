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
//!
//! **Multi-element cells:** Cells with multiple `InlineElem` entries
//! (e.g. `~~strikethrough~~ **bold**` parsed as `[Text(strikethrough), Text(" "), Text(bold)]`)
//! are rendered as a single inline `LayoutJob` rather than as vertically-
//! stacked labels. Stacking the elements as separate `Label` widgets under
//! `top_down` made the cell as wide as the *widest single token* of any
//! element (e.g. `$180–$210` at ~107 px) and ignored `set_max_width(inner_w)`,
//! because a single `Label` with no whitespace in its text cannot wrap and
//! the placer reports the label's natural width — which then overflowed the
//! column by ~10 px and pushed every subsequent cell in the same row right
//! by the same amount. Inline `LayoutJob` rendering makes the entire cell
//! content flow as a single wrapped line within `inner_w`, matching the
//! "one styled run after another, wrap at whitespace" semantic the
//! cell measurer already assumes when it sums per-element widths into
//! `max_content`. Cells that contain non-text elements (`Link`, `Image`,
//! `Html`) fall back to the per-element rendering — they cannot be
//! represented as `TextFormat` runs in a `LayoutJob`.

use crate::markdown::InlineElem;
use crate::ui::table_width::TablePadding;
use eframe::egui;
use egui::RichText;
use egui::epaint::text::{LayoutJob, TextWrapping};

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

        if is_text_only(cell) {
            // Multi-element text cells render as a single inline
            // `LayoutJob` so styled runs flow inline and the whole
            // line wraps within `inner_w`. See module-level note.
            let job = build_text_layout_job(cell, ui, inner_w);
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
            ui.set_min_width(inner_w);
            ui.set_max_width(inner_w);
            let content_res = ui.add(egui::Label::new(job).wrap());
            return content_res.rect.height() + pad_v;
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
        // `TBL-030`/`TBL-031`: `left_to_right(Align::TOP)` anchors Y at the cursor top
        // and flows elements inline so Link + Text content (e.g. "[Link](url) · **(555) 123-4567**")
        // renders on one line with wrap at the column boundary instead of
        // stacking each element vertically which, with `top_down` +
        // `main_wrap(true)` + a small cell height, forces a multi-column
        // wrap that renders each character on its own line at 210 px tall.
        let content_res = ui.with_layout(
            egui::Layout::left_to_right(egui::Align::TOP).with_main_wrap(true),
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

/// Returns `true` when the cell contains only text-like elements
/// (`Text` and `SoftBreak`). Cells with `Link`, `Image`, or `Html`
/// fall back to the per-element rendering path because those
/// variants are not representable as `TextFormat` runs in a
/// `LayoutJob`.
fn is_text_only(cell: &[InlineElem]) -> bool {
    cell.iter()
        .all(|e| matches!(e, InlineElem::Text(_, _) | InlineElem::SoftBreak))
}

/// Build a single inline `LayoutJob` from the cell's text-like
/// elements. Each `InlineElem::Text` becomes one `LayoutSection`
/// with the appropriate `TextFormat` (color for bold, italics,
/// strikethrough stroke, monospace background for code). The job
/// wraps at `max_width = inner_w`, so the cell cannot overflow its
/// column even if a single unbreakable token would otherwise
/// exceed `inner_w` (the FTWA `w_j >= min_content` invariant still
/// guarantees the wrap at the nearest word boundary).
///
/// `SoftBreak` is rendered as a literal `' '` so the
/// intra-cell spacing is preserved.
fn build_text_layout_job(cell: &[InlineElem], ui: &egui::Ui, inner_w: f32) -> LayoutJob {
    let body_font = ui
        .style()
        .text_styles
        .get(&egui::TextStyle::Body)
        .cloned()
        .unwrap_or_else(|| egui::FontId::proportional(14.0));
    let mono_font = ui
        .style()
        .text_styles
        .get(&egui::TextStyle::Monospace)
        .cloned()
        .unwrap_or_else(|| egui::FontId::monospace(13.0));
    let visuals = ui.visuals();
    let base_color = visuals.text_color();
    let strong_color = visuals.strong_text_color();
    let strikethrough_stroke = egui::Stroke {
        width: 1.0,
        color: base_color,
    };
    let code_bg = visuals.code_bg_color;

    let mut job = LayoutJob {
        wrap: TextWrapping {
            max_width: inner_w,
            max_rows: usize::MAX,
            break_anywhere: false,
            overflow_character: Some('\u{2026}'),
        },
        break_on_newline: true,
        halign: egui::Align::LEFT,
        ..Default::default()
    };
    for elem in cell {
        match elem {
            InlineElem::Text(t, style) => {
                let font = if style.code {
                    mono_font.clone()
                } else {
                    body_font.clone()
                };
                let color = if style.bold { strong_color } else { base_color };
                let background = if style.code {
                    code_bg
                } else {
                    egui::Color32::TRANSPARENT
                };
                let format = egui::epaint::text::TextFormat {
                    font_id: font,
                    color,
                    background,
                    italics: style.italic,
                    strikethrough: if style.strikethrough {
                        strikethrough_stroke
                    } else {
                        egui::Stroke::NONE
                    },
                    ..Default::default()
                };
                job.append(t, 0.0, format);
            }
            InlineElem::SoftBreak => {
                let format = egui::epaint::text::TextFormat {
                    font_id: body_font.clone(),
                    color: base_color,
                    ..Default::default()
                };
                job.append(" ", 0.0, format);
            }
            _ => unreachable!("is_text_only guarantees only Text/SoftBreak"),
        }
    }
    job
}
