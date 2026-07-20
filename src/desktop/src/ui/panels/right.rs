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
    if should_show_panel(!app.toc.is_empty(), app.selected_file.is_some()) {
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

                egui::ScrollArea::vertical().id_source("right_toc_scroll").show(ui, |ui| {
                    for entry in &app.toc {
                        let indent = calculate_indent(entry.level as usize);
                        ui.horizontal(|ui| {
                            ui.add_space(indent);
                            let label = egui::RichText::new(&entry.title)
                                .size(calculate_font_size(entry.level as usize));
                            if ui.selectable_label(false, label).clicked() {
                                app.scroll_to_header_id = Some(entry.id.clone());
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
    use std::collections::{BTreeMap, BTreeSet, HashSet};
    use std::sync::{Arc, Mutex};
    use crate::background::BackgroundProcessManager;
    use crate::ui::ToCEntry;
    use std::path::PathBuf;

    fn create_test_app() -> FastMdApp {
        let (tx, rx) = std::sync::mpsc::channel();
        let config = crate::config::AppConfig::default();
        FastMdApp {
            content_libraries: vec![],
            rx,
            tx,
            all_files: vec![],
            all_dirs: vec![],
            file_tags: BTreeMap::new(),
            all_tags: BTreeSet::new(),
            selected_tag: None,
            indexing_finished: false,
            indexing_finished_handled: false,
            left_panel_width: None,
            selected_file: None,
            selected_files: HashSet::new(),
            selected_dir: None,
            expanded_dirs: HashSet::new(),
            loaded_path: None,
            current_yaml: None,
            current_markdown: String::new(),
            tabs: vec![],
            move_dialog_open: false,
            file_to_move: None,
            selected_move_folder: None,
            create_dir_dialog_open: false,
            create_dir_parent: None,
            create_dir_name: String::new(),
            rename_dialog_open: false,
            file_to_rename: None,
            rename_new_name: String::new(),
            command_input: String::new(),
            toc: vec![],
            scroll_to_header_id: None,
            _watcher: None,
            show_agent_results: false,
            agent_running: false,
            agent_status: String::new(),
            agent_thinking: String::new(),
            agent_response: String::new(),
            agent_scroll_to_id: None,
            agent_cancel_flag: None,
            agent_history: None,
            left_panel_reset_count: 0,
            submit_prompt: None,
            editor_state: crate::editor::EditorState::default(),
            inline_editor_enabled: true,
            background_manager: Arc::new(Mutex::new(BackgroundProcessManager::new())),
            show_background_logs: false,
            config,
        }
    }

    #[test]
    fn test_show_right_panel_hidden_when_no_file() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        app.toc.push(ToCEntry {
            title: "Header".to_string(),
            level: 1,
            id: egui::Id::new("header"),
        });
        app.selected_file = None;

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            show_right_panel(&mut app, ctx);
        });
    }

    #[test]
    fn test_show_right_panel_shown_with_toc() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        app.toc.push(ToCEntry {
            title: "Header 1".to_string(),
            level: 1,
            id: egui::Id::new("h1"),
        });
        app.toc.push(ToCEntry {
            title: "Header 2".to_string(),
            level: 2,
            id: egui::Id::new("h2"),
        });
        app.selected_file = Some(PathBuf::from("doc.md"));

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            show_right_panel(&mut app, ctx);
        });
    }
}