//! Table row / column alignment tests.
//!
//! Covers:
//!
//! - Column left-edge + width consistency across all rows in a multi-row
//!   table with mixed single-line and wrapped cells (Laptop.md shape).
//! - Empty header cells (`| | |`) that must not collapse vs the data
//!   rows below (regression for the Laptop-spec key-value table).
//! - The G2-drop regression: when a long cell is the only positive-slack
//!   column, FTWA must keep the short cells at `max_content` and force
//!   the long cell to absorb the full deficit.
//! - The 7-column laptop-spec fixture that originally surfaced the G2 bug.
//! - Top-aligned single-line cells within tall rows (TBL-031).
//! - Top-aligned text in tall rows (no vertical centering).
//! - No internal vertical gap or single-item centering in tall cells.

use super::*;

/// Verifies that column 0 and subsequent columns are properly aligned
/// across rows and maintain uniform inter-column gutter spacing, even when
/// cell content requires word wrapping onto multiple lines.
#[test]
fn test_render_table_column_alignment_across_rows() {
    use eframe::epaint::{Shape, StrokeKind};

    let make = |t: &str| {
        vec![InlineElem::Text(
            t.to_string(),
            crate::ui::render::TextStyle::default(),
        )]
    };
    // Table with multi-word space-separated text that requires word wrapping
    // in a constrained viewport.
    let table: Vec<Vec<Vec<InlineElem>>> = vec![
        vec![
            make("Header column one with multi word text requiring word wrap"),
            make("Header column two long text"),
            make("H3"),
        ],
        vec![
            make("Alpha beta gamma delta epsilon"),
            make("Short"),
            make("Gamma delta"),
        ],
        vec![
            make("Row three column one extra text"),
            make("R3C2 multi word text"),
            make("R3C3"),
        ],
    ];

    // 260px viewport forces deficit regime and word-wrapping for long cells,
    // while remaining wide enough to avoid §3.6 horizontal scroll fallback.
    let output = render_table_with_paint_output_viewport(&table, 260.0);

    // Collect all cell border rects
    let mut rects: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            Shape::Rect(r)
                if r.fill == egui::Color32::TRANSPARENT
                    && r.stroke == egui::Stroke::NONE
                    && r.stroke_kind == StrokeKind::Inside =>
            {
                Some(r.rect)
            }
            _ => None,
        })
        .collect();

    assert_eq!(
        rects.len(),
        9,
        "Expected 9 cell borders for 3x3 table; got rects: {:?}",
        rects
    );

    // Sort by Y position (row) then X position (column)
    rects.sort_by(|a, b| {
        a.min
            .y
            .partial_cmp(&b.min.y)
            .unwrap()
            .then(a.min.x.partial_cmp(&b.min.x).unwrap())
    });

    // Group into 3 rows of 3 cells each
    let rows: [Vec<egui::Rect>; 3] = [
        vec![rects[0], rects[1], rects[2]],
        vec![rects[3], rects[4], rects[5]],
        vec![rects[6], rects[7], rects[8]],
    ];

    // Verify that word wrapping actually occurred in the long multi-word cells
    // (wrapped text galley has >1 row).
    let wrapped_text = output
        .shapes
        .iter()
        .find_map(|cs| match &cs.shape {
            egui::Shape::Text(t) if t.galley.text().contains("Header column one") => Some(t),
            _ => None,
        })
        .expect("expected Header column one text shape");

    assert!(
        wrapped_text.galley.rows.len() > 1,
        "Expected cell (0,0) text to word wrap, got {} row(s)",
        wrapped_text.galley.rows.len()
    );

    // For each column j ∈ 0..3, check that min.x and width match across all rows
    for (col, first_cell) in rows[0].iter().enumerate() {
        let first_min_x = first_cell.min.x;
        let first_width = first_cell.width();

        for (row, row_data) in rows.iter().enumerate().skip(1) {
            let min_x = row_data[col].min.x;
            let width = row_data[col].width();

            // Sub-pixel tolerance: FTWA distributes the deficit across
            // every positive-slack column (post-G2-drop), so the exact
            // column widths depend on the ratio of slacks and may shift
            // by a sub-pixel amount vs. an exact-integer expectation.
            // A 0.5 px tolerance still catches real misalignments.
            assert!(
                (min_x - first_min_x).abs() < 0.5,
                "Column {col} left border misaligned at row {row}: expected {first_min_x}, got {min_x}"
            );
            assert!(
                (width - first_width).abs() < 0.5,
                "Column {col} width mismatch at row {row}: expected {first_width}, got {width}"
            );
        }
    }

    // Verify gutter spacing between columns is 10px across all rows
    for (row, row_data) in rows.iter().enumerate() {
        let col0_right = row_data[0].max.x;
        let col1_left = row_data[1].min.x;
        let col1_right = row_data[1].max.x;
        let col2_left = row_data[2].min.x;

        let gutter_0_1 = col1_left - col0_right;
        let gutter_1_2 = col2_left - col1_right;

        assert!(
            (gutter_0_1 - 10.0).abs() < 0.5,
            "Row {row} gutter between Col 0 and Col 1 should be ~10.0, got {gutter_0_1}"
        );
        assert!(
            (gutter_1_2 - 10.0).abs() < 0.5,
            "Row {row} gutter between Col 1 and Col 2 should be ~10.0, got {gutter_1_2}"
        );
    }
}

