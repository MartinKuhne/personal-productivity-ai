//! Unit tests for the markdown table mapping and widget.
//!
//! The solve is driven by the same fixed-width fake the adapter sidecar
//! uses (5 px per body character) so solved widths are exact.

use super::*;
use crate::markdown::model::{InlineElem, RenderEvent, TextStyle};
use crate::markdown::parser::parse_markdown_to_events;
use crate::ui::theme::Palette;
use crate::ui_iced::table_width::{COLUMN_SPACING, TextMetrics};
use iced::font::{Family, Font};
use iced::{Rectangle, Size};
use std::sync::{Arc, Mutex};

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
fn test_striping_alternates_on_odd_rows() {
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
    let column = column_length(present.width);
    let _ = cell_element(Some(present), false, column);
    let _ = cell_element(Some(present), true, column);
    let _ = cell_element(None, false, column);
    let _ = cell_element(None, true, column);
}

#[test]
fn test_empty_cell_element_reserves_the_minimum_height() {
    let element = cell_element(None, false, column_length(40.0));
    assert_eq!(
        element.as_widget().size().height,
        Length::Fixed(EMPTY_CELL_HEIGHT)
    );
}

#[test]
fn test_content_cell_element_shrinks_to_fit() {
    let model = solve(&grid(2, 1), 1000.0);
    let element = cell_element(Some(&model.rows[1][0]), false, column_length(40.0));
    assert_eq!(element.as_widget().size().height, Length::Shrink);
}

/// Defect #17, construction level: a cell under a fixed column must declare
/// that fixed width, never a fluid one — iced's first layout pass skips a
/// fluid cell and the fixed column then collapses to 0 px.
#[test]
fn test_cell_element_declares_its_own_columns_width_never_fill() {
    let model = solve(&grid(2, 3), 1000.0);
    for (column, width) in model.column_widths.iter().enumerate() {
        let element = cell_element(Some(&model.rows[1][column]), false, column_length(*width));
        assert_eq!(
            element.as_widget().size().width,
            Length::Fixed(*width),
            "cell of column {column} must carry its column's solved width"
        );
        assert!(
            !element.as_widget().size().width.is_fill(),
            "cell of column {column} must not declare a fluid width"
        );
    }
}

/// A fluid column keeps a fluid cell — the rule is "same length as the
/// column", not "always fixed".
#[test]
fn test_cell_element_follows_a_fluid_column() {
    let model = solve(&grid(2, 1), 1000.0);
    let element = cell_element(Some(&model.rows[1][0]), false, Length::Fill);
    assert_eq!(element.as_widget().size().width, Length::Fill);
}

#[test]
fn test_scroll_width_adds_the_outer_padding_bands() {
    let model = solve(&grid(2, 3), 20.0);
    assert!(model.needs_horizontal_scroll);
    assert_eq!(
        scroll_width(&model),
        Length::Fixed(model.total_width + 2.0 * CELL_PADDING_X),
        "the scroll fallback must leave room for the two outer padding bands"
    );
}

// --- layout (defect #17) --------------------------------------------------
//
// Everything above solves models and constructs widgets; nothing above *lays a
// table out*. That is exactly the gap defect #17 lived in. iced folds an
// all-`Fixed` column set to `Shrink` (`Length::enclose`,
// `iced_core-0.14.0/src/length.rs:61-66`) and then silently rewrites the
// **first** column to `Length::Fill` (`iced_widget-0.14.2/src/table.rs:129-133`);
// its first layout pass skips every cell whose own width `is_fill()`
// (`table.rs:293`), so a cell declaring `Fill` under a fixed column left
// `metrics.columns` at zero, and the second pass clamped each fixed column
// down to that zero (`iced_core-0.14.0/src/layout/limits.rs:60-65`) while
// column 0 swallowed the viewport. The result was the reported "empty table".
//
// The egui suite that used to catch this (`ui/render/e2e_tests/table_*.rs`)
// was deleted in `23cc35a`. These probes restore it at the only tier that can
// see the collapse: measured bounds.

/// Sub-pixel tolerance for a measured bound.
const TOLERANCE: f32 = 0.01;

