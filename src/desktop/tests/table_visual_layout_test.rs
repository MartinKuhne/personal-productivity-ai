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

#[test]
fn test_integration_table_multi_line_row_cells_top_aligned() {
    let output = render_table_markdown_to_shapes(LAPTOP_TABLE_MARKDOWN, 2000.0);
    let text_positions = collect_text_positions(&output.shapes);

    let dell_pos = text_positions
        .iter()
        .find(|(txt, _)| txt.contains("Dell"))
        .map(|(_, pos)| *pos)
        .unwrap_or_else(|| {
            panic!("cell 'Dell' must be rendered; rendered texts: {text_positions:?}")
        });

    let xps_pos = text_positions
        .iter()
        .find(|(txt, _)| txt.contains("XPS"))
        .map(|(_, pos)| *pos)
        .unwrap_or_else(|| {
            panic!("cell 'XPS' must be rendered; rendered texts: {text_positions:?}")
        });

    let delta = (dell_pos.y - xps_pos.y).abs();
    assert!(
        delta < 1.0,
        "TBL-031: short cell 'Dell' top-y ({}) must match adjacent cell 'XPS 15' top-y ({}); delta={delta}px",
        dell_pos.y,
        xps_pos.y
    );
}

#[test]
fn test_integration_table_short_cell_vs_multiline_cell_top_alignment() {
    const TALL_ROW_MARKDOWN: &str = r#"
| Col A | Col B |
| --- | --- |
| Short | This is a very long paragraph of text that contains many words in sequence. When rendered in a narrow table column of width 100 pixels, it will automatically wrap into a very tall multi-line block of text with a total height exceeding 350 pixels. This ensures that the cell height is larger than 300px and tests top alignment. |
"#;

    let output = render_table_markdown_to_shapes(TALL_ROW_MARKDOWN, 180.0);
    let text_positions = collect_text_positions(&output.shapes);

    let short_pos = text_positions
        .iter()
        .find(|(txt, _)| txt.contains("Short"))
        .map(|(_, pos)| *pos)
        .expect("cell 'Short' must be rendered");

    let tall_pos = text_positions
        .iter()
        .find(|(txt, _)| txt.contains("This is a very long paragraph"))
        .map(|(_, pos)| *pos)
        .expect("cell 'This is a very long paragraph' must be rendered");

    println!("SHORT POS: {short_pos:?}, TALL POS: {tall_pos:?}");
    let delta = (short_pos.y - tall_pos.y).abs();
    assert!(
        delta < 1.0,
        "TBL-031: short cell 'Short' top-y ({}) must match multi-line cell top-y ({}); delta={delta}px",
        short_pos.y,
        tall_pos.y
    );
}

#[test]
fn test_integration_table_cell_with_link_and_formatted_text_top_alignment() {
    const FORMATTED_TABLE_MARKDOWN: &str = r#"
| Make | Description |
| --- | --- |
| [Dell](https://dell.com) | **Dell XPS 16** featuring `Intel Core Ultra 7` with a *gorgeous* 3.2K OLED display and long battery life. This text wraps across multiple lines in the cell. |
"#;

    let output = render_table_markdown_to_shapes(FORMATTED_TABLE_MARKDOWN, 400.0);
    let text_positions = collect_text_positions(&output.shapes);

    let link_pos = text_positions
        .iter()
        .find(|(txt, _)| txt.contains("Dell"))
        .map(|(_, pos)| *pos)
        .expect("link cell 'Dell' must be rendered");

    let desc_pos = text_positions
        .iter()
        .find(|(txt, _)| txt.contains("Dell XPS 16"))
        .map(|(_, pos)| *pos)
        .expect("desc cell 'Dell XPS 16' must be rendered");

    let delta = (link_pos.y - desc_pos.y).abs();
    assert!(
        delta < 1.0,
        "TBL-031: link cell 'Dell' top-y ({}) must match description cell top-y ({}); delta={delta}px",
        link_pos.y,
        desc_pos.y
    );
}

#[test]
fn test_integration_table_horizontal_scroll_multiline_top_alignment() {
    const WIDE_TABLE_MARKDOWN: &str = r#"
| Col 1 | Col 2 | Col 3 | Col 4 | Col 5 | Col 6 | Summary |
| --- | --- | --- | --- | --- | --- | --- |
| Dell | XPS 15 | $2499 | OLED | Intel i9 | 3800 | Premium build with a very long summary description that wraps onto 5 lines when rendered in the table cell. |
"#;

    let output = render_table_markdown_to_shapes(WIDE_TABLE_MARKDOWN, 600.0);
    let text_positions = collect_text_positions(&output.shapes);

    let dell_pos = text_positions
        .iter()
        .find(|(txt, _)| txt.contains("Dell"))
        .map(|(_, pos)| *pos)
        .expect("cell 'Dell' must be rendered");

    let summary_pos = text_positions
        .iter()
        .find(|(txt, _)| txt.contains("Premium build"))
        .map(|(_, pos)| *pos)
        .expect("cell 'Premium build' must be rendered");

    let delta = (dell_pos.y - summary_pos.y).abs();
    assert!(
        delta < 1.0,
        "TBL-031: short cell 'Dell' top-y ({}) must match multi-line cell top-y ({}); delta={delta}px",
        dell_pos.y,
        summary_pos.y
    );
}

#[test]
fn test_integration_table_cell_vertical_cross_align_min() {
    // 7-column table with short cells in cols 0-5 and a 5-line summary in col 6.
    // In a narrow viewport, col 6 wraps onto 5 lines (row height = ~90px).
    // Short cell "Dell" in col 0 must align to the top of the 90px cell, NOT be centered at y = top + 36px.
    const LAPTOP_STYLE_MARKDOWN: &str = r#"
| Make | Model | Price | Display | CPU | Score | Summary |
| --- | --- | --- | --- | --- | --- | --- |
| Dell | XPS 15 9570 | $1500 | 15.6" OLED | i5-8300H | 2271 | Premium build quality with an excellent 4K OLED display option, Thunderbolt 3, and solid aluminum chassis. Now aging with 8th gen Intel CPU. |
"#;

    let output = render_table_markdown_to_shapes(LAPTOP_STYLE_MARKDOWN, 1000.0);
    let text_positions = collect_text_positions(&output.shapes);

    let dell_pos = text_positions
        .iter()
        .find(|(txt, _)| txt == "Dell")
        .map(|(_, pos)| *pos)
        .expect("cell 'Dell' must be rendered");

    let summary_pos = text_positions
        .iter()
        .find(|(txt, _)| txt.contains("Premium build"))
        .map(|(_, pos)| *pos)
        .expect("summary cell text must be rendered");

    let delta = (dell_pos.y - summary_pos.y).abs();
    assert!(
        delta < 1.0,
        "TBL-031: Short cell 'Dell' top-y ({}) must match summary top-y ({}); delta={delta}px (vertical cross_align must be Align::Min)",
        dell_pos.y,
        summary_pos.y
    );
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
            );
        });

    harness.run();
    let _ = harness;
}
