//! Tests for [`super::scroll_view`] - the overlay layout-shift verdict, the
//! thumb's minimum length at extreme ratios, the recomputed contrast matrix the
//! `Judged` thumb is graded against, the keyboard and reach-end contract, and the
//! drag transcript a thumb settles through (`UIF-COMP-004`).

use crate::binding::GestureStart;
use crate::composite::scroll_view::{
    ARROW_STEP, AT_BOTTOM_TOLERANCE, Axis, LayoutMode, PAGE_KEEP, REACH_END_DISTANCE, ScrollKey,
    ScrollTick, THUMB_CHROME_CLASS, THUMB_MIN_LENGTH, THUMB_RADIUS, TICK_WIDTH, TRACK_WIDTH_ACTIVE,
    TRACK_WIDTH_RESERVED, TRACK_WIDTH_REST, ThumbDrag, TickKind, TrackPhase, at_bottom_within_2px,
    chrome_floor, clamp_offset, content_bounds, in_reach_end, is_scrollable, max_offset, page_step,
    resolve_phase, scroll_to_key, thumb_clears_chrome_floor, thumb_color, thumb_length,
    thumb_offset, thumb_rect, tick_rect, track_bounds, track_width,
};
use crate::token::color::Palette;
use crate::token::contrast::{ChromeClass, contrast_ratio};
use crate::token::{shape, size, space};
use iced::{Color, Point, Rectangle};

/// A contrast ratio compared at the four decimal places the pinned figures are
/// written to.
fn approx(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 5e-4
}

/// Whether a resolved distance is a real, positive measure.
///
/// A function rather than a bare comparison so the assertion is not between two
/// compile-time constants.
fn positive(value: f32) -> bool {
    value > 0.0
}

fn viewport(w: f32, h: f32) -> Rectangle {
    Rectangle::new(Point::new(4.0, 8.0), iced::Size::new(w, h))
}

fn palette() -> Palette {
    Palette::dark()
}

// ---------------------------------------------------------------------------
// 1 - the printed widths
// ---------------------------------------------------------------------------

#[test]
fn the_resting_track_is_the_small_space_rung() {
    assert_eq!(TRACK_WIDTH_REST, 8.0);
    assert_eq!(TRACK_WIDTH_REST, space::SM);
}

#[test]
fn the_active_track_is_the_splitter_grab() {
    assert_eq!(TRACK_WIDTH_ACTIVE, 10.0);
    assert_eq!(TRACK_WIDTH_ACTIVE, size::SPLITTER_GRAB);
}

#[test]
fn a_reserved_gutter_is_priced_at_the_widest_state() {
    assert_eq!(
        TRACK_WIDTH_RESERVED, TRACK_WIDTH_ACTIVE,
        "reserving the resting width would let a hover outgrow its own reservation"
    );
}

#[test]
fn the_thumb_floor_is_the_hit_target() {
    assert_eq!(THUMB_MIN_LENGTH, 24.0);
    assert_eq!(THUMB_MIN_LENGTH, size::HIT_TARGET_MIN);
}

#[test]
fn the_thumb_is_a_pill_and_the_tick_is_two_pixels() {
    assert_eq!(THUMB_RADIUS, shape::RADIUS_PILL);
    assert_eq!(TICK_WIDTH, 2.0);
    assert_eq!(TICK_WIDTH, space::XXS);
}

// ---------------------------------------------------------------------------
// 2 - phase resolution
// ---------------------------------------------------------------------------

#[test]
fn an_unscrollable_view_hides_its_bar() {
    assert!(!is_scrollable(100.0, 100.0));
    assert!(!is_scrollable(50.0, 100.0));
    assert!(is_scrollable(101.0, 100.0));
    assert_eq!(
        resolve_phase(false, true, false),
        TrackPhase::Hidden,
        "a bar you cannot move must not invite the interaction"
    );
}

#[test]
fn drag_outranks_hover_and_hover_outranks_rest() {
    assert_eq!(resolve_phase(true, false, false), TrackPhase::Rest);
    assert_eq!(resolve_phase(true, true, false), TrackPhase::Hovered);
    assert_eq!(resolve_phase(true, true, true), TrackPhase::Dragged);
    assert_eq!(resolve_phase(true, false, true), TrackPhase::Dragged);
}

