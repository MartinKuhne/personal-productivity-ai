//! Unit tests for pane width solvers (`UIF-PANE-002`).

use super::*;

#[test]
fn test_pane_constants_match_spec_specifications() {
    assert_eq!(LEFT_PANE_MIN_W, 150.0);
    assert_eq!(RIGHT_PANE_MIN_W, 120.0);
}

#[test]
fn test_solve_clamped_pane_width_happy_path() {
    let viewport_w = 1000.0;
    let min_px = 150.0;
    let max_px = 200.0; // 20% of 1000px

    // Within bounds
    let solved = solve_clamped_pane_width(viewport_w, 180.0, min_px, max_px);
    assert_eq!(solved, 180.0);
}

#[test]
fn test_solve_clamped_pane_width_clamps_to_min_and_max() {
    let viewport_w = 1000.0;
    let min_px = 150.0;
    let max_px = 200.0;

    // Below minimum
    let solved_low = solve_clamped_pane_width(viewport_w, 100.0, min_px, max_px);
    assert_eq!(solved_low, 150.0);

    // Above maximum
    let solved_high = solve_clamped_pane_width(viewport_w, 250.0, min_px, max_px);
    assert_eq!(solved_high, 200.0);
}

#[test]
fn test_solve_clamped_pane_width_respects_min_when_max_is_smaller() {
    // When viewport is 640px, 20% is 128px < 150px. Left pane must never collapse below 150px.
    let viewport_w = 640.0;
    let min_px = 150.0;
    let max_px = 128.0;

    let solved = solve_clamped_pane_width(viewport_w, 128.0, min_px, max_px);
    assert_eq!(solved, 150.0);

    let solved_zero = solve_clamped_pane_width(viewport_w, 0.0, min_px, max_px);
    assert_eq!(solved_zero, 150.0);
}

#[test]
fn test_solve_clamped_pane_width_bounded_by_viewport() {
    // If max_px exceeds viewport, viewport bounds it
    let viewport_w = 400.0;
    let min_px = 150.0;
    let max_px = 600.0;

    let solved = solve_clamped_pane_width(viewport_w, 500.0, min_px, max_px);
    assert_eq!(solved, 400.0);
}

#[test]
fn test_solve_clamped_pane_width_degenerate_inputs() {
    // Non-finite requested_w
    let solved_nan = solve_clamped_pane_width(1000.0, f32::NAN, 150.0, 200.0);
    assert_eq!(solved_nan, 150.0);

    let solved_inf = solve_clamped_pane_width(1000.0, f32::INFINITY, 150.0, 200.0);
    assert_eq!(solved_inf, 200.0);

    // Negative min_px and max_px
    let solved_neg = solve_clamped_pane_width(1000.0, 50.0, -100.0, -50.0);
    assert_eq!(solved_neg, 0.0);

    // NaN viewport
    let solved_nan_vp = solve_clamped_pane_width(f32::NAN, 180.0, 150.0, 200.0);
    assert_eq!(solved_nan_vp, 180.0);
}

#[test]
fn test_auto_fit_pane_width_empty_list_returns_min_w() {
    let solved = auto_fit_pane_width(&[], 13.0, 16.0, 16.0, 150.0, 300.0);
    assert_eq!(solved, 150.0);
    assert!(!solved.is_nan());
}

#[test]
fn test_auto_fit_pane_width_short_labels_clamped_to_min_w() {
    let labels = ["a", "b", "short"];
    // 5 chars * (13 * 0.6) = 39.0 + 16.0 + 16.0 = 71.0 < 150.0
    let solved = auto_fit_pane_width(&labels, 13.0, 16.0, 16.0, 150.0, 300.0);
    assert_eq!(solved, 150.0);
}

#[test]
fn test_auto_fit_pane_width_fits_longest_visible_label() {
    let labels = [
        "short",
        "medium_label_name",
        "very_long_directory_name_that_needs_width",
    ];
    let font_size = 14.0;
    let icon_w = 16.0;
    let padding = 16.0;
    let min_w = 150.0;
    let max_w = 500.0;

    let solved = auto_fit_pane_width(&labels, font_size, icon_w, padding, min_w, max_w);

    let longest = "very_long_directory_name_that_needs_width";
    let expected = longest.chars().count() as f32 * (font_size * 0.6) + icon_w + padding;
    assert_eq!(solved, expected);
    assert!(solved >= min_w && solved <= max_w);
}

#[test]
fn test_auto_fit_pane_width_clamps_to_max_w() {
    let labels =
        ["this_is_an_exceptionally_long_path_that_exceeds_the_maximum_pane_width_allowance"];
    let solved = auto_fit_pane_width(&labels, 14.0, 16.0, 16.0, 150.0, 250.0);
    assert_eq!(solved, 250.0);
}

#[test]
fn test_auto_fit_pane_width_degenerate_inputs_never_nan() {
    let labels = ["test"];
    // NaN font_size
    let solved_nan_font = auto_fit_pane_width(&labels, f32::NAN, 16.0, 16.0, 150.0, 300.0);
    assert_eq!(solved_nan_font, 150.0);
    assert!(!solved_nan_font.is_nan());

    // NaN icon_w and padding
    let solved_nan_icon = auto_fit_pane_width(&labels, 14.0, f32::NAN, f32::NAN, 150.0, 300.0);
    assert!(solved_nan_icon >= 150.0);
    assert!(!solved_nan_icon.is_nan());

    // max_w < min_w
    let solved_inverted = auto_fit_pane_width(&labels, 14.0, 16.0, 16.0, 200.0, 100.0);
    assert_eq!(solved_inverted, 200.0);
}

#[test]
fn test_auto_fit_pane_width_unicode_counting() {
    // Multi-byte Unicode characters counted by character, not byte length
    let labels = ["🦀🦀🦀🦀🦀"]; // 5 chars, but 20 UTF-8 bytes
    let font_size = 10.0;
    let icon_w = 0.0;
    let padding = 0.0;
    let min_w = 10.0;
    let max_w = 100.0;

    let solved = auto_fit_pane_width(&labels, font_size, icon_w, padding, min_w, max_w);
    // 5 chars * 6.0 = 30.0
    assert_eq!(solved, 30.0);
}
