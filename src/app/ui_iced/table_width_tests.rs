//! Unit tests for the iced FTWA adapter ([TASK-015]).
//!
//! Covers the three stages of the adapter contract: available-width →
//! solver-input mapping, the [`IcedTextMeasurer`] cell measurements, and
//! solver-output → iced column-width mapping. Measurement is driven by a
//! fixed-width fake ([`fixed_width`]: 5 px per body character, 10 px per
//! monospace character) so every assertion is exact.

use super::*;
use crate::markdown::model::{InlineElem, TextStyle};
use crate::markdown::table_width::CellTokens;
use iced::font::{Family, Font};

/// Deterministic fake measurer: 5 px per body char, 10 px per mono char.
fn fixed_width(text: &str, font: Font, _size: f32) -> f32 {
    if font.family == Family::Monospace {
        text.chars().count() as f32 * 10.0
    } else {
        text.chars().count() as f32 * 5.0
    }
}

/// Measurer over the fake, with the FastMD line height.
fn fake_measurer() -> IcedTextMeasurer<'static> {
    IcedTextMeasurer::new(
        &fixed_width,
        TextMetrics {
            body_size: 14.0,
            mono_size: 13.0,
            line_height: 18.0,
        },
    )
}

/// One-cell table shorthand.
fn cell(text: &str) -> Vec<InlineElem> {
    vec![InlineElem::Text(text.to_string(), TextStyle::default())]
}

/// A `rows x cols` table of `"c<r>c<c>"` labels.
fn grid(rows: usize, cols: usize) -> Vec<Vec<Vec<InlineElem>>> {
    (0..rows)
        .map(|r| {
            (0..cols)
                .map(|c| cell(&format!("r{r}c{c}")))
                .collect::<Vec<_>>()
        })
        .collect()
}

// --- heuristic width ---------------------------------------------------

#[test]
fn test_heuristic_width_scales_with_character_count() {
    assert_eq!(heuristic_width("", Font::DEFAULT, 10.0), 0.0);
    assert_eq!(
        heuristic_width("ab", Font::DEFAULT, 10.0),
        2.0 * PROPORTIONAL_AVG_ADVANCE * 10.0
    );
    assert_eq!(
        heuristic_width("abcd", Font::DEFAULT, 10.0),
        4.0 * PROPORTIONAL_AVG_ADVANCE * 10.0
    );
}

#[test]
fn test_heuristic_width_monospace_is_wider_than_proportional() {
    let body = heuristic_width("abc", Font::DEFAULT, 14.0);
    let mono = heuristic_width("abc", Font::MONOSPACE, 14.0);
    assert!(
        mono > body,
        "mono ({mono}) must advance more per character than proportional ({body})"
    );
}

#[test]
fn test_heuristic_width_counts_characters_not_bytes() {
    // 'é' is 2 UTF-8 bytes but one scalar value.
    assert_eq!(
        heuristic_width("é", Font::DEFAULT, 10.0),
        heuristic_width("a", Font::DEFAULT, 10.0)
    );
}

// --- available-width mapping -------------------------------------------

#[test]
fn test_solver_available_width_subtracts_outer_padding_only() {
    assert_eq!(solver_available_width(100.0), 100.0 - 2.0 * CELL_PADDING_X);
    assert_eq!(solver_available_width(20.0), 20.0 - 2.0 * CELL_PADDING_X);
}

#[test]
fn test_solver_available_width_clamps_at_zero() {
    assert_eq!(solver_available_width(0.0), 0.0);
    assert_eq!(solver_available_width(-500.0), 0.0);
    assert_eq!(solver_available_width(-2.0 * CELL_PADDING_X), 0.0);
}

#[test]
fn test_column_spacing_is_twice_cell_padding_plus_separator() {
    // The iced identity `spacing_x = 2 * padding_x + separator_x` with
    // separator_x = 0 must land on COLUMN_SPACING.
    assert_eq!(2.0 * CELL_PADDING_X, COLUMN_SPACING);
    assert_eq!(2.0 * CELL_PADDING_Y, ROW_SPACING);
}

// --- measurer: cell content --------------------------------------------

