//! Off-viewport guards + table row-geometry regressions.
//!
//! Covers the catch-all table regressions that don't fit the
//! column-alignment, FTWA, or interaction submodules:
//!
//! - Off-viewport text guards at narrow (§3.6 fallback) and wide
//!   (FTWA) viewports (MD-013).
//! - Multi-frame row-height stability (no per-frame oscillation).
//! - Empty cells not inflating `row_max_h` (the phantom-row-height
//!   regression from `2022 sports car.md`).
//! - Single-line cell text top-aligned in a row with a multi-line
//!   neighbor (TBL-031 / `top_down` layout fix).
//! - Cell text top-aligned + compact row height in a multi-line
//!   fixture.

use super::*;

// ---- Off-viewport text guards (MD-013) -------------------------

/// Render the long-table-row fixture at a phone-sized viewport
/// and assert no `Shape::Text` is permanently outside its
/// containing clip rect. Catches the F1 (horizontal clip) and
/// F3 (table column overflow) failure modes; the §3.6 fallback
/// in `render_table` must wrap the table in a horizontal
/// `ScrollArea` so cell text remains reachable.
///
/// Mirrors the production render path: `render_markdown` is
/// always wrapped in a vertical `ScrollArea` by the center panel
/// (`src/ui/panels/center.rs:364`). The test must do the same,
/// otherwise tall content trivially exceeds the viewport's
/// bottom edge and triggers a false positive.
#[test]
fn render_markdown_no_offscreen_text_at_narrow_viewport() {
    use crate::ui::test_helpers::offscreen::assert_no_offscreen_text;

    // Markdown body that contains a table wider than the viewport
    // plus surrounding prose and headings. The fixture lives under
    // `src/test/wiki/Travel/long-table-row.md` per
    // `src/test/wiki/AGENTS.md` §"Fixtures only".
    let md = include_str!("../../../../../test/wiki/Travel/long-table-row.md");

    // 320 px is the iPhone 5 / SE 1st-gen width — the narrowest
    // viewport we expect to support. A 6-column table with long
    // text guarantees `decision.needs_horizontal_scroll == true`
    // on this viewport, exercising the §3.6 fallback path.
    let viewport_width: f32 = 320.0;
    let viewport_height: f32 = 800.0;

    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(viewport_width, viewport_height),
        )),
        ..egui::RawInput::default()
    };
    let mut scroll_id = None;
    let mut pending_toggles = Vec::new();

    let ctx = egui::Context::default();
    let output = ctx.run_ui(raw, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("main_markdown_scroll")
                .show(ui, |ui| {
                    render_markdown(
                        ui,
                        md,
                        &mut scroll_id,
                        &mut pending_toggles,
                        crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                    );
                });
        });
    });

    assert_no_offscreen_text(&output.shapes);
}

/// Same fixture at a desktop-sized viewport (the FTWA path, not
/// the §3.6 fallback). The table fits in columns; the helper
/// must still report no off-viewport text inside the vertical
/// scroll content rect.
#[test]
fn render_markdown_no_offscreen_text_at_wide_viewport() {
    use crate::ui::test_helpers::offscreen::assert_no_offscreen_text;

    let md = include_str!("../../../../../test/wiki/Travel/long-table-row.md");

    let viewport_width: f32 = 1600.0;
    let viewport_height: f32 = 800.0;

    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(viewport_width, viewport_height),
        )),
        ..egui::RawInput::default()
    };
    let mut scroll_id = None;
    let mut pending_toggles = Vec::new();

    let ctx = egui::Context::default();
    let output = ctx.run_ui(raw, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("main_markdown_scroll")
                .show(ui, |ui| {
                    render_markdown(
                        ui,
                        md,
                        &mut scroll_id,
                        &mut pending_toggles,
                        crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                    );
                });
        });
    });

    assert_no_offscreen_text(&output.shapes);
}

