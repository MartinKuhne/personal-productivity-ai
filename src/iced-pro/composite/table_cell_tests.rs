//! Tests for the framework-owned table cell (`super::table_cell`,
//! `UIF-COMP-007`, decision **D-8**).

use super::*;
use crate::composite::table_column::{ColumnSpec, WidthPolicy, resolve_widths};
use crate::token::color::Palette;
use crate::token::contrast::{contrast_ratio, meets_chrome_contrast};
use crate::token::{shape, space};
use iced::{Color, Rectangle};

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

fn palette() -> Palette {
    Palette::dark()
}

/// A three-column layout wide enough that no cell is degenerate.
fn layout() -> crate::composite::table_column::ColumnLayout {
    resolve_widths(
        &[
            ColumnSpec::fixed(0, "a", 100.0),
            content(1, 140.0, 40.0),
            proportional(2, 1.0, 60.0),
        ],
        500.0,
    )
}

fn row() -> Rectangle {
    rect(0.0, 64.0, 500.0, BODY_ROW_MIN_HEIGHT)
}

/// A content column with its minimum set, spelled short so the tests read
/// as widths rather than as builder chains.
fn content(id: u16, preferred: f32, min: f32) -> ColumnSpec {
    ColumnSpec::content(id, "c", preferred).with_min(min)
}

/// A proportional column with its minimum set.
fn proportional(id: u16, weight: f32, min: f32) -> ColumnSpec {
    ColumnSpec::proportional(id, "c", weight).with_min(min)
}

// ---------------------------------------------------------------------------
// Bands
// ---------------------------------------------------------------------------

#[test]
fn each_band_maps_to_its_specified_surface() {
    let palette = palette();
    assert_eq!(CellBand::Base.colour(&palette), palette.surface_base);
    assert_eq!(CellBand::Raised.colour(&palette), palette.surface_raised);
    assert_eq!(CellBand::Hover.colour(&palette), palette.surface_hover);
    assert_eq!(CellBand::Selected.colour(&palette), palette.surface_active);
}

#[test]
fn band_slugs_are_stable() {
    assert_eq!(CellBand::Base.slug(), "base");
    assert_eq!(CellBand::Raised.slug(), "raised");
    assert_eq!(CellBand::Hover.slug(), "hover");
    assert_eq!(CellBand::Selected.slug(), "selected");
}

#[test]
fn only_the_resting_band_is_unpainted() {
    assert!(!CellBand::Base.is_painted());
    assert!(CellBand::Raised.is_painted());
    assert!(CellBand::Hover.is_painted());
    assert!(CellBand::Selected.is_painted());
}

#[test]
fn selection_outranks_the_pointer_and_the_zebra() {
    assert_eq!(row_band(true, true, true), CellBand::Selected);
    assert_eq!(row_band(true, true, false), CellBand::Selected);
    assert_eq!(row_band(true, false, true), CellBand::Selected);
    assert_eq!(row_band(true, false, false), CellBand::Selected);
}

#[test]
fn the_pointer_outranks_the_zebra() {
    assert_eq!(row_band(false, true, true), CellBand::Hover);
    assert_eq!(row_band(false, true, false), CellBand::Hover);
}

#[test]
fn the_zebra_only_shows_when_nothing_above_it_arrives() {
    assert_eq!(row_band(false, false, true), CellBand::Raised);
    assert_eq!(row_band(false, false, false), CellBand::Base);
}

// ---------------------------------------------------------------------------
// Foreground
// ---------------------------------------------------------------------------

#[test]
fn the_selected_row_gets_the_stronger_text() {
    let palette = palette();
    assert_eq!(
        foreground_colour(CellBand::Base, &palette),
        palette.text_primary
    );
    assert_eq!(
        foreground_colour(CellBand::Raised, &palette),
        palette.text_primary
    );
    assert_eq!(
        foreground_colour(CellBand::Hover, &palette),
        palette.text_primary
    );
    assert_eq!(
        foreground_colour(CellBand::Selected, &palette),
        palette.text_strong,
        "the spec prints no foreground for a selected cell; the tree's rule applies"
    );
}

