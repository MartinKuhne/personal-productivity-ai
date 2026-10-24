//! Tests for [`super::tree_row`] - the width-relative indent policy that
//! keeps the label floor at any depth, the row's slot grid, and the D-6
//! promotion that makes every mark visible on the band it is actually
//! painted over (`UIF-COMP-006`, decision **D-8**).

use crate::composite::list_tile::{Leading, LeadingExtent, TileSlots};
use crate::composite::tree_row::{
    CHEVRON_COLUMN, INDENT_STEP, LABEL_RUNG, MIN_INDENT_STEP, MIN_LABEL_WIDTH, ROW_HEIGHT_COMPACT,
    ROW_HEIGHT_STANDARD, ROW_INSET_X, ROW_INSET_Y, SELECTION_BAR_WIDTH, band_colour,
    chevron_colour, content_width, disabled_label_in_band, guide_colour, guide_layers,
    indent_allowance, indent_geometry, label_fits_row, label_floor_holds, locked_over,
    promoted_over, resolve_style, row_layer, row_padding, row_slots, selection_bar_colour,
    selection_bar_layer, warning_colour,
};
use crate::state::BaseState;
use crate::token::color::Palette;
use crate::token::contrast::{MIN_CHROME_CONTRAST, contrast_ratio, meets_chrome_contrast};
use crate::token::{shape, size, space};
use iced::{Color, Rectangle};

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rectangle {
    Rectangle {
        x,
        y,
        width,
        height,
    }
}

/// A contrast ratio compared at the four decimal places the pinned figures
/// are written to; `f32` does not reproduce a printed decimal exactly.
fn approx(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 5e-4
}

fn palette() -> Palette {
    Palette::dark()
}

/// Every band a tree row's marks can end up painted over.
fn bands(p: &Palette) -> Vec<(&'static str, Color)> {
    vec![
        ("surface.base", p.surface_base),
        ("surface.raised", p.surface_raised),
        ("surface.hover", p.surface_hover),
        ("surface.active", p.surface_active),
        ("accent.fill", p.accent_fill),
        ("accent.fill_active", p.accent_fill_active),
    ]
}

// ---------------------------------------------------------------------------
// 1 - token bindings
// ---------------------------------------------------------------------------

#[test]
fn every_geometry_constant_reads_a_token() {
    assert_eq!(INDENT_STEP, space::LG);
    assert_eq!(MIN_INDENT_STEP, space::XS);
    assert_eq!(CHEVRON_COLUMN, size::HIT_TARGET_MIN);
    assert_eq!(ROW_INSET_X, space::XS);
    assert_eq!(ROW_INSET_Y, space::XXS);
    assert_eq!(SELECTION_BAR_WIDTH, shape::BORDER_FOCUS);
    assert_eq!(SELECTION_BAR_WIDTH, 2.0, "§3B.6 prices the bar at 2px");
    assert_eq!(ROW_HEIGHT_COMPACT, size::HIT_TARGET_MIN);
    assert_eq!(ROW_HEIGHT_STANDARD, size::CONTROL_SM);
}

#[test]
fn the_label_floor_is_four_extra_large_steps() {
    assert_eq!(MIN_LABEL_WIDTH, space::XL * 4.0);
    assert_eq!(MIN_LABEL_WIDTH, 96.0);
}

#[test]
fn the_label_rung_is_the_compact_ui_voice() {
    assert_eq!(LABEL_RUNG.line_height, 18.0);
}

#[test]
fn the_vertical_inset_keeps_the_line_box_inside_both_rungs() {
    assert!(label_fits_row(ROW_HEIGHT_COMPACT));
    assert!(label_fits_row(ROW_HEIGHT_STANDARD));
    assert!(
        !label_fits_row(20.0),
        "a 20px row leaves 16px of interior against an 18px line box"
    );
    assert!(
        !label_fits_row(LABEL_RUNG.line_height + ROW_INSET_Y),
        "the inset has to come out of somewhere"
    );
}

// ---------------------------------------------------------------------------
// 2 - the allowance and the floor
// ---------------------------------------------------------------------------

#[test]
fn the_allowance_is_what_is_left_of_the_fixed_parts() {
    assert_eq!(indent_allowance(128.0), 0.0);
    assert_eq!(indent_allowance(288.0), 160.0);
    assert_eq!(
        indent_allowance(64.0),
        0.0,
        "a row narrower than the fixed parts has no indent room at all"
    );
}

