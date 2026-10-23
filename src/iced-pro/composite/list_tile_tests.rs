//! Tests for [`super::list_tile`] — the three-slot grid decision **D-8**
//! puts in this crate, the hit targets both tile kinds owe, and the
//! measured reason a selected tile's subtitle cannot stay muted
//! (`UIF-COMP-005`).

use crate::composite::list_tile::{
    BORDER_WIDTH, LEADING_LARGE, LEADING_SMALL, Leading, LeadingExtent, PADDING_HORIZONTAL,
    PADDING_VERTICAL, SLOT_GAP, SUBTITLE_RUNG, TILE_HEIGHTS, TITLE_RUNG, TRAILING_TARGET,
    TWO_LINE_HEIGHT, TWO_LINE_PARTS_HEIGHT, TWO_LINE_UNACCOUNTED, TileLines, TileSlots, hit_target,
    meets_hit_target_floor, resolve_style, row_layer, slots, subtitle_on, tile_height,
    trailing_targets,
};
use crate::state::BaseState;
use crate::token::color::Palette;
use crate::token::contrast::{
    MIN_CHROME_CONTRAST, MIN_TEXT_CONTRAST, contrast_ratio, meets_text_contrast,
};
use crate::token::{shape, size, space};
use iced::{Color, Point, Rectangle};

/// A contrast ratio compared at the four decimal places the pinned figures
/// are written to.
///
/// `f32` arithmetic does not reproduce a printed decimal exactly, so an
/// exact `==` on a ratio tests the representation rather than the
/// measurement.
fn approx(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 5e-4
}

fn tile(lines: TileLines, width: f32) -> Rectangle {
    Rectangle::new(
        Point::new(2.0, 50.0),
        iced::Size::new(width, tile_height(lines)),
    )
}

// ---------------------------------------------------------------------------
// 1 — the two rungs
// ---------------------------------------------------------------------------

#[test]
fn the_single_line_rung_is_the_control_rung() {
    assert_eq!(tile_height(TileLines::Single), 32.0);
    assert_eq!(tile_height(TileLines::Single), size::CONTROL_MD);
}

#[test]
fn the_two_line_rung_is_the_printed_44() {
    assert_eq!(tile_height(TileLines::Two), TWO_LINE_HEIGHT);
    assert_eq!(TWO_LINE_HEIGHT, 44.0);
}

#[test]
fn the_two_line_row_is_two_pixels_taller_than_its_own_parts_compose() {
    assert_eq!(TWO_LINE_PARTS_HEIGHT, 42.0);
    assert_eq!(TWO_LINE_UNACCOUNTED, 2.0);
    assert_eq!(
        TWO_LINE_PARTS_HEIGHT + TWO_LINE_UNACCOUNTED,
        TWO_LINE_HEIGHT,
        "the delta is the whole disagreement, and nothing more"
    );
}

#[test]
fn the_single_line_row_composes_exactly_from_its_printed_parts() {
    let composed = PADDING_VERTICAL + LEADING_LARGE + PADDING_VERTICAL;

    assert_eq!(composed, tile_height(TileLines::Single));
}

#[test]
fn both_rungs_clear_the_hit_target_floor() {
    assert!(meets_hit_target_floor(TileLines::Single));
    assert!(meets_hit_target_floor(TileLines::Two));
    assert_eq!(
        TILE_HEIGHTS,
        [tile_height(TileLines::Single), tile_height(TileLines::Two)]
    );

    for height in TILE_HEIGHTS {
        assert!(height >= size::HIT_TARGET_MIN);
    }
}

#[test]
fn the_two_text_rungs_are_what_3b5_prints() {
    assert_eq!(TITLE_RUNG.size, 13.0);
    assert_eq!(TITLE_RUNG.line_height, 18.0);
    assert_eq!(SUBTITLE_RUNG.size, 12.0);
    assert_eq!(SUBTITLE_RUNG.line_height, 16.0);
}

#[test]
fn every_grid_measurement_is_a_token_read() {
    assert_eq!(PADDING_VERTICAL, space::XS);
    assert_eq!(PADDING_HORIZONTAL, space::MD);
    assert_eq!(SLOT_GAP, space::SM);
    assert_eq!(LEADING_SMALL, size::ICON_MD);
    assert_eq!(LEADING_LARGE, size::HIT_TARGET_MIN);
    assert_eq!(TRAILING_TARGET, size::HIT_TARGET_MIN);
    assert_eq!(BORDER_WIDTH, shape::BORDER_HAIRLINE);
}

// ---------------------------------------------------------------------------
// 2 — the leading slot
// ---------------------------------------------------------------------------