#[test]
fn every_foreground_clears_its_own_band() {
    let palette = palette();
    for band in [
        CellBand::Base,
        CellBand::Raised,
        CellBand::Hover,
        CellBand::Selected,
    ] {
        let fg = foreground_colour(band, &palette);
        let bg = band.colour(&palette);
        assert!(
            meets_chrome_contrast(fg, bg),
            "{band:?} foreground measures {} against its band",
            contrast_ratio(fg, bg)
        );
    }
}

// ---------------------------------------------------------------------------
// Grid lines — the D-6 promotion
// ---------------------------------------------------------------------------

#[test]
fn the_subtle_line_survives_on_the_two_light_bands() {
    let palette = palette();
    assert_eq!(
        grid_line_colour(CellBand::Base, &palette),
        palette.border_subtle
    );
    assert_eq!(
        grid_line_colour(CellBand::Raised, &palette),
        palette.border_subtle
    );
}

#[test]
fn the_subtle_line_is_promoted_on_the_two_heavy_bands() {
    let palette = palette();
    assert_eq!(
        grid_line_colour(CellBand::Hover, &palette),
        palette.border_strong,
        "border.subtle vanishes on surface.hover and must be promoted"
    );
    assert_eq!(
        grid_line_colour(CellBand::Selected, &palette),
        palette.border_strong,
        "border.subtle vanishes on surface.active and must be promoted"
    );
}

#[test]
fn every_promoted_line_actually_meets_the_floor() {
    let palette = palette();
    for band in [
        CellBand::Base,
        CellBand::Raised,
        CellBand::Hover,
        CellBand::Selected,
    ] {
        let measured = grid_line_contrast(band, &palette);
        assert!(
            measured >= MIN_GRID_LINE_CONTRAST_ON_BAND - f32::EPSILON,
            "{band:?} grid line measures {measured}, under the floor"
        );
    }
}

/// The four figures the module docs claim, pinned so a palette edit that
/// moves one of them fails here rather than silently invalidating the
/// documented reasoning.
#[test]
fn the_documented_subtle_line_ratios_are_what_the_palette_actually_gives() {
    let palette = palette();
    let subtle = palette.border_subtle;
    assert!(approx(contrast_ratio(subtle, palette.surface_base), 1.3495));
    assert!(approx(
        contrast_ratio(subtle, palette.surface_raised),
        1.2691
    ));
    assert!(approx(
        contrast_ratio(subtle, palette.surface_hover),
        1.1012
    ));
    assert!(approx(
        contrast_ratio(subtle, palette.surface_active),
        1.0314
    ));
}

#[test]
fn the_threshold_sits_strictly_between_the_kept_and_promoted_bands() {
    let palette = palette();
    let kept_hover = contrast_ratio(palette.border_subtle, palette.surface_raised);
    let lost_hover = contrast_ratio(palette.border_subtle, palette.surface_hover);
    assert!(MIN_GRID_LINE_CONTRAST_ON_BAND < kept_hover);
    assert!(MIN_GRID_LINE_CONTRAST_ON_BAND > lost_hover);
}

#[test]
fn the_selection_outline_needs_no_promotion() {
    let palette = palette();
    let outline = selection_outline_colour(&palette);
    assert_eq!(outline, palette.accent_border);
    assert!(
        meets_chrome_contrast(outline, palette.surface_active),
        "accent.border on surface.active measures {}",
        contrast_ratio(outline, palette.surface_active)
    );
    assert!(approx(
        contrast_ratio(outline, palette.surface_active),
        4.7922
    ));
}

// ---------------------------------------------------------------------------
// Resolved style
// ---------------------------------------------------------------------------

#[test]
fn the_resolved_style_agrees_with_the_individual_resolvers() {
    let palette = palette();
    for band in [
        CellBand::Base,
        CellBand::Raised,
        CellBand::Hover,
        CellBand::Selected,
    ] {
        let style = CellStyle::resolve(band, &palette);
        assert_eq!(style.band, band);
        assert_eq!(style.background, band.colour(&palette));
        assert_eq!(style.foreground, foreground_colour(band, &palette));
        assert_eq!(style.grid_line, grid_line_colour(band, &palette));
        assert_eq!(style.focus, focus_indicator(band, &palette));
    }
}