/// Verifies that tables with empty header cells (e.g. `| | |`) render
/// empty cells in row 0 at the full column width matching subsequent data rows,
/// preventing row 0 cell collapse or misalignment (regression test for Laptop.md).
#[test]
fn test_render_table_empty_header_cells_alignment() {
    use eframe::epaint::{Shape, StrokeKind};

    let make = |t: &str| {
        vec![InlineElem::Text(
            t.to_string(),
            crate::ui::render::TextStyle::default(),
        )]
    };
    // Table with empty header row 0 (common in key-value markdown tables like Laptop.md)
    let table: Vec<Vec<Vec<InlineElem>>> = vec![
        vec![vec![], vec![]],
        vec![make("Price"), make("$1,399.99 (Costco Members)")],
        vec![make("MSRP"), make("$1,799.99")],
    ];

    let output = render_table_with_paint_output(&table);

    // Collect all cell border rects
    let mut rects: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            Shape::Rect(r)
                if r.fill == egui::Color32::TRANSPARENT
                    && r.stroke == egui::Stroke::NONE
                    && r.stroke_kind == StrokeKind::Inside =>
            {
                Some(r.rect)
            }
            _ => None,
        })
        .collect();

    assert_eq!(rects.len(), 6, "Expected 6 cell borders for 3x2 table");

    // Sort by Y position (row) then X position (column)
    rects.sort_by(|a, b| {
        a.min
            .y
            .partial_cmp(&b.min.y)
            .unwrap()
            .then(a.min.x.partial_cmp(&b.min.x).unwrap())
    });

    let rows: [Vec<egui::Rect>; 3] = [
        vec![rects[0], rects[1]],
        vec![rects[2], rects[3]],
        vec![rects[4], rects[5]],
    ];

    // For column 0 and column 1, empty row 0 cells must match row 1 & 2 width and left edge
    for (col, (header_cell, ref_cell)) in rows[0].iter().zip(rows[1].iter()).enumerate() {
        let col_width = ref_cell.width();
        let col_min_x = ref_cell.min.x;

        let row0_width = header_cell.width();
        let row0_min_x = header_cell.min.x;

        assert!(
            (row0_min_x - col_min_x).abs() < 1e-3,
            "Empty header row 0 cell in column {col} misaligned: expected min.x {col_min_x}, got {row0_min_x}"
        );
        assert!(
            (row0_width - col_width).abs() < 1e-3,
            "Empty header row 0 cell in column {col} collapsed: expected width {col_width}, got {row0_width}"
        );
    }
}

