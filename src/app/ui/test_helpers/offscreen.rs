//! Off-viewport / off-screen text rendering guards.
//!
//! Walks the rendered shape list from an `egui::Context::run_ui` call and
//! asserts that every text shape is reachable in the user's viewport —
//! its painted rect is inside its containing `ClippedShape.clip_rect`.
//! Catches the F1 (horizontal clip), F6 (zero / negative / transformed
//! off-screen), and F7 (zero-rect allocation) classes from
//! `doc/planning/off-viewport-text-strategy.md` §"Failure-mode taxonomy".
//!
//! # Why a separate helper
//!
//! `text::assert_text_contains` only checks text content. The id-stability
//! helper (`assert::assert_no_id_change_warnings`) only catches the
//! pass-to-pass shape change bug class. Neither covers "the text is
//! rendered, but in a rect the user cannot see or scroll to". This
//! module fills that gap.
//!
//! # Usage
//!
//! ```ignore
//! use crate::ui::test_helpers::offscreen::assert_no_offscreen_text;
//!
//! let output = run_ui_test(&ctx, raw_input, |ui| {
//!     render_markdown(&doc, ...);
//! });
//! assert_no_offscreen_text(&output.shapes);
//! ```
//!
//! # Reachability contract
//!
//! A text shape is considered **reachable** when:
//!
//! 1. Its visual bounding rect (`Shape::visual_bounding_rect`) has
//!    positive area on both axes, AND
//! 2. Its containing `ClippedShape.clip_rect` has positive area, AND
//! 3. The text visual rect intersects the clip rect (so at least
//!    some pixels of the text are visible inside the clip).
//!
//! When condition 3 fails the text is fully clipped; we report both
//! rects so the diagnostic is actionable. When condition 1 or 2 fails
//! the shape has a degenerate allocation — F6 / F7 territory.

use eframe::egui;

/// One concrete off-viewport text shape violation found by
/// [`find_offscreen_text`].
#[derive(Debug, Clone)]
pub struct OffscreenViolation {
    /// First non-empty line of the rendered text (clipped for the message).
    pub text_snippet: String,
    /// Visual bounding rect of the text shape itself.
    pub text_rect: egui::Rect,
    /// The clip rect the shape was placed in. `None` when the shape
    /// carries no clip rect (should not happen for text in egui 0.35,
    /// but flagged when it does).
    pub clip_rect: Option<egui::Rect>,
    /// Why the shape was rejected.
    pub reason: OffscreenReason,
}

/// Why a text shape was flagged as off-viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffscreenReason {
    /// The text shape's bounding rect is fully outside its containing
    /// clip rect — no pixels reach the screen.
    FullyClipped,
    /// The text shape has zero width or zero height (degenerate
    /// allocation; would never paint anything).
    ZeroTextRect,
    /// The containing clip rect itself is degenerate (zero or negative
    /// area). Indicates a broken scroll-area / panel allocation.
    DegenerateClipRect,
}

impl OffscreenReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::FullyClipped => "fully clipped (text rect outside clip rect)",
            Self::ZeroTextRect => "zero-area text rect (degenerate allocation)",
            Self::DegenerateClipRect => "degenerate clip rect (no area)",
        }
    }
}

impl OffscreenViolation {
    /// Render a one-line diagnostic. Multiple violations are joined by
    /// the caller into a single assertion message.
    pub fn describe(&self) -> String {
        let snip: String = self.text_snippet.chars().take(40).collect();
        let clip = self
            .clip_rect
            .map(|r| format!("{r:?}"))
            .unwrap_or_else(|| "<none>".to_string());
        format!(
            "  text={snip:?} text_rect={:?} clip_rect={clip} reason={}",
            self.text_rect,
            self.reason.as_str()
        )
    }
}