/// The defect #17 document verbatim: an all-empty header row and an all-empty
/// (`|||||`) body row inside a four-column table.
const DEFECT17_DOC: &str = "\
|   |   |   |   |
|---|---|---|---|
|||||
|7/17|Thu|12:10 FRANKFURT INT.|Zeitweise Regen|
|7/18|Fri||Wolkig, vereinzelt Regen|
|7/19|Sat|Wedding|Mehr Sonne als Wolken|
|7/20|Sun||Ueberwiegend wolkig|
";

/// What one headless layout pass reports about a mounted table.
#[derive(Debug)]
struct Measured {
    /// Every container's bounds, depth-first: index 0 is the framed table, the
    /// following `rows * columns` entries are the cells in document order
    /// (header row first, then each body row left to right).
    containers: Vec<Rectangle>,
    /// Content width of the first `scrollable` in the tree — present only when
    /// the horizontal-scroll fallback mounted one.
    scroll_content_width: Option<f32>,
}

/// Lays `element` out headless in a window of `viewport` and collects the
/// geometry iced reports through its widget operations.
///
/// iced 0.14's `Table` takes no `Id` and its text widgets report no operation
/// candidate, so a cell is reached through the container that owns it: the
/// traversal is depth-first and each cell is exactly one container.
fn measure(element: Element<'static, Message>, viewport: Size) -> Measured {
    let containers: Arc<Mutex<Vec<Rectangle>>> = Arc::new(Mutex::new(Vec::new()));
    let scroll: Arc<Mutex<Option<f32>>> = Arc::new(Mutex::new(None));
    let container_sink = Arc::clone(&containers);
    let scroll_sink = Arc::clone(&scroll);

    let mut ui =
        iced_test::Simulator::with_size(iced_test::core::Settings::default(), viewport, element);
    // A selector that never matches keeps `iced_test` walking the whole tree;
    // the bounds are collected on the way past.
    let _ = ui.find(move |candidate: iced_test::selector::Candidate<'_>| {
        use iced_test::selector::Candidate;
        match candidate {
            Candidate::Container { bounds, .. } => {
                let _ = container_sink.lock().map(|mut hits| hits.push(bounds));
            }
            Candidate::Scrollable { content_bounds, .. } => {
                let _ = scroll_sink
                    .lock()
                    .map(|mut width| *width = Some(content_bounds.width));
            }
            _ => {}
        }
        None::<()>
    });

    Measured {
        containers: sole_owner(containers),
        scroll_content_width: sole_owner(scroll),
    }
}

/// Takes the value out of the collector a selector filled, asserting the
/// selector was its only owner and its lock was never poisoned.
fn sole_owner<T: std::fmt::Debug>(collector: Arc<Mutex<T>>) -> T {
    Arc::try_unwrap(collector)
        .expect("the selector is the only owner")
        .into_inner()
        .expect("the lock is never poisoned")
}

/// The measured bounds of every cell of `model`, row-major.
fn measure_cells(model: &TableModel, viewport: Size) -> Vec<Vec<Rectangle>> {
    let columns = model.column_count();
    let hits = measure(table_widget(model), viewport);
    assert_eq!(
        hits.containers.len(),
        1 + model.rows.len() * columns,
        "the framed table plus exactly one container per cell: {:?}",
        hits.containers
    );

    let mut cells = Vec::with_capacity(model.rows.len());
    for (row, row_cells) in model.rows.iter().enumerate() {
        cells.push(
            (0..row_cells.len())
                .map(|column| hits.containers[1 + row * columns + column])
                .collect(),
        );
    }
    cells
}

/// The x a cell of `column` must start at, given the solved widths and the
/// gutters the iced `Table` paints between columns.
fn expected_cell_x(model: &TableModel, column: usize) -> f32 {
    CELL_PADDING_X
        + model.column_widths[..column]
            .iter()
            .map(|width| width + COLUMN_SPACING)
            .sum::<f32>()
}

/// The width the framed grid occupies: the solved columns, their gutters and
/// the two outer padding bands the iced `Table` paints around the grid.
fn natural_table_width(model: &TableModel) -> f32 {
    model.total_width + 2.0 * CELL_PADDING_X
}

