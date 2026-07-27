//! Text-content assertion helpers for rendered output.
//!
//! Walks an `egui::FullOutput` (or its `.shapes` slice) and extracts all
//! rendered text via the `Shape::Text` variant. Use the extracted text to
//! assert that a panel rendered the expected strings.
//!
//! # Why this exists
//!
//! Many Tier 2 panel smoke tests verify only that the panel did not
//! panic and did not mutate unrelated app state. Per the Q12 policy in
//! `doc/planning/egui-testing.md`, those tests should instead assert on
//! **rendered content** — at minimum the panel's stable header. This
//! module provides the workhorse helper.
//!
//! # Usage
//!
//! ```ignore
//! use crate::ui::test_helpers::text::{assert_text_contains,
//!     assert_text_contains_all};
//!
//! let output = ctx.run_ui(raw_input, |ui| {
//!     show_left_panel(&mut app, ui);
//! });
//!
//! // Header-only assertion (Q12 borderline case).
//! assert_text_contains(&output.shapes, crate::ui::strings::WORKSPACE_HEADER);
//!
//! // Multiple strings (stable-content assertion).
//! assert_text_contains_all(&output.shapes, &[
//!     crate::ui::strings::WORKSPACE_HEADER,
//!     crate::ui::strings::NO_MARKDOWN_FILES,
//! ]);
//! ```

use eframe::egui;

/// Walk the rendered output and collect every `Shape::Text`'s text content.
///
/// Handles `Shape::Vec` (nested shapes) recursively. Non-text shapes are
/// skipped.
pub fn extract_text(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
    let mut out = Vec::new();
    for cs in shapes {
        extract_text_from_shape(&cs.shape, &mut out);
    }
    out
}

fn extract_text_from_shape(shape: &egui::Shape, out: &mut Vec<String>) {
    match shape {
        egui::Shape::Text(text_shape) => {
            out.push(text_shape.galley.text().to_string());
        }
        egui::Shape::Vec(shapes) => {
            for s in shapes {
                extract_text_from_shape(s, out);
            }
        }
        _ => {}
    }
}

/// Assert that at least one rendered text string contains `needle`.
///
/// Use this for the Q12 borderline case: a panel has only a stable
/// header, body is dynamic, assert the header is present.
pub fn assert_text_contains(shapes: &[egui::epaint::ClippedShape], needle: &str) {
    let texts = extract_text(shapes);
    assert!(
        texts.iter().any(|t| t.contains(needle)),
        "expected rendered output to contain {needle:?}; got {} text shape(s): {}",
        texts.len(),
        debug_summary(&texts),
    );
}

/// Assert that every string in `needles` is contained in the rendered output.
///
/// Use this when the panel has multiple stable, locatable strings (e.g.
/// header + empty-state message).
pub fn assert_text_contains_all(
    shapes: &[egui::epaint::ClippedShape],
    needles: &[&str],
) {
    let texts = extract_text(shapes);
    for needle in needles {
        assert!(
            texts.iter().any(|t| t.contains(needle)),
            "expected rendered output to contain {needle:?}; got {} text shape(s): {}",
            texts.len(),
            debug_summary(&texts),
        );
    }
}

fn debug_summary(texts: &[String]) -> String {
    let preview: Vec<&str> = texts
        .iter()
        .filter(|t| !t.trim().is_empty())
        .take(5)
        .map(String::as_str)
        .collect();
    if preview.is_empty() {
        "<no non-empty text shapes>".to_string()
    } else {
        format!("[{}]", preview.join(" | "))
    }
}
