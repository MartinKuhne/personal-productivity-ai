//! Shared test helpers for the e2e test submodules.
//!
//! All items are `pub(crate)` so they're visible to the parent
//! `e2e_tests::mod` and to every sibling test submodule. The parent
//! re-exports them with `pub(crate) use helpers::*;` so each test
//! submodule can pick them up via `use super::*;`, matching the
//! `use super::*;` style of the original monolithic `mod e2e_tests`.

#![cfg(test)]

use super::*;

// ---------------------------------------------------------------------------
// Table-rendering helpers (wires the full measure → ftwa → render path so
// tests can assert on the returned decision rather than on pixels — this
// project is on eframe 0.27 and `egui_kittest` requires egui 0.31+).
// ---------------------------------------------------------------------------

/// Renders `table_cells` inside a CentralPanel with `viewport_width`
/// and returns the `ColumnWidths` decision the renderer used.
pub(crate) fn render_table_with_viewport(
    table_cells: &[Vec<Vec<InlineElem>>],
    viewport_width: f32,
) -> crate::ui::table_width::ColumnWidths {
    render_table_with_viewport_and_strategy(
        table_cells,
        viewport_width,
        crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
    )
}

pub(crate) fn render_table_with_viewport_and_strategy(
    table_cells: &[Vec<Vec<InlineElem>>],
    viewport_width: f32,
    strategy: crate::ui::table_width::DeficitStrategy,
) -> crate::ui::table_width::ColumnWidths {
    let ctx = egui::Context::default();
    // `screen_rect` defines the window's pixel dimensions in egui 0.27.
    // Without it, the default (small) rectangle makes `ui.available_width()`
    // unreliable for FTWA tests. Note: `ui.available_width()` inside the
    // `CentralPanel` is then `screen_rect.width() - 16px` (egui's default
    // outer margin), so e.g. a 300px screen rect yields ~284px available.
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(viewport_width, 600.0),
        )),
        ..egui::RawInput::default()
    };
    let mut captured: Option<crate::ui::table_width::ColumnWidths> = None;
    let _ = ctx.run_ui(raw, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            let (max_w, min_w, breakpoints) = crate::ui::table_width::measure(
                table_cells,
                crate::ui::table_width::TablePadding::ZERO,
                ui,
            );
            let gutter = 10.0_f32;
            let avail =
                (ui.available_width() - (max_w.len() as f32 - 1.0).max(0.0) * gutter).max(0.0);
            let decision =
                crate::ui::table_width::ftwa(&max_w, &min_w, &breakpoints, avail, strategy);
            captured = Some(decision.clone());
            render_table(ui, table_cells, 0, strategy);
        });
    });
    captured.expect("ctx.run should have populated `captured`")
}

/// Like `render_table_with_viewport_and_strategy` but threads a non-default
/// `global_padding` through `render_table_with_config` so US3 width/height
/// accounting picks up the resolved padding (`TBL-033`).
pub(crate) fn render_table_with_viewport_and_padding(
    table_cells: &[Vec<Vec<InlineElem>>],
    viewport_width: f32,
    padding: crate::ui::table_width::TablePadding,
) -> crate::ui::table_width::ColumnWidths {
    let strategy = crate::ui::table_width::DeficitStrategy::ProportionalToSlack;
    let ctx = egui::Context::default();
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(viewport_width, 600.0),
        )),
        ..egui::RawInput::default()
    };
    let mut captured: Option<crate::ui::table_width::ColumnWidths> = None;
    let _ = ctx.run_ui(raw, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            let (max_w, min_w, breakpoints) =
                crate::ui::table_width::measure(table_cells, padding, ui);
            let gutter = 10.0_f32;
            let avail =
                (ui.available_width() - (max_w.len() as f32 - 1.0).max(0.0) * gutter).max(0.0);
            let decision =
                crate::ui::table_width::ftwa(&max_w, &min_w, &breakpoints, avail, strategy);
            captured = Some(decision.clone());
        });
    });
    captured.expect("ctx.run should have populated `captured`")
}

/// Paint variants of the padding-aware viewport helper — produces
/// `FullOutput` for shape inspection (used by US3 border/junction tests).
pub(crate) fn render_table_with_paint_output_and_padding(
    table_cells: &[Vec<Vec<InlineElem>>],
    viewport_width: f32,
    padding: crate::ui::table_width::TablePadding,
) -> egui::FullOutput {
    let strategy = crate::ui::table_width::DeficitStrategy::ProportionalToSlack;
    let config = crate::ui::table_width::TableRenderConfig {
        global_padding: padding,
    };
    let ctx = egui::Context::default();
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(viewport_width, 1600.0),
        )),
        ..egui::RawInput::default()
    };
    let _ = ctx.run_ui(raw.clone(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            render_table_with_config(ui, table_cells, 0, strategy, &config);
        });
    });
    ctx.run_ui(raw, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            render_table_with_config(ui, table_cells, 0, strategy, &config);
        });
    })
}

