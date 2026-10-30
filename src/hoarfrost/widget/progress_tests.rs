//! Tests for the progress resolvers and geometry — `UIF-ATOM-009`, §3.9.
//!
//! Five groups, in order:
//!
//! 1. **The bar.** Track, radius, and the three status fills measured
//!    against the track from raw sRGB.
//! 2. **The indeterminate segment.** Its width, its travel, the way
//!    `phase` wraps, and the static striped form under injected
//!    reduced motion.
//! 3. **The spinner.** Both diameters, the borrowed stroke, and the arc
//!    that swaps to `text.strong` inside a solid button.
//! 4. **Layers.** What is painted, where, and what is deliberately
//!    absent.
//! 5. **Determinism.** The same injected phase always gives the same
//!    frame — the property the whole no-clock rule exists to buy.

use super::*;
use crate::token::contrast;

/// Every status, in declaration order.
const STATUSES: [ProgressStatus; 3] = [
    ProgressStatus::Active,
    ProgressStatus::Paused,
    ProgressStatus::Error,
];

/// Assert a measured ratio against its true value to four places.
fn assert_ratio(actual: f32, expected: f32, what: &str) {
    let measured = f64::from(actual);
    let wanted = f64::from(expected);
    assert!(
        (measured - wanted).abs() < 0.000_5,
        "{what}: expected {wanted:.4}, measured {measured:.4}"
    );
}

/// A track of a known size for the layer tests.
fn track() -> Rectangle {
    Rectangle::new(Point::new(10.0, 20.0), iced::Size::new(200.0, 4.0))
}

#[test]
fn the_track_is_four_pixels_of_pill_on_surface_active() {
    let palette = Palette::dark();
    let style = resolve_style(&ProgressModel::indeterminate(), &palette);

    assert_eq!(style.height, TRACK_HEIGHT);
    assert_eq!(TRACK_HEIGHT, size::PROGRESS_BAR_HEIGHT);
    assert_eq!(style.height, 4.0);
    assert_eq!(style.radius, shape::RADIUS_PILL);
    assert_eq!(style.track, palette.surface_active);
}

#[test]
fn each_status_fills_with_the_colour_section_3_9_assigns() {
    let palette = Palette::dark();

    assert_eq!(
        fill_colour(ProgressStatus::Active, &palette),
        palette.accent_border
    );
    assert_eq!(
        fill_colour(ProgressStatus::Paused, &palette),
        palette.status_warning
    );
    assert_eq!(
        fill_colour(ProgressStatus::Error, &palette),
        palette.status_danger_text
    );
}

#[test]
fn the_recomputed_fill_contrasts_match_every_figure_3_9_prints() {
    let palette = Palette::dark();
    let track_colour = palette.surface_active;

    // Unlike §3.8's badge column, every one of §3.9's printed ratios
    // survives recomputation. Pinned so a retune that breaks one fails
    // here rather than shipping.
    assert_ratio(
        contrast::contrast_ratio(palette.accent_border, track_colour),
        4.7922,
        "accent.border on surface.active (Active)",
    );
    assert_ratio(
        contrast::contrast_ratio(palette.status_warning, track_colour),
        4.9256,
        "status.warning on surface.active (Paused)",
    );
    assert_ratio(
        contrast::contrast_ratio(palette.status_danger_text, track_colour),
        4.7223,
        "status.danger.text on surface.active (Error)",
    );
}

#[test]
fn every_status_fill_clears_the_judged_chrome_floor() {
    let palette = Palette::dark();

    for status in STATUSES {
        let fill = fill_colour(status, &palette);
        let ratio = contrast::contrast_ratio(fill, palette.surface_active);
        assert_eq!(progress_chrome_class(fill, &palette), ChromeClass::Judged);
        assert!(
            ratio >= contrast::MIN_CHROME_CONTRAST,
            "{status:?} fill reaches only {ratio:.4}:1 against the track"
        );
    }
}

#[test]
fn the_indeterminate_segment_is_thirty_five_percent_of_the_track() {
    let palette = Palette::dark();
    let segment = indeterminate_segment(ProgressStatus::Active, 0.5, false, &palette);

    assert_eq!(segment.width_fraction, INDETERMINATE_SEGMENT_FRACTION);
    assert_eq!(
        segment.width_fraction * track().width,
        70.0,
        "35% of a 200px track"
    );
    assert!(!segment.striped);
}

#[test]
fn the_segment_slides_in_rather_than_appearing_whole_at_the_left_edge() {
    let palette = Palette::dark();
    let start = indeterminate_segment(ProgressStatus::Active, 0.0, false, &palette);

    assert_eq!(start.offset_fraction, -INDETERMINATE_SEGMENT_FRACTION);
    assert!(
        start.offset_fraction < 0.0,
        "at phase 0 the block must still be off-track, entering"
    );
}

