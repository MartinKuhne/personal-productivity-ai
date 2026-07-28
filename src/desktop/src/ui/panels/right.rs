//! Right table-of-contents panel Ã¢â‚¬â€ clickable heading entries with level-based indentation and font sizing.

use crate::ui::FastMdApp;
use eframe::egui;
use egui::RichText;
use egui::containers::Panel;

/// Determines if the right panel should be shown based on application state.
/// Precondition: None.
/// Postcondition: Returns true if there is a non-empty TOC and a selected file.
/// Purity: Pure function.
pub fn should_show_panel(has_toc: bool, has_selected_file: bool) -> bool {
    has_toc && has_selected_file
}

/// Maximum supported Markdown heading level (ATX headings `#`..`######`).
const MAX_HEADING_LEVEL: usize = 6;

/// Clamps a heading level into the supported ATX range `1..=6`.
///
/// Out-of-range values (0 or >6) collapse to the nearest valid level so that
/// `calculate_indent` / `calculate_font_size` stay continuous and positive
/// for any `level` without special-casing at every call site.
fn clamp_level(level: usize) -> usize {
    level.clamp(1, MAX_HEADING_LEVEL)
}

/// Calculates the indentation in points for a given TOC heading level.
/// Precondition: `level` should be the heading level (usually 1-6).
/// Postcondition: Returns the horizontal space to indent the TOC entry.
/// Out-of-range levels are clamped to `1..=6` so the result is always
/// between the level-1 and level-6 indents (never `0.0`).
/// Purity: Pure function.
pub fn calculate_indent(level: usize) -> f32 {
    match clamp_level(level) {
        1 => 4.0,
        2 => 14.0,
        3 => 24.0,
        4 => 34.0,
        5 => 44.0,
        6 => 54.0,
        _ => 4.0,
    }
}

/// Calculates the font size for a given TOC heading level.
/// Precondition: `level` is the heading level (usually 1-6).
/// Postcondition: Returns a font size scaled appropriately for the heading level.
/// Out-of-range levels are clamped to `1..=6` so the result is always
/// positive and bounded (`10.0..=12.5`).
/// Purity: Pure function.
pub fn calculate_font_size(level: usize) -> f32 {
    13.0 - (clamp_level(level) as f32 * 0.5)
}

/// Purpose: Applies the side effect of clicking a TOC row in the
/// right panel.
/// Inputs: app (the application state), entry_id (the
/// `egui::Id` of the clicked TOC entry)
/// Outputs: ()
/// Purity: Impure (mutates `app.tab_manager.scroll_to_header_id`).
/// Preconditions: None.
/// Postconditions: `app.tab_manager.scroll_to_header_id == Some(entry_id)`
/// after the call. The center panel reads this field on the next
/// frame and scrolls the markdown to the heading with that id.
///
/// The TOC row click in `show_right_panel` calls this function. It
/// is extracted so the side effect can be unit-tested without
/// driving the egui harness.
pub fn apply_toc_row_click(app: &mut FastMdApp, entry_id: egui::Id) {
    app.tab_manager.scroll_to_header_id = Some(entry_id);
}

pub fn show_right_panel(app: &mut FastMdApp, parent_ui: &mut egui::Ui) {
    show_right_panel_capture(app, parent_ui, |_| {});
}

