//! Tests for [`size`] — control heights, hit targets and icon sizes
//! (`UIF-TOK-002`).
//!
//! Pins every value to the spec table, then enforces the two rules that
//! make the set safe: no interactive control may fall under the 24×24 hit
//! target floor, and no icon may be drawn larger than the smallest control
//! it can be placed inside.
//!
//! A third group — the status bar, the unsaved-state dot and the splitter
//! grab band — is pinned the same way, and those tests are as much about
//! keeping each of them *off* the two scales it is not part of as about
//! its own value.

// Every assertion here targets compile-time token values on purpose: pinning a
// constant is the whole point, so a retuned rung must fail the suite rather
// than pass silently. `clippy::assertions_on_constants` cannot tell a spec pin
// from a tautology.
#![allow(clippy::assertions_on_constants)]

use super::*;

use crate::token::shape;

/// Every token carries the exact logical-pixel value the `UIF-TOK-002`
/// table specifies.
#[test]
fn test_size_values_match_spec_table() {
    assert_eq!(HIT_TARGET_MIN, 24.0, "size.hit_target.min must be 24px");
    assert_eq!(BAR_STATUS, 24.0, "size.bar.status must be 24px");
    assert_eq!(CONTROL_SM, 28.0, "size.control.sm must be 28px");
    assert_eq!(CONTROL_MD, 32.0, "size.control.md must be 32px");
    assert_eq!(CONTROL_LG, 36.0, "size.control.lg must be 36px");
    assert_eq!(INDICATOR_DOT, 6.0, "size.indicator.dot must be 6px");
    assert_eq!(SPLITTER_GRAB, 10.0, "size.splitter.grab must be 10px");
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
    let tokens: [(&'static str, f32); 10] = [
        ("size.hit_target.min", HIT_TARGET_MIN),
        ("size.bar.status", BAR_STATUS),
        ("size.control.sm", CONTROL_SM),
        ("size.control.md", CONTROL_MD),
        ("size.control.lg", CONTROL_LG),
        ("size.indicator.dot", INDICATOR_DOT),
        ("size.splitter.grab", SPLITTER_GRAB),
        ("size.icon.sm", ICON_SM),
        ("size.icon.md", ICON_MD),
        ("size.icon.lg", ICON_LG),
    ];
    for (name, value) in tokens {
        assert!(value > 0.0, "{name} ({value}) must be strictly positive");
    }
}

// ---------------------------------------------------------------------------
// The third group: bar, marker and grab band
// ---------------------------------------------------------------------------
//
// These three are pinned as much for what they are *not* as for what they
// are. Each shares the ladder with the control and icon rungs, and each
// would be quietly wrong if it were read as a member of either scale.

/// The status bar shares [`HIT_TARGET_MIN`]'s number without sharing its
/// role: one is a bar's height, the other the rule a control is measured
/// against. The equality is pinned so a retune of either has to be
/// deliberate, and the floor relationship is pinned so a 24px bar can
/// still host a minimum-size control without clipping it.
#[test]
fn test_the_status_bar_is_a_bar_not_a_control() {
    assert_eq!(
        BAR_STATUS, HIT_TARGET_MIN,
        "size.bar.status ({BAR_STATUS}) and size.hit_target.min \
         ({HIT_TARGET_MIN}) coincide on the ladder but not in meaning"
    );
    assert!(
        BAR_STATUS >= HIT_TARGET_MIN,
        "a {BAR_STATUS}px status bar must still host a {HIT_TARGET_MIN}px \
         control rather than clip it"
    );
}

/// The unsaved-state dot is a marker, not a glyph. It stays below every
/// icon rung — drawn at glyph size an unsaved flag reads as an action —
/// and inside the bar it normally sits in without filling its height.
#[test]
fn test_the_indicator_dot_is_a_marker_not_a_glyph() {
    for (name, icon) in icon_table() {
        assert!(
            INDICATOR_DOT < icon,
            "size.indicator.dot ({INDICATOR_DOT}) must stay below {name} ({icon})"
        );
    }
    assert!(
        INDICATOR_DOT < BAR_STATUS,
        "the dot ({INDICATOR_DOT}) must sit inside a status bar ({BAR_STATUS}) \
         without filling it"
    );
}

/// The splitter grab band is wider than the hairline rule it is centred
/// over, narrower than the smallest control rung, and — deliberately —
/// below the hit-target floor. All three are pinned so the spec's own
/// `10` cannot be "corrected" into a control-sized value by someone who
/// reads the shortfall as a bug.
#[test]
fn test_the_splitter_grab_is_a_band_not_a_control() {
    assert!(
        SPLITTER_GRAB > shape::BORDER_HAIRLINE,
        "the grab band ({SPLITTER_GRAB}) must be wider than the {}px rule it is \
         centred over, or there is nothing to grab",
        shape::BORDER_HAIRLINE
    );
    assert!(
        SPLITTER_GRAB < HIT_TARGET_MIN,
        "the spec's {SPLITTER_GRAB}px grab band sits below the {HIT_TARGET_MIN}px \
         control floor on purpose: a strip found by feel between two panes is not \
         a control that advertises itself"
    );
    assert!(
        SPLITTER_GRAB < CONTROL_SM,
        "the grab band ({SPLITTER_GRAB}) must not eat into the smallest control \
         rung ({CONTROL_SM}) that may sit beside a pane"
    );
}
