//! Tests for [`size`] — control heights, hit targets and icon sizes
//! (`UIF-TOK-002`).
//!
//! Pins every value to the spec table, then enforces the two rules that
//! make the set safe: no interactive control may fall under the 24×24 hit
//! target floor, and no icon may be drawn larger than the smallest control
//! it can be placed inside.

// Every assertion here targets compile-time token values on purpose: pinning a
// constant is the whole point, so a retuned rung must fail the suite rather
// than pass silently. `clippy::assertions_on_constants` cannot tell a spec pin
// from a tautology.
#![allow(clippy::assertions_on_constants)]

use super::*;

/// Every token carries the exact logical-pixel value the `UIF-TOK-002`
/// table specifies.
#[test]
fn test_size_values_match_spec_table() {
    assert_eq!(HIT_TARGET_MIN, 24.0, "size.hit_target.min must be 24px");
    assert_eq!(CONTROL_SM, 28.0, "size.control.sm must be 28px");
    assert_eq!(CONTROL_MD, 32.0, "size.control.md must be 32px");
    assert_eq!(CONTROL_LG, 36.0, "size.control.lg must be 36px");
    assert_eq!(ICON_SM, 14.0, "size.icon.sm must be 14px");
    assert_eq!(ICON_MD, 16.0, "size.icon.md must be 16px");
    assert_eq!(ICON_LG, 20.0, "size.icon.lg must be 20px");
}

/// The control rung table, in ascending order.
fn control_table() -> [(&'static str, f32); 3] {
    [
        ("size.control.sm", CONTROL_SM),
        ("size.control.md", CONTROL_MD),
        ("size.control.lg", CONTROL_LG),
    ]
}

/// The icon rung table, in ascending order.
fn icon_table() -> [(&'static str, f32); 3] {
    [
        ("size.icon.sm", ICON_SM),
        ("size.icon.md", ICON_MD),
        ("size.icon.lg", ICON_LG),
    ]
}

/// The hit-target floor is strictly positive; a zero or negative floor
/// would make the rule unenforceable.
#[test]
fn test_hit_target_min_is_positive() {
    assert!(
        HIT_TARGET_MIN > 0.0,
        "size.hit_target.min ({HIT_TARGET_MIN}) must be strictly positive"
    );
}

/// Every control height clears the mandatory hit-target floor. This is the
/// `24 × 24` rule: it applies to both dimensions of an interactive
/// control, so the smallest rung still has to clear it.
#[test]
fn test_every_control_clears_hit_target_floor() {
    for (name, height) in control_table() {
        assert!(
            height >= HIT_TARGET_MIN,
            "{name} ({height}) is below the {HIT_TARGET_MIN}px hit-target floor"
        );
    }
}

/// The control rungs are strictly ordered, so "sm/md/lg" can never mean
/// the same height for two different control classes.
#[test]
fn test_control_rungs_are_strictly_ordered() {
    assert!(
        CONTROL_SM < CONTROL_MD,
        "size.control.sm ({CONTROL_SM}) must be smaller than size.control.md ({CONTROL_MD})"
    );
    assert!(
        CONTROL_MD < CONTROL_LG,
        "size.control.md ({CONTROL_MD}) must be smaller than size.control.lg ({CONTROL_LG})"
    );
}

/// Every icon fits inside the smallest control rung. An icon larger than
/// `size.control.sm` could not be placed in a compact navbar button
/// without overflowing it.
#[test]
fn test_every_icon_fits_in_the_smallest_control() {
    for (name, icon) in icon_table() {
        assert!(
            icon <= CONTROL_SM,
            "{name} ({icon}) must not exceed the smallest control height size.control.sm ({CONTROL_SM})"
        );
    }
}

/// Icons are strictly ordered, and each leaves visible padding inside the
/// rung it normally sits in — an icon that exactly filled its control
/// would read as clipped.
#[test]
fn test_icon_rungs_are_strictly_ordered_and_inset() {
    assert!(
        ICON_SM < ICON_MD,
        "size.icon.sm ({ICON_SM}) must be smaller than size.icon.md ({ICON_MD})"
    );
    assert!(
        ICON_MD < ICON_LG,
        "size.icon.md ({ICON_MD}) must be smaller than size.icon.lg ({ICON_LG})"
    );
    for (name, icon) in icon_table() {
        assert!(
            icon < CONTROL_SM,
            "{name} ({icon}) must leave inset padding inside size.control.sm ({CONTROL_SM})"
        );
    }
}

/// Every sizing token is strictly positive; a zero-height control or a
/// zero-size glyph would collapse silently.
#[test]
fn test_all_sizes_are_positive() {
    let tokens: [(&'static str, f32); 7] = [
        ("size.hit_target.min", HIT_TARGET_MIN),
        ("size.control.sm", CONTROL_SM),
        ("size.control.md", CONTROL_MD),
        ("size.control.lg", CONTROL_LG),
        ("size.icon.sm", ICON_SM),
        ("size.icon.md", ICON_MD),
        ("size.icon.lg", ICON_LG),
    ];
    for (name, value) in tokens {
        assert!(value > 0.0, "{name} ({value}) must be strictly positive");
    }
}