/// Regression test for the G2-drop: when a 3-column table has two
/// single-token cells (`"free"`, `"beer"`) and one long multi-word
/// cell, the FTWA deficit distribution must keep the short cells at
/// their max-content width (no slack → not in the wrap set) and
/// force the long cell to absorb the entire deficit. On a 300 px
/// viewport the long cell must word-wrap; the two short cells
/// must stay single-line.
#[test]
fn test_render_table_short_cells_unwrap_long_cell_wraps() {
    use eframe::epaint::Shape;

    let make = |t: &str| {
        vec![InlineElem::Text(
            t.to_string(),
            crate::ui::render::TextStyle::default(),
        )]
    };
    // Mirrors the markdown `| free | beer |  Amber Pale Ale Imperial
    // IPA Lager Pilsner Helles Poster Stout Blond Hefeweizen |`.
    let table: Vec<Vec<Vec<InlineElem>>> = vec![vec![
        make("free"),
        make("beer"),
        make(
            "Amber Pale Ale Imperial IPA Lager Pilsner Helles \
             Poster Stout Blond Hefeweizen",
        ),
    ]];

    let output = render_table_with_paint_output_viewport(&table, 300.0);

    // Find the text shape for each cell by its exact galley text.
    let find_text = |needle: &str| -> &eframe::epaint::TextShape {
        output
            .shapes
            .iter()
            .find_map(|cs| match &cs.shape {
                Shape::Text(t) if t.galley.text() == needle => Some(t),
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected text shape with content {needle:?}"))
    };
    let free_text = find_text("free");
    let beer_text = find_text("beer");
    let long_text =
        find_text("Amber Pale Ale Imperial IPA Lager Pilsner Helles Poster Stout Blond Hefeweizen");

    // Short, single-token cells must not word-wrap. Their width is
    // pinned at `max_content` (no slack → not in the wrap set), so
    // the galley stays on a single line.
    assert_eq!(
        free_text.galley.rows.len(),
        1,
        "short cell \"free\" must not be word-wrapped; got {} row(s)",
        free_text.galley.rows.len()
    );
    assert_eq!(
        beer_text.galley.rows.len(),
        1,
        "short cell \"beer\" must not be word-wrapped; got {} row(s)",
        beer_text.galley.rows.len()
    );

    // The long cell absorbs the full deficit (it is the only
    // positive-slack column) and must word-wrap onto multiple lines.
    assert!(
        long_text.galley.rows.len() > 1,
        "long cell must be word-wrapped; got {} row(s)",
        long_text.galley.rows.len()
    );
}

/// 7-column table where six of the seven cells have short or
/// medium-length content and one ("Summary") has a long sentence.
/// Mirrors a real laptop-spec table from the user's test corpus:
///
/// ```text
/// | Make | Model and Model Number | Market Price (Original) | Display | Processor | PassMark Single / Multi | Summary |
/// |------|----------------------|------------------------|---------|-----------|------------------------|---------|
/// | Dell | XPS 15 9570 | ~$1,500-$2,000 (discontinued) | 15.6" FHD ... | Intel Core i5-8300H (4C/8T) | 2,271 / 7,545 | Premium build, ... |
/// ```
///
/// Captured bug (pre-fix incorrect behavior at 1000 px viewport):
/// with G2 dropped, the FTWA was distributing the deficit across
/// every positive-slack column proportionally, which overallocated
/// the "Summary" column and squeezed columns 2..6 to widths that
/// forced their content onto a second row:
///
/// ```text
/// Dell                                                    -> 1 row  (OK)
/// XPS 15 9570                                             -> 2 rows (BUG — should be 1)
/// ~$1,500-$2,000 (discontinued)                           -> 2 rows (BUG — should be 1)
/// 15.6" FHD (1920x1080) IPS or 4K OLED, 60Hz              -> 2 rows (BUG — should be 1)
/// Intel Core i5-8300H (4C/8T)                             -> 2 rows (BUG — should be 1)
/// 2,271 / 7,545                                           -> 2 rows (BUG — should be 1)
/// Premium build, ... (Summary)                            -> 3 rows (OK — long cell wraps)
/// ```
///
/// **Fix**: G2 (minimum-cardinality wrap set) is back. The Summary
/// column's slack (747.88 px) alone covers the entire deficit
/// (660.69 px), so the wrap set is just `{Summary}`; every other
/// column stays at its `max_content` width and renders on a
/// single line. The unit-level equivalent is pinned in
/// `audit_g2_one_big_slack_column_absorbs_entire_deficit` in
/// `src/markdown/table_width/mod.rs`.
///
/// This test is now a **passing regression guard**: if G2 is ever
/// dropped again, this test will start failing on `XPS 15 9570` /
/// `2,271 / 7,545` / etc. (the columns with positive slack that
/// should NOT be in the wrap set).
#[test]
fn test_render_table_laptop_spec_short_cells_unwrap_long_wraps() {
    use eframe::epaint::Shape;

    let make = |t: &str| {
        vec![InlineElem::Text(
            t.to_string(),
            crate::ui::render::TextStyle::default(),
        )]
    };
    // One data row, seven columns. The cell content matches a
    // laptop-spec table the user hit on real markdown.
    let table: Vec<Vec<Vec<InlineElem>>> = vec![vec![
        make("Dell"),
        make("XPS 15 9570"),
        make("~$1,500-$2,000 (discontinued)"),
        make("15.6\" FHD (1920x1080) IPS or 4K OLED, 60Hz"),
        make("Intel Core i5-8300H (4C/8T)"),
        make("2,271 / 7,545"),
        make(
            "Premium build, excellent keyboard, great 4K OLED \
             option, Thunderbolt 3. Now aging with 8th gen Intel. \
             Shows the value of modern efficiency.",
        ),
    ]];

    // 1000 px viewport: 7 columns + 6 gutters of 10 px = 60 px
    // gutters, so 940 px of content. Wide enough that the six
    // short/medium cells fit on one line and the Summary cell
    // absorbs the deficit.
    let output = render_table_with_paint_output_viewport(&table, 1000.0);

    // Find the text shape for each cell by its exact galley text.
    let find_text = |needle: &str| -> &eframe::epaint::TextShape {
        output
            .shapes
            .iter()
            .find_map(|cs| match &cs.shape {
                Shape::Text(t) if t.galley.text() == needle => Some(t),
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected text shape with content {needle:?}"))
    };

    // The six short / medium cells. Each must stay on a single line.
    let single_line_cells = [
        "Dell",
        "XPS 15 9570",
        "~$1,500-$2,000 (discontinued)",
        "15.6\" FHD (1920x1080) IPS or 4K OLED, 60Hz",
        "Intel Core i5-8300H (4C/8T)",
        "2,271 / 7,545",
    ];
    for cell in single_line_cells {
        let shape = find_text(cell);
        assert_eq!(
            shape.galley.rows.len(),
            1,
            "cell {cell:?} must not be word-wrapped; got {} row(s)",
            shape.galley.rows.len()
        );
    }

    // The long Summary cell must word-wrap onto multiple lines.
    let summary = find_text(
        "Premium build, excellent keyboard, great 4K OLED \
         option, Thunderbolt 3. Now aging with 8th gen Intel. \
         Shows the value of modern efficiency.",
    );
    assert!(
        summary.galley.rows.len() > 1,
        "Summary cell must be word-wrapped; got {} row(s)",
        summary.galley.rows.len()
    );
}

/// Regression test: all cells in the same row must share the same top Y
/// coordinate (min.y). egui::Grid centers cells vertically by default — so
/// when col 1 wraps to two lines, col 0 ("Make") would appear lower without
/// the `top_down` layout wrapper added to `render_table_cell`.
#[test]
fn test_render_table_cells_top_aligned_within_row() {
    use eframe::epaint::{Shape, StrokeKind};

    let make = |t: &str| {
        vec![InlineElem::Text(
            t.to_string(),
            crate::ui::render::TextStyle::default(),
        )]
    };
    let table: Vec<Vec<Vec<InlineElem>>> = vec![
        vec![
            make("Make"),
            make("Model and Model Number"),
            make("Market Price (Original)"),
        ],
        vec![
            make("Dell"),
            make("XPS 15 9570"),
            make("~$1,500-$2,000 (discontinued)"),
        ],
    ];

    let output = render_table_with_paint_output(&table);

    let mut rects: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            Shape::Rect(r)
                if r.fill == egui::Color32::TRANSPARENT
                    && r.stroke == egui::Stroke::NONE
                    && r.stroke_kind == StrokeKind::Inside =>
            {
                Some(r.rect)
            }
            _ => None,
        })
        .collect();

    assert_eq!(rects.len(), 6, "Expected 6 cell borders for 2x3 table");

    // Sort by Y-bucket (15px) then X
    rects.sort_by(|a, b| {
        let y_a = (a.min.y / 15.0).round() as i32;
        let y_b = (b.min.y / 15.0).round() as i32;
        y_a.cmp(&y_b)
            .then_with(|| a.min.x.partial_cmp(&b.min.x).unwrap())
    });

    let row0 = &rects[0..3];
    let row1 = &rects[3..6];

    // Within each row all cells must share the same top Y (top-aligned)
    for (r_idx, row) in [row0, row1].iter().enumerate() {
        let expected_min_y = row[0].min.y;
        for (c_idx, rect) in row.iter().enumerate() {
            assert!(
                (rect.min.y - expected_min_y).abs() < 1.0,
                "Row {r_idx} col {c_idx} top misaligned: expected min.y {expected_min_y}, got {}",
                rect.min.y
            );
        }
    }
}

/// TBL-031: a short (single-line) cell in a row that also contains a
/// multi-line cell must top-align — `pos.y` for the short cell's
/// text shape must match the tall cell's first-text-shape `pos.y`
/// within 2 px. The cell renderer is expected to use `top_down`
/// layout; without it `egui::Grid` would center cells vertically
/// and the short cell would drift down by half the row-height
/// delta. This test covers two distinct table shapes that
/// historically triggered this regression:
///
/// * 2×2 table with a 4-line multi-line `Processor` cell and a
///   single-word `Dell` cell (3 render passes — measure + paint +
///   paint, the path that originally surfaced the bug).
/// * 2×2 table with a long `Summary` cell and a `PassMark` value
///   cell (2 render passes — measure + paint).
///
/// Consolidated from 2 formerly-separate tests
/// (`test_render_table_cell_text_is_top_aligned_in_tall_row` and
/// `..._no_internal_vertical_gap_or_centering`) that both asserted
/// the same `|short.y - tall.y| ≤ 2.0` invariant on different
/// fixtures. The 2 px tolerance is intentional: the Grid two-pass
/// measure-paint path can drift by 1-2 px on the first paint pass
/// before settling on the second.
#[test]
fn test_render_table_cell_text_top_aligned_with_tall_neighbor() {
    let make = |t: &str| {
        vec![InlineElem::Text(
            t.to_string(),
            crate::ui::render::TextStyle::default(),
        )]
    };
    let long_processor = "Intel Core Ultra 7 256V (8C/8T Lunar Lake) high performance mobile processor \
         with dedicated NPU for artificial intelligence workloads.";
    let long_summary = "Premium build, excellent keyboard, great 4K OLED option, \
         Thunderbolt 3. Now aging with 8th gen Intel. Shows the value of modern efficiency.";

    let processor_table: Vec<Vec<Vec<InlineElem>>> = vec![
        vec![make("Make"), make("Processor")],
        vec![make("Dell"), make(long_processor)],
    ];
    let summary_table: Vec<Vec<Vec<InlineElem>>> = vec![
        vec![make("PassMark"), make("Summary")],
        vec![make("2,271 / 7,545"), make(long_summary)],
    ];

    struct Case {
        label: &'static str,
        table: Vec<Vec<Vec<InlineElem>>>,
        viewport: f32,
        passes: usize,
        short_needle: &'static str,
        tall_needle: &'static str,
    }
    let cases: &[Case] = &[
        Case {
            label: "3-pass: Dell vs long Processor cell",
            table: processor_table,
            viewport: 300.0,
            passes: 3,
            short_needle: "Dell",
            tall_needle: "Intel Core",
        },
        Case {
            label: "2-pass: PassMark vs long Summary cell",
            table: summary_table,
            viewport: 200.0,
            passes: 2,
            short_needle: "2,271 / 7,545",
            tall_needle: "Premium build",
        },
    ];

    for case in cases {
        let ctx = egui::Context::default();
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(case.viewport, 800.0),
            )),
            ..egui::RawInput::default()
        };

        // Drive the same `n passes` pattern the production renderer
        // uses. The first pass measures row heights in Grid memory;
        // subsequent passes paint with the cached heights. We
        // capture the last pass's output for assertions.
        let mut last_output = None;
        for _ in 0..case.passes {
            last_output = Some(ctx.run_ui(raw.clone(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    render_table(
                        ui,
                        &case.table,
                        0,
                        crate::ui::table_width::DeficitStrategy::BreakpointWaterFill,
                    );
                });
            }));
        }
        let output = last_output.expect("at least one render pass");

        let find_text = |needle: &str| -> egui::Pos2 {
            output
                .shapes
                .iter()
                .find_map(|cs| match &cs.shape {
                    egui::Shape::Text(t)
                        if t.galley.text() == needle || t.galley.text().contains(needle) =>
                    {
                        Some(t.pos)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| {
                    panic!(
                        "[{label}] expected a text shape for {needle:?}",
                        label = case.label
                    )
                })
        };

        let short_y = find_text(case.short_needle);
        let tall_y = find_text(case.tall_needle);
        let delta = (short_y.y - tall_y.y).abs();
        assert!(
            delta <= 2.0,
            "[{label}] expected short cell top-y ({short_y_y:.1}) to match tall cell top-y \
             ({tall_y_y:.1}); delta={delta:.1}px (Grid centered the short cell instead of \
             top-aligning — TBL-031 / `top_down` layout regression)",
            label = case.label,
            short_y_y = short_y.y,
            tall_y_y = tall_y.y,
        );
    }
}
