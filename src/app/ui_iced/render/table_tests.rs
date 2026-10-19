//! Unit tests for the markdown table mapping and widget ([TASK-015]).
//!
//! The solve is driven by the same fixed-width fake the adapter sidecar
//! uses (5 px per body character) so solved widths are exact.

use super::*;
use crate::markdown::model::{InlineElem, TextStyle};
use crate::ui::theme::Palette;
use crate::ui_iced::table_width::{COLUMN_SPACING, TextMetrics};
use iced::font::{Family, Font};

fn palette() -> Palette {
    Palette::fastmd_dark()
}

fn fixed_width(text: &str, font: Font, _size: f32) -> f32 {
    if font.family == Family::Monospace {
        text.chars().count() as f32 * 10.0
    } else {
        text.chars().count() as f32 * 5.0
    }
}

fn measurer() -> IcedTextMeasurer<'static> {
    IcedTextMeasurer::new(
        &fixed_width,
        TextMetrics {
            body_size: 14.0,
            mono_size: 13.0,
            line_height: 18.0,
        },
    )
}

fn cell(text: &str) -> Vec<InlineElem> {
    vec![InlineElem::Text(text.to_string(), TextStyle::default())]
}

fn grid(rows: usize, cols: usize) -> Vec<Vec<Vec<InlineElem>>> {
    (0..rows)
        .map(|r| (0..cols).map(|c| cell(&format!("r{r}c{c}"))).collect())
        .collect()
}

fn solve(cells: &[Vec<Vec<InlineElem>>], width: f32) -> TableModel {
    table_model(
        cells,
        width,
        DeficitStrategy::HybridMinPenaltyWaterFill,
        TablePadding::ZERO,
        &measurer(),
        palette(),
    )
}

// --- shape -------------------------------------------------------------

#[test]
fn test_empty_table_maps_to_no_rows_and_no_columns() {
    let model = solve(&[], 800.0);
    assert!(model.rows.is_empty());
    assert!(model.column_widths.is_empty());
    assert_eq!(model.column_count(), 0);
    assert!(!model.needs_horizontal_scroll);
    assert!(model.header_row().is_none());
    assert!(model.body_rows().is_empty());
}

#[test]
fn test_two_by_two_table_shape() {
    let model = solve(&grid(2, 2), 1000.0);
    assert_eq!(model.column_count(), 2);
    assert_eq!(model.rows.len(), 2);
    assert_eq!(model.body_rows().len(), 1, "row 0 is the header");
    assert_eq!(model.header_row().unwrap().len(), 2);
    assert!(!model.needs_horizontal_scroll);
}

#[test]
fn test_single_column_table() {
    let model = solve(&[vec![cell("head")], vec![cell("body")]], 1000.0);
    assert_eq!(model.column_count(), 1);
    assert_eq!(model.column_widths.len(), 1);
    assert!(model.column_widths[0] > 0.0);
    assert!(!model.needs_horizontal_scroll);
}

#[test]
fn test_many_columns_are_all_present() {
    let model = solve(&grid(3, 8), 4000.0);
    assert_eq!(model.column_count(), 8);
    for row in &model.rows {
        assert_eq!(row.len(), 8);
    }
    assert!(
        model.column_widths.iter().all(|w| *w > 0.0),
        "{:?}",
        model.column_widths
    );
}

// --- width solving ------------------------------------------------------

#[test]
fn test_surplus_width_pins_columns_to_max_content() {
    // "head" = 4 chars = 20 px; "body" = 4 chars = 20 px.
    let model = solve(&[vec![cell("head")], vec![cell("body")]], 1000.0);
    assert_eq!(model.column_widths, vec![20.0]);
}

#[test]
fn test_every_cell_carries_its_column_width() {
    let model = solve(&grid(3, 3), 2000.0);
    for (column, width) in model.column_widths.iter().enumerate() {
        for row in &model.rows {
            assert_eq!(
                row[column].width, *width,
                "cell (row, {column}) must carry its column's solved width"
            );
        }
    }
}

#[test]
fn test_zero_available_width_forces_the_scroll_fallback() {
    let model = solve(&grid(2, 3), 0.0);
    assert!(
        model.needs_horizontal_scroll,
        "0 px cannot hold three columns' min_content"
    );
    assert_eq!(model.column_count(), 3);
    assert!(
        model.column_widths.iter().all(|w| *w > 0.0),
        "the fallback keeps every column at its token floor: {:?}",
        model.column_widths
    );
}

#[test]
fn test_very_narrow_viewport_falls_back_to_scroll() {
    let model = solve(&grid(2, 4), 30.0);
    assert!(model.needs_horizontal_scroll);
}

#[test]
fn test_wide_viewport_fits_inside_the_available_width() {
    let available = 1200.0;
    let model = solve(&grid(4, 4), available);
    assert!(!model.needs_horizontal_scroll);
    let spent: f32 = model.column_widths.iter().sum::<f32>()
        + (model.column_count() as f32 - 1.0) * COLUMN_SPACING
        + 2.0 * CELL_PADDING_X;
    assert!(
        spent <= available + f32::EPSILON,
        "solved grid wants {spent} px from {available} px"
    );
}