#[test]
fn the_label_floor_is_a_property_of_width_not_of_depth() {
    assert!(label_floor_holds(128.0));
    assert!(label_floor_holds(4_000.0));
    assert!(!label_floor_holds(127.9));
    assert!(!label_floor_holds(0.0));
}

// ---------------------------------------------------------------------------
// 3 - the three tiers
// ---------------------------------------------------------------------------

#[test]
fn depth_zero_has_no_indent_at_all() {
    let geometry = indent_geometry(0, 4_000.0);
    assert_eq!(geometry.width, 0.0);
    assert_eq!(geometry.guides.len(), 0);
    assert_eq!(geometry.collapsed_levels, 0);
    assert_eq!(geometry.step, INDENT_STEP);
}

#[test]
fn tier_one_is_the_nominal_sixteen_pixels() {
    let geometry = indent_geometry(8, 256.0);
    assert_eq!(geometry.step, INDENT_STEP);
    assert_eq!(geometry.width, 8.0 * INDENT_STEP);
    assert_eq!(geometry.guides.len(), 8);
    assert_eq!(geometry.collapsed_levels, 0);
}

#[test]
fn tier_two_compresses_every_level_and_still_draws_every_guide() {
    let geometry = indent_geometry(9, 256.0);
    assert!(geometry.step < INDENT_STEP);
    assert!(geometry.step >= MIN_INDENT_STEP);
    assert_eq!(geometry.width, indent_allowance(256.0));
    assert_eq!(
        geometry.guides.len(),
        9,
        "compression must not drop a level - every level keeps its line"
    );
    assert_eq!(geometry.collapsed_levels, 0);
}

#[test]
fn tier_three_collapses_onto_the_last_guide_and_reports_it() {
    let geometry = indent_geometry(40, 256.0);
    assert_eq!(geometry.step, MIN_INDENT_STEP);
    assert_eq!(geometry.width, indent_allowance(256.0));
    assert_eq!(geometry.guides.len(), 32);
    assert_eq!(geometry.collapsed_levels, 8);
}

#[test]
fn a_row_with_no_allowance_indents_nothing_however_deep() {
    let geometry = indent_geometry(64, 128.0);
    assert_eq!(geometry.width, 0.0);
    assert_eq!(geometry.guides.len(), 0);
    assert_eq!(geometry.collapsed_levels, 64);
}

#[test]
fn the_tier_boundaries_are_exclusive_where_they_should_be() {
    // allowance 128: depth 8 lands exactly on the nominal edge, depth 9 does not.
    assert_eq!(indent_geometry(8, 256.0).step, INDENT_STEP);
    assert_ne!(indent_geometry(9, 256.0).step, INDENT_STEP);
    // depth 32 is exactly the floor step, depth 33 is past it.
    assert_eq!(indent_geometry(32, 256.0).collapsed_levels, 0);
    assert_eq!(indent_geometry(33, 256.0).collapsed_levels, 1);
}

// ---------------------------------------------------------------------------
// 4 - twenty levels, at widths that force every tier
// ---------------------------------------------------------------------------

#[test]
fn twenty_levels_survive_every_width_from_the_floor_up() {
    for width in [128.0, 144.0, 160.0, 192.0, 256.0, 512.0, 2_048.0] {
        for depth in 0..=20usize {
            let geometry = indent_geometry(depth, width);
            assert!(
                geometry.width <= indent_allowance(width),
                "depth {depth} at width {width} took {} of an allowance of {}",
                geometry.width,
                indent_allowance(width)
            );
            assert!(
                width - geometry.width >= MIN_LABEL_WIDTH + 2.0 * ROW_INSET_X + CHEVRON_COLUMN,
                "depth {depth} at width {width} eroded the label floor"
            );
            assert_eq!(
                geometry.guides.len() + geometry.collapsed_levels,
                depth,
                "depth {depth} at width {width}: every level is either drawn or reported collapsed"
            );
            if geometry.width > 0.0 {
                assert!(
                    geometry.step >= MIN_INDENT_STEP,
                    "depth {depth} at width {width} drew a step below the floor"
                );
            }
        }
    }
}