#[test]
fn only_the_selected_band_carries_an_outline() {
    let palette = palette();
    assert_eq!(
        CellStyle::resolve(CellBand::Selected, &palette).outline,
        palette.accent_border
    );
    for band in [CellBand::Base, CellBand::Raised, CellBand::Hover] {
        assert_eq!(
            CellStyle::resolve(band, &palette).outline,
            Color::TRANSPARENT,
            "{band:?} must not draw a selection outline"
        );
    }
}

#[test]
fn the_focus_ring_is_inset_so_it_cannot_cross_a_column() {
    let palette = palette();
    for band in [
        CellBand::Base,
        CellBand::Raised,
        CellBand::Hover,
        CellBand::Selected,
    ] {
        let focus = focus_indicator(band, &palette);
        assert_eq!(
            focus.placement,
            crate::widget::focus::FocusPlacement::Inset,
            "{band:?} focus must sit inside the cell"
        );
    }
}

// ---------------------------------------------------------------------------
// Cell geometry
// ---------------------------------------------------------------------------

#[test]
fn cells_take_their_column_x_and_the_row_y() {
    let layout = layout();
    let row = row();
    let cell = cell_rect(&layout, 1, row);
    assert!(approx(cell.x, layout.x_at(1)));
    assert!(approx(cell.width, layout.width_at(1)));
    assert!(approx(cell.y, row.y));
    assert!(approx(cell.height, row.height));
}

#[test]
fn cells_tile_the_row_with_no_gap_or_overlap() {
    let layout = layout();
    let row = row();
    for index in 1..layout.len() {
        let previous = cell_rect(&layout, index - 1, row);
        let current = cell_rect(&layout, index, row);
        assert!(
            approx(previous.x + previous.width, current.x),
            "gap between column {} and {}: {} vs {}",
            index - 1,
            index,
            previous.x + previous.width,
            current.x
        );
    }
}

#[test]
fn content_rect_removes_the_padding_pair() {
    let cell = rect(10.0, 20.0, 200.0, BODY_ROW_MIN_HEIGHT);
    let inner = content_rect(cell);
    assert!(approx(inner.x, 10.0 + CELL_PADDING_X));
    assert!(approx(inner.y, 20.0 + CELL_PADDING_Y));
    assert!(approx(inner.width, 200.0 - CELL_PADDING_X * 2.0));
    assert!(approx(
        inner.height,
        BODY_ROW_MIN_HEIGHT - CELL_PADDING_Y * 2.0
    ));
}

#[test]
fn the_padding_pair_reads_vertical_then_horizontal() {
    // §3B.7 prints `[4px, 12px]`. The reading is pinned here so a
    // transposition cannot slip through as "still 4 and 12".
    assert!(approx(CELL_PADDING_Y, space::XS));
    assert!(approx(CELL_PADDING_X, space::MD));
    // Compile-time rather than runtime: both sides are constants, so a const
    // assertion keeps the reading pinned even if this test never runs.
    const { assert!(CELL_PADDING_X > CELL_PADDING_Y) };
}

#[test]
fn a_column_narrower_than_its_padding_clamps_instead_of_going_negative() {
    let cell = rect(0.0, 0.0, 8.0, 8.0);
    let inner = content_rect(cell);
    assert!(approx(inner.width, 0.0));
    assert!(approx(inner.height, 0.0));
    assert!(inner.width >= 0.0);
    assert!(inner.height >= 0.0);
}

#[test]
fn content_x_places_by_alignment() {
    let cell = rect(0.0, 0.0, 100.0, BODY_ROW_MIN_HEIGHT);
    let inner = content_rect(cell);
    assert!(approx(content_x(cell, 20.0, CellAlign::Start), inner.x,));
    assert!(approx(
        content_x(cell, 20.0, CellAlign::End),
        inner.x + inner.width - 20.0
    ));
    assert!(approx(
        content_x(cell, 20.0, CellAlign::Center),
        inner.x + (inner.width - 20.0) / 2.0
    ));
}

