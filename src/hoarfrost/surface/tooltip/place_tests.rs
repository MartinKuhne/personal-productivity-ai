//! Tests for tooltip placement.
//!
//! Every expectation is derived from the tokens the solver is built on — `space::XS` for
//! the trigger gap, `space::SM` for the viewport margin — rather than from transcribed
//! pixel numbers, so a token change moves the expectations with it instead of turning
//! these tests red for no reason.

use super::*;
use crate::token::{size, space};
use iced::{Point, Rectangle, Size};

const EPS: f32 = 1e-4;

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rectangle {
    Rectangle::new(Point::new(x, y), Size::new(w, h))
}

fn viewport(w: f32, h: f32) -> Rectangle {
    rect(0.0, 0.0, w, h)
}

/// The canonical navbar: a 36px bar with a 28px entry inset inside it.
fn navbar() -> Rectangle {
    rect(0.0, 0.0, 1280.0, size::CONTROL_LG)
}

fn entry(x: f32, w: f32) -> Rectangle {
    rect(x, 4.0, w, size::CONTROL_SM)
}

// ---------------------------------------------------------------------------
// The anchor rect itself
// ---------------------------------------------------------------------------

/// The `Band` anchor is the whole design of this module, so its composition is asserted
/// directly before any solver behaviour is: vertical from the bar, horizontal from the
/// entry.
#[test]
fn band_anchor_mixes_band_y_with_control_x() {
    let bar = navbar();
    let ctl = entry(100.0, 180.0);
    let a = TooltipAnchor::band(bar, ctl);
    let r = a.rect();
    assert_eq!(r.x, ctl.x, "x comes from the entry");
    assert_eq!(r.width, ctl.width, "width comes from the entry");
    assert_eq!(r.y, bar.y, "y comes from the bar");
    assert_eq!(r.height, bar.height, "height comes from the bar");
}

#[test]
fn control_anchor_rect_is_the_control() {
    let ctl = rect(10.0, 20.0, 30.0, 40.0);
    assert_eq!(TooltipAnchor::Control(ctl).rect(), ctl);
}

#[test]
fn pointer_anchor_rect_is_a_zero_rect_at_the_cursor() {
    let r = TooltipAnchor::Pointer(Point::new(7.0, 9.0)).rect();
    assert_eq!((r.x, r.y), (7.0, 9.0));
    assert_eq!((r.width, r.height), (0.0, 0.0));
}

#[test]
fn band_and_control_map_to_control_anchors_with_the_tooltip_alignment() {
    for a in [
        TooltipAnchor::band(navbar(), entry(100.0, 180.0)),
        TooltipAnchor::Control(entry(100.0, 180.0)),
    ] {
        match a.to_anchor() {
            Anchor::Control { alignment, .. } => assert_eq!(alignment, TOOLTIP_ALIGNMENT),
            other => panic!("expected a Control anchor, got {other:?}"),
        }
    }
    assert!(matches!(
        TooltipAnchor::Pointer(Point::new(1.0, 2.0)).to_anchor(),
        Anchor::Pointer { .. }
    ));
}

// ---------------------------------------------------------------------------
// UIF-NAV-003: below the navbar
// ---------------------------------------------------------------------------

/// The requirement's core case: the tooltip hangs **below the bar**, not below the
/// button. Anchoring to the button would put it at `32 + 4 = 36`, i.e. exactly on the
/// bar's bottom edge and reading as part of the bar.
#[test]
fn navbar_tooltip_sits_below_the_bar_not_below_the_button() {
    let bar = navbar();
    let ctl = entry(100.0, 180.0);
    let p = TooltipAnchor::band(bar, ctl).resolve(Size::new(240.0, 24.0), viewport(1280.0, 800.0));

    let bar_bottom = bar.y + bar.height;
    assert!(
        p.origin.y > bar_bottom,
        "tooltip top {} must clear the bar bottom {bar_bottom}",
        p.origin.y
    );
    assert!(
        (p.origin.y - (bar_bottom + TOOLTIP_GAP)).abs() <= EPS,
        "gap below the bar must be TOOLTIP_GAP, got {}",
        p.origin.y - bar_bottom
    );
    assert_eq!(
        p.origin.y,
        bar_bottom + space::XS,
        "and that gap is space::XS"
    );
    assert!(!p.flipped_vertical, "there is room below, so no flip");
    assert!(!p.was_clamped());
}

#[test]
fn navbar_tooltip_left_edge_lines_up_with_the_entry() {
    let ctl = entry(100.0, 180.0);
    let p =
        TooltipAnchor::band(navbar(), ctl).resolve(Size::new(240.0, 24.0), viewport(1280.0, 800.0));
    assert_eq!(p.origin.x, ctl.x);
}