/// Helper: build a table where every column has the same `cell_text`
/// in both the header and the (single) data row. Used to make
/// column-width measurements identical so the FTWA widths reflect
/// the algorithm's own distribution rather than font-metric noise.
pub(crate) fn build_uniform_table(cell_text: &str, n_columns: usize) -> Vec<Vec<Vec<InlineElem>>> {
    let make_cell = || {
        vec![InlineElem::Text(
            cell_text.to_string(),
            crate::ui::render::TextStyle::default(),
        )]
    };
    let row: Vec<Vec<InlineElem>> = (0..n_columns).map(|_| make_cell()).collect();
    vec![row.clone(), row]
}

/// Helper: build a table where one column (the "wide" one) has much
/// longer text than the others. The other columns use `narrow_text`.
pub(crate) fn build_dissimilar_table(
    narrow_text: &str,
    wide_text: &str,
) -> Vec<Vec<Vec<InlineElem>>> {
    let make = |t: &str| {
        vec![InlineElem::Text(
            t.to_string(),
            crate::ui::render::TextStyle::default(),
        )]
    };
    vec![
        vec![make(narrow_text), make(wide_text), make(narrow_text)],
        vec![make(narrow_text), make(wide_text), make(narrow_text)],
    ]
}

/// Helper to render `table_cells` in a specified viewport width and return `FullOutput`.
pub(crate) fn render_table_with_paint_output_viewport(
    table_cells: &[Vec<Vec<InlineElem>>],
    viewport_width: f32,
) -> egui::FullOutput {
    let ctx = egui::Context::default();
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(viewport_width, 1600.0),
        )),
        ..egui::RawInput::default()
    };
    // Pass 1: measure row heights in Grid
    let _ = ctx.run_ui(raw.clone(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            render_table(
                ui,
                table_cells,
                0,
                crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
            );
        });
    });
    // Pass 2: paint with resolved Grid row heights stored in memory
    ctx.run_ui(raw, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            render_table(
                ui,
                table_cells,
                0,
                crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
            );
        });
    })
}

/// Renders `table_cells` in a wide viewport so the FTWA path runs
/// (not the §3.6 horizontal-scroll fallback) and returns the
/// `FullOutput` for shape inspection.
pub(crate) fn render_table_with_paint_output(
    table_cells: &[Vec<Vec<InlineElem>>],
) -> egui::FullOutput {
    render_table_with_paint_output_viewport(table_cells, 800.0)
}

// ---------------------------------------------------------------------------
// egui 0.35 platform-output helpers
// ---------------------------------------------------------------------------

/// Helper: read the most recent `OutputCommand::CopyText(_)` from
/// a `&PlatformOutput`. The full `PlatformOutput` survives on
/// the `FullOutput` returned by `ctx.run_ui` (the per-frame
/// `ctx.output` view is reset between frames), so tests should
/// hand us the post-frame output.
///
/// egui 0.35 replaced the `PlatformOutput::copied_text` field
/// with `PlatformOutput::commands: Vec<OutputCommand>`. Copy
/// requests now live as `OutputCommand::CopyText(String)` entries
/// in the commands vector. This helper drains the most recent
/// `CopyText` command, returning the empty string when none
/// has been emitted.
pub(crate) fn commands_capture(platform: &egui::PlatformOutput) -> String {
    platform
        .commands
        .iter()
        .rev()
        .find_map(|cmd| match cmd {
            egui::OutputCommand::CopyText(text) => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Default-config dispatch wrapper (test-only).
//
// Was previously in ui/render/table/dispatch.rs as a pub(crate) fn
// with a 3-hop visibility chain (dispatch::render_table ->
// ui::render::render_table re-export -> e2e_tests::render_table re-export).
// Moved here so it lives next to the other test helpers; the original
// dispatch.rs and the two re-exports were deleted.
// ---------------------------------------------------------------------------

/// Render a markdown table with the default [crate::ui::table_width::TableRenderConfig].
///
/// Equivalent to calling ender_table_with_config with
/// &TableRenderConfig::default() (global padding = ZERO).
/// Production dispatch uses ender_table_with_config directly.
pub(crate) fn render_table(
    ui: &mut egui::Ui,
    table_cells: &[Vec<Vec<InlineElem>>],
    table_ordinal: usize,
    strategy: crate::ui::table_width::DeficitStrategy,
) {
    super::render_table_with_config(
        ui,
        table_cells,
        table_ordinal,
        strategy,
        &crate::ui::table_width::TableRenderConfig::default(),
    )
}
