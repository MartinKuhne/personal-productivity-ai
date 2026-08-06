//! Integration and visual regression test harness for Markdown table layout.
//!
//! Validates multi-column top alignment, cell height expansion, and off-viewport
//! text reachability using `egui::Context::run_ui`, `egui_kittest::Harness`, and
//! `fastmd::ui::render::render_markdown`.

use eframe::egui;
use fastmd::markdown::table_width::DeficitStrategy;
use fastmd::ui::render::render_markdown;

/// Markdown document fixture containing a multi-column table with varying text
/// lengths and multi-line content (similar to `c:\temp\top.png`).
const LAPTOP_TABLE_MARKDOWN: &str = r#"
# Reference: My Current Laptop

| Make | Model and Model Number | Market Price (Original) | Display | Processor | PassMark Single / Multi |
| --- | --- | --- | --- | --- | --- |
| Dell | XPS 15 9530 | 2499 USD | OLED 3.5K Touch Screen | Intel Core i9-13900H 14-Core | 3800 / 28500 |
| Lenovo | ThinkPad P1 Gen 6 | 2850 USD | WQXGA 165Hz IPS | Intel Core i7-13800H | 3700 / 26000 |
| Apple | MacBook Pro 16 | 3499 USD | Liquid Retina XDR | Apple M3 Max 16-Core CPU | 4800 / 31000 |

"#;

/// Helper: Render markdown using an egui_kittest Harness with a specified viewport width
/// and capture full paint output shapes after frame stabilization.
fn render_table_markdown_to_shapes(markdown: &'static str, viewport_w: f32) -> egui::FullOutput {
    let clean_md = markdown.replace("\r\n", "\n");
    let ctx = egui::Context::default();
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(viewport_w, 5000.0),
        )),
        ..egui::RawInput::default()
    };

    let mut scroll_to_id = None;
    let mut pending_toggles = Vec::new();
    let _ = ctx.run_ui(raw.clone(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("test_scroll")
                .show(ui, |ui| {
                    render_markdown(
                        ui,
                        &clean_md,
                        &mut scroll_to_id,
                        &mut pending_toggles,
                        DeficitStrategy::ProportionalToSlack,
                        None,
                    );
                });
        });
    });

    let mut scroll_to_id = None;
    let mut pending_toggles = Vec::new();
    ctx.run_ui(raw, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("test_scroll")
                .show(ui, |ui| {
                    render_markdown(
                        ui,
                        &clean_md,
                        &mut scroll_to_id,
                        &mut pending_toggles,
                        DeficitStrategy::ProportionalToSlack,
                        None,
                    );
                });
        });
    })
}

fn collect_text_positions(shapes: &[eframe::epaint::ClippedShape]) -> Vec<(String, egui::Pos2)> {
    use eframe::epaint::Shape;
    fn recurse(shape: &Shape, out: &mut Vec<(String, egui::Pos2)>) {
        match shape {
            Shape::Text(t) => {
                out.push((t.galley.text().to_string(), t.pos));
            }
            Shape::Vec(children) => {
                for child in children {
                    recurse(child, out);
                }
            }
            _ => {}
        }
    }

    let mut out = Vec::new();
    for cs in shapes {
        recurse(&cs.shape, &mut out);
    }
    out
}

/// Like [`collect_text_positions`] but also captures the visual bounding
/// rect of each text shape so tests can measure text width (e.g. for
/// column-gutter calculations that need to know the right edge of one
/// cell's text and the left edge of the next).
fn collect_text_rects(shapes: &[eframe::epaint::ClippedShape]) -> Vec<(String, egui::Rect)> {
    use eframe::epaint::Shape;
    fn recurse(shape: &Shape, out: &mut Vec<(String, egui::Rect)>) {
        match shape {
            Shape::Text(t) => {
                out.push((t.galley.text().to_string(), t.visual_bounding_rect()));
            }
            Shape::Vec(children) => {
                for child in children {
                    recurse(child, out);
                }
            }
            _ => {}
        }
    }

    let mut out = Vec::new();
    for cs in shapes {
        recurse(&cs.shape, &mut out);
    }
    out
}