#[test]
fn test_padding_widens_every_column() {
    let padded = TablePadding {
        top: 0.0,
        bottom: 0.0,
        left: 3.0,
        right: 3.0,
    };
    let model = table_model(
        &[vec![cell("head")]],
        1000.0,
        DeficitStrategy::HybridMinPenaltyWaterFill,
        padded,
        &measurer(),
        palette(),
    );
    assert_eq!(model.column_widths, vec![20.0 + 6.0]);
}

// --- cell content -------------------------------------------------------

#[test]
fn test_header_cells_are_flagged_and_strong_coloured() {
    let model = solve(&grid(3, 2), 1000.0);
    for cell_model in model.header_row().unwrap() {
        assert!(cell_model.is_header);
        assert!(
            !cell_model.spans.is_empty(),
            "header cells carry their content"
        );
        for span in &cell_model.spans {
            assert_eq!(
                span.color,
                palette().text_strong,
                "header text is emphasised by colour"
            );
        }
    }
}

#[test]
fn test_body_cells_are_not_header_flagged_and_use_body_colour() {
    let model = solve(&grid(2, 2), 1000.0);
    for row in model.body_rows() {
        for cell_model in row {
            assert!(!cell_model.is_header);
            assert_eq!(cell_model.spans[0].color, palette().text);
        }
    }
}

#[test]
fn test_blank_cell_maps_to_empty_spans() {
    let cells = vec![vec![cell("a"), cell("b")], vec![cell("c"), Vec::new()]];
    let model = solve(&cells, 1000.0);
    assert!(
        model.rows[1][1].spans.is_empty(),
        "an empty cell has no styled runs"
    );
    assert!(!model.rows[1][1].is_header, "row 1 is a body row");
}

#[test]
fn test_cell_with_link_keeps_its_link_target() {
    let cells = vec![vec![vec![InlineElem::Link(
        "https://x.test".to_string(),
        "click".to_string(),
    )]]];
    let model = solve(&cells, 1000.0);
    assert_eq!(
        model.rows[0][0].spans[0].link.as_deref(),
        Some("https://x.test")
    );
    assert!(model.rows[0][0].spans[0].underline);
}

#[test]
fn test_cell_with_code_run_is_monospaced() {
    let cells = vec![vec![vec![InlineElem::Text(
        "code".to_string(),
        TextStyle {
            code: true,
            ..Default::default()
        },
    )]]];
    let model = solve(&cells, 1000.0);
    assert_eq!(model.rows[0][0].spans[0].font, Font::MONOSPACE);
}

// --- row helpers ---------------------------------------------------------

#[test]
fn test_body_rows_data_indexes_are_sequential() {
    let model = solve(&grid(4, 2), 1000.0);
    let rows = model.body_rows_data();
    assert_eq!(rows.len(), 3, "4 rows minus the header");
    for (position, row) in rows.iter().enumerate() {
        assert_eq!(row.index, position);
        assert_eq!(row.cells.len(), 2);
    }
}

#[test]
fn test_stripe_parity_matches_the_legacy_rule() {
    assert!(!TableModel::is_striped(0));
    assert!(TableModel::is_striped(1));
    assert!(!TableModel::is_striped(2));
    assert!(TableModel::is_striped(3));
}

#[test]
fn test_body_rows_of_a_header_only_table_is_empty() {
    let model = solve(&[vec![cell("only header")]], 1000.0);
    assert_eq!(model.rows.len(), 1);
    assert!(model.body_rows().is_empty());
    assert!(model.body_rows_data().is_empty());
}

// --- widget construction --------------------------------------------------

#[test]
fn test_table_widget_builds_for_a_normal_table() {
    let model = solve(&grid(3, 3), 1000.0);
    let element = table_widget(&model);
    let _ = element;
}

#[test]
fn test_table_widget_builds_for_the_scroll_fallback() {
    let model = solve(&grid(2, 4), 20.0);
    assert!(model.needs_horizontal_scroll);
    let element = table_widget(&model);
    let _ = element;
}

#[test]
fn test_table_widget_builds_for_an_empty_model() {
    let model = solve(&[], 500.0);
    let element = table_widget(&model);
    let _ = element;
}

#[test]
fn test_cell_element_builds_for_present_and_absent_cells() {
    let model = solve(&grid(2, 2), 1000.0);
    let present = &model.rows[1][0];
    let _ = cell_element(Some(present), false);
    let _ = cell_element(Some(present), true);
    let _ = cell_element(None, false);
    let _ = cell_element(None, true);
}

#[test]
fn test_empty_cell_element_reserves_the_minimum_height() {
    let element = cell_element(None, false);
    assert_eq!(
        element.as_widget().size().height,
        Length::Fixed(EMPTY_CELL_HEIGHT)
    );
}

#[test]
fn test_content_cell_element_shrinks_to_fit() {
    let model = solve(&grid(2, 1), 1000.0);
    let element = cell_element(Some(&model.rows[1][0]), false);
    assert_eq!(element.as_widget().size().height, Length::Shrink);
}
