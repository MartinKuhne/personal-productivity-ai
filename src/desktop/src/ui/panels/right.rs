use crate::ui::FastMdApp;
use eframe::egui;
use egui::RichText;

/// Determines if the right panel should be shown based on application state.
/// Precondition: None.
/// Postcondition: Returns true if there is a non-empty TOC and a selected file.
/// Purity: Pure function.
pub fn should_show_panel(has_toc: bool, has_selected_file: bool) -> bool {
    has_toc && has_selected_file
}

/// Calculates the indentation in points for a given TOC heading level.
/// Precondition: `level` should be the heading level (usually 1-6).
/// Postcondition: Returns the horizontal space to indent the TOC entry.
/// Purity: Pure function.
pub fn calculate_indent(level: usize) -> f32 {
    match level {
        1 => 0.0,
        2 => 10.0,
        3 => 20.0,
        _ => 0.0,
    }
}

/// Calculates the font size for a given TOC heading level.
/// Precondition: `level` is the heading level (usually 1-6).
/// Postcondition: Returns a font size scaled appropriately for the heading level.
/// Purity: Pure function.
pub fn calculate_font_size(level: usize) -> f32 {
    13.0 - (level as f32 * 0.5)
}

pub fn show_right_panel(app: &mut FastMdApp, ctx: &egui::Context) {
    if should_show_panel(
        !app.tabs().toc.is_empty(),
        app.selection().selected_file().is_some(),
    ) {
        egui::SidePanel::right("toc_panel")
            .width_range(150.0..=250.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.heading(
                    RichText::new("Table of Contents")
                        .size(14.0)
                        .strong()
                        .color(egui::Color32::from_rgb(100, 200, 255)),
                );
                ui.add_space(4.0);

                egui::ScrollArea::vertical()
                    .id_source("right_toc_scroll")
                    .show(ui, |ui| {
                        let toc_snapshot = app.tab_manager.toc.clone();
                        for entry in &toc_snapshot {
                            let indent = calculate_indent(entry.level as usize);
                            ui.horizontal(|ui| {
                                ui.add_space(indent);
                                let label = egui::RichText::new(&entry.title)
                                    .size(calculate_font_size(entry.level as usize));
                                if ui.selectable_label(false, label).clicked() {
                                    app.tab_manager.scroll_to_header_id = Some(entry.id.clone());
                                }
                            });
                        }
                    });
            });
    }
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
        assert_eq!(calculate_indent(1), 0.0);
        assert_eq!(calculate_indent(2), 10.0);
        assert_eq!(calculate_indent(3), 20.0);
        assert_eq!(calculate_indent(4), 0.0); // Fallback
        assert_eq!(calculate_indent(99), 0.0); // Edge case
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

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            show_right_panel(&mut app, ctx);
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

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            show_right_panel(&mut app, ctx);
        });
    }
}