#[test]
fn twenty_levels_at_a_narrow_width_still_show_the_label() {
    // 160px is the tightest row that still clears the floor: allowance 32,
    // so twenty levels draw eight guides and collapse twelve onto the last.
    let geometry = indent_geometry(20, 160.0);
    assert_eq!(geometry.guides.len(), 8);
    assert_eq!(geometry.collapsed_levels, 12);
    assert!(label_floor_holds(160.0));
    assert_eq!(geometry.width, 32.0);
}

#[test]
fn deeper_never_means_less_label() {
    for width in [128.0, 160.0, 256.0, 1_024.0] {
        let mut previous = 0.0;
        for depth in 0..=64usize {
            let geometry = indent_geometry(depth, width);
            assert!(
                geometry.width >= previous,
                "depth {depth} at width {width} shrank the indent below its parent's"
            );
            previous = geometry.width;
        }
    }
}

#[test]
fn guides_are_ordered_and_inside_the_indent_region() {
    for width in [160.0, 256.0, 512.0] {
        for depth in 1..=24usize {
            let geometry = indent_geometry(depth, width);
            let mut last = f32::MIN;
            for guide in &geometry.guides {
                assert!(
                    *guide >= ROW_INSET_X,
                    "depth {depth}: guide {guide} left of the inset"
                );
                assert!(
                    *guide <= ROW_INSET_X + geometry.width + shape::BORDER_HAIRLINE,
                    "depth {depth}: guide {guide} escaped the indent region"
                );
                assert!(*guide > last, "depth {depth}: guides are not ordered");
                last = *guide;
            }
        }
    }
}

#[test]
fn feeding_content_width_back_resolves_to_the_nominal_tier() {
    for depth in 1..=64usize {
        let needed = content_width(depth, MIN_LABEL_WIDTH);
        let geometry = indent_geometry(depth, needed);
        assert_eq!(
            geometry.step, INDENT_STEP,
            "a row sized to content_width({depth}) must never compress"
        );
        assert_eq!(geometry.collapsed_levels, 0);
    }
}

#[test]
fn content_width_is_the_unclamped_sum() {
    assert_eq!(content_width(0, 100.0), 4.0 + 4.0 + 24.0 + 100.0);
    assert_eq!(content_width(3, 100.0), 4.0 + 4.0 + 48.0 + 24.0 + 100.0);
    assert_eq!(
        content_width(2, -50.0),
        content_width(2, 0.0),
        "a negative label width is clamped, not subtracted"
    );
}

// ---------------------------------------------------------------------------
// 5 - the row's slot grid
// ---------------------------------------------------------------------------

#[test]
fn the_padding_puts_the_indent_then_the_chevron_column_first() {
    let padding = row_padding(32.0);
    assert_eq!(padding.left, ROW_INSET_X + 32.0 + CHEVRON_COLUMN);
    assert_eq!(padding.right, ROW_INSET_X);
    assert_eq!(padding.vertical, ROW_INSET_Y);
}

#[test]
fn the_tile_starts_where_the_chevron_column_ends() {
    let row = rect(0.0, 0.0, 400.0, ROW_HEIGHT_STANDARD);
    let slots = row_slots(row, 3, Leading::None, true, 0.0);
    assert_eq!(
        slots.tile.center.x,
        slots.chevron.x + slots.chevron.width,
        "the label must begin exactly at the end of the disclosure column"
    );
}

#[test]
fn the_chevron_column_is_a_square_centred_in_the_row() {
    let row = rect(10.0, 100.0, 400.0, ROW_HEIGHT_STANDARD);
    let slots = row_slots(row, 2, Leading::None, true, 0.0);
    assert_eq!(slots.chevron.width, CHEVRON_COLUMN);
    assert_eq!(slots.chevron.height, CHEVRON_COLUMN);
    assert!(approx(
        slots.chevron.y,
        100.0 + (ROW_HEIGHT_STANDARD - CHEVRON_COLUMN) / 2.0
    ));
}

#[test]
fn the_chevron_column_shifts_right_with_the_indent() {
    let row = rect(0.0, 0.0, 800.0, ROW_HEIGHT_STANDARD);
    let shallow = row_slots(row, 1, Leading::None, true, 0.0);
    let deep = row_slots(row, 4, Leading::None, true, 0.0);
    assert!(deep.chevron.x > shallow.chevron.x);
    assert_eq!(
        deep.chevron.x - shallow.chevron.x,
        deep.indent.width - shallow.indent.width
    );
}

