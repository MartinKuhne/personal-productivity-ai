//! Tests for `panels/right.rs`.

use super::*;
use crate::ui::test_helpers::run_ui_test;

#[test]
fn test_should_show_panel() {
    assert!(should_show_panel(true, true));
    assert!(!should_show_panel(false, true));
    assert!(!should_show_panel(true, false));
    assert!(!should_show_panel(false, false));
}

#[test]
fn test_calculate_indent() {
    assert_eq!(calculate_indent(1), 4.0);
    assert_eq!(calculate_indent(2), 14.0);
    assert_eq!(calculate_indent(3), 24.0);
    assert_eq!(calculate_indent(4), 34.0);
    assert_eq!(calculate_indent(5), 44.0);
    assert_eq!(calculate_indent(6), 54.0);
    // Out-of-range levels clamp to the nearest valid level, never 0.0.
    assert_eq!(calculate_indent(0), 4.0);
    assert_eq!(calculate_indent(7), 54.0);
    assert_eq!(calculate_indent(99), 54.0);
    assert!(calculate_indent(usize::MAX) > 0.0);
}

/// Tier 1 test for the TOC row click effect. The click sets
/// `app.orchestrator.tab_manager.scroll_to_header_id` to `Some(entry_id_str)`;
/// the center panel reads this on the next frame, converts it
/// to an `egui::Id`, and scrolls the markdown to the heading
/// with that id. We verify the effect without driving the
/// egui harness.
#[test]
fn test_apply_toc_row_click_sets_scroll_to_header_id() {
    let mut app = create_test_app();
    let id = "intro".to_string();
    assert!(
        app.orchestrator.tab_manager.scroll_to_header_id.is_none(),
        "scroll_to_header_id must start as None"
    );
    apply_toc_row_click(&mut app, &id);
    assert_eq!(
        app.orchestrator.tab_manager.scroll_to_header_id,
        Some(id),
        "TOC row click must set scroll_to_header_id to the clicked entry's id"
    );
}

/// Tier 1 test: clicking a different TOC row overwrites the
/// previous `scroll_to_header_id`. The center panel's
/// scroll-to-id consumption is the only place that clears the
/// field between frames; a back-to-back click without a frame
/// in between should leave the latest id.
#[test]
fn test_apply_toc_row_click_overwrites_previous_scroll_target() {
    let mut app = create_test_app();
    let id_a = "a".to_string();
    let id_b = "b".to_string();
    apply_toc_row_click(&mut app, &id_a);
    apply_toc_row_click(&mut app, &id_b);
    assert_eq!(app.orchestrator.tab_manager.scroll_to_header_id, Some(id_b));
}

/// Tier 4 click test: clicking a TOC row in the right panel
/// must fire the `on_click("toc_row")` callback. Same pattern
/// as `test_batch_button_click_opens_dialog` in
/// `panels/top.rs`. The click handler also sets
/// `app.orchestrator.tab_manager.scroll_to_header_id`, but the harness
/// owns `&mut app` for its lifetime, so the side effect is
/// observed via the captured `&'static str` event name.
#[test]
fn test_toc_row_click_captures_event() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;

    let mut harness = stateful_harness(Vec::<&'static str>::new(), |ui, captured| {
        let mut app = create_test_app();
        app.tabs_mut().toc.push(crate::ui::ToCEntry {
            title: "Introduction".to_string(),
            level: 1,
            id: "intro".to_string(),
        });
        *app.selection_mut().selected_file_mut() = Some(std::path::PathBuf::from("doc.md"));
        show_right_panel_capture(&mut app, ui, |event| {
            captured.push(event);
        });
    });
    harness.fit_contents();
    // The right panel renders rows from `app.orchestrator.tab_manager.toc`.
    // Locate the row by its title (a substring match is
    // sufficient).
    harness.get_by_label("Introduction").click();
    // Bounded step count to avoid spinning the harness forever
    // on the repaint-after-click loop.
    harness.run_steps(2);
    harness.run_steps(2);

    let captured = harness.state();
    assert!(
        captured.contains(&"toc_row"),
        "clicking a TOC row must fire the `toc_row` on_click event; \
             got: {:?}",
        captured
    );
}

#[test]
fn test_calculate_font_size() {
    assert_eq!(calculate_font_size(1), 12.5);
    assert_eq!(calculate_font_size(2), 12.0);
    assert_eq!(calculate_font_size(3), 11.5);

    // Property-based check equivalent for boundaries (1 to 6)
    for level in 1..=6 {
        let expected = 13.0 - (level as f32 * 0.5);
        assert_eq!(calculate_font_size(level), expected);
    }

    // Out-of-range levels clamp, staying positive and within the
    // level-1..=6 font-size range.
    assert_eq!(calculate_font_size(0), 12.5);
    assert_eq!(calculate_font_size(7), 10.0);
    assert_eq!(calculate_font_size(26), 10.0);
    assert!(calculate_font_size(usize::MAX) > 0.0);
}

// --- UI / window tests (R-7: merged from `mod ui_tests`) ---

use crate::ui::ToCEntry;
use std::path::PathBuf;

fn create_test_app() -> FastMdApp {
    FastMdApp::empty_state(crate::config::AppConfig::default())
}

#[test]
fn test_show_right_panel_hidden_when_no_file() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();
    app.tabs_mut().toc.push(ToCEntry {
        title: "Header".to_string(),
        level: 1,
        id: "header".to_string(),
    });
    *app.selection_mut().selected_file_mut() = None;

    let _ = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
        show_right_panel(&mut app, ui);
    });
}

