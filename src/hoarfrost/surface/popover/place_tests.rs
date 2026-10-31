//! Tests for the teaching popover's placement — `UIF-ATOM-007`, §3.7.
//!
//! Three groups:
//!
//! 1. **The side → alignment mapping.** That each `PreferredSide`
//!    translates to the solver's axes it is supposed to, and that the
//!    mirror pairs are what the enum claims.
//! 2. **Placement through the real solver.** That the resolved origin is
//!    the solver's own arithmetic — the trigger gap, the flip, the
//!    clamp — and not a second placement model wearing the solver's
//!    name.
//! 3. **The gap.** That the popover keeps the solver's trigger gap and
//!    that the gap is wide enough for a pointer to cross.

use super::*;
use crate::geometry::anchor::{ControlAlignment, HorizontalAlign, VerticalAlign};
use crate::token::space;
use iced::{Point, Rectangle, Size};

/// A roomy viewport with an 8px inset margin.
fn viewport() -> Rectangle {
    Rectangle::new(Point::ORIGIN, Size::new(1200.0, 800.0))
}

/// A trigger well inside the viewport.
fn trigger() -> Rectangle {
    Rectangle::new(Point::new(100.0, 100.0), Size::new(120.0, 28.0))
}

/// A card inside §3.7's band.
fn card() -> Size {
    Size::new(300.0, 140.0)
}

// ---------------------------------------------------------------------------
// 1. The side → alignment mapping
// ---------------------------------------------------------------------------

#[test]
fn below_is_the_fixed_menu_default_alignment() {
    assert_eq!(
        PreferredSide::Below.alignment(),
        ControlAlignment::new(VerticalAlign::Below, HorizontalAlign::Start),
    );
}

#[test]
fn above_keeps_the_start_alignment() {
    assert_eq!(
        PreferredSide::Above.alignment(),
        ControlAlignment::new(VerticalAlign::Above, HorizontalAlign::Start),
    );
}

#[test]
fn the_two_sides_reuse_the_submenu_axes() {
    assert_eq!(
        PreferredSide::Trailing.alignment(),
        ControlAlignment::new(VerticalAlign::Top, HorizontalAlign::Trailing),
    );
    assert_eq!(
        PreferredSide::Leading.alignment(),
        ControlAlignment::new(VerticalAlign::Top, HorizontalAlign::Leading),
    );
}

#[test]
fn every_mirror_pair_mirrors_on_its_own_axis() {
    assert_eq!(PreferredSide::Below.mirror(), PreferredSide::Above);
    assert_eq!(PreferredSide::Above.mirror(), PreferredSide::Below);
    assert_eq!(PreferredSide::Leading.mirror(), PreferredSide::Trailing);
    assert_eq!(PreferredSide::Trailing.mirror(), PreferredSide::Leading);

    for side in [
        PreferredSide::Below,
        PreferredSide::Above,
        PreferredSide::Leading,
        PreferredSide::Trailing,
    ] {
        assert_eq!(
            side.mirror().mirror(),
            side,
            "mirroring twice must be the identity",
        );
    }
}

#[test]
fn only_the_two_side_placements_are_horizontal() {
    assert!(!PreferredSide::Below.is_horizontal());
    assert!(!PreferredSide::Above.is_horizontal());
    assert!(PreferredSide::Leading.is_horizontal());
    assert!(PreferredSide::Trailing.is_horizontal());
}

#[test]
fn below_is_the_default_side() {
    assert_eq!(PreferredSide::default(), PreferredSide::Below);
}

// ---------------------------------------------------------------------------
// 2. Placement through the real solver
// ---------------------------------------------------------------------------

#[test]
fn below_lands_one_trigger_gap_under_the_trigger() {
    let placement = resolve(trigger(), PreferredSide::Below, card(), viewport());
    assert_eq!(placement.origin.x, 100.0);
    assert_eq!(placement.origin.y, 100.0 + 28.0 + POPOVER_GAP);
    assert!(!placement.was_flipped());
    assert!(!placement.was_clamped());
}

#[test]
fn above_lands_one_trigger_gap_over_the_trigger_when_there_is_room() {
    let low_trigger = Rectangle::new(Point::new(100.0, 400.0), Size::new(120.0, 28.0));
    let placement = resolve(low_trigger, PreferredSide::Above, card(), viewport());
    assert_eq!(placement.origin.y, 400.0 - 140.0 - POPOVER_GAP);
    assert!(!placement.flipped_vertical);
}