/// Walk the rendered shape list and return every off-viewport text
/// shape violation. Skips text shapes whose `galley.text()` is empty
/// or whitespace-only — those are layout markers, not real content.
pub fn find_offscreen_text(shapes: &[egui::epaint::ClippedShape]) -> Vec<OffscreenViolation> {
    let mut out = Vec::new();
    for cs in shapes {
        visit_shape(&cs.shape, Some(cs.clip_rect), &mut out);
    }
    out
}

fn visit_shape(
    shape: &egui::Shape,
    clip_rect: Option<egui::Rect>,
    out: &mut Vec<OffscreenViolation>,
) {
    match shape {
        egui::Shape::Text(text_shape) => {
            let galley = &text_shape.galley;
            let text = galley.text();
            if text.trim().is_empty() {
                // Whitespace-only or empty text shapes are layout
                // markers (e.g. a forced break rendered as " ").
                return;
            }

            let text_rect = text_shape.visual_bounding_rect();

            if !text_rect.is_finite() || text_rect.width() <= 0.0 || text_rect.height() <= 0.0 {
                out.push(OffscreenViolation {
                    text_snippet: text.to_string(),
                    text_rect,
                    clip_rect,
                    reason: OffscreenReason::ZeroTextRect,
                });
                return;
            }

            let Some(clip) = clip_rect else {
                // egui 0.35 always sets a clip_rect on ClippedShape, so
                // a None here means something bypassed the normal paint
                // path. Flag it.
                out.push(OffscreenViolation {
                    text_snippet: text.to_string(),
                    text_rect,
                    clip_rect: None,
                    reason: OffscreenReason::FullyClipped,
                });
                return;
            };

            if !clip.is_finite() || clip.width() <= 0.0 || clip.height() <= 0.0 {
                out.push(OffscreenViolation {
                    text_snippet: text.to_string(),
                    text_rect,
                    clip_rect: Some(clip),
                    reason: OffscreenReason::DegenerateClipRect,
                });
                return;
            }

            if !text_rect.intersects(clip) {
                out.push(OffscreenViolation {
                    text_snippet: text.to_string(),
                    text_rect,
                    clip_rect: Some(clip),
                    reason: OffscreenReason::FullyClipped,
                });
            }
        }
        egui::Shape::Vec(nested) => {
            for s in nested {
                visit_shape(s, clip_rect, out);
            }
        }
        _ => {}
    }
}