/// The gap must not be an artefact of the preferred side: when the solver flips above, the
/// mirrored rule keeps the same `space::XS` separation.
#[test]
fn the_gap_survives_a_vertical_flip() {
    // A bottom tab strip: no room below, so the tooltip must go above.
    let bar = rect(0.0, 264.0, 1280.0, size::CONTROL_LG);
    let ctl = rect(100.0, 268.0, 180.0, size::CONTROL_SM);
    let tip = Size::new(240.0, 100.0);

    let p = TooltipAnchor::band(bar, ctl).resolve(tip, viewport(1280.0, 300.0));

    assert!(
        p.flipped_vertical,
        "no room below a bottom strip, so it flips"
    );
    let tip_bottom = p.origin.y + tip.height;
    let gap = bar.y - tip_bottom;
    assert!(
        (gap - TOOLTIP_GAP).abs() <= EPS,
        "flipped above, the tooltip's bottom edge must still sit TOOLTIP_GAP ({TOOLTIP_GAP}) \
         clear of the bar's top edge; got gap {gap} (tip bottom {tip_bottom}, bar top {})",
        bar.y
    );
    assert_eq!(
        gap,
        space::XS,
        "the flipped gap is the same token as the resting gap"
    );
    assert!(
        !p.was_clamped(),
        "the flip landed cleanly inside the margins"
    );
}

/// A plain control anchor keeps the same relationship to its own bottom edge, which is the
/// dialog-button and menu-row case.
#[test]
fn control_anchor_hangs_below_its_own_bottom_edge() {
    let ctl = rect(200.0, 100.0, 120.0, 32.0);
    let p = TooltipAnchor::Control(ctl).resolve(Size::new(160.0, 24.0), viewport(1280.0, 800.0));
    assert_eq!(p.origin.y, ctl.y + ctl.height + space::XS);
    assert_eq!(p.origin.x, ctl.x);
    assert!(!p.was_flipped());
}

// ---------------------------------------------------------------------------
// Horizontal behaviour near the right edge
// ---------------------------------------------------------------------------

/// Near the right edge the `Start` preference mirrors to `End`: the tooltip's right edge
/// lines up with the anchor's right edge rather than hanging off-screen.
#[test]
fn near_the_right_edge_the_tooltip_mirrors_to_right_edge_alignment() {
    let bar = navbar();
    let ctl = entry(1100.0, 150.0);
    let tip = Size::new(300.0, 24.0);
    let view = viewport(1280.0, 800.0);

    let p = TooltipAnchor::band(bar, ctl).resolve(tip, view);

    assert!(
        p.flipped_horizontal,
        "cannot hang 300px from x=1100 in a 1280 view"
    );
    assert!(
        (p.origin.x + tip.width - (ctl.x + ctl.width)).abs() <= EPS,
        "right edges should be level: tip right {}, entry right {}",
        p.origin.x + tip.width,
        ctl.x + ctl.width
    );
    assert!(
        p.origin.x + tip.width <= 1280.0 - space::SM + EPS,
        "stays inside the margin"
    );
    assert!(!p.was_clamped(), "mirroring solved it; no clamp needed");
}

/// A tooltip wider than the viewport inset cannot be mirrored into place either, so the
/// clamp pins its leading edge to the near margin and reports the constraint.
#[test]
fn a_tooltip_wider_than_the_viewport_is_clamped_not_lost() {
    let bar = rect(0.0, 0.0, 400.0, size::CONTROL_LG);
    let ctl = rect(300.0, 4.0, 80.0, size::CONTROL_SM);
    let tip = Size::new(500.0, 24.0);

    let p = TooltipAnchor::band(bar, ctl).resolve(tip, viewport(400.0, 800.0));

    assert!(p.clamped_horizontal, "500px cannot fit a 384px inset");
    assert!(
        (p.origin.x - space::SM).abs() <= EPS,
        "leading edge pinned to the near margin, got {}",
        p.origin.x
    );
    assert!(p.origin.x >= 0.0, "never pushed off-screen");
}

// ---------------------------------------------------------------------------
// Vertical behaviour at the top of the window
// ---------------------------------------------------------------------------

/// A tooltip anchored at the very top of the viewport still lands below the bar rather
/// than being pushed up out of sight.
#[test]
fn a_top_of_window_anchor_stays_inside_the_viewport() {
    let bar = rect(0.0, 0.0, 800.0, size::CONTROL_LG);
    let ctl = rect(8.0, 4.0, 120.0, size::CONTROL_SM);
    let p = TooltipAnchor::band(bar, ctl).resolve(Size::new(200.0, 24.0), viewport(800.0, 600.0));
    assert!(p.origin.y >= space::SM);
    assert!(p.origin.y + 24.0 <= 600.0 - space::SM);
}

