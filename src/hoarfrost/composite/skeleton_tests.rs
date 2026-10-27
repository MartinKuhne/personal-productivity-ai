//! Unit tests for `skeleton` placeholder composite (`UIF-COMP-014`, `UIF-PRIN-003`).

use super::*;
use crate::token::color::Palette;

#[test]
fn test_skeleton_builders() {
    let s = Skeleton::new(100.0, 32.0);
    assert_eq!(s.width, 100.0);
    assert_eq!(s.height, 32.0);
    assert_eq!(s.radius, DEFAULT_RADIUS);
    assert_eq!(s.phase, 0.0);
    assert!(!s.reduced_motion);

    let row = Skeleton::row(24.0);
    assert_eq!(row.width, 0.0);
    assert_eq!(row.height, 24.0);

    let circle = Skeleton::circle(40.0);
    assert_eq!(circle.width, 40.0);
    assert_eq!(circle.height, 40.0);
    assert_eq!(circle.radius, 20.0);

    let custom = s.with_radius(8.0).with_phase(0.5).with_reduced_motion(true);
    assert_eq!(custom.radius, 8.0);
    assert_eq!(custom.phase, 0.5);
    assert!(custom.reduced_motion);
}

#[test]
fn test_tokens_and_metrics_pin() {
    assert_eq!(
        DEFAULT_RADIUS, 4.0,
        "default radius must be radius.widget per §3B.14"
    );
    assert_eq!(
        BORDER_WIDTH, 1.0,
        "border width must be 1px hairline per §3B.14"
    );
    assert_eq!(
        TREE_ROW_HEIGHT, 24.0,
        "tree row height must be 24px (space.xl) per §3B.14"
    );
    assert_eq!(
        TABLE_ROW_HEIGHT, 32.0,
        "table row height must be 32px (size.control.md) per §3B.14"
    );
    assert_eq!(
        TABLE_ROW_COMPACT_HEIGHT, 28.0,
        "compact table row height must be 28px (size.control.sm) per §3B.14"
    );
    assert_eq!(
        SHIMMER_PERIOD_MS, 1200,
        "shimmer cycle duration must be 1200ms per §3B.14"
    );
}

#[test]
fn test_shimmer_interpolation_bounds() {
    let palette = Palette::dark();

    // Phase 0.25 -> sin(0.25 * TAU) = sin(PI/2) = 1.0 -> factor 1.0 (surface.hover)
    let s_top = Skeleton::new(100.0, 32.0).with_phase(0.25);
    let style_top = resolve_style(&s_top, &palette);
    assert_eq!(style_top.background, palette.surface_hover);
    assert_eq!(style_top.border_color, Color::TRANSPARENT);
    assert_eq!(style_top.border_width, 0.0);

    // Phase 0.75 -> sin(0.75 * TAU) = sin(3PI/2) = -1.0 -> factor 0.0 (surface.raised)
    let s_bot = Skeleton::new(100.0, 32.0).with_phase(0.75);
    let style_bot = resolve_style(&s_bot, &palette);
    assert_eq!(style_bot.background, palette.surface_raised);
    assert_eq!(style_bot.border_color, Color::TRANSPARENT);
    assert_eq!(style_bot.border_width, 0.0);
}

#[test]
fn test_reduced_motion_static_rendering() {
    let palette = Palette::dark();

    let s = Skeleton::new(100.0, 32.0)
        .with_phase(0.25)
        .with_reduced_motion(true);
    let style = resolve_style(&s, &palette);

    // Reduced motion must ignore the phase and lock to surface.raised with 1px border.subtle
    assert_eq!(style.background, palette.surface_raised);
    assert_eq!(style.border_color, palette.border_subtle);
    assert_eq!(style.border_width, BORDER_WIDTH);
    assert_eq!(style.radius, DEFAULT_RADIUS);
}

#[test]
fn test_interpolate_color_clamp() {
    let c1 = Color::from_rgb(0.0, 0.0, 0.0);
    let c2 = Color::from_rgb(1.0, 1.0, 1.0);

    let mid = interpolate_color(c1, c2, 0.5);
    assert!((mid.r - 0.5).abs() < 1e-4);
    assert!((mid.g - 0.5).abs() < 1e-4);
    assert!((mid.b - 0.5).abs() < 1e-4);

    let under = interpolate_color(c1, c2, -1.0);
    assert_eq!(under, c1);

    let over = interpolate_color(c1, c2, 2.0);
    assert_eq!(over, c2);
}

#[test]
fn test_helper_function_and_element_conversion() {
    let s = skeleton(150.0, 28.0);
    assert_eq!(s.width, 150.0);
    assert_eq!(s.height, 28.0);

    let _element: Element<'_, ()> = s.into();
}