#[test]
fn hidden_paints_nothing_in_overlay_mode() {
    assert_eq!(track_width(TrackPhase::Hidden, LayoutMode::Overlay), 0.0);
}

#[test]
fn overlay_mode_grows_with_the_phase() {
    assert_eq!(
        track_width(TrackPhase::Rest, LayoutMode::Overlay),
        TRACK_WIDTH_REST
    );
    assert_eq!(
        track_width(TrackPhase::Hovered, LayoutMode::Overlay),
        TRACK_WIDTH_ACTIVE
    );
    assert_eq!(
        track_width(TrackPhase::Dragged, LayoutMode::Overlay),
        TRACK_WIDTH_ACTIVE
    );
}

#[test]
fn reserved_mode_is_constant_across_every_phase() {
    for phase in [
        TrackPhase::Hidden,
        TrackPhase::Rest,
        TrackPhase::Hovered,
        TrackPhase::Dragged,
    ] {
        assert_eq!(
            track_width(phase, LayoutMode::Reserved),
            TRACK_WIDTH_RESERVED,
            "reserved mode varied at {phase:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 3 - the layout-shift verdict
// ---------------------------------------------------------------------------

#[test]
fn overlay_content_is_identical_across_every_track_phase() {
    // `content_bounds` has no phase parameter, so this is the invariant stated as
    // a signature and checked as a fact: the 8px -> 10px growth is paint-only.
    let v = viewport(400.0, 300.0);
    for phase in [
        TrackPhase::Hidden,
        TrackPhase::Rest,
        TrackPhase::Hovered,
        TrackPhase::Dragged,
    ] {
        let _ = phase;
        let content = content_bounds(v, LayoutMode::Overlay, Axis::Vertical);
        assert_eq!(content.width, 400.0, "overlay content shrank at {phase:?}");
        assert_eq!(content.height, 300.0);
        assert_eq!(content.x, v.x);
        assert_eq!(content.y, v.y);
    }
}

#[test]
fn reserved_content_is_also_phase_invariant() {
    let v = viewport(400.0, 300.0);
    let a = content_bounds(v, LayoutMode::Reserved, Axis::Vertical);
    let b = content_bounds(v, LayoutMode::Reserved, Axis::Vertical);
    assert_eq!(a, b);
}

#[test]
fn a_reserved_gutter_costs_the_widest_track_on_the_scroll_axis() {
    let v = viewport(400.0, 300.0);
    let vertical = content_bounds(v, LayoutMode::Reserved, Axis::Vertical);
    assert_eq!(vertical.width, 400.0 - TRACK_WIDTH_RESERVED);
    assert_eq!(
        vertical.height, 300.0,
        "the vertical gutter must not eat height"
    );

    let horizontal = content_bounds(v, LayoutMode::Reserved, Axis::Horizontal);
    assert_eq!(horizontal.height, 300.0 - TRACK_WIDTH_RESERVED);
    assert_eq!(horizontal.width, 400.0);
}

#[test]
fn a_narrow_viewport_clamps_instead_of_going_negative() {
    let v = viewport(4.0, 300.0);
    let content = content_bounds(v, LayoutMode::Reserved, Axis::Vertical);
    assert_eq!(content.width, 0.0);
}

#[test]
fn the_track_lies_against_the_trailing_edge() {
    let v = viewport(400.0, 300.0);
    let track = track_bounds(v, TrackPhase::Rest, LayoutMode::Overlay, Axis::Vertical);
    assert_eq!(track.width, TRACK_WIDTH_REST);
    assert_eq!(track.x + track.width, v.x + v.width);
    assert_eq!(track.y, v.y);
    assert_eq!(track.height, 300.0);

    let h = track_bounds(v, TrackPhase::Rest, LayoutMode::Overlay, Axis::Horizontal);
    assert_eq!(h.height, TRACK_WIDTH_REST);
    assert_eq!(h.y + h.height, v.y + v.height);
}

#[test]
fn the_track_grows_inward_so_it_never_leaves_the_viewport() {
    let v = viewport(400.0, 300.0);
    for phase in [TrackPhase::Rest, TrackPhase::Hovered, TrackPhase::Dragged] {
        let track = track_bounds(v, phase, LayoutMode::Overlay, Axis::Vertical);
        assert!(track.x + track.width <= v.x + v.width + 1e-4, "{phase:?}");
        assert!(track.x >= v.x, "{phase:?} grew outside the viewport");
    }
}

// ---------------------------------------------------------------------------
// 4 - thumb length and the extreme-ratio floor
// ---------------------------------------------------------------------------

#[test]
fn a_proportional_thumb_fills_what_it_should() {
    // content 800, viewport 400 -> the viewport is half the content, so the
    // thumb is half the 400px track.
    assert_eq!(thumb_length(800.0, 400.0), 200.0);
}

#[test]
fn an_extreme_ratio_is_lifted_to_the_hit_target_floor() {
    // Proportionally this is 1.0px, which is not a target.
    let len = thumb_length(10_000.0, 100.0);
    assert_eq!(len, THUMB_MIN_LENGTH);
    assert_eq!(len, 24.0);
}

#[test]
fn the_thumb_never_exceeds_the_track() {
    assert!(thumb_length(401.0, 400.0) <= 400.0);
    assert!(thumb_length(400.0, 400.0) <= 400.0);
    assert!(thumb_length(100_000.0, 400.0) <= 400.0);
}

#[test]
fn a_view_with_nothing_to_scroll_fills_its_track() {
    // Content shorter than the viewport is not scrollable, so the bar is Hidden
    // and the length is moot - but it must still be a whole, in-bounds thumb
    // rather than something negative or overflowing.
    assert_eq!(thumb_length(100.0, 400.0), 400.0);
    assert_eq!(thumb_length(400.0, 400.0), 400.0);
}

#[test]
fn a_zero_viewport_still_yields_a_hittable_thumb() {
    assert_eq!(thumb_length(1000.0, 0.0), THUMB_MIN_LENGTH);
}

#[test]
fn a_zero_content_length_does_not_divide() {
    assert!(thumb_length(0.0, 400.0).is_finite());
    assert!(thumb_length(0.0, 0.0).is_finite());
}

#[test]
fn the_floor_holds_across_every_ratio_that_needs_it() {
    for content in [1_000.0, 5_000.0, 50_000.0, 1_000_000.0] {
        for view in [50.0, 100.0, 200.0] {
            assert!(
                thumb_length(content, view) >= THUMB_MIN_LENGTH,
                "content {content} / viewport {view} produced an unhittable thumb"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 5 - thumb travel
// ---------------------------------------------------------------------------

#[test]
fn the_thumb_starts_at_the_top_at_zero_offset() {
    let len = thumb_length(800.0, 400.0);
    assert_eq!(thumb_offset(0.0, 800.0, 400.0, len), 0.0);
}

#[test]
fn the_thumb_reaches_the_far_end_of_its_travel_at_max_offset() {
    let len = thumb_length(800.0, 400.0);
    let max = max_offset(800.0, 400.0);
    let lead = thumb_offset(max, 800.0, 400.0, len);
    assert!(
        approx(lead, 400.0 - len),
        "thumb ended at {lead}, expected {}",
        400.0 - len
    );
}

#[test]
fn the_thumb_travels_monotonically_with_the_offset() {
    let len = thumb_length(2_000.0, 300.0);
    let max = max_offset(2_000.0, 300.0);
    let mut previous = -1.0;
    for step in 0..=20 {
        let offset = max * step as f32 / 20.0;
        let lead = thumb_offset(offset, 2_000.0, 300.0, len);
        assert!(lead >= previous, "thumb went backwards at step {step}");
        previous = lead;
    }
}

#[test]
fn an_unscrollable_view_has_no_travel() {
    assert_eq!(max_offset(100.0, 400.0), 0.0);
    assert_eq!(thumb_offset(0.0, 100.0, 400.0, 24.0), 0.0);
}

#[test]
fn the_thumb_stays_inside_its_track_at_every_offset() {
    let v = viewport(400.0, 300.0);
    let track = track_bounds(v, TrackPhase::Rest, LayoutMode::Overlay, Axis::Vertical);
    let max = max_offset(3_000.0, 300.0);
    for step in 0..=20 {
        let offset = max * step as f32 / 20.0;
        let thumb = thumb_rect(track, offset, 3_000.0, 300.0, Axis::Vertical);
        assert!(thumb.y >= track.y - 1e-4, "thumb escaped the top at {step}");
        assert!(
            thumb.y + thumb.height <= track.y + track.height + 1e-4,
            "thumb escaped the bottom at {step}"
        );
        assert_eq!(thumb.width, track.width);
    }
}

#[test]
fn the_horizontal_thumb_travels_along_x() {
    let v = viewport(600.0, 200.0);
    let track = track_bounds(
        v,
        TrackPhase::Dragged,
        LayoutMode::Overlay,
        Axis::Horizontal,
    );
    let max = max_offset(2_400.0, 600.0);
    let start = thumb_rect(track, 0.0, 2_400.0, 600.0, Axis::Horizontal);
    let end = thumb_rect(track, max, 2_400.0, 600.0, Axis::Horizontal);
    assert_eq!(start.x, track.x);
    assert!(end.x > start.x, "the horizontal thumb never moved");
    assert!(end.x + end.width <= track.x + track.width + 1e-4);
    assert_eq!(start.height, track.height);
}

// ---------------------------------------------------------------------------
// 6 - thumb colour and the recomputed contrast matrix
// ---------------------------------------------------------------------------

#[test]
fn the_thumb_is_strong_at_rest_and_accented_while_dragged() {
    let p = palette();
    assert_eq!(thumb_color(TrackPhase::Hidden, &p), Color::TRANSPARENT);
    assert_eq!(thumb_color(TrackPhase::Rest, &p), p.border_strong);
    assert_eq!(thumb_color(TrackPhase::Hovered, &p), p.border_strong);
    assert_eq!(thumb_color(TrackPhase::Dragged, &p), p.accent_border);
}

#[test]
fn the_thumb_is_graded_as_judged_chrome_not_a_coincident_fill() {
    assert_eq!(THUMB_CHROME_CLASS, ChromeClass::Judged);
    assert_eq!(chrome_floor(), 3.0);
    assert_eq!(
        THUMB_CHROME_CLASS.minimum_contrast(),
        Some(3.0),
        "the thumb must carry the ordinary active-chrome floor"
    );
}

#[test]
fn border_strong_clears_every_surface_it_can_be_scrolled_over() {
    let p = palette();
    let surfaces: [(&str, Color); 7] = [
        ("surface.sunken", p.surface_sunken),
        ("surface.base", p.surface_base),
        ("surface.raised", p.surface_raised),
        ("surface.code", p.surface_code),
        ("surface.overlay", p.surface_overlay),
        ("surface.hover", p.surface_hover),
        ("surface.active", p.surface_active),
    ];
    for (name, surface) in surfaces {
        let ratio = contrast_ratio(p.border_strong, surface);
        assert!(
            thumb_clears_chrome_floor(p.border_strong, surface),
            "border.strong over {name} measured {ratio}, under the 3.00:1 Judged floor"
        );
    }
}

#[test]
fn the_recomputed_matrix_pins_the_measured_figures() {
    // Recomputed from raw sRGB with WCAG 2.1 relative luminance. The spec prints
    // only "4.01:1 - 4.51:1", which spans sunken..base and never prices the
    // hovered or pressed surfaces a scrollbar actually passes over mid-gesture.
    let p = palette();
    let cases: [(&str, Color, f32); 7] = [
        ("surface.sunken", p.surface_sunken, 4.5854),
        ("surface.base", p.surface_base, 4.5062),
        ("surface.raised", p.surface_raised, 4.2379),
        ("surface.code", p.surface_code, 4.1672),
        ("surface.overlay", p.surface_overlay, 4.0128),
        ("surface.hover", p.surface_hover, 3.6772),
        ("surface.active", p.surface_active, 3.2377),
    ];
    for (name, surface, expected) in cases {
        let actual = contrast_ratio(p.border_strong, surface);
        assert!(
            approx(actual, expected),
            "border.strong over {name} measured {actual:.4}, expected {expected:.4}"
        );
    }
}

#[test]
fn the_printed_range_understates_the_contract() {
    let p = palette();
    let printed_low = 4.01;
    let worst = contrast_ratio(p.border_strong, p.surface_active);
    assert!(
        worst < printed_low,
        "the spec's printed floor is below the real worst case, so the disagreement \
         is not where this test expects it"
    );
    assert!(
        worst >= chrome_floor(),
        "the worst case {worst} no longer clears the Judged floor"
    );
}

#[test]
fn the_dragged_accent_clears_every_surface_too() {
    let p = palette();
    let surfaces: [(&str, Color); 7] = [
        ("surface.sunken", p.surface_sunken),
        ("surface.base", p.surface_base),
        ("surface.raised", p.surface_raised),
        ("surface.code", p.surface_code),
        ("surface.overlay", p.surface_overlay),
        ("surface.hover", p.surface_hover),
        ("surface.active", p.surface_active),
    ];
    for (name, surface) in surfaces {
        let ratio = contrast_ratio(p.accent_border, surface);
        assert!(
            ratio >= chrome_floor(),
            "accent.border over {name} measured {ratio}, under the Judged floor"
        );
    }
}

// ---------------------------------------------------------------------------
// 7 - bottom and reach-end
// ---------------------------------------------------------------------------

#[test]
fn at_bottom_uses_the_two_pixel_tolerance() {
    assert_eq!(AT_BOTTOM_TOLERANCE, 2.0);
    assert_eq!(AT_BOTTOM_TOLERANCE, space::XXS);
    assert!(at_bottom_within_2px(700.0, 1000.0, 300.0));
    assert!(at_bottom_within_2px(698.0, 1000.0, 300.0));
    assert!(!at_bottom_within_2px(697.0, 1000.0, 300.0));
}

#[test]
fn an_unscrollable_view_is_always_at_bottom() {
    assert!(at_bottom_within_2px(0.0, 100.0, 300.0));
}

#[test]
fn reach_end_fires_in_the_final_64px() {
    assert_eq!(REACH_END_DISTANCE, 64.0);
    assert!(in_reach_end(636.0, 1000.0, 300.0));
    assert!(in_reach_end(700.0, 1000.0, 300.0));
    assert!(!in_reach_end(635.0, 1000.0, 300.0));
}

#[test]
fn reach_end_never_fires_on_an_unscrollable_view() {
    assert!(
        !in_reach_end(0.0, 100.0, 300.0),
        "a view with nothing to load must not fire a pagination trigger"
    );
}

// ---------------------------------------------------------------------------
// 8 - keyboard scrolling
// ---------------------------------------------------------------------------

#[test]
fn the_arrow_step_is_one_compact_tree_row() {
    // §3B.4 now prints the distance rather than leaving it open: the
    // arrow steps by `size.hit_target.min` (24px), which the section
    // glosses as "1 compact tree row". The earlier `type.body` reading
    // of 20px was an invented value and is gone.
    assert_eq!(ARROW_STEP, size::HIT_TARGET_MIN);
    assert_eq!(ARROW_STEP, 24.0);
    assert!(positive(ARROW_STEP));
}

#[test]
fn a_page_keeps_a_row_of_context() {
    // §3B.4 writes the page step as `(viewport_length - 24.0).max(24.0)`,
    // and the keep is the same hit-target minimum the arrow step uses.
    assert_eq!(PAGE_KEEP, size::HIT_TARGET_MIN);
    assert_eq!(page_step(400.0), 400.0 - PAGE_KEEP);
    assert_eq!(page_step(400.0), 376.0);
}

#[test]
fn a_tiny_viewport_still_pages_at_least_one_line() {
    assert_eq!(page_step(4.0), ARROW_STEP);
    assert_eq!(page_step(0.0), ARROW_STEP);
}

#[test]
fn down_and_up_move_one_line() {
    assert_eq!(
        scroll_to_key(100.0, ScrollKey::Down, 1000.0, 300.0),
        Some(100.0 + ARROW_STEP)
    );
    assert_eq!(
        scroll_to_key(100.0, ScrollKey::Up, 1000.0, 300.0),
        Some(100.0 - ARROW_STEP)
    );
}

#[test]
fn paging_moves_a_full_page() {
    let next = scroll_to_key(0.0, ScrollKey::PageDown, 1000.0, 300.0).expect("moved");
    assert_eq!(next, page_step(300.0));
    let back = scroll_to_key(next, ScrollKey::PageUp, 1000.0, 300.0).expect("moved back");
    assert_eq!(back, 0.0);
}

#[test]
fn home_and_end_jump_to_the_extremes() {
    assert_eq!(
        scroll_to_key(500.0, ScrollKey::Home, 1000.0, 300.0),
        Some(0.0)
    );
    assert_eq!(
        scroll_to_key(0.0, ScrollKey::End, 1000.0, 300.0),
        Some(700.0)
    );
}

#[test]
fn every_key_clamps_at_the_ends_instead_of_overscrolling() {
    let max = max_offset(1000.0, 300.0);
    for key in [ScrollKey::Down, ScrollKey::PageDown, ScrollKey::End] {
        assert_eq!(
            scroll_to_key(max, key, 1000.0, 300.0),
            None,
            "{key:?} overshot the end"
        );
    }
    for key in [ScrollKey::Up, ScrollKey::PageUp, ScrollKey::Home] {
        assert_eq!(
            scroll_to_key(0.0, key, 1000.0, 300.0),
            None,
            "{key:?} overshot the start"
        );
    }
}

#[test]
fn a_key_from_an_out_of_range_offset_lands_inside_the_range() {
    let next = scroll_to_key(5_000.0, ScrollKey::End, 1000.0, 300.0);
    assert!(
        next.is_none() || next.unwrap() <= max_offset(1000.0, 300.0),
        "an out-of-range offset escaped the content"
    );
    assert_eq!(clamp_offset(5_000.0, 1000.0, 300.0), 700.0);
    assert_eq!(clamp_offset(-5_000.0, 1000.0, 300.0), 0.0);
}

#[test]
fn an_unscrollable_view_moves_for_no_key() {
    for key in [
        ScrollKey::Up,
        ScrollKey::Down,
        ScrollKey::PageUp,
        ScrollKey::PageDown,
        ScrollKey::Home,
        ScrollKey::End,
    ] {
        assert_eq!(
            scroll_to_key(0.0, key, 100.0, 300.0),
            None,
            "{key:?} scrolled a view that cannot scroll"
        );
    }
}

// ---------------------------------------------------------------------------
// 9 - tick annotations
// ---------------------------------------------------------------------------

#[test]
fn a_tick_is_a_two_pixel_band_across_the_track() {
    let v = viewport(400.0, 300.0);
    let track = track_bounds(v, TrackPhase::Rest, LayoutMode::Overlay, Axis::Vertical);
    let tick = ScrollTick {
        position: 500.0,
        kind: TickKind::SearchHit,
    };
    let rect = tick_rect(track, &tick, 1000.0, Axis::Vertical);
    assert_eq!(rect.height, TICK_WIDTH);
    assert_eq!(rect.width, track.width);
    assert!(rect.y >= track.y);
    assert!(rect.y + rect.height <= track.y + track.height + 1e-4);
}

#[test]
fn ticks_are_ordered_by_their_content_position() {
    let v = viewport(400.0, 300.0);
    let track = track_bounds(v, TrackPhase::Rest, LayoutMode::Overlay, Axis::Vertical);
    let early = tick_rect(
        track,
        &ScrollTick {
            position: 100.0,
            kind: TickKind::Heading,
        },
        1000.0,
        Axis::Vertical,
    );
    let late = tick_rect(
        track,
        &ScrollTick {
            position: 900.0,
            kind: TickKind::Error,
        },
        1000.0,
        Axis::Vertical,
    );
    assert!(late.y > early.y);
}

#[test]
fn a_tick_at_the_very_end_stays_inside_the_track() {
    let v = viewport(400.0, 300.0);
    let track = track_bounds(v, TrackPhase::Rest, LayoutMode::Overlay, Axis::Vertical);
    let rect = tick_rect(
        track,
        &ScrollTick {
            position: 10_000.0,
            kind: TickKind::Error,
        },
        10_000.0,
        Axis::Vertical,
    );
    assert!(rect.y + rect.height <= track.y + track.height + 1e-4);
}

#[test]
fn a_zero_content_length_does_not_divide_a_tick() {
    let v = viewport(400.0, 300.0);
    let track = track_bounds(v, TrackPhase::Rest, LayoutMode::Overlay, Axis::Vertical);
    let rect = tick_rect(
        track,
        &ScrollTick {
            position: 0.0,
            kind: TickKind::SearchHit,
        },
        0.0,
        Axis::Vertical,
    );
    assert!(rect.y.is_finite());
    assert_eq!(rect.y, track.y);
}

#[test]
fn a_horizontal_tick_sits_on_the_bottom_track() {
    let v = viewport(600.0, 200.0);
    let track = track_bounds(v, TrackPhase::Rest, LayoutMode::Overlay, Axis::Horizontal);
    let rect = tick_rect(
        track,
        &ScrollTick {
            position: 300.0,
            kind: TickKind::Heading,
        },
        600.0,
        Axis::Horizontal,
    );
    assert_eq!(rect.width, TICK_WIDTH);
    assert_eq!(rect.height, track.height);
}

// ---------------------------------------------------------------------------
// 10 - the drag transcript
// ---------------------------------------------------------------------------

#[test]
fn a_drag_that_never_began_emits_nothing() {
    let mut drag = ThumbDrag::new(0.0);
    assert!(!drag.is_active());
    assert!(drag.drag(100.0, 1000.0, 300.0).is_none());
    assert!(drag.release().is_none());
    assert!(drag.cancel().is_none());
    assert_eq!(drag.offset(), 0.0);
}

#[test]
fn a_began_drag_emits_live_ticks() {
    let mut drag = ThumbDrag::new(0.0);
    let _ = drag.begin();
    assert!(drag.is_active());
    let live = drag.drag(120.0, 1000.0, 300.0).expect("live");
    assert_eq!(*live.value(), 120.0);
    assert_eq!(drag.offset(), 120.0);
}

#[test]
fn a_drag_settles_exactly_once() {
    let mut drag = ThumbDrag::new(0.0);
    let _ = drag.begin();
    let _ = drag.drag(300.0, 1000.0, 300.0);
    let commit = drag.release().expect("commit");
    assert_eq!(*commit.value(), 300.0);
    assert!(!drag.is_active());
    assert!(
        drag.release().is_none(),
        "a settled drag must not commit a second time"
    );
}

#[test]
fn cancelling_a_drag_restores_the_offset_the_user_was_looking_at() {
    let mut drag = ThumbDrag::new(250.0);
    let _ = drag.begin();
    let _ = drag.drag(600.0, 1000.0, 300.0);
    let outcome = drag.cancel().expect("cancel");
    assert_eq!(*outcome.restored(), 250.0);
    assert_eq!(drag.offset(), 250.0);
    assert!(!drag.is_active());
}

#[test]
fn a_drag_clamps_mid_flight_so_the_view_cannot_be_pushed_past_the_end() {
    let mut drag = ThumbDrag::new(0.0);
    let _ = drag.begin();
    let live = drag.drag(99_000.0, 1000.0, 300.0).expect("live");
    assert_eq!(*live.value(), 700.0);
    let commit = drag.release().expect("commit");
    assert_eq!(*commit.value(), 700.0);
}

#[test]
fn a_second_begin_is_refused_so_the_outer_capture_governs() {
    let mut drag = ThumbDrag::new(100.0);
    let _ = drag.begin();
    let _ = drag.drag(400.0, 1000.0, 300.0);
    let second = drag.begin();
    assert_eq!(second, GestureStart::AlreadyOpen);
    let outcome = drag.cancel().expect("cancel");
    assert_eq!(
        *outcome.restored(),
        100.0,
        "the inner begin must not have shortened the outer revert"
    );
}

#[test]
fn a_drag_cancels_clean_after_a_release() {
    let mut drag = ThumbDrag::new(0.0);
    let _ = drag.begin();
    let _ = drag.drag(200.0, 1000.0, 300.0);
    let _ = drag.release();
    assert!(
        drag.cancel().is_none(),
        "cancelling an already-settled drag must not emit a second outcome"
    );
    assert_eq!(drag.offset(), 200.0);
}