#[test]
fn above_flips_below_when_the_top_edge_is_blocked() {
    let placement = resolve(trigger(), PreferredSide::Above, card(), viewport());
    assert!(
        placement.flipped_vertical,
        "a card that cannot fit above must flip, not clip",
    );
    assert_eq!(placement.origin.y, 100.0 + 28.0 + POPOVER_GAP);
}

#[test]
fn trailing_hangs_beside_the_trigger() {
    let placement = resolve(trigger(), PreferredSide::Trailing, card(), viewport());
    assert_eq!(placement.origin.x, 100.0 + 120.0 + space::XXS);
    assert_eq!(placement.origin.y, 100.0);
    assert!(!placement.was_flipped());
}

#[test]
fn leading_flips_trailing_when_the_left_edge_is_blocked() {
    let placement = resolve(trigger(), PreferredSide::Leading, card(), viewport());
    assert!(
        placement.flipped_horizontal,
        "a card that cannot fit to the left must flip to the right",
    );
    assert_eq!(placement.origin.x, 100.0 + 120.0 + space::XXS);
}

#[test]
fn leading_lands_left_of_the_trigger_when_there_is_room() {
    let right_trigger = Rectangle::new(Point::new(800.0, 400.0), Size::new(120.0, 28.0));
    let placement = resolve(right_trigger, PreferredSide::Leading, card(), viewport());
    assert_eq!(placement.origin.x, 800.0 - 300.0 - space::XXS);
    assert!(!placement.flipped_horizontal);
}

#[test]
fn the_card_rect_is_the_placement_rect() {
    let placement = resolve(trigger(), PreferredSide::Below, card(), viewport());
    let rect = placement.rect();
    assert_eq!(rect, Rectangle::new(placement.origin, placement.size));
}

#[test]
fn a_degenerate_trigger_still_resolves() {
    let degenerate = Rectangle::new(Point::new(10.0, 10.0), Size::new(0.0, 0.0));
    let placement = resolve(degenerate, PreferredSide::Below, card(), viewport());
    assert!(placement.origin.x.is_finite());
    assert!(placement.origin.y.is_finite());
}

#[test]
fn a_card_bigger_than_the_viewport_reports_itself_scrollable_or_clamped() {
    let huge = Size::new(4000.0, 2000.0);
    let placement = resolve(trigger(), PreferredSide::Below, huge, viewport());
    assert!(
        placement.was_clamped(),
        "a card larger than the inset must report the constraint rather \
         than pretend it fits",
    );
}

#[test]
fn placement_is_deterministic() {
    for side in [
        PreferredSide::Below,
        PreferredSide::Above,
        PreferredSide::Leading,
        PreferredSide::Trailing,
    ] {
        let a = resolve(trigger(), side, card(), viewport());
        let b = resolve(trigger(), side, card(), viewport());
        assert_eq!(a, b, "{side:?} must be a pure function");
    }
}

// ---------------------------------------------------------------------------
// 3. The gap
// ---------------------------------------------------------------------------

#[test]
fn the_gap_is_the_solvers_trigger_gap_not_a_second_number() {
    assert_eq!(POPOVER_GAP, space::XS);
    assert_eq!(POPOVER_GAP, 4.0);
}

#[test]
fn the_gap_is_wide_enough_to_see_and_narrow_enough_to_cross() {
    // The pointer has to travel from the trigger to the footer without
    // the card vanishing on the way. `space.xs` is the smallest rung
    // that is visibly a gap and small enough that crossing it is not a
    // precision task.
    assert_eq!(
        POPOVER_GAP.partial_cmp(&space::XXS),
        Some(std::cmp::Ordering::Greater),
        "the gap must be at least one visible rung"
    );
    assert_eq!(
        POPOVER_GAP.partial_cmp(&space::SM),
        Some(std::cmp::Ordering::Less),
        "crossing the gap must not be a precision task"
    );
}

#[test]
fn the_gap_is_the_same_on_the_flipped_side() {
    let below = resolve(trigger(), PreferredSide::Below, card(), viewport());
    let above_flipped = resolve(trigger(), PreferredSide::Above, card(), viewport());
    assert!(above_flipped.flipped_vertical);
    assert_eq!(below.origin.y, above_flipped.origin.y);
}