/// Regression test: Verify that table row heights remain 100% stable
/// across multiple consecutive render frames (frame 1 through 10) on the
/// same egui::Context, with zero inter-frame height growth or oscillation.
#[test]
fn test_render_table_multi_frame_height_stability() {
    let md = "| Col 1 | Col 2 |\n|---|---|\n| Short cell | Multi line cell with long text wrapping into several lines |\n";
    let ctx = egui::Context::default();
    let mut frame_1_heights: Vec<f32> = Vec::new();

    for frame in 0..30 {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            ..egui::RawInput::default()
        };
        let mut scroll_id = None;
        let mut pending_toggles = Vec::new();

        let output = ctx.run_ui(raw, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                render_markdown(
                    ui,
                    md,
                    &mut scroll_id,
                    &mut pending_toggles,
                    crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
                );
            });
        });

        let current_rect_heights: Vec<f32> = output
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Rect(r)
                    if r.fill == egui::Color32::TRANSPARENT
                        && r.stroke == egui::Stroke::NONE
                        && r.stroke_kind == egui::StrokeKind::Inside =>
                {
                    Some(r.rect.height())
                }
                _ => None,
            })
            .collect();

        assert_eq!(
            current_rect_heights.len(),
            4,
            "Expected 4 cell rects (2x2 table)"
        );

        if frame == 1 {
            frame_1_heights = current_rect_heights.clone();
        } else if frame > 1 {
            for (idx, (h_base, h_curr)) in frame_1_heights
                .iter()
                .zip(current_rect_heights.iter())
                .enumerate()
            {
                assert!(
                    (h_base - h_curr).abs() < 0.01,
                    "Frame {frame}: Cell rect {idx} height changed from {h_base} (frame 1) to {h_curr}!"
                );
            }
        }
    }
}

/// Regression test: empty cells must not inflate `row_max_h` via
/// `set_min_size` — two round-trip frames must produce stable, compact
/// row heights for a table that is mostly empty cells (mirrors the
/// `2022 sports car.md` fixture that triggered the phantom-whitespace
/// bug). Pinned by the `allocate_space` fix in `render_table_cell`.
#[test]
fn test_table_empty_cells_no_phantom_row_height() {
    // A 3-column table: header row all-empty, one data row with text
    // only in the first column and empty in the other two.
    let empty: Vec<InlineElem> = vec![];
    let make = |t: &str| {
        vec![InlineElem::Text(
            t.to_string(),
            crate::ui::render::TextStyle::default(),
        )]
    };
    let table: Vec<Vec<Vec<InlineElem>>> = vec![
        // row 0: all-empty header
        vec![empty.clone(), empty.clone(), empty.clone()],
        // row 1: text in col 0, empty in cols 1 and 2
        vec![make("BRZ/GR86"), empty.clone(), empty.clone()],
        // row 2: text only in col 0
        vec![make("Camaro 1SS"), empty.clone(), empty.clone()],
    ];

    // Two-pass render (mirrors production): first pass measures,
    // second pass uses cached row heights.
    let strategy = crate::ui::table_width::DeficitStrategy::ProportionalToSlack;
    let config = crate::ui::table_width::TableRenderConfig::default();
    let ctx = egui::Context::default();
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800.0, 1600.0),
        )),
        ..egui::RawInput::default()
    };

    // Pass 1
    let _ = ctx.run_ui(raw.clone(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            render_table_with_config(ui, &table, 0, strategy, &config);
        });
    });
    // Pass 2 — row heights now come from the cache.
    let output2 = ctx.run_ui(raw, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            render_table_with_config(ui, &table, 0, strategy, &config);
        });
    });

    // Collect cell-frame rects (TRANSPARENT fill, NONE stroke)
    let rects: Vec<egui::Rect> = output2
        .shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::Shape::Rect(r)
                if r.fill == egui::Color32::TRANSPARENT && r.stroke == egui::Stroke::NONE =>
            {
                Some(r.rect)
            }
            _ => None,
        })
        .collect();

    assert_eq!(rects.len(), 9, "Expected 9 cell rects for 3x3 table");

    // Every row's height should be compact — no more than 2× the body
    // line height. Excessive whitespace would show as >34 px rows.
    let body_h = 14.0_f32; // approximate body font size
    let max_allowed = body_h * 2.5;
    for (i, r) in rects.iter().enumerate() {
        assert!(
            r.height() <= max_allowed,
            "Cell rect {i} height {:.1} exceeds {max_allowed:.1} — phantom whitespace detected",
            r.height()
        );
    }
}