#[test]
fn the_segment_reaches_the_right_edge_just_before_the_loop_restarts() {
    let palette = Palette::dark();
    let near_end = indeterminate_segment(ProgressStatus::Active, 0.999, false, &palette);

    assert!(
        near_end.offset_fraction > 1.0 - 0.01,
        "at phase 0.999 the block should be leaving the track, got {}",
        near_end.offset_fraction
    );

    // Phase 1.0 *is* phase 0.0: the loop restarts rather than parking
    // the block off the right-hand end of the track.
    let restart = indeterminate_segment(ProgressStatus::Active, 1.0, false, &palette);
    assert_eq!(restart.offset_fraction, -INDETERMINATE_SEGMENT_FRACTION);
}

#[test]
fn the_segment_sits_mid_track_at_the_middle_of_its_travel() {
    let palette = Palette::dark();
    let mid = indeterminate_segment(ProgressStatus::Active, 0.5, false, &palette);

    assert!((mid.offset_fraction - 0.325).abs() < 1e-6);
}

#[test]
fn phase_wraps_so_a_host_need_not_track_the_loop() {
    let palette = Palette::dark();
    let one_turn = indeterminate_segment(ProgressStatus::Active, 0.25, false, &palette);
    let plus_a_turn = indeterminate_segment(ProgressStatus::Active, 1.25, false, &palette);
    let plus_three_turns = indeterminate_segment(ProgressStatus::Active, 3.25, false, &palette);

    assert_eq!(one_turn, plus_a_turn);
    assert_eq!(one_turn, plus_three_turns);
}

#[test]
fn a_negative_phase_wraps_forward_instead_of_running_backwards() {
    let palette = Palette::dark();
    let negative = indeterminate_segment(ProgressStatus::Active, -0.25, false, &palette);
    let forward = indeterminate_segment(ProgressStatus::Active, 0.75, false, &palette);

    assert_eq!(negative, forward);
    assert!(negative.offset_fraction >= -INDETERMINATE_SEGMENT_FRACTION);
}

#[test]
fn the_segment_never_leaves_the_track_it_travels_on() {
    let palette = Palette::dark();

    for step in 0..=100 {
        let phase = step as f32 / 10.0;
        let segment = indeterminate_segment(ProgressStatus::Active, phase, false, &palette);
        assert!(
            segment.offset_fraction >= -INDETERMINATE_SEGMENT_FRACTION - 1e-5
                && segment.offset_fraction <= 1.0 + 1e-5,
            "phase {phase} put the segment at {}",
            segment.offset_fraction
        );
    }
}

#[test]
fn reduced_motion_stops_the_segment_and_stripes_it() {
    let palette = Palette::dark();
    let static_segment = indeterminate_segment(ProgressStatus::Active, 0.0, true, &palette);

    assert!(static_segment.striped);
    assert_eq!(
        static_segment.offset_fraction, 0.0,
        "a static segment sits at the start of the track"
    );
    assert_eq!(
        static_segment.width_fraction,
        INDETERMINATE_SEGMENT_FRACTION
    );
}

#[test]
fn under_reduced_motion_the_phase_stops_mattering() {
    let palette = Palette::dark();
    let a = indeterminate_segment(ProgressStatus::Active, 0.1, true, &palette);
    let b = indeterminate_segment(ProgressStatus::Active, 7.9, true, &palette);

    assert_eq!(
        a, b,
        "nothing may move when the host has said motion is reduced"
    );
}

#[test]
fn the_stripe_period_is_a_private_rendering_detail_not_a_token() {
    assert_eq!(stripe_period(), STRIPE_PERIOD);
    assert!(stripe_period() > 0.0);
    assert!(stripe_period() < SPINNER_SMALL);
}

#[test]
fn the_two_spinner_diameters_are_the_two_spec_rungs() {
    let palette = Palette::dark();
    let small = spinner_style(false, false, &palette);
    let large = spinner_style(true, false, &palette);

    assert_eq!(small.diameter, SPINNER_SMALL);
    assert_eq!(SPINNER_SMALL, size::SPINNER_SM);
    assert_eq!(small.diameter, 14.0);
    assert_eq!(large.diameter, SPINNER_MEDIUM);
    assert_eq!(SPINNER_MEDIUM, size::SPINNER_MD);
    assert_eq!(large.diameter, 20.0);
}

#[test]
fn the_stroke_is_two_pixels_borrowed_from_the_only_two_pixel_weight_that_exists() {
    let palette = Palette::dark();
    let style = spinner_style(false, false, &palette);

    assert_eq!(style.stroke, SPINNER_STROKE);
    assert_eq!(style.stroke, 2.0);
    assert_eq!(
        SPINNER_STROKE,
        shape::BORDER_FOCUS,
        "§2.3's closed stroke set has exactly one 2px weight and §3.9's \
         spinner has no token of its own — see the module docs"
    );
}

