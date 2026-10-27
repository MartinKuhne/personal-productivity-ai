//! Tests for [`super::virtual_list`] — the recycling bound that still holds
//! at a million rows, the exact `index × height` mapping, and a thumb that
//! answers to the document rather than to the handful of rows on hand
//! (`UIF-COMP-006`).

use crate::composite::scroll_view::{self, Axis, THUMB_MIN_LENGTH};
use crate::composite::virtual_list::{
    ACCUMULATION_ILLUSTRATION, ACCUMULATION_TARGET, COMPACT_ROW_HEIGHT, DEFAULT_OVERSCAN_ROWS,
    RowRung, STANDARD_ROW_HEIGHT, VirtualMetrics,
};
use crate::token::size;
use iced::Rectangle;

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rectangle {
    Rectangle {
        x,
        y,
        width,
        height,
    }
}

// ---------------------------------------------------------------------------
// 1 — token bindings
// ---------------------------------------------------------------------------

#[test]
fn rungs_bind_to_tokens_rather_than_literals() {
    assert_eq!(COMPACT_ROW_HEIGHT, size::HIT_TARGET_MIN);
    assert_eq!(STANDARD_ROW_HEIGHT, size::CONTROL_SM);
    assert_eq!(RowRung::Compact.height(), COMPACT_ROW_HEIGHT);
    assert_eq!(RowRung::Standard.height(), STANDARD_ROW_HEIGHT);
}

#[test]
fn both_rungs_clear_the_hit_target_floor() {
    assert!(RowRung::Compact.meets_hit_target_floor());
    assert!(RowRung::Standard.meets_hit_target_floor());
}

#[test]
fn the_two_rungs_are_distinguishable() {
    assert_ne!(
        RowRung::Compact.height(),
        RowRung::Standard.height(),
        "a compact row that is not smaller than a standard row is one rung, not two"
    );
}

// ---------------------------------------------------------------------------
// 2 — the variable-height refusal, made numeric
// ---------------------------------------------------------------------------

#[test]
fn accumulating_a_height_drifts_off_the_product() {
    let drift = ACCUMULATION_ILLUSTRATION - ACCUMULATION_TARGET;
    assert_ne!(
        ACCUMULATION_ILLUSTRATION, ACCUMULATION_TARGET,
        "the illustration only means something if the drift is real"
    );
    assert!(
        drift.abs() >= 1e-4,
        "a thousand additions of 0.1 land {drift:.6} away from 100.0; the size \
         and sign of the error are both the point, since a prefix sum walks it \
         into every row below"
    );
}

#[test]
fn a_fixed_height_never_drifts() {
    let metrics = VirtualMetrics::new(100_000, RowRung::Compact, 240.0);
    for index in 0..100_000usize {
        assert_eq!(
            metrics.row_top(index),
            index as f32 * COMPACT_ROW_HEIGHT,
            "row {index} must be the product, not a running total"
        );
        assert_eq!(
            metrics.row_bottom(index),
            metrics.row_top(index) + COMPACT_ROW_HEIGHT,
            "row {index}'s bottom must be its top plus one row height"
        );
    }
}

// ---------------------------------------------------------------------------
// 3 — the recycling bound
// ---------------------------------------------------------------------------

#[test]
fn the_bound_carries_no_term_for_total_rows() {
    let small = VirtualMetrics::new(100, RowRung::Compact, 240.0);
    let huge = VirtualMetrics::new(1_000_000, RowRung::Compact, 240.0);
    assert_eq!(
        small.realized_count_bound(),
        huge.realized_count_bound(),
        "the bound is a function of the viewport alone"
    );
    assert_eq!(
        huge.realized_count_bound(),
        10 + 1 + 2 * DEFAULT_OVERSCAN_ROWS,
        "ceil(240/24) + 1 partial + the overscan on both edges"
    );
}

#[test]
fn a_million_rows_realise_what_a_hundred_thousand_do() {
    let a = VirtualMetrics::new(100_000, RowRung::Standard, 280.0);
    let b = VirtualMetrics::new(1_000_000, RowRung::Standard, 280.0);
    for step in 0..256usize {
        let offset = (step * 977) as f32;
        assert_eq!(
            a.realized_count(offset),
            b.realized_count(offset),
            "at offset {offset} the two lists must build the same number of rows"
        );
    }
}

#[test]
fn the_realized_count_never_exceeds_the_bound() {
    let metrics = VirtualMetrics::new(100_000, RowRung::Compact, 250.0);
    let bound = metrics.realized_count_bound();
    for step in 0..512usize {
        let offset = (step * 131) as f32;
        assert!(
            metrics.realized_count(offset) <= bound,
            "offset {offset} realised {} rows, over the bound of {bound}",
            metrics.realized_count(offset)
        );
    }
}

