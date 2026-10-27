//! Tests for the header chrome and the resize/sort hit separation
//! (`super::table_header`, `UIF-COMP-007`).

use super::*;
use crate::composite::table_column::resolve_widths;
use crate::token::typography::Weight;
use iced::{Point, Rectangle};

const TOL: f32 = 1e-3;

fn approx(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < TOL
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rectangle {
    Rectangle {
        x,
        y,
        width,
        height,
    }
}

/// The header band the tests press against.
fn header() -> Rectangle {
    rect(0.0, 0.0, 400.0, HEADER_HEIGHT)
}

/// Three columns of 100 / 140 / 160, with the sortable and resizable
/// flags set per column.
fn specs(sortable: [bool; 3], resizable: [bool; 3]) -> Vec<ColumnSpec> {
    [(0u16, 100.0), (1u16, 140.0), (2u16, 160.0)]
        .into_iter()
        .enumerate()
        .map(|(index, (id, width))| {
            ColumnSpec::fixed(id, "c", width)
                .with_sortable(sortable[index])
                .with_resizable(resizable[index])
        })
        .collect()
}

fn layout_of(specs: &[ColumnSpec]) -> crate::composite::table_column::ColumnLayout {
    resolve_widths(specs, 400.0)
}

fn at(x: f32) -> Point {
    Point::new(x, HEADER_HEIGHT / 2.0)
}

/// Do two rectangles share any area?
fn overlaps(a: Rectangle, b: Rectangle) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

#[test]
fn the_header_geometry_matches_the_spec_figures() {
    assert!(approx(HEADER_HEIGHT, 32.0));
    assert!(approx(RESIZE_GRAB, 10.0));
    assert!(approx(RESIZE_REACH, 5.0));
    assert!(approx(HEADER_BORDER_WIDTH, shape::BORDER_HAIRLINE));
    assert!(approx(SORT_INDICATOR_SIZE, size::ICON_SM));
}

#[test]
fn the_title_rung_permits_the_weight_it_is_worn_at() {
    // G-12 says the scale has no 12px SemiBold *emphasis* rung. The
    // claim here is narrower and true: SemiBold is among CAPTION's
    // allowed weights, which is how a menu section heading wears it too.
    assert!(approx(TITLE_RUNG.size, 12.0));
    assert!(TITLE_RUNG.allows(TITLE_WEIGHT));
    assert_eq!(TITLE_WEIGHT, Weight::SemiBold);
}

// ---------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------

#[test]
fn the_handle_is_centred_on_the_divider() {
    let specs = specs([true; 3], [true; 3]);
    let layout = layout_of(&specs);
    let handle = resize_handle_rect(&layout, 0, header());
    assert!(approx(handle.x, 100.0 - RESIZE_REACH));
    assert!(approx(handle.width, RESIZE_GRAB));
    assert!(approx(handle.x + RESIZE_REACH, 100.0));
}

#[test]
fn the_sort_target_excludes_the_resize_band() {
    let specs = specs([true; 3], [true; 3]);
    let layout = layout_of(&specs);
    let target = sort_target_rect(&specs, &layout, 0, header());
    assert!(approx(target.x, 0.0));
    assert!(approx(target.width, 100.0 - RESIZE_REACH));
}

#[test]
fn a_non_resizable_column_keeps_its_whole_cell_as_sort_target() {
    let specs = specs([true; 3], [false; 3]);
    let layout = layout_of(&specs);
    let target = sort_target_rect(&specs, &layout, 0, header());
    let cell = header_cell_rect(&layout, 0, header());
    assert_eq!(target, cell);
}

#[test]
fn no_sort_target_overlaps_any_live_handle() {
    let specs = specs([true; 3], [true; 3]);
    let layout = layout_of(&specs);
    let live: Vec<usize> = (0..layout.len())
        .filter(|&index| column_is_resizable(&specs, index))
        .collect();
    assert_eq!(
        live,
        vec![0, 1],
        "the final column must have no live handle"
    );
    for index in 0..layout.len() {
        let target = sort_target_rect(&specs, &layout, index, header());
        for &handle_index in &live {
            let handle = resize_handle_rect(&layout, handle_index, header());
            assert!(
                !overlaps(target, handle),
                "column {index} target {:?} overlaps column {handle_index} handle {:?}",
                (target.x, target.width),
                (handle.x, handle.width)
            );
        }
    }
}

#[test]
fn a_column_between_two_live_dividers_is_inset_on_both_sides() {
    let specs = specs([true; 3], [true; 3]);
    let layout = layout_of(&specs);
    let cell = header_cell_rect(&layout, 1, header());
    let target = sort_target_rect(&specs, &layout, 1, header());
    assert!(approx(target.x, cell.x + RESIZE_REACH));
    assert!(approx(target.width, cell.width - RESIZE_REACH * 2.0));
}

#[test]
fn the_final_column_is_inset_only_at_its_leading_edge() {
    // The final column has no handle of its own, but its leading edge is
    // column 1's live divider, so that five pixels still belong to the
    // handle rather than to the sort target.
    let specs = specs([true; 3], [true; 3]);
    let layout = layout_of(&specs);
    let cell = header_cell_rect(&layout, 2, header());
    let target = sort_target_rect(&specs, &layout, 2, header());
    assert!(approx(target.x, cell.x + RESIZE_REACH));
    assert!(approx(target.width, cell.width - RESIZE_REACH));
}

#[test]
fn adjacent_handles_do_not_overlap() {
    let specs = specs([true; 3], [true; 3]);
    let layout = layout_of(&specs);
    for index in 1..layout.len() {
        let previous = resize_handle_rect(&layout, index - 1, header());
        let current = resize_handle_rect(&layout, index, header());
        assert!(
            !overlaps(previous, current),
            "handles {} and {} collide at these widths",
            index - 1,
            index
        );
    }
}

#[test]
fn the_indicator_sits_inside_the_sort_target_and_outside_every_band() {
    let specs = specs([true; 3], [true; 3]);
    let layout = layout_of(&specs);
    let live: Vec<usize> = (0..layout.len())
        .filter(|&index| column_is_resizable(&specs, index))
        .collect();
    for index in 0..layout.len() {
        let target = sort_target_rect(&specs, &layout, index, header());
        let mark = sort_indicator_rect(target);
        assert!(mark.x >= target.x);
        assert!(mark.x + mark.width <= target.x + target.width + TOL);
        assert!(mark.y >= target.y);
        assert!(mark.y + mark.height <= target.y + target.height + TOL);

        for &handle_index in &live {
            let handle = resize_handle_rect(&layout, handle_index, header());
            assert!(
                !overlaps(mark, handle),
                "column {index}: the indicator is painted inside column {handle_index}'s grab band"
            );
        }
    }
}

#[test]
fn a_target_narrower_than_the_indicator_clamps_instead_of_escaping() {
    let tiny = rect(50.0, 0.0, 4.0, HEADER_HEIGHT);
    let mark = sort_indicator_rect(tiny);
    assert!(approx(mark.x, tiny.x));
    assert!(mark.x >= tiny.x);
}

#[test]
fn the_bottom_border_sits_on_the_header_trailing_edge() {
    let band = rect(0.0, 10.0, 400.0, HEADER_HEIGHT);
    let line = bottom_border_layer_rect(band);
    assert!(approx(line.y + line.height, band.y + band.height));
    assert!(approx(line.width, band.width));
    assert!(approx(line.height, HEADER_BORDER_WIDTH));
}

// ---------------------------------------------------------------------------
// The negative-test matrix: resize must not sort, a click must not resize
// ---------------------------------------------------------------------------

#[test]
fn a_centre_click_sorts_and_does_not_resize() {
    let specs = specs([true; 3], [true; 3]);
    let layout = layout_of(&specs);
    assert_eq!(
        press_at(&specs, &layout, header(), at(50.0)),
        HeaderPress::Sort { column: 0 }
    );
}

#[test]
fn a_press_exactly_on_the_divider_resizes_and_does_not_sort() {
    let specs = specs([true; 3], [true; 3]);
    let layout = layout_of(&specs);
    assert_eq!(
        press_at(&specs, &layout, header(), at(100.0)),
        HeaderPress::Resize { column: 0 }
    );
}

#[test]
fn a_press_five_pixels_left_of_the_divider_resizes() {
    let specs = specs([true; 3], [true; 3]);
    let layout = layout_of(&specs);
    assert_eq!(
        press_at(&specs, &layout, header(), at(95.0)),
        HeaderPress::Resize { column: 0 }
    );
}

#[test]
fn a_press_just_inside_the_band_from_the_left_resizes() {
    let specs = specs([true; 3], [true; 3]);
    let layout = layout_of(&specs);
    assert_eq!(
        press_at(&specs, &layout, header(), at(99.9)),
        HeaderPress::Resize { column: 0 }
    );
}

#[test]
fn a_press_just_right_of_the_divider_resizes_the_left_column_not_the_right_one() {
    // The band straddles the divider into column 1's cell. A per-cell hit
    // test would sort column 1 here and destroy the user's ordering.
    let specs = specs([true; 3], [true; 3]);
    let layout = layout_of(&specs);
    assert_eq!(
        press_at(&specs, &layout, header(), at(104.9)),
        HeaderPress::Resize { column: 0 }
    );
}

#[test]
fn a_press_just_outside_the_band_sorts_the_right_column() {
    let specs = specs([true; 3], [true; 3]);
    let layout = layout_of(&specs);
    assert_eq!(
        press_at(&specs, &layout, header(), at(105.0)),
        HeaderPress::Sort { column: 1 }
    );
}

#[test]
fn a_press_just_outside_the_band_sorts_the_left_column() {
    let specs = specs([true; 3], [true; 3]);
    let layout = layout_of(&specs);
    assert_eq!(
        press_at(&specs, &layout, header(), at(94.9)),
        HeaderPress::Sort { column: 0 }
    );
}

#[test]
fn a_small_accidental_drag_from_the_title_does_not_start_a_resize() {
    // Press on the title, drift three pixels toward the divider. Every
    // intermediate x still resolves to the sort, so a jittery hand can
    // never fall into the handle on the way.
    let specs = specs([true; 3], [true; 3]);
    let layout = layout_of(&specs);
    for x in [50.0, 51.0, 52.0, 53.0] {
        assert_eq!(
            press_at(&specs, &layout, header(), at(x)),
            HeaderPress::Sort { column: 0 },
            "a drift to {x} must not become a resize"
        );
    }
}

#[test]
fn a_press_on_the_handle_never_reports_a_sort() {
    let specs = specs([true; 3], [true; 3]);
    let layout = layout_of(&specs);
    for x in [95.0, 97.5, 100.0, 102.5, 104.9] {
        let press = press_at(&specs, &layout, header(), at(x));
        assert!(
            matches!(press, HeaderPress::Resize { .. }),
            "x={x} resolved to {press:?} inside the grab band"
        );
    }
}

#[test]
fn a_non_sortable_resizable_column_resizes_on_the_band_and_nothing_else() {
    let specs = specs([false, true, true], [true; 3]);
    let layout = layout_of(&specs);
    assert_eq!(
        press_at(&specs, &layout, header(), at(95.0)),
        HeaderPress::Resize { column: 0 }
    );
    assert_eq!(
        press_at(&specs, &layout, header(), at(50.0)),
        HeaderPress::Nothing
    );
}

#[test]
fn a_sortable_non_resizable_column_sorts_right_up_to_its_edge() {
    let specs = specs([true; 3], [false; 3]);
    let layout = layout_of(&specs);
    assert_eq!(
        press_at(&specs, &layout, header(), at(99.9)),
        HeaderPress::Sort { column: 0 }
    );
    assert_eq!(
        press_at(&specs, &layout, header(), at(100.0)),
        HeaderPress::Sort { column: 1 }
    );
}

#[test]
fn a_frozen_column_reports_nothing_anywhere_in_its_cell() {
    let specs = specs([false; 3], [false; 3]);
    let layout = layout_of(&specs);
    for x in [1.0, 50.0, 99.0, 100.0, 101.0, 239.0, 300.0, 399.0] {
        assert_eq!(
            press_at(&specs, &layout, header(), at(x)),
            HeaderPress::Nothing,
            "a frozen column claimed x={x}"
        );
    }
}

#[test]
fn the_last_column_is_never_resizable() {
    let specs = specs([true; 3], [true; 3]);
    assert!(!column_is_resizable(&specs, 2));
    assert!(column_is_resizable(&specs, 0));
    assert!(column_is_resizable(&specs, 1));
}

#[test]
fn a_press_near_the_tables_right_edge_sorts_rather_than_resizes() {
    let specs = specs([true; 3], [true; 3]);
    let layout = layout_of(&specs);
    assert_eq!(
        press_at(&specs, &layout, header(), at(399.0)),
        HeaderPress::Sort { column: 2 }
    );
}

#[test]
fn a_column_narrower_than_the_reach_has_no_sort_target() {
    // The handle covers the whole cell, so the cell cannot sort. This is
    // the degenerate case the disjointness rule makes explicit rather
    // than leaving to chance.
    let narrow = vec![
        ColumnSpec::fixed(0, "c", 4.0)
            .with_sortable(true)
            .with_resizable(true),
        ColumnSpec::fixed(1, "c", 396.0)
            .with_sortable(true)
            .with_resizable(false),
    ];
    let layout = resolve_widths(&narrow, 400.0);
    let target = sort_target_rect(&narrow, &layout, 0, header());
    assert!(approx(target.width, 0.0));
    assert_eq!(
        press_at(&narrow, &layout, header(), at(2.0)),
        HeaderPress::Resize { column: 0 }
    );
}

#[test]
fn a_press_outside_the_header_band_is_nothing() {
    let specs = specs([true; 3], [true; 3]);
    let layout = layout_of(&specs);
    assert_eq!(
        press_at(
            &specs,
            &layout,
            header(),
            Point::new(50.0, HEADER_HEIGHT + 1.0)
        ),
        HeaderPress::Nothing
    );
    assert_eq!(
        press_at(&specs, &layout, header(), Point::new(-1.0, 4.0)),
        HeaderPress::Nothing
    );
}

#[test]
fn no_x_resolves_to_both_a_resize_and_a_sort() {
    // Sweep the whole header: whatever a pixel resolves to, it resolves
    // to exactly one thing. The two affordances cannot both claim a pixel.
    let specs = specs([true; 3], [true; 3]);
    let layout = layout_of(&specs);
    let mut x = 0.0;
    while x < 400.0 {
        let press = press_at(&specs, &layout, header(), at(x));
        let resize_hits = (0..layout.len())
            .filter(|&index| {
                column_is_resizable(&specs, index)
                    && contains(resize_handle_rect(&layout, index, header()), at(x))
            })
            .count();
        let sort_hits = (0..layout.len())
            .filter(|&index| {
                column_is_sortable(&specs, index)
                    && contains(sort_target_rect(&specs, &layout, index, header()), at(x))
            })
            .count();
        match press {
            HeaderPress::Resize { column } => {
                assert!(resize_hits >= 1, "x={x} claimed a resize with no handle");
                assert_eq!(sort_hits, 0, "x={x} sits in a sort target and a handle");
                assert!(
                    column + 1 < layout.len(),
                    "x={x} resolved a resize on the final column"
                );
            }
            HeaderPress::Sort { column } => {
                assert!(sort_hits >= 1, "x={x} claimed a sort with no target");
                assert_eq!(resize_hits, 0, "x={x} sits in a handle and a sort target");
                assert!(column < layout.len());
            }
            HeaderPress::Nothing => {}
        }
        x += 0.25;
    }
}

// ---------------------------------------------------------------------------
// Sort cycling
// ---------------------------------------------------------------------------

#[test]
fn the_sort_cycle_is_ascending_descending_none() {
    let once = cycle_sort(None, 1);
    assert_eq!(once, Some((1, SortDirection::Ascending)));
    let twice = cycle_sort(once, 1);
    assert_eq!(twice, Some((1, SortDirection::Descending)));
    let thrice = cycle_sort(twice, 1);
    assert_eq!(thrice, None);
}

#[test]
fn clicking_a_different_column_restarts_at_ascending() {
    let mut state = cycle_sort(None, 0);
    state = cycle_sort(state, 0);
    assert_eq!(state, Some((0, SortDirection::Descending)));
    state = cycle_sort(state, 2);
    assert_eq!(state, Some((2, SortDirection::Ascending)));
}

#[test]
fn the_indicator_only_shows_on_the_sorted_column() {
    let sort = Some((1, SortDirection::Descending));
    assert_eq!(sort_indicator(sort, 1), Some(SortDirection::Descending));
    assert_eq!(sort_indicator(sort, 0), None);
    assert_eq!(sort_indicator(sort, 2), None);
    assert_eq!(sort_indicator(None, 1), None);
}

#[test]
fn the_glyphs_are_the_ones_the_spec_prints() {
    assert_eq!(SortDirection::Ascending.glyph(), "▲");
    assert_eq!(SortDirection::Descending.glyph(), "▼");
    assert_eq!(SortDirection::Ascending.slug(), "asc");
    assert_eq!(SortDirection::Descending.slug(), "desc");
}

// ---------------------------------------------------------------------------
// Colour
// ---------------------------------------------------------------------------

#[test]
fn the_header_uses_the_specified_channels() {
    let palette = Palette::dark();
    assert_eq!(band_colour(&palette), palette.surface_raised);
    assert_eq!(title_colour(&palette), palette.text_strong);
    assert_eq!(indicator_colour(&palette), palette.accent_text);
    assert_eq!(border_colour(&palette), palette.border_strong);
}

#[test]
fn the_sort_indicator_reads_on_the_header_band() {
    let palette = Palette::dark();
    let measured = indicator_contrast(&palette);
    assert!(
        crate::token::contrast::meets_chrome_contrast(
            indicator_colour(&palette),
            band_colour(&palette)
        ),
        "accent.text on surface.raised measures {measured}"
    );
    assert!(approx(measured, 10.0215));
}

#[test]
fn the_header_border_reads_on_its_own_band() {
    let palette = Palette::dark();
    let measured = border_contrast(&palette);
    assert!(
        crate::token::contrast::meets_chrome_contrast(
            border_colour(&palette),
            band_colour(&palette)
        ),
        "border.strong on surface.raised measures {measured}"
    );
    assert!(approx(measured, 4.2379));
}

// ---------------------------------------------------------------------------
// Title placement
// ---------------------------------------------------------------------------

#[test]
fn the_title_follows_the_columns_alignment() {
    let cell = rect(100.0, 0.0, 140.0, HEADER_HEIGHT);
    let inner_x = cell.x + CELL_TITLE_PADDING_X;
    let inner_width = cell.width - CELL_TITLE_PADDING_X * 2.0;
    assert!(approx(title_x(cell, 40.0, CellAlign::Start), inner_x));
    assert!(approx(
        title_x(cell, 40.0, CellAlign::End),
        inner_x + inner_width - 40.0
    ));
    assert!(approx(
        title_x(cell, 40.0, CellAlign::Center),
        inner_x + (inner_width - 40.0) / 2.0
    ));
}

#[test]
fn a_title_wider_than_its_cell_starts_at_the_cell_not_before_it() {
    let cell = rect(100.0, 0.0, 40.0, HEADER_HEIGHT);
    let x = title_x(cell, 500.0, CellAlign::End);
    assert!(x >= cell.x + CELL_TITLE_PADDING_X);
}

#[test]
fn a_degenerate_cell_does_not_produce_a_negative_inner_width() {
    let cell = rect(0.0, 0.0, 4.0, HEADER_HEIGHT);
    let x = title_x(cell, 0.0, CellAlign::End);
    assert!(x >= cell.x);
}