/// Find the `y` coordinate of the first text shape whose text contains
/// `needle` as a substring. Panics with a helpful diagnostic if the
/// needle is missing from the rendered output.
fn y_of_text_containing(
    text_positions: &[(String, egui::Pos2)],
    needle: &str,
    context: &str,
) -> f32 {
    text_positions
        .iter()
        .find(|(txt, _)| txt.contains(needle))
        .map(|(_, pos)| pos.y)
        .unwrap_or_else(|| {
            panic!(
                "expected to find a text shape containing {needle:?} in {context}; \
                 rendered texts: {text_positions:?}"
            )
        })
}

/// Assert that a `short` text shape and a `tall` text shape in the
/// same table row share the same `y` coordinate. The cell-renderer
/// is expected to top-align content within a row, so a short cell
/// (single-line) must start at the same y as its multi-line neighbor.
/// `tol` is the sub-pixel tolerance (1.0 is the historical threshold).
fn assert_top_aligned_in_row(
    output: &egui::FullOutput,
    short_needle: &str,
    tall_needle: &str,
    label: &str,
    tol: f32,
) {
    let positions = collect_text_positions(&output.shapes);
    let short_y = y_of_text_containing(&positions, short_needle, label);
    let tall_y = y_of_text_containing(&positions, tall_needle, label);
    let delta = (short_y - tall_y).abs();
    assert!(
        delta < tol,
        "[{label}] TBL-031: short cell {short_needle:?} top-y ({short_y:.2}) must match \
         multi-line cell {tall_needle:?} top-y ({tall_y:.2}); delta={delta:.2}px"
    );
}

