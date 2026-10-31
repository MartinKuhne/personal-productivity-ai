//! Shared helpers for the crate's test sidecars.
//!
//! These are the assertions and constructors that several sidecars defined
//! identically. They live here so a rule has one definition rather than nine:
//! a tolerance, a rounding threshold or a rectangle shape now changes in one
//! place, and a new sidecar gets them by import instead by copy.
//!
//! Test-only. Nothing here is reachable from a release build, and nothing here
//! owns product behaviour - each helper mirrors exactly what the sidecars it
//! came from already did.

use iced::{Color, Rectangle};

/// Whether `actual` is within `5e-4` of `expected`.
///
/// The tolerance is deliberately a literal rather than a named constant: it is
/// the rounding slack for a laid-out `f32` advance, and every sidecar that used
/// it agreed on this value. Sidecars that measure at a looser `1e-3` keep their
/// own local `approx` - a different contract gets a different helper.
///
/// Purity: pure.
pub(crate) fn approx(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 5e-4
}

/// Assert that `actual` matches `expected` to four decimal places.
///
/// The failure message names `what` and prints both values at the precision the
/// comparison actually guarantees, so a failing ratio identifies itself.
///
/// Purity: pure until it panics.
///
/// # Panics
///
/// Panics when the two ratios differ by `0.0005` or more.
pub(crate) fn assert_ratio(actual: f32, expected: f32, what: &str) {
    let measured = f64::from(actual);
    let wanted = f64::from(expected);
    assert!(
        (measured - wanted).abs() < 0.000_5,
        "{what}: expected {wanted:.4}, measured {measured:.4}"
    );
}

/// Format a colour as the `RGB(r, g, b)` text the contrast tests compare.
///
/// Purity: pure.
pub(crate) fn rgb8(color: Color) -> String {
    format!(
        "RGB({}, {}, {})",
        (color.r * 255.0).round(),
        (color.g * 255.0).round(),
        (color.b * 255.0).round()
    )
}

/// A rectangle at `(x, y)` of the given size.
///
/// Equivalent to `Rectangle::new(Point::new(x, y), Size::new(width, height))`,
/// which is what some sidecars spelled out; both spellings collapsed here.
///
/// Purity: pure.
pub(crate) fn rect(x: f32, y: f32, width: f32, height: f32) -> Rectangle {
    Rectangle {
        x,
        y,
        width,
        height,
    }
}