#[test]
fn the_guides_span_the_full_row_height_so_they_join_across_rows() {
    let row = rect(0.0, 50.0, 400.0, ROW_HEIGHT_STANDARD);
    let slots = row_slots(row, 4, Leading::None, true, 0.0);
    assert_eq!(slots.guides.len(), 4);
    for guide in &slots.guides {
        assert_eq!(guide.y, row.y);
        assert_eq!(guide.height, row.height);
        assert_eq!(guide.width, shape::BORDER_HAIRLINE);
    }
    assert_ne!(
        slots.indent.height, slots.guides[0].height,
        "the indent is inset; the guide is not, and they must differ"
    );
}

#[test]
fn the_selection_bar_runs_the_row_at_its_left_edge() {
    let row = rect(20.0, 30.0, 400.0, ROW_HEIGHT_COMPACT);
    let slots = row_slots(row, 3, Leading::None, true, 0.0);
    assert_eq!(slots.selection_bar.x, row.x);
    assert_eq!(slots.selection_bar.width, SELECTION_BAR_WIDTH);
    assert_eq!(slots.selection_bar.height, row.height);
}

#[test]
fn the_selection_bar_cannot_outgrow_an_absurd_row() {
    let slots = row_slots(rect(0.0, 0.0, 1.0, 1.0), 5, Leading::None, true, 0.0);
    assert!(slots.selection_bar.width <= 1.0);
    assert!(slots.indent.width >= 0.0);
    assert!(slots.tile.center.width >= 0.0);
}

#[test]
fn the_tile_is_a_real_list_tile_grid() {
    let row = rect(0.0, 0.0, 400.0, ROW_HEIGHT_STANDARD);
    let slots = row_slots(row, 2, Leading::Icon(LeadingExtent::Small), true, 60.0);
    let tile: TileSlots = slots.tile;
    assert!(tile.leading.width > 0.0);
    assert!(tile.trailing.width >= 60.0);
    assert!(tile.center.width > 0.0);
    assert!(tile.leading.x < tile.center.x);
    assert!(tile.center.x < tile.trailing.x);
}

#[test]
fn the_disclosure_column_cannot_collapse_for_a_leaf() {
    // `row_slots` takes no `expandable` flag, so there is no call path that
    // hands the row its own expandability. A leaf's label therefore starts
    // exactly where a branch's does, which is what keeps a mixed tree from
    // jogging every file title in from of it.
    let row = rect(0.0, 0.0, 400.0, ROW_HEIGHT_STANDARD);
    for depth in 0..=12usize {
        let slots = row_slots(row, depth, Leading::None, true, 0.0);
        assert_eq!(
            slots.chevron.width, CHEVRON_COLUMN,
            "depth {depth}: the disclosure column is a fixed column, not a fitted one"
        );
        assert_eq!(
            slots.tile.center.x,
            slots.chevron.x + CHEVRON_COLUMN,
            "depth {depth}: the label must start at the column's end whether or not \
             anything is drawn inside it"
        );
    }
}

#[test]
fn a_leading_mark_shifts_the_label_by_its_own_slot_and_nothing_else() {
    // The tree's alignment guarantee lives in the chevron column, not in the
    // tile: the tile's leading slot is fitted, so a list that mixed marked
    // and unmarked rows would jog - and says so by taking `leading` per row.
    let row = rect(0.0, 0.0, 400.0, ROW_HEIGHT_STANDARD);
    let bare = row_slots(row, 2, Leading::None, true, 0.0);
    let marked = row_slots(row, 2, Leading::Checkbox, true, 0.0);
    assert_eq!(bare.chevron.x, marked.chevron.x);
    assert_eq!(bare.indent.width, marked.indent.width);
    assert!(marked.tile.center.x > bare.tile.center.x);
    assert_eq!(
        marked.tile.center.x - bare.tile.center.x,
        marked.tile.leading.width + list_tile_gap(),
        "the shift is exactly the leading slot plus one gap"
    );
}

fn list_tile_gap() -> f32 {
    crate::composite::list_tile::SLOT_GAP
}

// ---------------------------------------------------------------------------
// 6 - the contrast matrix, recomputed from raw sRGB
// ---------------------------------------------------------------------------