#[test]
fn test_measure_cell_single_token_max_equals_min() {
    let measurer = fake_measurer();
    let (max, min, tokens) = measurer.measure_cell(&cell("hello"));
    assert_eq!(max, 25.0);
    assert_eq!(min, 25.0, "a single token's min equals its max");
    assert_eq!(tokens.token_widths, vec![25.0]);
}

#[test]
fn test_measure_cell_multi_word_splits_into_tokens() {
    let measurer = fake_measurer();
    let (max, min, tokens) = measurer.measure_cell(&cell("hello world"));
    assert_eq!(max, 55.0, "max is the whole single-line string");
    assert_eq!(min, 25.0, "min is the longest token");
    assert_eq!(tokens.token_widths, vec![25.0, 25.0]);
}

#[test]
fn test_measure_cell_min_is_the_longest_token() {
    let measurer = fake_measurer();
    let (max, min, tokens) = measurer.measure_cell(&cell("a bb cccc"));
    assert_eq!(max, 9.0 * 5.0);
    assert_eq!(min, 20.0);
    assert_eq!(tokens.token_widths, vec![5.0, 10.0, 20.0]);
}

#[test]
fn test_measure_cell_fragmented_token_stays_one_token() {
    // Mirrors the legacy UI `test_measure_cell_fragmented_tokens`: two adjacent
    // runs with no whitespace between them are ONE unbreakable token.
    let measurer = fake_measurer();
    let fragmented = vec![
        InlineElem::Text("super".to_string(), TextStyle::default()),
        InlineElem::Text("califragilistic".to_string(), TextStyle::default()),
    ];
    let single = cell("supercalifragilistic");

    let (_, min_frag, _) = measurer.measure_cell(&fragmented);
    let (_, min_single, _) = measurer.measure_cell(&single);
    assert_eq!(
        min_frag, min_single,
        "fragmented token min must equal the joined single-token min"
    );
    assert_eq!(min_frag, 20.0 * 5.0);
}

#[test]
fn test_measure_cell_code_run_measures_as_monospace() {
    let measurer = fake_measurer();
    let code_cell = vec![InlineElem::Text(
        "ab".to_string(),
        TextStyle {
            code: true,
            ..Default::default()
        },
    )];
    let (max, min, _) = measurer.measure_cell(&code_cell);
    assert_eq!(max, 20.0, "mono char = 10 px, so 2 chars = 20 px");
    assert_eq!(min, 20.0);
}

#[test]
fn test_measure_cell_image_uses_placeholder_text() {
    let measurer = fake_measurer();
    let (max, _, _) = measurer.measure_cell(&[InlineElem::Image("x.png".to_string())]);
    // "[Image: x.png]" is 14 characters.
    assert_eq!(max, 14.0 * 5.0);
}

#[test]
fn test_measure_cell_soft_break_counts_as_one_space() {
    let measurer = fake_measurer();
    let (max, _, _) = measurer.measure_cell(&[InlineElem::SoftBreak]);
    assert_eq!(max, 5.0);
}

#[test]
fn test_measure_cell_empty_cell_is_all_zero() {
    let measurer = fake_measurer();
    let (max, min, tokens) = measurer.measure_cell(&[]);
    assert_eq!((max, min), (0.0, 0.0));
    assert!(tokens.token_widths.is_empty());
}

#[test]
fn test_measure_cell_whitespace_only_closes_the_open_token() {
    let measurer = fake_measurer();
    let elems = vec![
        InlineElem::Text("hello".to_string(), TextStyle::default()),
        InlineElem::Text("   ".to_string(), TextStyle::default()),
        InlineElem::Text("world".to_string(), TextStyle::default()),
    ];
    let (max, min, tokens) = measurer.measure_cell(&elems);
    assert_eq!(max, 13.0 * 5.0);
    assert_eq!(min, 25.0);
    assert_eq!(tokens.token_widths, vec![25.0, 25.0]);
}

