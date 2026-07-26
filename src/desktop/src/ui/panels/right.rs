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

pub fn show_right_panel(app: &mut FastMdApp, parent_ui: &mut egui::Ui) {
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
        .size_range(150.0..=250.0)
        .resizable(true)
        .show(parent_ui, |ui| {
            if !toc_visible {
                ui.set_invisible();
            }
            ui.add_space(4.0);
            ui.heading(
                RichText::new("Table of Contents")
                    .size(14.0)
                    .strong()
                    .color(egui::Color32::from_rgb(100, 200, 255)),
            );
            ui.add_space(4.0);

            egui::ScrollArea::vertical()
                .id_salt("right_toc_scroll")
                .show(ui, |ui| {
                    let toc_snapshot = app.tab_manager.toc.clone();
                    for (i, entry) in toc_snapshot.iter().enumerate() {
                        let indent = calculate_indent(entry.level as usize);
                        ui.push_id((i, entry.id, "toc_item"), |ui| {
                            ui.horizontal(|ui| {
                                ui.add_space(indent);
                                let label = egui::RichText::new(&entry.title)
                                    .size(calculate_font_size(entry.level as usize));
                                if ui.selectable_label(false, label).clicked() {
                                    app.tab_manager.scroll_to_header_id = Some(entry.id);
                                }
                            })
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
        assert_eq!(should_show_panel(true, true), true);
        assert_eq!(should_show_panel(false, true), false);
        assert_eq!(should_show_panel(true, false), false);
        assert_eq!(should_show_panel(false, false), false);
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
}

#[cfg(test)]
mod ui_tests {
    use super::*;
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
}
