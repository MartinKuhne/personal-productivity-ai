//! Pure boundary solvers for workspace pane dimensions (`UIF-PANE-002`).
//!
//! Provides pure solver functions for calculating clamped pane widths and auto-fitting
//! directory tree / navigation pane widths based on item labels and tokenized metrics
//! without coupling to engine rendering or stateful widget trees.
//!
//! Unit tests live in the sibling `pane_solver_tests.rs` sidecar.

/// Left pane minimum width constant (150px) per §2.2 / §4.3.2 (`UIF-PANE-002`).
pub const LEFT_PANE_MIN_W: f32 = 150.0;

/// Right pane minimum width constant (120px) per §2.2 / §4.3.2 (`UIF-PANE-002`).
pub const RIGHT_PANE_MIN_W: f32 = 120.0;

/// Solve clamped pane width against viewport width, requested width, and [min_px, max_px] bounds (`UIF-PANE-002`).
///
/// Clamps `requested_w` into `[min_px, max_px]`, ensuring the pane width never collapses
/// below `min_px` (e.g. [`LEFT_PANE_MIN_W`]) and does not exceed the available `viewport_w`.
/// If `max_px < min_px`, `min_px` takes precedence.
/// Degenerate inputs (negative or non-finite values) are safely sanitised.
#[must_use]
pub fn solve_clamped_pane_width(
    viewport_w: f32,
    requested_w: f32,
    min_px: f32,
    max_px: f32,
) -> f32 {
    let floor = if min_px.is_finite() && min_px > 0.0 {
        min_px
    } else {
        0.0
    };
    let mut ceiling = if max_px.is_finite() {
        max_px.max(floor)
    } else {
        floor
    };
    if viewport_w.is_finite() && viewport_w >= floor {
        ceiling = ceiling.min(viewport_w);
    }
    if requested_w.is_nan() || requested_w < floor {
        floor
    } else if requested_w > ceiling {
        ceiling
    } else {
        requested_w
    }
}

/// Calculate auto-fit pane width from a list of item labels and font/layout metrics (`UIF-PANE-002`).
///
/// Takes label slices, font_size, icon_w, padding, and clamps between min_w and max_w.
/// Returns the smallest width that fits the longest visible label without wrapping.
/// Empty lists or degenerate inputs safely resolve to `min_w` and never produce `NaN`.
#[must_use]
pub fn auto_fit_pane_width(
    labels: &[&str],
    font_size: f32,
    icon_w: f32,
    padding: f32,
    min_w: f32,
    max_w: f32,
) -> f32 {
    let floor = if min_w.is_finite() && min_w > 0.0 {
        min_w
    } else {
        0.0
    };
    let ceiling = if max_w.is_finite() {
        max_w.max(floor)
    } else {
        floor
    };
    let safe_font_size = if font_size.is_finite() && font_size > 0.0 {
        font_size
    } else {
        0.0
    };
    let safe_icon_w = if icon_w.is_finite() && icon_w > 0.0 {
        icon_w
    } else {
        0.0
    };
    let safe_padding = if padding.is_finite() && padding > 0.0 {
        padding
    } else {
        0.0
    };

    let mut max_content_w = 0.0_f32;
    for label in labels {
        let char_count = label.chars().count() as f32;
        let text_w = char_count * (safe_font_size * 0.6);
        let row_w = text_w + safe_icon_w + safe_padding;
        if row_w > max_content_w {
            max_content_w = row_w;
        }
    }

    max_content_w.clamp(floor, ceiling)
}

#[cfg(test)]
#[path = "pane_solver_tests.rs"]
mod pane_solver_tests;