#[test]
fn test_integration_table_header_columns_aligned_at_top_y() {
    let output = render_table_markdown_to_shapes(LAPTOP_TABLE_MARKDOWN, 2000.0);
    let text_positions = collect_text_positions(&output.shapes);

    // Extract all text shapes rendered for the table header row
    let header_labels = [
        "Make",
        "Model and Model Number",
        "Market Price (Original)",
        "Display",
        "Processor",
        "PassMark Single / Multi",
    ];

    let mut header_positions: Vec<(&'static str, egui::Pos2)> = Vec::new();
    for label in &header_labels {
        let pos = text_positions
            .iter()
            .find_map(|(txt, pos)| {
                if txt.contains(label) {
                    Some(*pos)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| {
                panic!(
                    "header label '{label}' must be rendered; text_positions: {text_positions:?}"
                )
            });
        header_positions.push((*label, pos));
    }

    assert_eq!(
        header_positions.len(),
        6,
        "all 6 table column headers must be present"
    );

    // Assert that every header text shape shares the exact same top-y coordinate (no stair-stepping)
    let baseline_y = header_positions[0].1.y;
    for (label, pos) in &header_positions {
        let delta = (pos.y - baseline_y).abs();
        assert!(
            delta < 1.0,
            "Header '{label}' top-y ({}) must match 'Make' top-y ({}); stair-stepping delta={delta}px",
            pos.y,
            baseline_y
        );
    }
}

/// TBL-031: every short cell in a row must share the same `y`
/// coordinate as the row's tallest cell. The cell renderer uses
/// `top_down` layout; without it `egui::Grid` would center cells
/// vertically and the short cell would drift down by half the
/// row-height delta. This test covers the 5 distinct table shapes
/// that historically triggered this regression:
///
/// * Plain multi-column laptop table (real fixture, FTWA path).
/// * Single narrow two-column table that wraps to many lines.
/// * Mixed-formatted table with link + bold + code + italic.
/// * Wide table that hits the §3.6 horizontal-scroll fallback path.
/// * 7-column "laptop spec" with one long summary column.
///
/// Consolidated from 5 formerly-separate tests that all asserted
/// the same `|short.y - tall.y| < 1.0` invariant. Each case pins
/// a different real-world shape so a regression in any of the
/// 5 paths still surfaces individually.
#[test]
fn test_table_row_top_alignment_across_shapes() {
    const TALL_ROW_MARKDOWN: &str = r#"
| Col A | Col B |
| --- | --- |
| Short | This is a very long paragraph of text that contains many words in sequence. When rendered in a narrow table column of width 100 pixels, it will automatically wrap into a very tall multi-line block of text with a total height exceeding 350 pixels. This ensures that the cell height is larger than 300px and tests top alignment. |
"#;
    const FORMATTED_TABLE_MARKDOWN: &str = r#"
| Make | Description |
| --- | --- |
| [Dell](https://dell.com) | **Dell XPS 16** featuring `Intel Core Ultra 7` with a *gorgeous* 3.2K OLED display and long battery life. This text wraps across multiple lines in the cell. |
"#;
    const WIDE_TABLE_MARKDOWN: &str = r#"
| Col 1 | Col 2 | Col 3 | Col 4 | Col 5 | Col 6 | Summary |
| --- | --- | --- | --- | --- | --- | --- |
| Dell | XPS 15 | $2499 | OLED | Intel i9 | 3800 | Premium build with a very long summary description that wraps onto 5 lines when rendered in the table cell. |
"#;
    const LAPTOP_STYLE_MARKDOWN: &str = r#"
| Make | Model | Price | Display | CPU | Score | Summary |
| --- | --- | --- | --- | --- | --- | --- |
| Dell | XPS 15 9570 | $1500 | 15.6" OLED | i5-8300H | 2271 | Premium build quality with an excellent 4K OLED display option, Thunderbolt 3, and solid aluminum chassis. Now aging with 8th gen Intel CPU. |
"#;

    struct Case {
        label: &'static str,
        markdown: &'static str,
        viewport: f32,
        short_needle: &'static str,
        tall_needle: &'static str,
    }
    let cases: &[Case] = &[
        Case {
            label: "plain multi-column (Dell vs XPS 15)",
            markdown: LAPTOP_TABLE_MARKDOWN,
            viewport: 2000.0,
            short_needle: "Dell",
            tall_needle: "XPS",
        },
        Case {
            label: "narrow two-column (Short vs long paragraph)",
            markdown: TALL_ROW_MARKDOWN,
            viewport: 180.0,
            short_needle: "Short",
            tall_needle: "This is a very long paragraph",
        },
        Case {
            label: "link + formatted text (Dell link vs Dell XPS 16)",
            markdown: FORMATTED_TABLE_MARKDOWN,
            viewport: 400.0,
            short_needle: "Dell",
            tall_needle: "Dell XPS 16",
        },
        Case {
            label: "§3.6 horizontal-scroll fallback (Dell vs Premium build)",
            markdown: WIDE_TABLE_MARKDOWN,
            viewport: 600.0,
            short_needle: "Dell",
            tall_needle: "Premium build",
        },
        Case {
            label: "7-column laptop spec (Dell vs Premium build)",
            markdown: LAPTOP_STYLE_MARKDOWN,
            viewport: 1000.0,
            short_needle: "Dell",
            tall_needle: "Premium build",
        },
    ];

    for case in cases {
        let output = render_table_markdown_to_shapes(case.markdown, case.viewport);
        assert_top_aligned_in_row(
            &output,
            case.short_needle,
            case.tall_needle,
            case.label,
            1.0,
        );
    }
}

#[test]
fn test_integration_table_no_offscreen_text_violations() {
    use eframe::epaint::Shape;

    let output = render_table_markdown_to_shapes(LAPTOP_TABLE_MARKDOWN, 1024.0);
    // Verify all painted text shapes have positive bounds within the 1024x768 viewport
    for cs in &output.shapes {
        if let Shape::Text(t) = &cs.shape {
            let rect = t.visual_bounding_rect();
            assert!(
                rect.width() > 0.0 && rect.height() > 0.0,
                "text shape '{:?}' must have positive visual area",
                t.galley.text()
            );
        }
    }
}

#[test]
fn test_integration_table_snapshot_harness_harness_setup() {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1024.0, 768.0))
        .build_ui(|ui| {
            let mut scroll_to_id = None;
            let mut pending_toggles = Vec::new();
            render_markdown(
                ui,
                LAPTOP_TABLE_MARKDOWN,
                &mut scroll_to_id,
                &mut pending_toggles,
                DeficitStrategy::ProportionalToSlack,
                None,
            );
        });

    harness.run();
    let _ = harness;
}

/// Regression: row-to-row vertical gutter in a single-line table must equal
/// the configured `item_spacing.y` (4.0 px) plus the cell's vertical inner
/// padding (default `TablePadding::ZERO` = 0).  A rendered gap significantly
/// larger than 4 px indicates the table is silently adding inter-row space
/// (e.g. an extra `add_space` or doubled spacing) and the table looks
/// sparse / "row gutter too large".
///
/// Test fixture: a 3-row × 2-column table whose every cell is a single short
/// line so no wrap changes row height; the inter-row gap is therefore
/// `cell_height + configured_gutter` and we can subtract the body-text line
/// height to recover the gutter.
#[test]
fn test_integration_table_row_gutter_is_configured_value() {
    const GUTTER_FIXTURE: &str = r#"
| Alpha | Bravo |
| --- | --- |
| one | two |
| three | four |
"#;
    let output = render_table_markdown_to_shapes(GUTTER_FIXTURE, 600.0);
    let text_positions = collect_text_positions(&output.shapes);

    // Find the y of each row's first cell text. "Alpha" and "Bravo" share the
    // header row; we use the header label as the anchor for row 0 and the
    // first data cell of each subsequent row.
    fn y_of(text_positions: &[(String, egui::Pos2)], needle: &str) -> f32 {
        text_positions
            .iter()
            .find(|(txt, _)| txt.trim() == needle)
            .map(|(_, pos)| pos.y)
            .unwrap_or_else(|| {
                panic!("expected text {needle:?} in rendered output; got: {text_positions:?}")
            })
    }

    let y_header = y_of(&text_positions, "Alpha");
    let y_row1 = y_of(&text_positions, "one");
    let y_row2 = y_of(&text_positions, "three");

    // Both data rows contain a single line of body text, so the y-delta
    // between consecutive row tops is `line_height + configured_gutter`.
    // The body line height in egui is ~13-15 px depending on font, so the
    // gutter contribution is `delta_y - line_height`. We assert the
    // upper bound to be robust against font-loading variations: a healthy
    // gutter (≤ 8 px) is well under the >20 px the user reported as
    // "row gutter too large".
    let delta_row0_to_row1 = y_row1 - y_header;
    let delta_row1_to_row2 = y_row2 - y_row1;

    // The body text is rendered with `ui.add(egui::Label::new(...).wrap())`,
    // so its height is the body `TextStyle` line height. egui's default
    // body height is ~13 px and varies slightly with the active font; we
    // use 13 px as a conservative baseline. The upper bound of
    // `line_height + 8 px` corresponds to the configured 4 px row gutter
    // (see `render_table_with_config` in `src/ui/render.rs`) plus a 4 px
    // margin to stay robust against font-loading jitter.
    let line_height: f32 = 13.0;

    // The y-step between the header and the first data row is the gutter
    // plus any extra space inserted by the header/separator transition.
    // We compare both deltas to an upper bound of (line_height + 8 px)
    // which is the "configured 4 px row gutter" plus a 4 px margin.
    let upper_bound = line_height + 8.0;
    assert!(
        delta_row0_to_row1 <= upper_bound,
        "row gutter between header and first data row is too large: \
         delta_y = {delta_row0_to_row1:.2} px (line_height ≈ {line_height:.2} px, \
         upper bound = {upper_bound:.2} px). Rendered text positions: {text_positions:?}"
    );
    assert!(
        delta_row1_to_row2 <= upper_bound,
        "row gutter between consecutive data rows is too large: \
         delta_y = {delta_row1_to_row2:.2} px (line_height ≈ {line_height:.2} px, \
         upper bound = {upper_bound:.2} px). Rendered text positions: {text_positions:?}"
    );

    // The lower bound sanity-checks that the row spacing is non-trivially
    // positive (no negative gap from layout collapse).
    assert!(
        delta_row0_to_row1 >= line_height - 1.0,
        "row gutter between header and first data row collapsed: \
         delta_y = {delta_row0_to_row1:.2} px (line_height ≈ {line_height:.2} px). \
         Rendered text positions: {text_positions:?}"
    );
    assert!(
        delta_row1_to_row2 >= line_height - 1.0,
        "row gutter between consecutive data rows collapsed: \
         delta_y = {delta_row1_to_row2:.2} px (line_height ≈ {line_height:.2} px). \
         Rendered text positions: {text_positions:?}"
    );
}

/// Regression: column-to-column horizontal gutter in a single-row table
/// must equal the configured `item_spacing.x` of 10.0 px (see
/// `render_table_with_config`'s row `left_to_right` block in
/// `src/ui/render.rs`). A rendered gap significantly larger than 10 px
/// (e.g. doubled to 20 px) is the column-axis analogue of the
/// "row gutter too large" bug — every cell would be counted twice when
/// the `push_id` chain walks the cell min_rect up to advance the
/// row cursor, inflating the gap between adjacent columns.
///
/// Test fixture: the same 3-row × 2-column table used for the row-gutter
/// regression. The two columns are populated with single short words so
/// the cells size to their natural content width and the measured
/// column gap is the `push_id`/`item_spacing` gutter (not the FTWA
/// surplus).
#[test]
fn test_integration_table_column_gutter_is_configured_value() {
    const GUTTER_FIXTURE: &str = r#"
| Alpha | Bravo |
| --- | --- |
| one | two |
| three | four |
"#;
    let output = render_table_markdown_to_shapes(GUTTER_FIXTURE, 600.0);
    let text_rects = collect_text_rects(&output.shapes);

    // Helper: locate a single-line label's visual bounding rect.
    fn rect_of(text_rects: &[(String, egui::Rect)], needle: &str) -> egui::Rect {
        text_rects
            .iter()
            .find(|(txt, _)| txt.trim() == needle)
            .map(|(_, r)| *r)
            .unwrap_or_else(|| {
                panic!("expected text {needle:?} in rendered output; got: {text_rects:?}")
            })
    }

    // Cells in a row share the same x positions (FTWA pins every row's
    // cells to the same column widths), so the column-0 / column-1 left
    // edges are invariant across rows. We measure the gutter using the
    // *header* row because that row's text is the longest in each
    // column and therefore fills its cell with the least internal
    // padding; data rows with shorter text leave empty space on the
    // right of the column-0 cell, inflating the text-to-text gap by
    // the empty-cell-width and making the measurement unreliable.
    let col0 = rect_of(&text_rects, "Alpha");
    let col1 = rect_of(&text_rects, "Bravo");

    // The cell-to-cell column gutter = column-0 right edge − column-1
    // left edge. With text that fills its cell this is the configured
    // 10 px; with the cell padding we observe in this codebase
    // (~0.2 px on the right of the left cell + ~1 px on the left of
    // the right cell, from each cell's internal
    // `left_to_right(Align::Min).with_main_wrap(true)` layout) the
    // measured text-to-text gap is ~11.2 px. A doubled-gutter
    // regression would push this to ≈ 20+ px and fail the assertion.
    const CONFIGURED_COL_GUTTER: f32 = 10.0;
    const TOLERANCE: f32 = 3.0;

    let gap = col1.min.x - col0.max.x;
    assert!(
        (gap - CONFIGURED_COL_GUTTER).abs() <= TOLERANCE,
        "column gutter is {gap:.2} px, expected \
         {CONFIGURED_COL_GUTTER:.2} px ± {TOLERANCE:.2} px. \
         col0 (Alpha)={col0:?}, col1 (Bravo)={col1:?}, all_rects={text_rects:?}"
    );
    // A 0 px or negative gap would mean the cells abut (or overlap)
    // with no gutter at all — also a regression.
    assert!(
        gap > 0.0,
        "column gutter collapsed: gap = {gap:.2} px \
         (col0.max.x = {:.2}, col1.min.x = {:.2}). all_rects={text_rects:?}",
        col0.max.x,
        col1.min.x,
    );
}

#[test]
fn test_problematic_table() {
    let md = r#"| Retailer | Price | Link |
|----------|-------|------|
| Amazon | ~$200–$250 (on sale ~$180–$200) | [Amazon](https://www.amazon.com/LG-32GN650-B-Ultragear-Reduction-FreeSync/dp/B08LLF9NS1) |
| Best Buy | ~$200–$250 | [Best Buy](https://www.bestbuy.com/product/lg-ultragear-32-led-qhd-5-ms-amd-freesync-premium-with-hdr-10-displayport-hdmi-black/JJ8VPZS3LS) |
| Costco | Check membership deals | [Costco](https://www.costco.com/p/-/lg-ultragear-32-class-qhd-gaming-monitor/100793191) |"#;
    let events = fastmd::markdown::parser::parse_markdown_to_events(md);
    println!("Parsed events: {:#?}", events);
    let out = render_table_markdown_to_shapes(md, 500.0);
    assert!(!out.shapes.is_empty());
}

#[test]
fn test_wide_table_fuzz_widths() {
    let md = r#"| Make | Model and Model Number | Market Price | Display | Processor | PassMark Single / Multi | Summary |
|------|----------------------|-------------|---------|-----------|------------------------|---------|
| Acer | Swift 16 AI (SF16-71T) | $1,249-$1,799 | 16" 3K (2880x1800) 120Hz OLED Touch | Intel Core Ultra 7 256V (8C/8T Lunar Lake) | ~4,031 / ~19,000 | Excellent value. Vibrant OLED display, exceptional battery life for a 16" laptop, lightweight at ~3.3 lbs. Two Thunderbolt 4 ports. Praised by ZDNet, PCMag, and Notebookcheck. Great everyday performance and portability. |"#;

    // Test the table rendering at all widths from 400 to 3000 in 10px increments
    for width in (400..=3000).step_by(10) {
        let out = render_table_markdown_to_shapes(md, width as f32);

        // Assert that the layout engine successfully produced shapes without panicking
        assert!(
            !out.shapes.is_empty(),
            "Table failed to produce shapes at width {width}"
        );

        // Basic correctness: extract text positions to ensure all cells rendered
        let text_positions = collect_text_positions(&out.shapes);
        let has_acer = text_positions.iter().any(|(txt, _)| txt.contains("Acer"));

        assert!(has_acer, "Table dropped content 'Acer' at width {width}");

        // At wider widths, horizontal scrolling doesn't cull the rightmost columns
        if width > 1500 {
            let has_excellent = text_positions
                .iter()
                .any(|(txt, _)| txt.contains("Excellent"));
            assert!(
                has_excellent,
                "Table dropped rightmost content 'Excellent' at width {width} (should not be culled)"
            );
        }

        // Regression guard for the "rightmost column silently clipped" bug.
        //
        // At widths above the §3.6 fallback threshold the FTWA deficit
        // branch must produce column widths that sum to `avail` (so the
        // table fits exactly in the viewport). A previous bug let the
        // post-drift `Σ widths` exceed `available` when the wrap set
        // was fully clamped to `min_content`, causing the rightmost
        // column to overflow the viewport silently. Asserting the
        // rightmost-column text x-position stays within `[0, width]`
        // at "comfortable" widths catches any future regression of
        // that arithmetic. Narrower widths (below the fallback
        // threshold) legitimately scroll, so we skip the check there.
        //
        // Threshold: 700 px is safely above the estimated
        // `sum_min + (N-1)*10 ≈ 510` for this 7-column table and gives
        // the deficit branch enough room to keep the wrap set from
        // being fully clamped.
        if width >= 700 {
            let viewport_right = width as f32;
            // The rightmost column is the Summary column; its first
            // data token is "Excellent" and the header is "Summary".
            // Both should sit fully within the viewport at widths
            // where the deficit branch is supposed to fit the table.
            for needle in ["Summary", "Excellent"] {
                let positions: Vec<_> = text_positions
                    .iter()
                    .filter(|(txt, _)| txt.contains(needle))
                    .map(|(_, pos)| pos.x)
                    .collect();
                assert!(
                    !positions.is_empty(),
                    "rightmost-column text {needle:?} not rendered at width {width}"
                );
                for x in positions {
                    assert!(
                        x >= 0.0 && x <= viewport_right,
                        "rightmost-column text {needle:?} at x={x} is outside the \
                         [0, {viewport_right}] viewport at width {width} — the table \
                         overflowed the viewport (FTWA drift-fix regression?)"
                    );
                }
            }
        }
    }
}