#[test]
fn the_bound_is_tight_rather_than_mere_safety_margin() {
    let metrics = VirtualMetrics::new(100_000, RowRung::Compact, 240.0);
    // A non-multiple offset keeps a partial row at each edge, so the
    // visible span is 11 rows and the overscan adds exactly six more.
    assert_eq!(
        metrics.realized_count(252.0),
        metrics.realized_count_bound()
    );
}

#[test]
fn overscan_widens_the_realized_span_by_exactly_its_own_width() {
    let tight = VirtualMetrics::with_row_height(10_000, 24.0, 240.0, 0);
    let loose = VirtualMetrics::with_row_height(10_000, 24.0, 240.0, 4);
    let offset = 252.0;
    assert_eq!(
        loose.realized_count(offset),
        tight.realized_count(offset) + 8,
        "four rows of overscan on each of two edges"
    );
}

// ---------------------------------------------------------------------------
// 4 — the visible window
// ---------------------------------------------------------------------------

#[test]
fn a_row_the_viewport_only_touches_is_not_visible() {
    let metrics = VirtualMetrics::with_row_height(100, 24.0, 96.0, 0);
    assert_eq!(
        metrics.visible_range(0.0),
        0..4,
        "rows 0..4 fill the viewport exactly; row 4 starts on the bottom edge \
         and must not be realised by a touch"
    );
}

#[test]
fn a_partially_visible_row_is_visible() {
    let metrics = VirtualMetrics::with_row_height(100, 24.0, 100.0, 0);
    assert_eq!(metrics.visible_range(0.0), 0..5);
}

#[test]
fn a_fractional_offset_keeps_the_row_it_straddles() {
    let metrics = VirtualMetrics::with_row_height(100, 24.0, 96.0, 0);
    assert_eq!(metrics.visible_range(12.0), 0..5);
    assert_eq!(metrics.first_visible(12.0), 0);
}

#[test]
fn first_visible_walks_one_row_per_row_height() {
    let metrics = VirtualMetrics::new(1000, RowRung::Compact, 240.0);
    // Only the rows that can actually lead the viewport: past `max_offset`
    // the list has scrolled as far as it goes and the leading row stops.
    let leadable = (metrics.max_offset() / COMPACT_ROW_HEIGHT) as usize;
    assert_eq!(leadable, 990, "23760 of travel over 24px rows");
    for index in 0..=leadable {
        assert_eq!(metrics.first_visible(metrics.row_top(index)), index);
    }
    assert_eq!(metrics.first_visible(metrics.row_top(999)), leadable);
}

#[test]
fn the_realized_span_clamps_at_both_ends_of_the_list() {
    let metrics = VirtualMetrics::with_row_height(10, 24.0, 48.0, 3);
    assert_eq!(metrics.realized_range(0.0), 0..5);
    assert_eq!(
        metrics.realized_range(metrics.max_offset()),
        5..10,
        "at the bottom of the list the trailing overscan has nowhere to go"
    );
}