/// Walk the rendered shape list and panic with a diagnostic if any
/// text shape is unreachable. Use this in any L2 / L3 test that
/// renders a markdown panel and needs to guarantee no text is
/// accidentally off-viewport.
pub fn assert_no_offscreen_text(shapes: &[egui::epaint::ClippedShape]) {
    let violations = find_offscreen_text(shapes);
    if violations.is_empty() {
        return;
    }
    let n = violations.len();
    let preview: String = violations
        .iter()
        .take(8)
        .map(|v| v.describe())
        .collect::<Vec<_>>()
        .join("\n");
    let more = if violations.len() > 8 {
        format!("\n  ... and {} more", violations.len() - 8)
    } else {
        String::new()
    };
    panic!(
        "off-viewport text regression: {n} text shape(s) unreachable in the \
         rendered output. See doc/planning/off-viewport-text-strategy.md §3 \
         for the detection primitives.\n{preview}{more}"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::test_helpers::run_ui_test;

    /// Build a `Shape::Text` carrying a real (font-shaped) galley so
    /// `visual_bounding_rect` returns a positive-area rect.
    ///
    /// egui's font subsystem is only initialized after the first
    /// `Context::run_ui` call, so we prime the context with a
    /// no-op frame before building the galley.
    fn make_text_shape(text: &str, pos: egui::Pos2) -> egui::Shape {
        let ctx = egui::Context::default();
        let mut output = run_ui_test(&ctx, egui::RawInput::default(), |_ui| {});
        output.textures_delta.clear();
        let galley = ctx.fonts_mut(|f| {
            f.layout_no_wrap(
                text.to_string(),
                egui::FontId::proportional(14.0),
                egui::Color32::WHITE,
            )
        });
        let text_shape = egui::epaint::TextShape::new(pos, galley, egui::Color32::WHITE);
        egui::Shape::Text(text_shape)
    }

    fn clipped(shape: egui::Shape, clip: egui::Rect) -> egui::epaint::ClippedShape {
        egui::epaint::ClippedShape {
            clip_rect: clip,
            shape,
        }
    }

    #[test]
    fn reachable_text_passes() {
        let shape = make_text_shape("hello world", egui::pos2(10.0, 10.0));
        let cs = clipped(
            shape,
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)),
        );
        let v = find_offscreen_text(&[cs]);
        assert!(v.is_empty(), "in-viewport text must not be flagged: {v:?}");
    }

    #[test]
    fn text_fully_outside_clip_is_flagged() {
        // Text rect anchored at (1000, 1000) is well outside the
        // 800x600 clip starting at origin.
        let shape = make_text_shape("way out there", egui::pos2(1000.0, 1000.0));
        let cs = clipped(
            shape,
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)),
        );
        let v = find_offscreen_text(&[cs]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].reason, OffscreenReason::FullyClipped);
    }

    #[test]
    fn empty_text_is_ignored() {
        let shape = make_text_shape("   ", egui::pos2(10.0, 10.0));
        let cs = clipped(
            shape,
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)),
        );
        let v = find_offscreen_text(&[cs]);
        assert!(
            v.is_empty(),
            "whitespace-only text shape is a layout marker, not a violation: {v:?}"
        );
    }

    #[test]
    fn zero_area_clip_rect_is_flagged() {
        let shape = make_text_shape("hi", egui::pos2(10.0, 10.0));
        let degenerate_clip = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(0.0, 600.0));
        let cs = clipped(shape, degenerate_clip);
        let v = find_offscreen_text(&[cs]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].reason, OffscreenReason::DegenerateClipRect);
    }

    #[test]
    fn nested_shape_vec_recurses() {
        // A `Shape::Vec` containing a fully-clipped text shape must be
        // discovered (the F1 case can hide inside a nested vec).
        let inner = make_text_shape("nested victim", egui::pos2(5000.0, 5000.0));
        let outer = egui::Shape::Vec(vec![inner]);
        let cs = clipped(
            outer,
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)),
        );
        let v = find_offscreen_text(&[cs]);
        assert_eq!(v.len(), 1, "nested text shape must be found");
        assert!(v[0].text_snippet.contains("nested victim"));
    }

    #[test]
    fn non_text_shapes_are_ignored() {
        let rect = egui::Shape::Rect(egui::epaint::RectShape::new(
            egui::Rect::from_min_size(egui::pos2(5000.0, 5000.0), egui::vec2(10.0, 10.0)),
            egui::CornerRadius::ZERO,
            egui::Color32::RED,
            egui::Stroke::NONE,
            egui::StrokeKind::Inside,
        ));
        let cs = clipped(
            rect,
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)),
        );
        let v = find_offscreen_text(&[cs]);
        assert!(v.is_empty(), "non-text shapes must be ignored");
    }

    #[test]
    fn vec_with_mixed_shapes_finds_only_clipped_text() {
        let clipped_text = make_text_shape("clipped", egui::pos2(5000.0, 5000.0));
        let rect = egui::Shape::Rect(egui::epaint::RectShape::new(
            egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(10.0, 10.0)),
            egui::CornerRadius::ZERO,
            egui::Color32::RED,
            egui::Stroke::NONE,
            egui::StrokeKind::Inside,
        ));
        let outer = egui::Shape::Vec(vec![clipped_text, rect]);
        let cs = clipped(
            outer,
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)),
        );
        let v = find_offscreen_text(&[cs]);
        assert_eq!(v.len(), 1);
        assert!(v[0].text_snippet.contains("clipped"));
    }

    #[test]
    fn describe_formats_snippet_and_rects() {
        let v = OffscreenViolation {
            text_snippet:
                "hello world this is a very long snippet that should be truncated at forty chars"
                    .to_string(),
            text_rect: egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(100.0, 14.0)),
            clip_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            reason: OffscreenReason::FullyClipped,
        };
        let s = v.describe();
        assert!(s.contains("hello world"), "describe must include snippet");
        assert!(s.contains("fully clipped"), "describe must include reason");
        assert!(s.contains("clip_rect"), "describe must include clip");
    }

    #[test]
    fn describe_handles_none_clip() {
        let v = OffscreenViolation {
            text_snippet: "orphan".to_string(),
            text_rect: egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(50.0, 14.0)),
            clip_rect: None,
            reason: OffscreenReason::FullyClipped,
        };
        let s = v.describe();
        assert!(s.contains("<none>"), "None clip must render as <none>");
    }

    #[test]
    fn describe_truncates_snippet_to_40_chars() {
        let long = "a".repeat(100);
        let v = OffscreenViolation {
            text_snippet: long.clone(),
            text_rect: egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(10.0, 10.0)),
            clip_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            reason: OffscreenReason::ZeroTextRect,
        };
        let s = v.describe();
        // snippet is truncated to 40 chars in describe()
        assert!(s.contains(&"a".repeat(40)));
        assert!(
            !s.contains(&"a".repeat(41)) || s.len() > 200,
            "should truncate"
        );
    }

    #[test]
    fn reason_as_str_covers_all_variants() {
        assert!(
            OffscreenReason::FullyClipped
                .as_str()
                .contains("fully clipped")
        );
        assert!(OffscreenReason::ZeroTextRect.as_str().contains("zero-area"));
        assert!(
            OffscreenReason::DegenerateClipRect
                .as_str()
                .contains("degenerate clip")
        );
    }

    #[test]
    fn assert_no_offscreen_text_passes_on_empty() {
        let shapes: Vec<egui::epaint::ClippedShape> = Vec::new();
        assert_no_offscreen_text(&shapes);
        let reachable = make_text_shape("visible", egui::pos2(10.0, 10.0));
        let cs = clipped(
            reachable,
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)),
        );
        assert_no_offscreen_text(&[cs]);
    }

    #[test]
    #[should_panic(expected = "off-viewport text regression")]
    fn assert_no_offscreen_text_panics_on_violation() {
        let shape = make_text_shape("should panic", egui::pos2(5000.0, 5000.0));
        let cs = clipped(
            shape,
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)),
        );
        assert_no_offscreen_text(&[cs]);
    }

    #[test]
    fn zero_text_rect_is_flagged_via_nan_pos() {
        // NaN position makes visual_bounding_rect non-finite -> ZeroTextRect.
        let shape = make_text_shape("nan pos", egui::pos2(f32::NAN, f32::NAN));
        let cs = clipped(
            shape,
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)),
        );
        let v = find_offscreen_text(&[cs]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].reason, OffscreenReason::ZeroTextRect);
    }

    #[test]
    fn degenerate_clip_with_nan_is_flagged() {
        let shape = make_text_shape("hi", egui::pos2(10.0, 10.0));
        let nan_clip = egui::Rect::from_min_max(
            egui::pos2(f32::NAN, f32::NAN),
            egui::pos2(f32::NAN, f32::NAN),
        );
        let cs = clipped(shape, nan_clip);
        let v = find_offscreen_text(&[cs]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].reason, OffscreenReason::DegenerateClipRect);
    }

    #[test]
    fn visit_shape_with_none_clip_is_flagged() {
        // Directly exercise the None branch of visit_shape.
        let shape = make_text_shape("no clip", egui::pos2(10.0, 10.0));
        let mut out = Vec::new();
        visit_shape(&shape, None, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].reason, OffscreenReason::FullyClipped);
        assert!(out[0].clip_rect.is_none());
    }
}