#[test]
fn test_trailing_whitespace_single_part_run_closes_the_token_like_the_legacy_renderer() {
    // Parity lock for the walker's `else if ends_ws` branch: a single-part
    // run that ends with whitespace closes the pending token immediately and
    // it is measured with the *previous* run's font, so the two runs stay two
    // separate tokens (they do NOT merge into one unbreakable token).
    let measurer = fake_measurer();
    let elems = vec![
        InlineElem::Text("ab ".to_string(), TextStyle::default()),
        InlineElem::Text("cd ".to_string(), TextStyle::default()),
    ];
    let (max, min, tokens) = measurer.measure_cell(&elems);
    assert_eq!(max, 30.0, "max counts every character, spaces included");
    assert_eq!(min, 10.0, "min is the longest single token");
    assert_eq!(tokens.token_widths, vec![10.0, 10.0]);
}

#[test]
fn test_space_width_and_line_height_come_from_the_metrics() {
    let measurer = fake_measurer();
    assert_eq!(measurer.space_width(), 5.0);
    assert_eq!(measurer.line_height(), 18.0);
}

#[test]
fn test_size_of_selects_by_font_family() {
    let measurer = fake_measurer();
    assert_eq!(measurer.size_of(Font::DEFAULT), 14.0);
    assert_eq!(measurer.size_of(Font::MONOSPACE), 13.0);
}

#[test]
fn test_font_handles_are_default_and_monospace() {
    assert_eq!(IcedTextMeasurer::body_font(), Font::DEFAULT);
    assert_eq!(IcedTextMeasurer::mono_font(), Font::MONOSPACE);
}

#[test]
fn test_heuristic_measurer_uses_the_heuristic_width_function() {
    let measurer = IcedTextMeasurer::heuristic(TextMetrics::FASTMD);
    let (max, _, _) = measurer.measure_cell(&cell("abcd"));
    assert_eq!(max, heuristic_width("abcd", Font::DEFAULT, 14.0));
}

// --- solver output → column widths -------------------------------------

#[test]
fn test_empty_table_solves_to_nothing() {
    let measurer = fake_measurer();
    let layout = solve_table_layout(
        &[],
        500.0,
        DeficitStrategy::HybridMinPenaltyWaterFill,
        TablePadding::ZERO,
        &measurer,
    );
    assert!(layout.rows.is_empty());
    assert!(column_widths(&layout).is_empty());
    assert!(!needs_horizontal_scroll(&layout));
}

#[test]
fn test_single_column_surplus_pins_to_max_content() {
    let measurer = fake_measurer();
    let layout = solve_table_layout(
        &[vec![cell("hello")]],
        1000.0,
        DeficitStrategy::HybridMinPenaltyWaterFill,
        TablePadding::ZERO,
        &measurer,
    );
    let widths = column_widths(&layout);
    assert_eq!(widths, vec![25.0], "surplus regime pins to max_content");
    assert!(!needs_horizontal_scroll(&layout));
}

#[test]
fn test_single_column_zero_width_forces_horizontal_scroll() {
    let measurer = fake_measurer();
    let layout = solve_table_layout(
        &[vec![cell("hello")]],
        0.0,
        DeficitStrategy::HybridMinPenaltyWaterFill,
        TablePadding::ZERO,
        &measurer,
    );
    assert!(
        needs_horizontal_scroll(&layout),
        "zero available width cannot hold even one column's min_content"
    );
    let widths = column_widths(&layout);
    assert_eq!(widths.len(), 1);
    assert!(
        widths[0] >= 25.0,
        "fallback keeps the never-break-a-token floor, got {}",
        widths[0]
    );
}

#[test]
fn test_very_narrow_viewport_falls_back_to_scroll() {
    let measurer = fake_measurer();
    let layout = solve_table_layout(
        &grid(2, 3),
        24.0,
        DeficitStrategy::HybridMinPenaltyWaterFill,
        TablePadding::ZERO,
        &measurer,
    );
    assert!(
        needs_horizontal_scroll(&layout),
        "24 px cannot fit three r#c# columns"
    );
    assert_eq!(column_widths(&layout).len(), 3);
}

#[test]
fn test_wide_viewport_fits_without_scroll() {
    let measurer = fake_measurer();
    let layout = solve_table_layout(
        &grid(2, 3),
        1000.0,
        DeficitStrategy::HybridMinPenaltyWaterFill,
        TablePadding::ZERO,
        &measurer,
    );
    assert!(!needs_horizontal_scroll(&layout));
    let widths = column_widths(&layout);
    assert_eq!(widths.len(), 3);
    let spent: f32 = widths.iter().sum::<f32>()
        + (widths.len() as f32 - 1.0) * COLUMN_SPACING
        + 2.0 * CELL_PADDING_X;
    assert!(
        spent <= 1000.0 + f32::EPSILON,
        "solved table must fit the viewport, wants {spent}"
    );
}