#[test]
fn each_leading_kind_reports_its_visual_extent() {
    assert_eq!(Leading::None.visual_extent(), 0.0);
    assert_eq!(Leading::Icon(LeadingExtent::Small).visual_extent(), 16.0);
    assert_eq!(Leading::Icon(LeadingExtent::Large).visual_extent(), 24.0);
    assert_eq!(Leading::Checkbox.visual_extent(), size::CHECKBOX_BOX);
    assert_eq!(Leading::Radio.visual_extent(), size::CHECKBOX_BOX);
}

#[test]
fn an_interactive_tile_inflates_a_small_mark_to_the_target_floor() {
    assert_eq!(Leading::Icon(LeadingExtent::Small).hit_extent(true), 24.0);
    assert_eq!(Leading::Checkbox.hit_extent(true), 24.0);
    assert_eq!(Leading::Radio.hit_extent(true), 24.0);
}

#[test]
fn a_large_mark_is_already_at_the_floor_and_does_not_grow() {
    assert_eq!(Leading::Icon(LeadingExtent::Large).hit_extent(true), 24.0);
    assert_eq!(Leading::Icon(LeadingExtent::Large).hit_extent(false), 24.0);
}

#[test]
fn an_inert_mark_owes_no_target() {
    assert_eq!(Leading::Icon(LeadingExtent::Small).hit_extent(false), 16.0);
    assert_eq!(Leading::Checkbox.hit_extent(false), 16.0);
    assert_eq!(Leading::None.hit_extent(false), 0.0);
    assert_eq!(Leading::None.hit_extent(true), 0.0);
}

#[test]
fn the_hit_extents_of_every_leading_kind_are_pinned() {
    let kinds = [
        Leading::None,
        Leading::Icon(LeadingExtent::Small),
        Leading::Icon(LeadingExtent::Large),
        Leading::Checkbox,
        Leading::Radio,
    ];
    let interactive: Vec<f32> = kinds.iter().map(|kind| kind.hit_extent(true)).collect();

    assert_eq!(interactive, vec![0.0, 24.0, 24.0, 24.0, 24.0]);
}

// ---------------------------------------------------------------------------
// 3 — the three-slot grid
// ---------------------------------------------------------------------------

#[test]
fn the_leading_slot_sits_at_the_interior_left_edge() {
    let bounds = tile(TileLines::Two, 320.0);
    let grid = slots(bounds, Leading::Checkbox, true, 48.0);

    assert_eq!(grid.leading.x, bounds.x + PADDING_HORIZONTAL);
    assert_eq!(grid.leading.width, 24.0);
}

#[test]
fn the_trailing_slot_is_reserved_against_the_right_edge() {
    let bounds = tile(TileLines::Two, 320.0);
    let grid = slots(bounds, Leading::Checkbox, true, 48.0);

    assert_eq!(
        grid.trailing.x + grid.trailing.width,
        bounds.x + bounds.width - PADDING_HORIZONTAL
    );
    assert_eq!(grid.trailing.width, 48.0);
}

#[test]
fn the_centre_slot_fills_between_the_two_and_touches_neither() {
    let bounds = tile(TileLines::Two, 320.0);
    let grid = slots(bounds, Leading::Checkbox, true, 48.0);

    assert_eq!(
        grid.center.x,
        grid.leading.x + grid.leading.width + SLOT_GAP
    );
    assert_eq!(
        grid.center.x + grid.center.width + SLOT_GAP,
        grid.trailing.x
    );
    assert!(grid.center.width > 0.0);
}

#[test]
fn an_empty_leading_slot_charges_no_gap_so_titles_stay_aligned() {
    let bounds = tile(TileLines::Single, 320.0);
    let marked = slots(bounds, Leading::Checkbox, true, 48.0);
    let bare = slots(bounds, Leading::None, true, 48.0);

    assert_eq!(bare.leading.width, 0.0);
    assert!(
        bare.center.x < marked.center.x,
        "an unmarked row must not inherit the marked row's indent"
    );
    assert_eq!(bare.center.x, bounds.x + PADDING_HORIZONTAL);
}

#[test]
fn an_empty_trailing_slot_charges_no_gap_either() {
    let bounds = tile(TileLines::Single, 320.0);
    let bare = slots(bounds, Leading::None, false, 0.0);

    assert_eq!(bare.trailing.width, 0.0);
    assert_eq!(
        bare.center.width,
        bounds.width - PADDING_HORIZONTAL - PADDING_HORIZONTAL
    );
}

#[test]
fn a_trailing_wider_than_the_row_is_clamped_and_the_centre_goes_to_zero() {
    let bounds = tile(TileLines::Single, 80.0);
    let grid = slots(bounds, Leading::Checkbox, true, 500.0);

    assert!(grid.trailing.width <= bounds.width);
    assert_eq!(grid.center.width, 0.0);
}

