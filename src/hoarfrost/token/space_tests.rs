//! Tests for [`space`] — the base-4 spatial grid (`UIF-TOK-001`).
//!
//! Two layers of assertion. First every rung is pinned to the literal value
//! in the spec table, so a silent retune fails. Then the structural
//! invariants the grid implies are checked over the whole rung table, so a
//! future insertion, reorder or retune that breaks the scale fails loudly
//! rather than quietly shifting every layout that reads it.

// Every assertion here targets compile-time token values on purpose: pinning a
// constant is the whole point, so a retuned rung must fail the suite rather
// than pass silently. `clippy::assertions_on_constants` cannot tell a spec pin
// from a tautology.
#![allow(clippy::assertions_on_constants)]

use super::*;

/// Tolerance for grid arithmetic on `f32` logical pixels.
const EPS: f32 = 1e-6;

/// Every rung carries the exact logical-pixel value the `UIF-TOK-001`
/// table specifies.
#[test]
fn test_space_values_match_spec_table() {
    assert_eq!(XXS, 2.0, "space.2xs must be 2px");
    assert_eq!(XS, 4.0, "space.xs must be 4px");
    assert_eq!(SM, 8.0, "space.sm must be 8px");
    assert_eq!(MD, 12.0, "space.md must be 12px");
    assert_eq!(LG, 16.0, "space.lg must be 16px");
    assert_eq!(XL, 24.0, "space.xl must be 24px");
    assert_eq!(XXL, 32.0, "space.2xl must be 32px");
}

/// The full rung table, in ascending order, as `(spec token, value)`.
///
/// Shared by the structural tests so each invariant is checked over every
/// rung instead of a hand-picked subset.
fn rung_table() -> [(&'static str, f32); 7] {
    [
        ("space.2xs", XXS),
        ("space.xs", XS),
        ("space.sm", SM),
        ("space.md", MD),
        ("space.lg", LG),
        ("space.xl", XL),
        ("space.2xl", XXL),
    ]
}

/// The scale is strictly increasing: no two rungs collapse onto the same
/// value, so a rung name always means strictly more space than the rung
/// below it.
#[test]
fn test_space_scale_is_strictly_increasing() {
    let rungs = rung_table();
    for pair in rungs.windows(2) {
        let (lower_name, lower) = pair[0];
        let (upper_name, upper) = pair[1];
        assert!(
            lower < upper,
            "{upper_name} ({upper}) must be strictly greater than {lower_name} ({lower})"
        );
    }
}

/// Every rung except the `XXS` half-step sits on the base-4 grid.
#[test]
fn test_space_grid_rungs_are_multiples_of_four() {
    for (name, value) in rung_table() {
        if name == "space.2xs" {
            continue;
        }
        assert!(
            (value % 4.0).abs() < EPS,
            "{name} ({value}) is off the base-4 spatial grid"
        );
    }
}

/// `XXS` is the *only* rung that is not a multiple of 4 — it is the
/// sanctioned 2px half-step, not an accident on the grid.
#[test]
fn test_space_xxs_is_the_only_half_step() {
    assert!(
        (XXS % 4.0).abs() >= EPS,
        "space.2xs ({XXS}) should be the sole off-grid half-step, not a grid rung"
    );
}

/// The half-step is exactly half of the smallest grid rung, which is what
/// makes it a half-step rather than an arbitrary smaller number.
#[test]
fn test_space_xxs_is_exactly_half_of_xs() {
    assert!(
        (XXS * 2.0 - XS).abs() < EPS,
        "space.2xs ({XXS}) must be exactly half of space.xs ({XS})"
    );
    assert!(
        (XS / 2.0 - XXS).abs() < EPS,
        "space.xs halved ({}) must land exactly on space.2xs ({XXS})",
        XS / 2.0
    );
}

/// No rung is zero or negative: a spacing token of zero would silently
/// collapse a layout rather than declare itself.
#[test]
fn test_space_rungs_are_positive() {
    for (name, value) in rung_table() {
        assert!(value > 0.0, "{name} ({value}) must be strictly positive");
    }
}

/// The scale spans a usable range for chrome layout: the smallest rung is
/// small enough for micro-gaps and the largest is wide enough to act as a
/// viewport margin.
#[test]
fn test_space_scale_spans_chrome_layout_range() {
    assert!(
        XXS <= 2.0,
        "space.2xs ({XXS}) must stay small enough for dense inline micro-gaps"
    );
    assert!(
        XXL >= 32.0,
        "space.2xl ({XXL}) must stay wide enough to serve as a minimum viewport margin"
    );
}