#[test]
fn test_show_right_panel_shown_with_toc() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();
    app.tabs_mut().toc.push(ToCEntry {
        title: "Header 1".to_string(),
        level: 1,
        id: "h1".to_string(),
    });
    app.tabs_mut().toc.push(ToCEntry {
        title: "Header 2".to_string(),
        level: 2,
        id: "h2".to_string(),
    });
    *app.selection_mut().selected_file_mut() = Some(PathBuf::from("doc.md"));

    let _ = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
        show_right_panel(&mut app, ui);
    });
}

/// Regression test for the right-panel "left side cut off" bug.
///
/// **Symptom.** The heading "Table of Contents" and every TOC row
/// showed only its right tail (e.g. "Table of Contents" → "tents",
/// "Candidates" → "ndidates"). The left edge of every row was
/// clipped, even though `ui.horizontal` is left-to-right and the
/// text is laid out starting at the panel's left edge.
///
/// **Root cause.** `egui::ScrollArea::vertical()` defaults to
/// `auto_shrink = [true, true]`. On the X axis that resolves to
/// `(direction_enabled = false, auto_shrink = true)` →
/// `inner_size.x = content_size.x` (the scroll area grows to fit
/// its content). Inside the scroll area, `ui.horizontal` runs with
/// `main_wrap = false` and `ui.selectable_label` does not wrap, so
/// a long TOC entry overflows horizontally. The placer's
/// `expand_to_include_rect` grows the parent Ui's `min_rect` to the
/// right to fit the overflowing child; `auto_shrink` then
/// propagates that into the scroll area's inner rect. Once a
/// single long row has shifted the placer's `min` to the right, the
/// panel's `clip_rect(outer_rect)` clips out the left side of every
/// subsequent row.
///
/// **Fix.** Pin the scroll area's inner width to the panel's width
/// via `.auto_shrink([false, true])`. Long rows are then clipped on
/// the right (the expected narrow-panel behavior) instead of
/// dragging the origin and clipping on the left.
///
/// This test reproduces the original bug: a 400px-wide window with
/// a TOC containing a 240-char title would, *without* the fix,
/// place the long row's left edge well past the panel's left edge.
/// The assertion is the regression — we expect the row's
/// `rect.left()` to match the panel's left edge (within a pixel of
/// rounding), not a value that has drifted to the right.
#[test]
fn test_show_right_panel_long_titles_anchor_at_panel_left_edge() {
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

    // 400px window. The panel's max_size is 30% of viewport width = 120px.
    // default_size is clamped to max_size, so panel will be 120px wide.
    const WINDOW_WIDTH: f32 = 400.0;
    const WINDOW_HEIGHT: f32 = 600.0;
    const MAX_PANEL_WIDTH: f32 = WINDOW_WIDTH * 0.3; // 120px
    const PANEL_WIDTH: f32 = MAX_PANEL_WIDTH; // default_size clamped to max_size
    const PIXEL_TOLERANCE: f32 = 2.0;

    // A title deliberately wider than the entire 400px window, so
    // that without the `auto_shrink` fix the placer would
    // absolutely grow the scroll area past the right edge and
    // shift the origin to the right of the panel's left clip.
    let long_title = "A".repeat(240);

    let mut app = create_test_app();
    app.tabs_mut().toc.push(ToCEntry {
        title: long_title.clone(),
        level: 1,
        id: "long".to_string(),
    });
    *app.selection_mut().selected_file_mut() = Some(PathBuf::from("doc.md"));

    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(WINDOW_WIDTH, WINDOW_HEIGHT))
        .build_ui(|ui| {
            show_right_panel(&mut app, ui);
        });
    harness.run();

    // Expected: panel occupies PANEL_WIDTH px and starts at
    // x = WINDOW_WIDTH - PANEL_WIDTH. The leftmost matching
    // node in the accesskit tree is the `ui.horizontal`
    // wrapper that contains the long row; the wrapper's
    // left edge is the panel's left edge (no extra frame
    // margin in the harness's default renderer for the
    // outer container).
    let expected_left = WINDOW_WIDTH - PANEL_WIDTH;

    // Locate the long TOC row by a substring of its label
    // (truncation may strip characters but the accesskit node
    // keeps the semantic text).
    let nodes: Vec<_> = harness
        .query_all_by_label_contains(&long_title[..32])
        .collect();
    assert!(
        !nodes.is_empty(),
        "long TOC row should be present in the accesskit tree"
    );

    // Collect rects of all matching nodes to find the leftmost —
    // the TOC row's outer container (the `ui.horizontal` wrapper)
    // is the widest matching node; the accesskit tree may also
    // emit separate `Button` / `Label` nodes for the clickable
    // area and the text. Pick the leftmost rect — that is the
    // one that would be clipped on the left if the origin drifts.
    let rects: Vec<_> = nodes.iter().map(|n| n.rect()).collect();

    // The TOC row's outer container (the `ui.horizontal` wrapper)
    // is the widest matching node; the accesskit tree may also
    // emit separate `Button` / `Label` nodes for the clickable
    // area and the text. Pick the leftmost rect — that is the
    // one that would be clipped on the left if the origin drifts.
    let left = rects
        .iter()
        .map(egui::Rect::left)
        .fold(f32::INFINITY, f32::min);

    assert!(
        (left - expected_left).abs() <= PIXEL_TOLERANCE,
        "long TOC row's left edge drifted: got {left:.2}, expected ~{expected_left:.2} \
             (tolerance {PIXEL_TOLERANCE:.1}). Without `.auto_shrink([false, true])` on the \
             scroll area, `ui.horizontal` + a non-wrapping `selectable_label` grows the \
             placer's min_rect to the right of the panel, and the panel's clip_rect hides \
             the left side of every row."
    );
}