#[test]
fn the_pinned_figures_recompute_from_the_palette() {
    let p = palette();
    assert!(approx(
        contrast_ratio(p.border_strong, p.surface_raised),
        4.2379
    ));
    assert!(approx(
        contrast_ratio(p.border_strong, p.surface_base),
        4.5062
    ));
    assert!(approx(
        contrast_ratio(p.border_strong, p.surface_hover),
        3.6772
    ));
    assert!(approx(
        contrast_ratio(p.border_strong, p.surface_active),
        3.2377
    ));
    assert!(approx(
        contrast_ratio(p.border_strong, p.accent_fill),
        1.0876
    ));
    assert!(approx(contrast_ratio(p.text_muted, p.accent_fill), 1.8363));
    assert!(approx(
        contrast_ratio(p.accent_border, p.accent_fill),
        1.6098
    ));
    assert!(approx(
        contrast_ratio(p.status_warning, p.accent_fill),
        1.6547
    ));
    assert!(approx(
        contrast_ratio(p.status_warning, p.surface_raised),
        6.4474
    ));
    assert!(approx(contrast_ratio(p.text_strong, p.accent_fill), 4.8020));
}

#[test]
fn border_subtle_would_have_been_unreadable_and_is_not_used() {
    let p = palette();
    assert!(
        !meets_chrome_contrast(p.border_subtle, p.surface_raised),
        "the spec's guide token measures {} on the raised pane; that is why \
         the guide reads border.strong",
        contrast_ratio(p.border_subtle, p.surface_raised)
    );
}

#[test]
fn every_resolved_mark_clears_the_chrome_minimum_on_every_band() {
    let p = palette();
    for (name, band) in bands(&p) {
        for (mark, colour) in [
            ("guide", guide_colour(band, &p)),
            ("chevron", chevron_colour(band, &p)),
            ("selection bar", selection_bar_colour(band, &p)),
            ("warning", warning_colour(band, &p)),
        ] {
            let ratio = contrast_ratio(colour, band);
            assert!(
                ratio >= MIN_CHROME_CONTRAST,
                "{mark} on {name} measures {ratio:.4}, under the {MIN_CHROME_CONTRAST}:1 floor"
            );
        }
    }
}

#[test]
fn marks_are_kept_where_their_nominal_token_shows_and_promoted_where_it_does_not() {
    let p = palette();
    for (name, band) in bands(&p) {
        let on_accent = name.starts_with("accent");
        assert_eq!(
            guide_colour(band, &p) == p.border_strong,
            !on_accent,
            "guide on {name}"
        );
        assert_eq!(
            chevron_colour(band, &p) == p.text_muted,
            !on_accent,
            "chevron on {name}"
        );
        assert_eq!(
            selection_bar_colour(band, &p) == p.accent_border,
            !on_accent,
            "selection bar on {name}"
        );
        assert_eq!(
            warning_colour(band, &p) == p.status_warning,
            !on_accent,
            "warning on {name}"
        );
        if on_accent {
            assert_eq!(guide_colour(band, &p), p.text_strong, "guide on {name}");
            assert_eq!(chevron_colour(band, &p), p.text_strong);
            assert_eq!(selection_bar_colour(band, &p), p.text_strong);
            assert_eq!(warning_colour(band, &p), p.text_strong);
        }
    }
}

#[test]
fn promotion_is_a_function_of_measured_contrast_not_of_a_name_list() {
    let p = palette();
    let visible = Color::from_rgb8(250, 250, 250);
    let invisible = Color::from_rgb8(95, 98, 236);
    assert_eq!(
        promoted_over(visible, p.accent_fill, &p),
        visible,
        "a mark that can be seen is left alone"
    );
    assert_eq!(
        promoted_over(invisible, p.accent_fill, &p),
        p.text_strong,
        "a near-identical mark is promoted rather than painted invisible"
    );
}