#[test]
fn test_many_columns_are_all_solved() {
    let measurer = fake_measurer();
    let layout = solve_table_layout(
        &grid(3, 12),
        2000.0,
        DeficitStrategy::HybridMinPenaltyWaterFill,
        TablePadding::ZERO,
        &measurer,
    );
    let widths = column_widths(&layout);
    assert_eq!(widths.len(), 12);
    assert!(
        widths.iter().all(|w| *w > 0.0),
        "every column gets a positive width: {widths:?}"
    );
}

#[test]
fn test_horizontal_padding_is_folded_into_every_column() {
    let measurer = fake_measurer();
    let padded = TablePadding {
        top: 0.0,
        bottom: 0.0,
        left: 4.0,
        right: 4.0,
    };
    let layout = solve_table_layout(
        &[vec![cell("hello")]],
        1000.0,
        DeficitStrategy::HybridMinPenaltyWaterFill,
        padded,
        &measurer,
    );
    // TBL-033: the assigned width covers content + horizontal padding.
    assert_eq!(column_widths(&layout), vec![25.0 + 8.0]);
}

#[test]
fn test_negative_padding_is_sanitised_before_solving() {
    let measurer = fake_measurer();
    let bad = TablePadding {
        top: -5.0,
        bottom: -5.0,
        left: -5.0,
        right: -5.0,
    };
    let layout = solve_table_layout(
        &[vec![cell("hello")]],
        1000.0,
        DeficitStrategy::HybridMinPenaltyWaterFill,
        bad,
        &measurer,
    );
    assert_eq!(
        column_widths(&layout),
        vec![25.0],
        "negative padding clamps to zero and must not shrink the column"
    );
}

#[test]
fn test_column_widths_take_the_max_across_ragged_rows() {
    // A ragged table: row 0 has one column, row 1 has two.
    let measurer = fake_measurer();
    let cells = vec![vec![cell("aa")], vec![cell("bb"), cell("cccc")]];
    let layout = solve_table_layout(
        &cells,
        1000.0,
        DeficitStrategy::HybridMinPenaltyWaterFill,
        TablePadding::ZERO,
        &measurer,
    );
    let widths = column_widths(&layout);
    assert_eq!(widths.len(), 2, "the widest row sets the column count");
    assert!(
        widths.iter().all(|w| *w > 0.0),
        "no column collapses to zero: {widths:?}"
    );
}

#[test]
fn test_column_length_pins_a_fixed_width() {
    assert_eq!(column_length(120.0), Length::Fixed(120.0));
    assert_eq!(column_length(0.0), Length::Fixed(0.0));
    assert_eq!(
        column_length(-42.0),
        Length::Fixed(0.0),
        "a negative solved width must not produce a negative Length"
    );
}

#[test]
fn test_solver_input_uses_the_declared_strategy_unchanged() {
    // Every strategy must solve the same surplus case to the same pinned
    // widths — the adapter passes the strategy through without touching it.
    let measurer = fake_measurer();
    let strategies = [
        DeficitStrategy::ProportionalToSlack,
        DeficitStrategy::BreakpointWaterFill,
        DeficitStrategy::WaterFillRatio,
        DeficitStrategy::LagrangePenalty,
        DeficitStrategy::HybridMinPenaltyWaterFill,
    ];
    for strategy in strategies {
        let layout = solve_table_layout(
            &[vec![cell("hello")]],
            1000.0,
            strategy,
            TablePadding::ZERO,
            &measurer,
        );
        assert_eq!(
            column_widths(&layout),
            vec![25.0],
            "surplus must be strategy-independent for {strategy:?}"
        );
    }
}

#[test]
fn test_cell_tokens_type_is_reachable_from_the_adapter() {
    // Guards the re-export surface the render layer relies on.
    let measurer = fake_measurer();
    let (_, _, tokens): (f32, f32, CellTokens) = measurer.measure_cell(&cell("x y"));
    assert_eq!(tokens.token_widths.len(), 2);
}