#[test]
fn is_realized_agrees_with_the_range() {
    let metrics = VirtualMetrics::new(100, RowRung::Standard, 280.0);
    for offset in [0.0, 13.0, 271.0, 2_500.0] {
        let range = metrics.realized_range(offset);
        for index in 0..100usize {
            assert_eq!(
                metrics.is_realized(index, offset),
                range.contains(&index),
                "row {index} at offset {offset}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 5 — addressing
// ---------------------------------------------------------------------------

#[test]
fn row_at_round_trips_against_row_top() {
    let metrics = VirtualMetrics::new(1000, RowRung::Compact, 240.0);
    for index in 0..1000usize {
        assert_eq!(metrics.row_at(metrics.row_top(index)), Some(index));
        assert_eq!(metrics.row_at(metrics.row_top(index) + 12.0), Some(index));
        assert_eq!(
            metrics.row_at(metrics.row_bottom(index) - 0.001),
            Some(index)
        );
    }
}

#[test]
fn row_at_refuses_what_no_row_covers() {
    let metrics = VirtualMetrics::new(10, RowRung::Compact, 96.0);
    assert_eq!(metrics.row_at(metrics.content_len()), None);
    assert_eq!(metrics.row_at(metrics.content_len() + 1.0), None);
    assert_eq!(metrics.row_at(-0.001), None);
    assert_eq!(metrics.row_at(f32::NAN), None);
    assert_eq!(metrics.row_at(f32::INFINITY), None);
}

#[test]
fn row_rect_places_a_row_under_the_viewport_top() {
    let metrics = VirtualMetrics::new(100, RowRung::Compact, 96.0);
    let viewport = rect(10.0, 20.0, 300.0, 96.0);
    assert_eq!(
        metrics.row_rect(3, viewport, 0.0),
        rect(10.0, 92.0, 300.0, 24.0)
    );
    assert_eq!(
        metrics.row_rect(3, viewport, 24.0),
        rect(10.0, 68.0, 300.0, 24.0)
    );
    assert_eq!(
        metrics.row_rect(0, viewport, 48.0),
        rect(10.0, -28.0, 300.0, 24.0),
        "a scrolled-past row reads above the viewport rather than being clamped into it"
    );
}

// ---------------------------------------------------------------------------
// 6 — reveal, the minimal-scroll rule
// ---------------------------------------------------------------------------

#[test]
fn a_fully_visible_row_needs_no_scroll() {
    let metrics = VirtualMetrics::with_row_height(100, 24.0, 96.0, 0);
    for index in 0..4usize {
        assert_eq!(
            metrics.offset_to_reveal(index, 0.0),
            None,
            "row {index} is already inside; moving the list would be the jump \
             this function exists to prevent"
        );
    }
}

#[test]
fn revealing_a_row_clears_the_edge_by_the_least_distance() {
    let metrics = VirtualMetrics::with_row_height(100, 24.0, 96.0, 0);
    assert_eq!(metrics.offset_to_reveal(4, 0.0), Some(24.0));
    assert_eq!(metrics.offset_to_reveal(1, 48.0), Some(24.0));
}

#[test]
fn revealing_clamps_inside_the_scrollable_range() {
    let metrics = VirtualMetrics::with_row_height(100, 24.0, 96.0, 0);
    assert_eq!(metrics.offset_to_reveal(99, 0.0), Some(2_304.0));
    assert_eq!(
        metrics.offset_to_reveal(99, 0.0),
        Some(metrics.max_offset())
    );
}

#[test]
fn revealing_refuses_a_row_that_does_not_exist() {
    let metrics = VirtualMetrics::with_row_height(10, 24.0, 96.0, 0);
    assert_eq!(metrics.offset_to_reveal(10, 0.0), None);
    assert_eq!(metrics.offset_to_reveal(usize::MAX, 0.0), None);
}

#[test]
fn a_zero_viewport_reveals_nothing() {
    let metrics = VirtualMetrics::with_row_height(100, 24.0, 0.0, 0);
    assert_eq!(metrics.offset_to_reveal(5, 0.0), None);
}

// ---------------------------------------------------------------------------
// 7 — the thumb answers to the document
// ---------------------------------------------------------------------------

#[test]
fn the_thumb_shrinks_as_the_document_grows() {
    let small = VirtualMetrics::with_row_height(200, 24.0, 2_400.0, 0);
    let big = VirtualMetrics::with_row_height(2_000, 24.0, 2_400.0, 0);
    assert!(
        (small.thumb_length() - 1_200.0).abs() < 1e-3,
        "2400² / 4800 = 1200, got {}",
        small.thumb_length()
    );
    assert!(
        (big.thumb_length() - 120.0).abs() < 1e-3,
        "2400² / 48000 = 120, got {}",
        big.thumb_length()
    );
}

#[test]
fn the_thumb_is_not_the_realized_extent() {
    let metrics = VirtualMetrics::new(1_000_000, RowRung::Compact, 240.0);
    let realized_pixels = metrics.realized_count(0.0) as f32 * COMPACT_ROW_HEIGHT;
    assert!(
        realized_pixels > 240.0,
        "the realised block is larger than the viewport, by design"
    );
    assert_eq!(
        metrics.thumb_length(),
        THUMB_MIN_LENGTH,
        "a thumb sized to the realised rows would read as a nearly-full track \
         and tell the user the document is short"
    );
}

#[test]
fn the_thumb_travels_the_track_from_end_to_end() {
    let metrics = VirtualMetrics::with_row_height(2_000, 24.0, 2_400.0, 0);
    let travel = 2_400.0 - metrics.thumb_length();
    assert_eq!(metrics.thumb_offset(0.0), 0.0);
    assert!(
        (metrics.thumb_offset(metrics.max_offset()) - travel).abs() < 1e-2,
        "at the bottom of the document the thumb must sit at the end of its travel"
    );
}

#[test]
fn the_thumb_rect_stays_inside_its_track() {
    let metrics = VirtualMetrics::with_row_height(2_000, 24.0, 2_400.0, 0);
    let track = rect(0.0, 0.0, 12.0, 2_400.0);
    for offset in [0.0, 1_200.0, metrics.max_offset()] {
        let thumb = metrics.thumb_rect(track, offset, Axis::Vertical);
        assert!(thumb.height >= THUMB_MIN_LENGTH);
        assert!(thumb.y >= -1e-3, "offset {offset}: thumb.y = {}", thumb.y);
        assert!(
            thumb.y + thumb.height <= 2_400.0 + 1e-2,
            "offset {offset}: thumb bottom {} overran the track",
            thumb.y + thumb.height
        );
    }
}

#[test]
fn the_horizontal_thumb_uses_the_other_axis() {
    let metrics = VirtualMetrics::with_row_height(2_000, 24.0, 2_400.0, 0);
    let track = rect(0.0, 100.0, 2_400.0, 12.0);
    let vertical = metrics.thumb_rect(track, 1_200.0, Axis::Vertical);
    let horizontal = metrics.thumb_rect(track, 1_200.0, Axis::Horizontal);
    assert_ne!(vertical.x, horizontal.x);
    assert_ne!(vertical.width, horizontal.width);
}

// ---------------------------------------------------------------------------
// 8 — sanitisation
// ---------------------------------------------------------------------------

#[test]
fn a_degenerate_row_height_falls_back_to_the_compact_rung() {
    for bad in [0.0, -8.0, f32::NAN, f32::NEG_INFINITY] {
        let metrics = VirtualMetrics::with_row_height(100, bad, 240.0, 3);
        assert_eq!(
            metrics.row_height(),
            COMPACT_ROW_HEIGHT,
            "{bad} would map every offset to the same row"
        );
    }
}

#[test]
fn a_degenerate_viewport_collapses_to_nothing_visible() {
    for bad in [-1.0, f32::NAN, f32::NEG_INFINITY] {
        let metrics = VirtualMetrics::with_row_height(100, 24.0, bad, 3);
        assert_eq!(metrics.viewport_len(), 0.0, "{bad} is not a viewport");
        assert_eq!(metrics.visible_range(0.0), 0..0);
        assert_eq!(metrics.realized_range(0.0), 0..0);
        assert_eq!(metrics.realized_count(0.0), 0);
        assert_eq!(metrics.first_visible(0.0), 0);
    }
}

#[test]
fn a_non_finite_offset_reads_as_the_top_of_the_list() {
    let metrics = VirtualMetrics::new(100, RowRung::Compact, 96.0);
    assert_eq!(metrics.visible_range(f32::NAN), metrics.visible_range(0.0));
    assert_eq!(
        metrics.visible_range(f32::INFINITY),
        metrics.visible_range(0.0),
        "a non-finite offset collapses to zero rather than to the far end, so a \
         poisoned scroll delta cannot fling the viewport to the bottom"
    );
    assert_eq!(metrics.thumb_offset(f32::NAN), metrics.thumb_offset(0.0));
    assert_eq!(metrics.first_visible(f32::NAN), 0);
}

#[test]
fn an_absurd_row_count_stays_finite_and_bounded() {
    let metrics = VirtualMetrics::new(usize::MAX, RowRung::Compact, 240.0);
    assert!(
        metrics.content_len().is_finite(),
        "got {}",
        metrics.content_len()
    );
    assert!(metrics.is_scrollable());
    assert!(metrics.realized_count(0.0) <= metrics.realized_count_bound());
}

// ---------------------------------------------------------------------------
// 9 — the empty list
// ---------------------------------------------------------------------------

#[test]
fn an_empty_list_is_inert() {
    let metrics = VirtualMetrics::new(0, RowRung::Compact, 240.0);
    assert_eq!(metrics.total_rows(), 0);
    assert_eq!(metrics.content_len(), 0.0);
    assert_eq!(metrics.max_offset(), 0.0);
    assert!(!metrics.is_scrollable());
    assert_eq!(metrics.visible_range(0.0), 0..0);
    assert_eq!(metrics.realized_range(0.0), 0..0);
    assert_eq!(metrics.realized_count(0.0), 0);
    assert!(!metrics.is_realized(0, 0.0));
    assert_eq!(metrics.row_at(0.0), None);
    assert_eq!(metrics.offset_to_reveal(0, 0.0), None);
    assert_eq!(metrics.first_visible(0.0), 0);
}

#[test]
fn a_list_shorter_than_the_viewport_does_not_scroll() {
    let metrics = VirtualMetrics::new(2, RowRung::Standard, 280.0);
    assert_eq!(metrics.content_len(), 56.0);
    assert_eq!(metrics.max_offset(), 0.0);
    assert!(!metrics.is_scrollable());
    assert_eq!(metrics.visible_range(0.0), 0..2);
    assert_eq!(metrics.realized_range(0.0), 0..2);
}

// ---------------------------------------------------------------------------
// 10 — accessors echo what they were given
// ---------------------------------------------------------------------------

#[test]
fn the_accessors_echo_the_constructor() {
    let metrics = VirtualMetrics::with_row_height(4_321, 26.0, 321.0, 7);
    assert_eq!(metrics.total_rows(), 4_321);
    assert_eq!(metrics.row_height(), 26.0);
    assert_eq!(metrics.viewport_len(), 321.0);
    assert_eq!(metrics.overscan(), 7);
    assert_eq!(
        metrics.max_offset(),
        scroll_view::max_offset(metrics.content_len(), 321.0)
    );
}
