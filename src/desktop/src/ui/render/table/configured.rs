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
//! preserved. The horizontal scrollbar is forced visible in that path
//! so the overflow is discoverable instead of looking like a silently-
//! clipped rightmost cell.
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

    let avail = ui.available_width().max(0.0);

    let layout = crate::ui::table_width::table_layout_cached(
        table_id,
        table_cells,
        global_padding,
        avail,
        strategy,
        ui,
    );

    let render_rows = |ui: &mut egui::Ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(10.0, 2.0);
            for (row_idx, row_cells) in layout.rows.iter().enumerate() {
                let is_striped = row_idx % 2 == 1;
                let bg_idx = if is_striped {
                    Some(ui.painter().add(egui::Shape::Noop))
                } else {
                    None
                };

                let row_response = ui.push_id((table_id, "row", row_idx), |ui| {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(10.0, 0.0);
                        for (col_idx, cell_layout) in row_cells.iter().enumerate() {
                            let cell_size = egui::vec2(cell_layout.width, cell_layout.height);
                            ui.push_id(("col", col_idx), |ui| {
                                ui.allocate_ui_with_layout(
                                    cell_size,
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| {
                                        render_table_cell(
                                            ui,
                                            &cell_layout.content,
                                            Some(cell_layout.width),
                                            Some(cell_layout.height),
                                            global_padding,
                                        )
                                    },
                                )
                            });
                        }
                    })
                });

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
    };

    if layout.needs_horizontal_scroll {
        egui::Frame::NONE
            .stroke(TABLE_PERIMETER_STROKE)
            .inner_margin(egui::Margin::ZERO)
            .show(ui, |ui| {
                // Always show the horizontal scrollbar in the fallback
                // path. The default `VisibleWhenNeeded` policy only paints
                // a scrollbar when the user has interacted (scroll wheel
                // / drag) — for a narrow viewport the table overflows from
                // the very first frame and the user has no visual cue
                // that the rightmost columns are reachable by scrolling.
                // Forcing the scrollbar visible makes the overflow
                // discoverable instead of looking like a render bug where
                // the rightmost cell is silently clipped.
                egui::ScrollArea::horizontal()
                    .id_salt(table_id.with("scroll"))
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .show(ui, |ui| {
                        render_rows(ui);
                    });
            });
        return;
    }

    egui::Frame::NONE
        .stroke(TABLE_PERIMETER_STROKE)
        .inner_margin(egui::Margin::ZERO)
        .show(ui, |ui| {
            render_rows(ui);
        });
}