/// The parser's table payload for a markdown document.
fn table_cells(markdown: &str) -> Vec<Vec<Vec<InlineElem>>> {
    parse_markdown_to_events(markdown)
        .iter()
        .find_map(|event| match event {
            RenderEvent::Table(cells) => Some(cells.clone()),
            _ => None,
        })
        .expect("the document contains a table")
}

/// A wiki fixture, read from the repo root (`CARGO_MANIFEST_DIR` is the root
/// because `[lib] path = "src/app/lib.rs"`).
fn wiki(relative: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("fixture {} should be readable: {err}", path.display()))
}

#[test]
fn test_defect17_table_lays_every_cell_out_at_its_solved_geometry() {
    let model = solve(&table_cells(DEFECT17_DOC), 900.0);
    assert_eq!(model.column_count(), 4);
    assert!(
        !model.needs_horizontal_scroll,
        "285 px of content fits a 900 px pane"
    );

    let cells = measure_cells(&model, Size::new(900.0, 600.0));
    assert_eq!(cells.len(), 6, "blank header + blank row + four data rows");
    let right_edge = natural_table_width(&model);

    for (row, rects) in cells.iter().enumerate() {
        assert_eq!(rects.len(), 4);
        for (column, rect) in rects.iter().enumerate() {
            let solved = model.column_widths[column];
            assert!(
                rect.width > 0.0,
                "cell ({row},{column}) collapsed to {} px",
                rect.width
            );
            assert!(
                (rect.x - expected_cell_x(&model, column)).abs() <= TOLERANCE,
                "cell ({row},{column}) starts at x={}, expected {}",
                rect.x,
                expected_cell_x(&model, column)
            );
            assert!(
                (rect.width - solved).abs() <= TOLERANCE,
                "cell ({row},{column}) is {} px wide, the solve assigned {solved}",
                rect.width
            );
            assert!(
                rect.x + rect.width <= right_edge + TOLERANCE,
                "cell ({row},{column}) ends at {}, past the table's own right edge \
                 ({right_edge}) — its content is off-screen",
                rect.x + rect.width
            );
        }
    }
}

#[test]
fn test_defect17_empty_header_row_holds_its_columns_open() {
    let model = solve(&table_cells(DEFECT17_DOC), 900.0);
    assert!(
        model
            .header_row()
            .unwrap()
            .iter()
            .all(|cell| cell.spans.is_empty()),
        "the fixture's header row is blank in every column"
    );

    let header = &measure_cells(&model, Size::new(900.0, 600.0))[0];
    for (column, rect) in header.iter().enumerate() {
        assert!(
            rect.width > 0.0,
            "blank header cell {column} collapsed to {} px",
            rect.width
        );
        assert!(
            rect.height >= EMPTY_CELL_HEIGHT - TOLERANCE,
            "a blank cell still reserves its line box, got {} px",
            rect.height
        );
    }
}

#[test]
fn test_all_empty_column_keeps_its_neighbours_on_the_grid() {
    let cells = vec![
        vec![cell("left"), Vec::new(), cell("right")],
        vec![cell("aaaa"), Vec::new(), cell("bbbb")],
    ];
    let model = solve(&cells, 600.0);
    assert_eq!(model.column_count(), 3);

    let rects = measure_cells(&model, Size::new(600.0, 400.0));
    for (row, rects) in rects.iter().enumerate() {
        assert!(
            rects[1].width > 0.0,
            "row {row}: the all-empty column collapsed to {} px",
            rects[1].width
        );
        for column in [0, 2] {
            assert!(
                (rects[column].x - expected_cell_x(&model, column)).abs() <= TOLERANCE,
                "row {row} column {column} drifted to x={}",
                rects[column].x
            );
        }
    }
}