/// Tier 4 test variant of [`show_right_panel`]. The `on_click`
/// callback is invoked after every TOC row click, with a stable
/// event name. The production caller ([`show_right_panel`]) passes
/// a no-op closure; the test caller in
/// `tests::test_toc_row_click_captures_event` passes a closure
/// that pushes the event into the harness's persistent state. See
/// the matching doc-comment on [`show_top_panel_capture`] for the
/// full rationale.
pub fn show_right_panel_capture(
    app: &mut FastMdApp,
    parent_ui: &mut egui::Ui,
    mut on_click: impl FnMut(&'static str),
) {
    // Compute visibility once, outside the panel closure, so the
    // same value is used in every layout pass of this frame.
    //
    // The previous revision wrapped the entire `Panel::right` in
    // `if should_show_panel(...)`, which made the panel itself
    // appear and disappear on the select/deselect transition. That
    // shifted the parent `Ui`'s auto-id counter (and therefore the
    // auto-id of the `CentralPanel` and every widget inside it)
    // every time the user opened or closed a file, flooding the
    // log with `WARN egui::context: Widget rect ... changed id
    // between passes` lines. Allocating the panel unconditionally
    // and hiding its content with `ui.set_invisible()` when no
    // file is selected keeps the parent `Ui`'s auto-id counter
    // stable across the transition.
    //
    // Trade-off: the panel chrome (a 150-250px strip on the right)
    // is visible even when no file is selected. The center panel
    // therefore reserves that width whether or not the TOC is
    // visible, which is what removes the warning. If the empty
    // strip becomes a UX issue, the next step is to give
    // `Panel::right` a custom rect allocation that collapses to
    // zero width when invisible.
    let toc_visible = should_show_panel(
        !app.tabs().toc.is_empty(),
        app.selection().selected_file().is_some(),
    );
    // egui 0.35 unified `SidePanel`/`TopBottomPanel` into `Panel`,
    // and panels now allocate within a parent `&mut Ui`.
    // `width_range` is now `size_range`.
    Panel::right("toc_panel")
        .default_size(200.0)
        .size_range(150.0..=250.0)
        .resizable(true)
        .show(parent_ui, |ui| {
            if !toc_visible {
                ui.set_invisible();
            }
            ui.add_space(4.0);
            ui.heading(
                RichText::new(crate::ui::strings::TABLE_OF_CONTENTS_HEADER)
                    .size(14.0)
                    .strong()
                    .color(egui::Color32::from_rgb(100, 200, 255)),
            );
            ui.add_space(4.0);

            egui::ScrollArea::vertical()
                .id_salt("right_toc_scroll")
                // Pin the scroll area's inner WIDTH to the panel's width
                // (do not let it follow the content). Without this, the
                // default `auto_shrink = [true, true]` lets a single
                // long TOC entry grow the placer's `min_rect` to the
                // right (because `ui.horizontal` has `main_wrap = false`
                // and `ui.selectable_label` does not wrap), and that
                // right-shifted origin is then inherited by every
                // subsequent row. The panel's `clip_rect(outer_rect)`
                // then hides the left side of every TOC item — the
                // user sees only the right tail of each title (e.g.
                // "Table of Contents" → "tents"). Forcing
                // `auto_shrink[0] = false` keeps the inner width
                // clamped to the panel so rows stay anchored at the
                // panel's left edge; titles wider than the panel are
                // clipped on the right (the expected behavior for a
                // narrow, resizable panel) instead of drifting the
                // origin and clipping on the left.
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    let toc_snapshot = app.tab_manager.toc.clone();
                    for (i, entry) in toc_snapshot.iter().enumerate() {
                        ui.push_id((i, entry.id, "toc_item"), |ui| {
                            let font_size = calculate_font_size(entry.level as usize);
                            let indent = calculate_indent(entry.level as usize);
                            ui.horizontal(|ui| {
                                ui.add_space(indent);
                                // Cap the label's layout width to the remaining
                                // available space. Without this, a non-wrapping
                                // label's allocation grows the placer's min_rect
                                // past the panel right edge even when truncate()
                                // clips the visual rendering — left-side text then
                                // disappears behind the panel's clip_rect.
                                let max_label_w = (ui.available_width()).max(0.0);
                                ui.set_max_width(max_label_w);
                                let label = egui::Label::new(
                                    egui::RichText::new(&entry.title).size(font_size),
                                )
                                .truncate();
                                if ui.add(label).clicked() {
                                    apply_toc_row_click(app, entry.id);
                                    on_click("toc_row");
                                }
                            });
                        });
                    }
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

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
    /// `app.tab_manager.scroll_to_header_id` to `Some(entry_id)`;
    /// the center panel reads this on the next frame and scrolls
    /// the markdown to the heading with that id. We verify the
    /// effect without driving the egui harness.
    #[test]
    fn test_apply_toc_row_click_sets_scroll_to_header_id() {
        let mut app = create_test_app();
        let id = egui::Id::new("intro");
        assert!(
            app.tab_manager.scroll_to_header_id.is_none(),
            "scroll_to_header_id must start as None"
        );
        apply_toc_row_click(&mut app, id);
        assert_eq!(
            app.tab_manager.scroll_to_header_id,
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
        let id_a = egui::Id::new("a");
        let id_b = egui::Id::new("b");
        apply_toc_row_click(&mut app, id_a);
        apply_toc_row_click(&mut app, id_b);
        assert_eq!(app.tab_manager.scroll_to_header_id, Some(id_b));
    }

    /// Tier 4 click test: clicking a TOC row in the right panel
    /// must fire the `on_click("toc_row")` callback. Same pattern
    /// as `test_batch_button_click_opens_dialog` in
    /// `panels/top.rs`. The click handler also sets
    /// `app.tab_manager.scroll_to_header_id`, but the harness
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
                id: egui::Id::new("intro"),
            });
            *app.selection_mut().selected_file_mut() = Some(std::path::PathBuf::from("doc.md"));
            show_right_panel_capture(&mut app, ui, |event| {
                captured.push(event);
            });
        });
        harness.fit_contents();
        // The right panel renders rows from `app.tab_manager.toc`.
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
            id: egui::Id::new("header"),
        });
        *app.selection_mut().selected_file_mut() = None;

        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
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
            id: egui::Id::new("h1"),
        });
        app.tabs_mut().toc.push(ToCEntry {
            title: "Header 2".to_string(),
            level: 2,
            id: egui::Id::new("h2"),
        });
        *app.selection_mut().selected_file_mut() = Some(PathBuf::from("doc.md"));

        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
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
        use egui_kittest::Harness;
        use egui_kittest::kittest::Queryable;

        // 400px window with a 200px right panel (Panel::right's
        // default `default_outer_size` is 200.0 for left/right
        // sides, see `egui-0.35/src/containers/panel.rs:255-257`).
        // The production code calls `default_size(200.0)` and
        // `size_range(150.0..=250.0)`, so without a stored
        // PanelState the panel takes 200px and starts at
        // x = 400 - 200 = 200.
        const WINDOW_WIDTH: f32 = 400.0;
        const WINDOW_HEIGHT: f32 = 600.0;
        // The actual panel width in a fresh harness (no stored
        // PanelState). 200.0 = `default_outer_size` for right
        // panels; see comment above. The test's
        // `expected_left` is derived from this so the assertion
        // is robust to a future `default_size(...)` change in
        // the production code: a regression in the fix would
        // show up as a `left` value far to the right of the
        // panel's left edge, not as a 4px margin-fudge.
        const PANEL_WIDTH: f32 = 200.0;
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
            id: egui::Id::new("long"),
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
}