#[test]
fn no_tile_width_produces_a_negative_extent() {
    for width in [0.0, 12.0, 24.0, 48.0, 96.0, 200.0] {
        let grid: TileSlots = slots(tile(TileLines::Two, width), Leading::Radio, true, 64.0);
        assert!(grid.leading.width >= 0.0, "width {width}");
        assert!(grid.center.width >= 0.0, "width {width}");
        assert!(grid.trailing.width >= 0.0, "width {width}");
    }
}

#[test]
fn every_slot_is_inside_its_tile() {
    let grid = slots(tile(TileLines::Two, 320.0), Leading::Checkbox, true, 48.0);
    let bounds = tile(TileLines::Two, 320.0);

    for slot in [grid.leading, grid.center, grid.trailing] {
        assert!(slot.x >= bounds.x);
        assert!(slot.x + slot.width <= bounds.x + bounds.width + 0.001);
    }
}

// ---------------------------------------------------------------------------
// 4 — the trailing targets
// ---------------------------------------------------------------------------

#[test]
fn each_trailing_target_is_the_24_square_the_spec_fixes() {
    for count in 1..=3 {
        for target in trailing_targets(tile(TileLines::Single, 320.0), count) {
            assert_eq!(target.width, TRAILING_TARGET);
            assert_eq!(target.height, TRAILING_TARGET);
            assert!(target.width >= size::HIT_TARGET_MIN);
        }
    }
}

#[test]
fn trailing_targets_are_right_aligned_and_never_overlap() {
    let bounds = tile(TileLines::Single, 320.0);
    let targets = trailing_targets(bounds, 3);
    let right = bounds.x + bounds.width - PADDING_HORIZONTAL;

    assert_eq!(
        targets.last().unwrap().x + targets.last().unwrap().width,
        right
    );

    for pair in targets.windows(2) {
        assert!(
            pair[0].x + pair[0].width + SLOT_GAP <= pair[1].x + 0.001,
            "two trailing targets collided"
        );
    }
}

#[test]
fn trailing_targets_are_vertically_centred_in_both_rungs() {
    for lines in [TileLines::Single, TileLines::Two] {
        let bounds = tile(lines, 320.0);
        for target in trailing_targets(bounds, 2) {
            let expected = bounds.y + (bounds.height - TRAILING_TARGET) / 2.0;
            assert_eq!(target.y, expected);
            assert!(target.y >= bounds.y);
            assert!(target.y + target.height <= bounds.y + bounds.height + 0.001);
        }
    }
}

#[test]
fn no_trailing_targets_means_an_empty_list_not_a_stray_quad() {
    assert!(trailing_targets(tile(TileLines::Single, 320.0), 0).is_empty());
}

// ---------------------------------------------------------------------------
// 5 — the row's own hit target
// ---------------------------------------------------------------------------

#[test]
fn an_interactive_tile_is_hit_across_its_whole_row() {
    let bounds = tile(TileLines::Two, 320.0);

    assert_eq!(hit_target(bounds, true), Some(bounds));
}

#[test]
fn a_non_interactive_tile_lets_the_pointer_through() {
    let bounds = tile(TileLines::Two, 320.0);

    assert_eq!(hit_target(bounds, false), None);
}

#[test]
fn the_row_target_is_wider_than_any_target_inside_it() {
    let bounds = tile(TileLines::Single, 320.0);
    let row = hit_target(bounds, true).expect("interactive");

    for target in trailing_targets(bounds, 3) {
        assert!(row.width > target.width);
        assert!(row.height >= target.height);
    }
}

// ---------------------------------------------------------------------------
// 6 — paint, selection and the promoted subtitle
// ---------------------------------------------------------------------------

#[test]
fn a_live_unselected_tile_paints_nothing_at_rest() {
    let palette = Palette::dark();
    let style = resolve_style(false, false, BaseState::Rest, &palette);

    assert_eq!(style.background, Color::TRANSPARENT);
    assert_eq!(style.border_width, 0.0);
    assert_eq!(style.title_colour, palette.text_primary);
    assert_eq!(style.subtitle_colour, palette.text_muted);
    assert!(row_layer(tile(TileLines::Single, 320.0), &style).is_none());
}

#[test]
fn an_unselected_tile_takes_the_hover_and_pressed_lifts() {
    let palette = Palette::dark();
    let hovered = resolve_style(false, false, BaseState::Hovered, &palette);
    let pressed = resolve_style(false, false, BaseState::Pressed, &palette);

    assert_eq!(hovered.background, palette.surface_hover);
    assert_eq!(pressed.background, palette.surface_active);
    assert!(row_layer(tile(TileLines::Single, 320.0), &hovered).is_some());
}

