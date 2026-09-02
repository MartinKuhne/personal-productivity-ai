//! Right table-of-contents panel — clickable heading entries with level-based indentation and font sizing.
//!
//! Unit tests live in the sibling `right_tests.rs` sidecar.

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
/// Purity: Impure (mutates `app.orchestrator.tabs.scroll_to_header_id`).
/// Preconditions: None.
/// Postconditions: `app.orchestrator.tabs.scroll_to_header_id == Some(entry_id_str)`
/// after the call. The center panel reads this field on the next
/// frame, converts it to an `egui::Id` at render time, and scrolls
/// the markdown to the heading with that id.
///
/// The TOC row click in `show_right_panel` calls this function. It
/// is extracted so the side effect can be unit-tested without
/// driving the egui harness.
pub fn apply_toc_row_click(entry_id: &str) -> crate::bus::events::user_command::UserCommand {
    crate::bus::events::user_command::UserCommand::ScrollToHeader(entry_id.to_string())
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
/// the matching doc-comment on `show_top_panel_capture` for the
/// full rationale.
#[tracing::instrument(skip_all, name = "ui.panel.right", level = "debug")]
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
    let ctx = parent_ui.ctx();
    let toc_visible = should_show_panel(
        !app.tabs().toc.is_empty(),
        app.selection().selected_file().is_some(),
    );

    let max_w = ctx.viewport_rect().width() * 0.3;
    let default_w = app
        .layout()
        .right_panel_width
        .unwrap_or(200.0)
        .max(150.0)
        .min(max_w);

    // egui 0.35 unified `SidePanel`/`TopBottomPanel` into `Panel`,
    // and panels now allocate within a parent `&mut Ui`.
    // `width_range` is now `size_range`.
    let panel_response = Panel::right("toc_panel")
        .default_size(default_w)
        .size_range(150.0..=max_w)
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
                .show_rows(
                    ui,
                    18.0,
                    app.orchestrator.tabs.toc.len(),
                    |ui, row_range| {
                        let toc_snapshot = app.orchestrator.tabs.toc.clone();
                        for i in row_range {
                            if let Some(entry) = toc_snapshot.get(i) {
                                ui.push_id((i, &entry.id, "toc_item"), |ui| {
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
                                            app.orchestrator
                                                .user_command_bus
                                                .publish(apply_toc_row_click(&entry.id));
                                            on_click("toc_row");
                                        }
                                    });
                                });
                            }
                        }
                    },
                );
        });

    // Capture the panel's actual width after user interaction
    let rect = panel_response.response.rect;
    app.layout_mut().right_panel_width = Some(rect.width());
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `right_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "right_tests.rs"]
mod tests;
