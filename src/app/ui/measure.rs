//! Centralised egui text-measurement helpers (RUST-021, RUST-023).
//!
//! egui 0.35's `FontsView::layout_no_wrap` takes `&mut self`, so measuring a
//! string requires the `Context::fonts_mut` seam (the `Context::fonts`
//! read-only accessor cannot call it). Several call sites — the left file-tree
//! width calculation and the table-width measurer — had grown their own copy
//! of that incantation. They now share the helpers here so the quirk (and the
//! "colour does not influence width" choice) lives in one place.
//!
//! Unit tests live in the sibling `measure_tests.rs` sidecar.

use eframe::egui;

/// Measures the single-line width of `text` in `font` using `color`.
///
/// Wraps the egui 0.35 `fonts_mut` + `layout_no_wrap` quirk so callers do not
/// repeat it. Text colour never affects the shaped width — egui only needs *a*
/// colour to build the galley — so callers pass whatever colour matches their
/// paint path (or [`egui::Color32::WHITE`] when measuring only).
pub(crate) fn text_width(
    ctx: &egui::Context,
    text: &str,
    font: egui::FontId,
    color: egui::Color32,
) -> f32 {
    ctx.fonts_mut(|f| f.layout_no_wrap(text.to_owned(), font, color).size().x)
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `measure_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "measure_tests.rs"]
mod measure_tests;