#[test]
fn the_selection_lock_is_by_role_not_by_threshold() {
    // §3B.6 locks the label, chevron, guides and bar because of what the
    // band IS, not because a measured ratio fell short. A candidate that
    // would clear the floor on the locked fill is locked anyway — that is
    // the whole difference from `promoted_over` above.
    let p = palette();
    assert!(
        meets_chrome_contrast(p.text_primary, p.accent_fill),
        "premise: text.primary clears 3:1 on accent.fill ({:.4})",
        contrast_ratio(p.text_primary, p.accent_fill)
    );
    assert_eq!(
        locked_over(p.text_primary, p.accent_fill, &p),
        p.text_strong,
        "a passing candidate is still locked, because the band is a locked fill"
    );

    // Off a locked fill the candidate passes through untouched, so an
    // unselected row keeps the tokens §3B.6 prices for it.
    assert_eq!(
        locked_over(p.border_strong, p.surface_base, &p),
        p.border_strong
    );
    assert_eq!(
        locked_over(p.text_muted, p.surface_raised, &p),
        p.text_muted
    );
    assert_eq!(
        locked_over(p.accent_border, p.surface_hover, &p),
        p.accent_border
    );
}

#[test]
fn the_locked_marks_do_not_depend_on_the_resting_tokens_contrast() {
    // The three role-locked marks resolve to text.strong on every locked
    // fill, whatever their resting token measures there.
    let p = palette();
    for fill in [p.accent_fill, p.accent_fill_active] {
        assert_eq!(guide_colour(fill, &p), p.text_strong, "guide on {fill:?}");
        assert_eq!(chevron_colour(fill, &p), p.text_strong);
        assert_eq!(selection_bar_colour(fill, &p), p.text_strong);
    }
    // The warning mark is NOT role-locked; it keeps its hue wherever it
    // clears, and is threshold-promoted only where it does not.
    assert_eq!(warning_colour(p.surface_base, &p), p.status_warning);
    assert_eq!(warning_colour(p.accent_fill, &p), p.text_strong);
    assert!(
        contrast_ratio(p.status_warning, p.accent_fill) < 3.0,
        "premise: the warning is sub-floor on the selection fill ({:.4})",
        contrast_ratio(p.status_warning, p.accent_fill)
    );
}

#[test]
fn the_disabled_label_lands_inside_the_legibility_band_on_every_pane_surface() {
    let p = palette();
    for (name, surface) in [
        ("surface.base", p.surface_base),
        ("surface.raised", p.surface_raised),
        ("surface.hover", p.surface_hover),
        ("surface.active", p.surface_active),
    ] {
        assert!(
            disabled_label_in_band(surface, &p),
            "text.disabled on {name} measures {} and must sit in [4.0, 6.0]",
            contrast_ratio(p.text_disabled, surface)
        );
    }
    assert!(approx(
        contrast_ratio(p.text_disabled, p.surface_base),
        5.9165
    ));
    assert!(approx(
        contrast_ratio(p.text_disabled, p.surface_active),
        4.2510
    ));
}

// ---------------------------------------------------------------------------
// 7 - the band, and the style precedence
// ---------------------------------------------------------------------------

#[test]
fn a_transparent_background_paints_its_marks_on_the_pane() {
    let p = palette();
    assert_eq!(
        band_colour(Color::TRANSPARENT, p.surface_raised),
        p.surface_raised
    );
    assert_eq!(band_colour(p.accent_fill, p.surface_raised), p.accent_fill);
}

#[test]
fn a_resting_row_paints_no_band() {
    let p = palette();
    let style = resolve_style(false, false, false, BaseState::Rest, p.surface_base, &p);
    assert_eq!(style.background, Color::TRANSPARENT);
    assert_eq!(style.band, p.surface_base);
    assert_eq!(style.selection_bar, Color::TRANSPARENT);
    assert_eq!(style.warning, Color::TRANSPARENT);
    assert_eq!(style.label_colour, p.text_primary);
    assert_eq!(style.trailing_colour, p.text_muted);
}

#[test]
fn hover_and_press_paint_their_surface_rungs() {
    let p = palette();
    let hover = resolve_style(false, false, false, BaseState::Hovered, p.surface_base, &p);
    assert_eq!(hover.background, p.surface_hover);
    assert_eq!(hover.band, p.surface_hover);
    let press = resolve_style(false, false, false, BaseState::Pressed, p.surface_base, &p);
    assert_eq!(press.background, p.surface_active);
    assert_eq!(press.band, p.surface_active);
}