/// The viewport-fuzz probe restored from the deleted egui suite: the table must
/// keep every cell measurable and inside its own grid at any pane width.
#[test]
fn slow_test_defect17_table_keeps_every_cell_on_the_grid_at_any_viewport_width() {
    let cells = table_cells(DEFECT17_DOC);
    for width in (200..=1200).step_by(40) {
        let viewport = Size::new(width as f32, 600.0);
        let model = solve(&cells, viewport.width);
        let hits = measure(table_widget(&model), viewport);
        let rects = measure_cells(&model, viewport);
        let right_edge = natural_table_width(&model);

        for (row, row_rects) in rects.iter().enumerate() {
            for (column, rect) in row_rects.iter().enumerate() {
                assert!(
                    rect.width > 0.0,
                    "{width}px viewport: cell ({row},{column}) is {} px wide",
                    rect.width
                );
                assert!(
                    rect.x >= 0.0 && rect.x + rect.width <= right_edge + TOLERANCE,
                    "{width}px viewport: cell ({row},{column}) spans [{}, {}], outside \
                     the table's own [0, {right_edge}]",
                    rect.x,
                    rect.x + rect.width
                );
                if !model.needs_horizontal_scroll {
                    assert!(
                        rect.x + rect.width <= viewport.width + TOLERANCE,
                        "{width}px viewport: cell ({row},{column}) ends at {} but only \
                         {width}px exist and no scroll fallback was mounted",
                        rect.x + rect.width
                    );
                }
            }
        }

        if model.needs_horizontal_scroll {
            assert!(
                hits.scroll_content_width
                    .is_some_and(|content| content >= right_edge - TOLERANCE),
                "{width}px viewport: the scroll fallback exposes {} px of content, \
                 the grid needs {right_edge} px",
                hits.scroll_content_width.unwrap_or_default()
            );
        }
    }
}

#[test]
fn test_wide_table_scroll_fallback_keeps_every_column_reachable() {
    let cells = table_cells(&wiki("test/wiki/Travel/long-table-row.md"));
    let model = solve(&cells, 200.0);
    assert!(
        model.needs_horizontal_scroll,
        "six long columns cannot fit 200 px: {:?}",
        model.column_widths
    );

    let viewport = Size::new(200.0, 600.0);
    let hits = measure(table_widget(&model), viewport);
    let right_edge = natural_table_width(&model);
    assert!(
        hits.scroll_content_width
            .is_some_and(|content| content >= right_edge - TOLERANCE),
        "the scrollable exposes {} px of content but the grid is {right_edge} px wide",
        hits.scroll_content_width.unwrap_or_default()
    );

    let rects = measure_cells(&model, viewport);
    let last = model.column_count() - 1;
    for (row, row_rects) in rects.iter().enumerate() {
        for (column, rect) in row_rects.iter().enumerate() {
            assert!(
                rect.width > 0.0,
                "cell ({row},{column}) collapsed to {} px",
                rect.width
            );
            assert!(
                rect.x + rect.width <= right_edge + TOLERANCE,
                "cell ({row},{column}) ends at {}, past the scrollable content \
                 ({right_edge}) — unreachable even when scrolled to the end",
                rect.x + rect.width
            );
        }
        assert!(
            row_rects[last].x > viewport.width,
            "the last column is meant to need scrolling, it starts at {}",
            row_rects[last].x
        );
    }
}

#[test]
fn test_negative_alignment_fixture_top_aligns_every_cell_in_a_row() {
    let cells = table_cells(&wiki("test/wiki/table_alignment_negative_test.md"));
    let model = solve(&cells, 700.0);
    let rects = measure_cells(&model, Size::new(700.0, 900.0));

    for (row, row_rects) in rects.iter().enumerate().skip(1) {
        let top = row_rects.iter().map(|rect| rect.y).fold(f32::MAX, f32::min);
        let tallest = row_rects
            .iter()
            .map(|rect| rect.height)
            .fold(0.0_f32, f32::max);
        let shortest = row_rects
            .iter()
            .map(|rect| rect.height)
            .fold(f32::MAX, f32::min);
        assert!(
            tallest > shortest + TOLERANCE,
            "row {row} has no wrapped cell, so this probe would prove nothing"
        );
        for (column, rect) in row_rects.iter().enumerate() {
            assert!(
                (rect.y - top).abs() <= TOLERANCE,
                "row {row} column {column} floats at y={top}+{} instead of the row's \
                 top edge",
                rect.y - top
            );
        }
    }
}

#[test]
fn test_an_empty_model_lays_out_as_an_empty_frame_instead_of_panicking() {
    let model = solve(&[], 500.0);
    let hits = measure(table_widget(&model), Size::new(500.0, 300.0));
    assert_eq!(
        hits.containers.len(),
        1,
        "a table with no columns renders its frame and nothing else"
    );
}