#[test]
fn a_selected_tile_keeps_its_fill_through_hover_and_press() {
    let palette = Palette::dark();
    let selected = resolve_style(true, false, BaseState::Rest, &palette);
    let hovered = resolve_style(true, false, BaseState::Hovered, &palette);

    assert_eq!(selected.background, palette.accent_fill);
    assert_eq!(hovered.background, palette.accent_fill);
    assert_eq!(selected.border_width, 0.0);
}

#[test]
fn a_dead_tile_paints_nothing_and_reads_disabled() {
    let palette = Palette::dark();
    let dead = resolve_style(true, true, BaseState::Rest, &palette);

    assert_eq!(dead.background, Color::TRANSPARENT);
    assert_eq!(dead.title_colour, palette.text_disabled);
    assert_eq!(dead.subtitle_colour, palette.text_disabled);
    assert_eq!(dead.leading_colour, palette.text_disabled);
    assert_eq!(dead.trailing_colour, palette.text_disabled);
    assert!(row_layer(tile(TileLines::Two, 320.0), &dead).is_none());
}

#[test]
fn a_muted_subtitle_cannot_be_read_on_the_selection_fill() {
    let palette = Palette::dark();

    assert!(!meets_text_contrast(
        palette.text_muted,
        palette.accent_fill
    ));
    assert!(
        approx(
            contrast_ratio(palette.text_muted, palette.accent_fill),
            1.8363
        ),
        "this is the measurement that forces the promotion"
    );
}

#[test]
fn the_subtitle_is_promoted_only_where_the_muted_caption_fails() {
    let palette = Palette::dark();

    assert_eq!(
        subtitle_on(palette.accent_fill, &palette),
        palette.text_strong
    );
    assert_eq!(
        subtitle_on(palette.surface_raised, &palette),
        palette.text_muted
    );
    assert_eq!(
        subtitle_on(palette.surface_hover, &palette),
        palette.text_muted
    );
    assert_eq!(
        subtitle_on(palette.surface_active, &palette),
        palette.text_muted
    );
}

#[test]
fn a_selected_tile_promotes_its_subtitle_in_the_resolved_style() {
    let palette = Palette::dark();
    let style = resolve_style(true, false, BaseState::Rest, &palette);

    assert_eq!(style.subtitle_colour, palette.text_strong);
    assert_ne!(style.subtitle_colour, palette.text_muted);
}

#[test]
fn every_text_a_selected_tile_paints_clears_the_selection_fill() {
    let palette = Palette::dark();
    let style = resolve_style(true, false, BaseState::Rest, &palette);

    for (name, colour) in [
        ("title", style.title_colour),
        ("subtitle", style.subtitle_colour),
        ("leading", style.leading_colour),
        ("trailing", style.trailing_colour),
    ] {
        assert!(
            contrast_ratio(colour, style.background) >= MIN_TEXT_CONTRAST,
            "{name} on the selection fill is {}",
            contrast_ratio(colour, style.background)
        );
    }
}

#[test]
fn the_selection_pairing_is_pinned_at_the_figure_3b6_prints() {
    let palette = Palette::dark();

    assert!(approx(
        contrast_ratio(palette.text_strong, palette.accent_fill),
        4.802
    ));
}

#[test]
fn a_selected_tile_wears_no_perimeter_because_accent_on_accent_cannot_be_seen() {
    let palette = Palette::dark();

    assert!(
        contrast_ratio(palette.accent_border, palette.accent_fill) < MIN_CHROME_CONTRAST,
        "if this ever clears the floor the no-border decision should be revisited"
    );
    assert!(approx(
        contrast_ratio(palette.accent_border, palette.accent_fill),
        1.6098
    ));

    let style = resolve_style(true, false, BaseState::Rest, &palette);
    assert_eq!(style.border_width, 0.0);
    assert_eq!(style.border_color, Color::TRANSPARENT);
}

#[test]
fn every_unselected_tile_text_clears_every_surface_it_rests_on() {
    let palette = Palette::dark();
    let style = resolve_style(false, false, BaseState::Hovered, &palette);

    assert!(contrast_ratio(style.title_colour, style.background) >= MIN_TEXT_CONTRAST);
    assert!(contrast_ratio(style.subtitle_colour, style.background) >= MIN_TEXT_CONTRAST);
}

#[test]
fn a_disabled_tile_is_read_on_the_card_not_on_its_own_fill() {
    let palette = Palette::dark();
    let dead = resolve_style(false, true, BaseState::Rest, &palette);

    assert!(approx(
        contrast_ratio(dead.title_colour, palette.surface_raised),
        5.5643
    ));
}