#[test]
fn the_arc_is_accent_text_outside_and_text_strong_inside_a_solid_button() {
    let palette = Palette::dark();

    assert_eq!(
        spinner_style(false, false, &palette).arc,
        palette.accent_text
    );
    assert_eq!(
        spinner_style(false, true, &palette).arc,
        palette.text_strong
    );
    assert_eq!(spinner_style(true, true, &palette).arc, palette.text_strong);
}

#[test]
fn the_accent_arc_would_not_have_been_readable_inside_a_solid_fill() {
    let palette = Palette::dark();

    assert_ratio(
        contrast::contrast_ratio(palette.accent_text, palette.accent_fill),
        2.5719,
        "accent.text on accent.fill",
    );
    assert_ratio(
        contrast::contrast_ratio(palette.accent_text, palette.status_danger_fill),
        2.7460,
        "accent.text on status.danger.fill",
    );
    assert_ratio(
        contrast::contrast_ratio(palette.text_strong, palette.accent_fill),
        4.8020,
        "text.strong on accent.fill",
    );
    assert_ratio(
        contrast::contrast_ratio(palette.text_strong, palette.status_danger_fill),
        5.1270,
        "text.strong on status.danger.fill",
    );
}

#[test]
fn the_spinner_track_is_a_resting_divider_and_the_arc_is_judged() {
    let palette = Palette::dark();
    let style = spinner_style(false, false, &palette);

    assert_eq!(style.track, palette.border_subtle);
    assert_eq!(
        spinner_arc_chrome_class(style.arc, &palette),
        ChromeClass::Judged
    );
    assert_eq!(
        spinner_arc_chrome_class(palette.border_subtle, &palette),
        ChromeClass::RestingDivider
    );
    assert_ratio(
        contrast::contrast_ratio(palette.accent_text, palette.surface_overlay),
        9.4891,
        "accent.text on surface.overlay — the figure §3.9 prints",
    );
}

#[test]
fn the_track_layer_paints_the_track_with_no_border() {
    let palette = Palette::dark();
    let style = resolve_style(&ProgressModel::indeterminate(), &palette);
    let layer = track_layer(track(), &style);

    assert_eq!(layer.bounds, track());
    assert_eq!(layer.fill, palette.surface_active);
    assert_eq!(layer.border.width, 0.0);
    assert_eq!(layer.border.color, Color::TRANSPARENT);
}

#[test]
fn the_fill_layer_is_absent_until_something_is_complete() {
    let palette = Palette::dark();
    let style = resolve_style(&ProgressModel::determinate(0, 4), &palette);

    assert!(fill_layer(track(), &style, 0.0).is_none());
    assert!(fill_layer(track(), &style, -0.5).is_none());
}

#[test]
fn the_fill_layer_grows_with_the_fraction_and_clamps_at_the_track() {
    let palette = Palette::dark();
    let style = resolve_style(&ProgressModel::determinate(1, 4), &palette);
    let quarter = fill_layer(track(), &style, 0.25).expect("a quarter fill");

    assert_eq!(quarter.bounds.width, 50.0);
    assert_eq!(quarter.bounds.x, 10.0);
    assert_eq!(quarter.bounds.height, 4.0);
    assert_eq!(quarter.fill, style.fill);

    let over = fill_layer(track(), &style, 1.5).expect("a clamped fill");
    assert_eq!(over.bounds.width, 200.0);
}

#[test]
fn the_segment_layer_places_the_block_at_the_resolved_offset() {
    let palette = Palette::dark();
    let segment = indeterminate_segment(ProgressStatus::Active, 0.5, false, &palette);
    let layer = segment_layer(track(), &segment);

    assert_eq!(layer.bounds.width, 70.0);
    assert!((layer.bounds.x - (10.0 + 200.0 * 0.325)).abs() < 1e-3);
    assert_eq!(layer.fill, palette.accent_border);
}

#[test]
fn the_stop_button_is_a_real_hit_target() {
    assert_eq!(STOP_BUTTON_SIZE, size::HIT_TARGET_MIN);
    assert_eq!(STOP_BUTTON_SIZE, 24.0);
    // Checked at compile time: the stop control must never be sized down
    // to the spinner it sits beside, or a tap would land on the arc.
    const _: () = assert!(STOP_BUTTON_SIZE > SPINNER_SMALL);
}

#[test]
fn the_resolvers_are_deterministic_for_an_injected_phase() {
    let palette = Palette::dark();

    for status in STATUSES {
        for step in 0..=20 {
            let phase = step as f32 / 7.0;
            for reduced in [false, true] {
                assert_eq!(
                    indeterminate_segment(status, phase, reduced, &palette),
                    indeterminate_segment(status, phase, reduced, &palette),
                    "{status:?} at phase {phase} (reduced {reduced}) resolved twice differently"
                );
            }
        }
    }
}

#[test]
fn resolving_a_bar_twice_gives_the_same_style() {
    let palette = Palette::dark();

    for status in STATUSES {
        let model = ProgressModel::determinate(3, 7).with_status(status);
        assert_eq!(
            resolve_style(&model, &palette),
            resolve_style(&model, &palette)
        );
    }
}