/// Regression test: after the cached-height round-trip, single-line cells
/// in a row that also contains a multi-line cell must start at the row's
/// top Y coordinate (not centred). Mirrors the sports-car table where
/// the Camaro row has `<br>` content making it 2 lines tall while other
/// cells in the same row are single-line. Pinned by the `top_down` layout
/// fix in `render_table_cell`.
#[test]
fn test_table_single_line_cell_text_at_row_top() {
    let make = |t: &str| {
        vec![InlineElem::Text(
            t.to_string(),
            crate::ui::render::TextStyle::default(),
        )]
    };
    // Row 0: header (short)
    // Row 1: col 0 has two lines (SoftBreak forces height ≈ 2 × line_h),
    //        col 1 is single-line short text.
    let tall_cell = vec![
        InlineElem::Text(
            "Line one".to_string(),
            crate::ui::render::TextStyle::default(),
        ),
        InlineElem::SoftBreak,
        InlineElem::Text(
            "Line two".to_string(),
            crate::ui::render::TextStyle::default(),
        ),
    ];
    let table: Vec<Vec<Vec<InlineElem>>> = vec![
        vec![make("Header A"), make("Header B")],
        vec![tall_cell, make("Short")],
    ];

    let strategy = crate::ui::table_width::DeficitStrategy::ProportionalToSlack;
    let config = crate::ui::table_width::TableRenderConfig::default();
    let ctx = egui::Context::default();
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800.0, 1600.0),
        )),
        ..egui::RawInput::default()
    };

    // Pass 1: measure
    let _ = ctx.run_ui(raw.clone(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            render_table_with_config(ui, &table, 0, strategy, &config);
        });
    });
    // Pass 2: paint with cached heights
    let output2 = ctx.run_ui(raw, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            render_table_with_config(ui, &table, 0, strategy, &config);
        });
    });

    // Collect cell-frame rects sorted by (row-bucket Y, X)
    let mut rects: Vec<egui::Rect> = output2
        .shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::Shape::Rect(r)
                if r.fill == egui::Color32::TRANSPARENT && r.stroke == egui::Stroke::NONE =>
            {
                Some(r.rect)
            }
            _ => None,
        })
        .collect();

    assert_eq!(rects.len(), 4, "Expected 4 cell rects (2×2 table)");

    rects.sort_by(|a, b| {
        let ya = (a.min.y / 10.0).round() as i32;
        let yb = (b.min.y / 10.0).round() as i32;
        ya.cmp(&yb)
            .then_with(|| a.min.x.partial_cmp(&b.min.x).unwrap())
    });

    // Row 1 rects: rects[2] = tall cell (col 0), rects[3] = short cell (col 1).
    // Both must share the same top Y (top-aligned, not centred).
    let tall_top = rects[2].min.y;
    let short_top = rects[3].min.y;
    assert!(
        (tall_top - short_top).abs() < 1.0,
        "Row 1 top-Y mismatch: tall cell y={tall_top:.1}, short cell y={short_top:.1} — text not top-aligned"
    );
}

/// Regression test: Verify that cell text in tall rows starts top-aligned
/// (matching Y coordinate) and that row height is compact (< 150px).
#[test]
fn test_render_table_top_aligned_and_compact_row_height() {
    let md = "| Short | Tall |\n|---|---|\n| Cell A | Line 1<br>Line 2<br>Line 3<br>Line 4 |\n";
    let ctx = egui::Context::default();
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800.0, 600.0),
        )),
        ..egui::RawInput::default()
    };
    let mut scroll_id = None;
    let mut pending_toggles = Vec::new();

    let output = ctx.run_ui(raw, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            render_markdown(
                ui,
                md,
                &mut scroll_id,
                &mut pending_toggles,
                crate::ui::table_width::DeficitStrategy::ProportionalToSlack,
            );
        });
    });

    // Find cell rects for Row 1
    let rects: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::Shape::Rect(r)
                if r.fill == egui::Color32::TRANSPARENT
                    && r.stroke == egui::Stroke::NONE
                    && r.stroke_kind == egui::StrokeKind::Inside =>
            {
                Some(r.rect)
            }
            _ => None,
        })
        .collect();

    assert_eq!(rects.len(), 4, "Expected 4 cell rects for 2x2 table");

    // Row 1 short cell (rects[2]) and tall cell (rects[3]) should have identical top Y
    assert!(
        (rects[2].min.y - rects[3].min.y).abs() < 1.0,
        "Row 1 cells top Y mismatch: short cell top Y = {}, tall cell top Y = {}",
        rects[2].min.y,
        rects[3].min.y
    );

    // Row 1 height should be compact and bounded
    assert!(
        rects[2].height() < 150.0,
        "Row height too tall: {}",
        rects[2].height()
    );
}