// ---------------------------------------------------------------------------
// Pointer anchor
// ---------------------------------------------------------------------------

#[test]
fn pointer_anchor_hangs_below_right_of_the_cursor() {
    let p = TooltipAnchor::Pointer(Point::new(500.0, 400.0))
        .resolve(Size::new(200.0, 24.0), viewport(1280.0, 800.0));
    assert_eq!(p.origin.x, 500.0 + space::XXS);
    assert_eq!(p.origin.y, 400.0 + space::XXS);
    assert!(!p.was_flipped());
}

#[test]
fn pointer_anchor_near_the_corner_flips_onto_the_screen() {
    let p = TooltipAnchor::Pointer(Point::new(1270.0, 790.0))
        .resolve(Size::new(200.0, 24.0), viewport(1280.0, 800.0));
    assert!(p.flipped_horizontal);
    assert!(p.flipped_vertical);
    assert!(p.origin.x + 200.0 <= 1280.0 - space::SM + EPS);
    assert!(p.origin.y + 24.0 <= 800.0 - space::SM + EPS);
}

// ---------------------------------------------------------------------------
// Degenerate inputs are the solver's, and defined
// ---------------------------------------------------------------------------

#[test]
fn non_finite_input_never_produces_nan() {
    let bad = rect(f32::NAN, f32::NAN, f32::NAN, f32::NAN);
    let p = TooltipAnchor::band(bad, bad)
        .resolve(Size::new(f32::NAN, f32::NAN), viewport(800.0, 600.0));
    assert!(
        p.origin.x.is_finite(),
        "x must be finite, got {}",
        p.origin.x
    );
    assert!(
        p.origin.y.is_finite(),
        "y must be finite, got {}",
        p.origin.y
    );
    assert!(p.size.width.is_finite() && p.size.height.is_finite());
}

#[test]
fn a_negative_extents_anchor_still_resolves() {
    let bar = rect(0.0, 0.0, 800.0, -50.0);
    let ctl = rect(50.0, 4.0, -20.0, 28.0);
    let p = TooltipAnchor::band(bar, ctl).resolve(Size::new(100.0, 24.0), viewport(800.0, 600.0));
    assert!(p.origin.x.is_finite() && p.origin.y.is_finite());
    assert!(
        p.origin.y >= space::SM,
        "clamped back inside the near margin"
    );
}

#[test]
fn a_zero_size_card_still_resolves_to_the_preferred_origin() {
    let bar = navbar();
    let ctl = entry(100.0, 180.0);
    let p = TooltipAnchor::band(bar, ctl).resolve(Size::ZERO, viewport(1280.0, 800.0));
    assert_eq!(p.origin.y, bar.y + bar.height + space::XS);
    assert!(!p.was_flipped());
}

/// The solver is a pure function: resolving the same anchor twice must be identical, which
/// is what lets a host re-resolve every frame without the tooltip jittering.
#[test]
fn resolving_is_deterministic_across_repeated_calls() {
    let a = TooltipAnchor::band(navbar(), entry(100.0, 180.0));
    let s = Size::new(240.0, 24.0);
    let v = viewport(1280.0, 800.0);
    assert_eq!(a.resolve(s, v), a.resolve(s, v));
}

/// The resolved rect is the origin and the size the solver was given, so a host can paint
/// `placement.rect()` without re-deriving anything.
#[test]
fn placement_rect_matches_origin_and_size() {
    let p = TooltipAnchor::band(navbar(), entry(100.0, 180.0))
        .resolve(Size::new(240.0, 24.0), viewport(1280.0, 800.0));
    let r = p.rect();
    assert_eq!(r.x, p.origin.x);
    assert_eq!(r.y, p.origin.y);
    assert_eq!(r.width, 240.0);
    assert_eq!(r.height, 24.0);
}

/// The gap constant this module publishes must be the same value the solver used, or the
/// doc claim that it is "not a second source of truth" would be false.
#[test]
fn published_gap_is_the_solvers_trigger_gap() {
    assert_eq!(TOOLTIP_GAP, space::XS);
    let p = TooltipAnchor::band(navbar(), entry(100.0, 180.0))
        .resolve(Size::new(240.0, 24.0), viewport(1280.0, 800.0));
    assert_eq!(p.origin.y - (size::CONTROL_LG), TOOLTIP_GAP);
}
