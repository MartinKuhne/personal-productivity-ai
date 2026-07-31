//! Markdown table rendering with a [`TableRenderConfig`].
//!
//! Per-column pixel widths are computed by `crate::ui::table_width::ftwa`
//! from egui-shaped max-content and min-content measurements; cells are
//! then pinned to their assigned width so the child Ui's available_width
//! matches the column width (see
//! `doc/planning/table-column-width-algorithm.md` §5 decision Q5). When
//! the available width falls below the sum of min-content
//! (`decision.needs_horizontal_scroll`), the table physically cannot fit
//! even with every column at its longest-token floor and we fall back to
//! the prior behaviour: a wrapping `ScrollArea` over max-content columns
//! (doc §3.6) so the strongest invariant — never split a token — is
//! preserved.
//!
//! Column spacing is 10.0 px and row spacing is 4.0 px. The available content
//! width passed to FTWA subtracts `(N - 1) * 10.0` for those gutters so the
//! assigned widths sum to the true content rect.
//!
//! The whole table — both the FTWA path and the §3.6 horizontal-scroll
//! fallback — is wrapped in a `Frame::NONE.stroke(TABLE_PERIMETER_STROKE)`
//! (medium gray, 1 px) so the table reads as a single bordered block on
//! screen.

use super::cell::render_table_cell;
use crate::markdown::InlineElem;
use crate::ui::table_width::TableRenderConfig;
use eframe::egui;

/// Medium-gray border drawn around the outer perimeter of every markdown
/// table. The frame that paints this stroke contains the whole table —
/// both the FTWA path and the §3.6 horizontal-scroll fallback — so the
/// line stays continuous at the table edge regardless of which path ran.
const TABLE_PERIMETER_STROKE: egui::Stroke = egui::Stroke {
    width: 1.0,
    color: egui::Color32::from_gray(120),
};

/// Render a markdown table using the Fair Table Width Algorithm (FTWA) with
/// the cross-cutting [`TableRenderConfig`] applied.
pub(crate) fn render_table_with_config(
    ui: &mut egui::Ui,
    table_cells: &[Vec<Vec<InlineElem>>],
    table_ordinal: usize,
    strategy: crate::ui::table_width::DeficitStrategy,
    config: &TableRenderConfig,
) {
    let n = table_cells.iter().map(|row| row.len()).max().unwrap_or(0);
    if n == 0 {
        return;
    }

    // Stable id derived from a table ordinal rather than `ui.next_auto_id()`
    // (a positional peek that shifts whenever any widget above the table
    // changes). Using a content-derived ordinal keeps the table's persisted
    // column-width cache stable across edits/reflows.
    let table_id = egui::Id::new("md_table").with(table_ordinal);
    let global_padding = config.global_padding.sanitised();

    let (max_w, min_w, breakpoints) =
        crate::ui::table_width::measure_cached(table_id, table_cells, global_padding, ui);
    let gutter = 10.0_f32;
    let avail = (ui.available_width() - (n as f32 - 1.0) * gutter).max(0.0);
    let decision = crate::ui::table_width::ftwa_cached(
        table_id,
        &max_w,
        &min_w,
        &breakpoints,
        avail,
        strategy,
        ui,
    );

    let render_rows = |ui: &mut egui::Ui, decision_widths: &[f32]| {
        let cached_heights_id = table_id.with("row_heights");
        let cached_row_heights: Option<Vec<f32>> = ui.ctx().data(|d| d.get_temp(cached_heights_id));
        let mut new_row_heights = Vec::with_capacity(table_cells.len());

        ui.vertical(|ui| {
            // Row gap: 2 px is sufficient visual separation; 4 px produced
            // visually excessive whitespace in tables with many rows.
            // Pinned by `test_table_empty_cells_no_phantom_row_height`.
            ui.spacing_mut().item_spacing = egui::vec2(10.0, 2.0);
            for (row_idx, row) in table_cells.iter().enumerate() {
                let is_striped = row_idx % 2 == 1;
                let bg_idx = if is_striped {
                    Some(ui.painter().add(egui::Shape::Noop))
                } else {
                    None
                };
                let target_h = cached_row_heights.as_ref().and_then(|h| h.get(row_idx).copied());

                let mut row_max_h: f32 = 0.0;
                let row_response = ui.push_id((table_id, "row", row_idx), |ui| {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(10.0, 0.0);
                        for j in 0..n {
                            let cell = row.get(j).map(|v| v.as_slice()).unwrap_or(&[]);
                            let w = decision_widths.get(j).copied();
                            if !decision.needs_horizontal_scroll {
                                debug_assert!(
                                    w.is_some_and(|w| w.is_finite() && w > 0.0),
                                    "FTWA invariant violated: table {table_ordinal} column {j} width = {w:?}"
                                );
                            }
                            let w = w.filter(|w| w.is_finite() && *w > 0.0);
                            let cell_res = ui.push_id(("col", j), |ui| {
                                // If we know the row's target height from a previous
                                // frame, reserve it up front so the cell's natural
                                // content sits at the top and the cell UI doesn't
                                // double-expand to 2× the target height (the bug
                                // that produced a ~17 px "phantom" gutter).
                                let cell_size = egui::vec2(
                                    w.unwrap_or_else(|| ui.available_width()),
                                    target_h.unwrap_or(0.0),
                                );
                                ui.allocate_ui_with_layout(
                                    cell_size,
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| {
                                        render_table_cell(
                                            ui,
                                            cell,
                                            w,
                                            target_h,
                                            global_padding,
                                        )
                                    },
                                )
                                .inner
                            });
                            let h = cell_res.inner;
                            if h.is_finite() {
                                row_max_h = row_max_h.max(h);
                            }
                        }
                    })
                });

                // Clamp the cached row height to at least one body-line so a
                // pathological all-empty row never caches 0.0 and then
                // defaults back to `min_h` on the next frame (which would
                // double-count via the empty-cell branch).
                let body_line_h = ui.text_style_height(&egui::TextStyle::Body);
                new_row_heights.push(row_max_h.max(body_line_h));

                if let Some(idx) = bg_idx {
                    ui.painter().set(
                        idx,
                        egui::Shape::rect_filled(
                            row_response.response.rect,
                            0.0,
                            ui.visuals().faint_bg_color,
                        ),
                    );
                }
            }
        });

        ui.ctx()
            .data_mut(|d| d.insert_temp(cached_heights_id, new_row_heights));
    };

    if decision.needs_horizontal_scroll {
        // §3.6 fallback: nothing can fit; preserve the never-break-token
        // invariant by letting content overflow into a horizontal ScrollArea.
        egui::Frame::NONE
            .stroke(TABLE_PERIMETER_STROKE)
            .inner_margin(egui::Margin::ZERO)
            .show(ui, |ui| {
                egui::ScrollArea::horizontal()
                    .id_salt(table_id.with("scroll"))
                    .show(ui, |ui| {
                        render_rows(ui, &max_w);
                    });
            });
        return;
    }

    // FTWA path: pin every cell to its assigned column width.
    egui::Frame::NONE
        .stroke(TABLE_PERIMETER_STROKE)
        .inner_margin(egui::Margin::ZERO)
        .show(ui, |ui| {
            render_rows(ui, &decision.widths);
        });
}