#[test]
fn overflowing_content_is_never_pushed_out_of_its_box() {
    let cell = rect(0.0, 0.0, 60.0, BODY_ROW_MIN_HEIGHT);
    let inner = content_rect(cell);
    for align in [CellAlign::Start, CellAlign::Center, CellAlign::End] {
        let x = content_x(cell, 500.0, align);
        assert!(
            approx(x, inner.x),
            "{align:?} moved overflowing content away from the box edge"
        );
        assert!(x >= inner.x);
    }
}

#[test]
fn truncation_is_reported_against_the_content_box() {
    let cell = rect(0.0, 0.0, 100.0, BODY_ROW_MIN_HEIGHT);
    let inner_width = content_rect(cell).width;
    assert!(!is_truncated(cell, inner_width));
    assert!(!is_truncated(cell, inner_width - 1.0));
    assert!(is_truncated(cell, inner_width + 1.0));
}

#[test]
fn content_centre_y_is_the_box_centre() {
    let cell = rect(0.0, 10.0, 100.0, 40.0);
    let inner = content_rect(cell);
    assert!(approx(content_centre_y(cell), inner.y + inner.height / 2.0));
}

// ---------------------------------------------------------------------------
// Separators
// ---------------------------------------------------------------------------

#[test]
fn the_bottom_separator_sits_on_the_cell_trailing_edge() {
    let cell = rect(5.0, 20.0, 200.0, 32.0);
    let line = bottom_separator(cell);
    assert!(approx(line.x, cell.x));
    assert!(approx(line.width, cell.width));
    assert!(approx(line.height, GRID_LINE_WIDTH));
    assert!(approx(line.y + line.height, cell.y + cell.height));
}

#[test]
fn the_trailing_divider_sits_on_the_cell_right_edge() {
    let cell = rect(5.0, 20.0, 200.0, 32.0);
    let line = trailing_divider(cell, false).expect("a non-final cell has a divider");
    assert!(approx(line.y, cell.y));
    assert!(approx(line.height, cell.height));
    assert!(approx(line.width, GRID_LINE_WIDTH));
    assert!(approx(line.x + line.width, cell.x + cell.width));
}

#[test]
fn the_final_column_draws_no_divider() {
    let cell = rect(400.0, 20.0, 100.0, 32.0);
    assert!(
        trailing_divider(cell, true).is_none(),
        "the table's right edge is the container border; a divider there doubles the stroke"
    );
}

#[test]
fn a_cell_too_narrow_for_a_divider_draws_none() {
    let cell = rect(0.0, 0.0, 0.5, 32.0);
    assert!(trailing_divider(cell, false).is_none());
    let empty = rect(0.0, 0.0, 0.0, 32.0);
    assert!(trailing_divider(empty, false).is_none());
}

#[test]
fn the_grid_line_weight_is_the_hairline() {
    assert!(approx(GRID_LINE_WIDTH, shape::BORDER_HAIRLINE));
}

#[test]
fn the_body_row_floor_is_the_spec_figure() {
    assert!(approx(BODY_ROW_MIN_HEIGHT, 32.0));
}

// ---------------------------------------------------------------------------
// Width policy interaction
// ---------------------------------------------------------------------------

#[test]
fn a_fixed_column_clips_its_content_rather_than_growing() {
    let layout = resolve_widths(&[ColumnSpec::fixed(0, "n", 60.0)], 500.0);
    let cell = cell_rect(&layout, 0, row());
    assert!(approx(cell.width, 60.0));
    assert!(is_truncated(cell, 200.0));
    assert!(approx(
        content_x(cell, 200.0, CellAlign::End),
        content_rect(cell).x
    ));
}

#[test]
fn a_content_column_never_widens_past_its_content() {
    let layout = resolve_widths(&[content(0, 80.0, 20.0)], 500.0);
    let cell = cell_rect(&layout, 0, row());
    assert!(approx(cell.width, 80.0));
    assert!(!is_truncated(cell, 80.0 - CELL_PADDING_X * 2.0));
}

#[test]
fn a_proportional_column_gives_its_content_more_room() {
    let layout = resolve_widths(&[proportional(0, 1.0, 20.0)], 500.0);
    let cell = cell_rect(&layout, 0, row());
    assert!(approx(cell.width, 500.0));
    assert!(!is_truncated(cell, 400.0));
    assert!(approx(WidthPolicy::Proportional(1.0).weight(), 1.0));
}
