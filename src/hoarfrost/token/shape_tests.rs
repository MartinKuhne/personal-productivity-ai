//! Tests for [`shape`] — corner radii and stroke weights (`UIF-TOK-003`).
//!
//! Pins every value to the spec table, then enforces the nesting rule for
//! radii (a contained surface is never rounder than its container) and the
//! two-weight rule for strokes (the attention stroke is strictly heavier
//! than the resting stroke).

// Every assertion here targets compile-time token values on purpose: pinning a
// constant is the whole point, so a retuned rung must fail the suite rather
// than pass silently. `clippy::assertions_on_constants` cannot tell a spec pin
// from a tautology.
#![allow(clippy::assertions_on_constants)]

use super::*;

/// Every token carries the exact logical-pixel value the `UIF-TOK-003`
/// table specifies.
#[test]
fn test_shape_values_match_spec_table() {
    assert_eq!(RADIUS_WINDOW, 8.0, "radius.window must be 8px");
    assert_eq!(RADIUS_CARD, 6.0, "radius.card must be 6px");
    assert_eq!(RADIUS_WIDGET, 4.0, "radius.widget must be 4px");
    assert_eq!(RADIUS_PILL, 999.0, "radius.pill must be 999px");
    assert_eq!(BORDER_HAIRLINE, 1.0, "border.hairline must be 1px");
    assert_eq!(BORDER_FOCUS, 2.0, "border.focus must be 2px");
}

/// The radius rungs in ascending containment order, as `(spec token, value)`.
fn radius_table() -> [(&'static str, f32); 4] {
    [
        ("radius.widget", RADIUS_WIDGET),
        ("radius.card", RADIUS_CARD),
        ("radius.window", RADIUS_WINDOW),
        ("radius.pill", RADIUS_PILL),
    ]
}

/// Radii nest: a widget is rounder-cornered than nothing but sits inside a
/// card, which sits inside the window. A contained surface must never be
/// rounder than the surface containing it.
#[test]
fn test_radius_nests_widget_under_card_under_window() {
    assert!(
        RADIUS_WIDGET < RADIUS_CARD,
        "radius.widget ({RADIUS_WIDGET}) must be smaller than radius.card ({RADIUS_CARD})"
    );
    assert!(
        RADIUS_CARD < RADIUS_WINDOW,
        "radius.card ({RADIUS_CARD}) must be smaller than radius.window ({RADIUS_WINDOW})"
    );
}

/// `radius.pill` dominates every other radius, so a pill is unambiguously
/// the roundest thing on screen and never reads as an ordinary rounded
/// corner at any rendered size.
#[test]
fn test_pill_radius_dominates_every_other_radius() {
    for (name, radius) in radius_table() {
        if name == "radius.pill" {
            continue;
        }
        assert!(
            RADIUS_PILL > radius,
            "radius.pill ({RADIUS_PILL}) must exceed {name} ({radius})"
        );
    }
}

/// The radius ladder is strictly ascending across the whole table, so no
/// future insertion can tie two containment levels together.
#[test]
fn test_radius_ladder_is_strictly_ascending() {
    let radii = radius_table();
    for pair in radii.windows(2) {
        let (lower_name, lower) = pair[0];
        let (upper_name, upper) = pair[1];
        assert!(
            lower < upper,
            "{upper_name} ({upper}) must be strictly greater than {lower_name} ({lower})"
        );
    }
}

/// The focus stroke is strictly heavier than the hairline, so a focused or
/// invalid element cannot be mistaken for one that merely has a border.
#[test]
fn test_focus_stroke_is_heavier_than_hairline() {
    assert!(
        BORDER_HAIRLINE < BORDER_FOCUS,
        "border.hairline ({BORDER_HAIRLINE}) must be thinner than border.focus ({BORDER_FOCUS})"
    );
}

/// The focus stroke is exactly twice the hairline — the attention weight
/// is a doubling, not an arbitrary step.
#[test]
fn test_focus_stroke_is_exactly_double_the_hairline() {
    assert_eq!(
        BORDER_HAIRLINE * 2.0,
        BORDER_FOCUS,
        "border.focus must be exactly twice border.hairline"
    );
}

/// Every shape token is strictly positive: a zero radius or a zero-width
/// stroke would render as nothing and hide the intent behind it.
#[test]
fn test_all_shape_tokens_are_positive() {
    for (name, radius) in radius_table() {
        assert!(radius > 0.0, "{name} ({radius}) must be strictly positive");
    }
    assert!(
        BORDER_HAIRLINE > 0.0,
        "border.hairline ({BORDER_HAIRLINE}) must be strictly positive"
    );
    assert!(
        BORDER_FOCUS > 0.0,
        "border.focus ({BORDER_FOCUS}) must be strictly positive"
    );
}