#[test]
fn selection_outranks_the_pointer() {
    let p = palette();
    for base in [BaseState::Rest, BaseState::Hovered, BaseState::Pressed] {
        let style = resolve_style(true, false, false, base, p.surface_base, &p);
        assert_eq!(
            style.background, p.accent_fill,
            "selection must not be dimmed by a resting or hovering pointer ({base:?})"
        );
        assert_eq!(style.label_colour, p.text_strong);
        assert_eq!(style.leading_colour, p.text_strong);
        assert_eq!(style.trailing_colour, p.text_strong);
        assert_ne!(style.selection_bar, Color::TRANSPARENT);
    }
}

#[test]
fn a_disabled_row_outranks_selection_and_shows_no_band() {
    let p = palette();
    for base in [BaseState::Rest, BaseState::Hovered, BaseState::Pressed] {
        let style = resolve_style(true, true, true, base, p.surface_raised, &p);
        assert_eq!(
            style.background,
            Color::TRANSPARENT,
            "a dead row under a pointer must not look live ({base:?})"
        );
        assert_eq!(style.band, p.surface_raised);
        assert_eq!(style.label_colour, p.text_disabled);
        assert_eq!(style.chevron, p.text_disabled);
        assert_eq!(style.selection_bar, Color::TRANSPARENT);
        assert_eq!(style.warning, Color::TRANSPARENT);
    }
}

#[test]
fn a_warning_shows_on_a_resting_row_and_is_promoted_on_the_selection_fill() {
    let p = palette();
    let resting = resolve_style(false, false, true, BaseState::Rest, p.surface_raised, &p);
    assert_eq!(resting.warning, p.status_warning);
    let selected = resolve_style(true, false, true, BaseState::Rest, p.surface_raised, &p);
    assert_eq!(
        selected.warning,
        p.text_strong,
        "status.warning on accent.fill measures {} - it would be invisible",
        contrast_ratio(p.status_warning, p.accent_fill)
    );
    assert_ne!(
        resting.warning, selected.warning,
        "the same warning on two bands resolves to two colours; that is the D-6 rule working"
    );
}

#[test]
fn the_band_always_matches_the_painted_background() {
    let p = palette();
    for selected in [false, true] {
        for disabled in [false, true] {
            for base in [BaseState::Rest, BaseState::Hovered, BaseState::Pressed] {
                let style = resolve_style(selected, disabled, false, base, p.surface_raised, &p);
                assert_eq!(
                    style.band,
                    band_colour(style.background, p.surface_raised),
                    "band must be what the marks are actually painted over \
                     (selected={selected} disabled={disabled} {base:?})"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 8 - layers
// ---------------------------------------------------------------------------

#[test]
fn the_band_layer_exists_only_when_there_is_a_band() {
    let p = palette();
    let bounds = rect(0.0, 0.0, 300.0, 28.0);
    let resting = resolve_style(false, false, false, BaseState::Rest, p.surface_base, &p);
    assert!(row_layer(bounds, &resting).is_none());
    let selected = resolve_style(true, false, false, BaseState::Rest, p.surface_base, &p);
    let layer = row_layer(bounds, &selected).expect("a selected row paints");
    assert_eq!(layer.bounds, bounds);
    assert_eq!(layer.fill, p.accent_fill);
}

#[test]
fn the_selection_bar_layer_exists_only_on_a_selected_row() {
    let p = palette();
    let bar = rect(0.0, 0.0, 3.0, 28.0);
    let resting = resolve_style(false, false, false, BaseState::Rest, p.surface_base, &p);
    assert!(selection_bar_layer(bar, &resting).is_none());
    let selected = resolve_style(true, false, false, BaseState::Rest, p.surface_base, &p);
    assert_eq!(
        selection_bar_layer(bar, &selected)
            .expect("selected paints a bar")
            .fill,
        selected.selection_bar
    );
}

#[test]
fn one_guide_layer_per_drawn_guide() {
    let p = palette();
    let row = rect(0.0, 0.0, 400.0, ROW_HEIGHT_STANDARD);
    let style = resolve_style(false, false, false, BaseState::Rest, p.surface_raised, &p);
    for depth in [0usize, 1, 5, 20] {
        let slots = row_slots(row, depth, Leading::None, true, 0.0);
        let layers = guide_layers(&slots, &style);
        assert_eq!(layers.len(), slots.guides.len(), "depth {depth}");
        for (layer, guide) in layers.iter().zip(&slots.guides) {
            assert_eq!(layer.bounds, *guide);
            assert_eq!(layer.fill, style.guide);
        }
    }
}
