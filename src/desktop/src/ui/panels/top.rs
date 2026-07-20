use crate::ui::FastMdApp;
use eframe::egui;
use egui::RichText;
use std::path::PathBuf;
use std::collections::BTreeMap;

/// Purpose: Generates the indexing status rich text based on whether indexing is finished.
/// Inputs: indexing_finished (boolean), file_count (usize)
/// Outputs: egui::RichText with appropriate message, color, and styling.
/// Purity: Pure function.
/// Preconditions: None.
/// Postconditions: Returns green text with "Indexing finished" if true, or italicized text with "Indexing workspace" if false.
pub fn build_indexing_status_text(indexing_finished: bool, file_count: usize) -> RichText {
    if indexing_finished {
        RichText::new(format!("Indexing finished ({} files)", file_count))
            .color(egui::Color32::from_rgb(100, 255, 100))
    } else {
        RichText::new(format!("Indexing workspace (found {} files)...", file_count))
            .italics()
    }
}

/// Purpose: Determines the display text for the tag filter combobox.
/// Inputs: selected_tag (optional string reference)
/// Outputs: String slice representing the text to show.
/// Purity: Pure function.
/// Preconditions: None.
/// Postconditions: Returns the tag name if one is selected, otherwise "Filter by Tag: All".
pub fn get_tag_filter_text(selected_tag: Option<&String>) -> &str {
    selected_tag.map(|s| s.as_str()).unwrap_or("Filter by Tag: All")
}

/// Purpose: Determines the next selected file after the active tag filter changes.
/// Inputs: selected_file (current selected file path), selected_tag (currently active tag filter), file_tags (mapping of files to their tags)
/// Outputs: Option<PathBuf> representing the new selected file.
/// Purity: Pure function.
/// Preconditions: None.
/// Postconditions: Returns `None` if an active tag is selected and it is not associated with the selected file. Otherwise returns the original selected file.
pub fn compute_next_selected_file(
    selected_file: Option<&PathBuf>,
    selected_tag: Option<&String>,
    file_tags: &BTreeMap<PathBuf, Vec<String>>,
) -> Option<PathBuf> {
    let selected = selected_file?;
    if let Some(active_tag) = selected_tag {
        if let Some(tags) = file_tags.get(selected) {
            if !tags.contains(active_tag) {
                return None;
            }
        } else {
            return None;
        }
    }
    Some(selected.clone())
}

pub fn show_top_panel(app: &mut FastMdApp, ctx: &egui::Context) {
    egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.heading(
                RichText::new("⚡ FastMD Viewer")
                    .strong()
                    .color(egui::Color32::from_rgb(100, 200, 255)),
            );
            ui.separator();
            ui.checkbox(&mut app.show_background_logs, "Show log");
            ui.separator();

            if !app.indexing_finished {
                ui.spinner();
            }

            ui.label(build_indexing_status_text(app.indexing_finished, app.all_files.len()));

            if app.indexing_finished {
                ui.separator();
                egui::ComboBox::from_id_source("tag_combobox")
                    .selected_text(get_tag_filter_text(app.selected_tag.as_ref()))
                    .show_ui(ui, |ui| {
                        let mut changed = ui
                            .selectable_value(&mut app.selected_tag, None, "All")
                            .changed();
                        for tag in &app.all_tags {
                            changed |= ui
                                .selectable_value(
                                    &mut app.selected_tag,
                                    Some(tag.clone()),
                                    tag,
                                )
                                .changed();
                        }
                        if changed {
                            app.selected_file = compute_next_selected_file(
                                app.selected_file.as_ref(),
                                app.selected_tag.as_ref(),
                                &app.file_tags,
                            );
                        }
                    });
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_indexing_status_text_finished() {
        let text = build_indexing_status_text(true, 42);
        assert_eq!(text.text(), "Indexing finished (42 files)");
    }

    #[test]
    fn test_build_indexing_status_text_unfinished() {
        let text = build_indexing_status_text(false, 10);
        assert_eq!(text.text(), "Indexing workspace (found 10 files)...");
    }

    #[test]
    fn test_get_tag_filter_text() {
        assert_eq!(get_tag_filter_text(None), "Filter by Tag: All");
        let tag = "Rust".to_string();
        assert_eq!(get_tag_filter_text(Some(&tag)), "Rust");
    }

    #[test]
    fn test_compute_next_selected_file_no_selected_file() {
        let file_tags = BTreeMap::new();
        assert_eq!(compute_next_selected_file(None, None, &file_tags), None);
    }

    #[test]
    fn test_compute_next_selected_file_no_tag() {
        let mut file_tags = BTreeMap::new();
        let path = PathBuf::from("test.md");
        file_tags.insert(path.clone(), vec!["Rust".to_string()]);
        
        assert_eq!(
            compute_next_selected_file(Some(&path), None, &file_tags),
            Some(path)
        );
    }

    #[test]
    fn test_compute_next_selected_file_tag_matches() {
        let mut file_tags = BTreeMap::new();
        let path = PathBuf::from("test.md");
        file_tags.insert(path.clone(), vec!["Rust".to_string()]);
        let tag = "Rust".to_string();
        
        assert_eq!(
            compute_next_selected_file(Some(&path), Some(&tag), &file_tags),
            Some(path)
        );
    }

    #[test]
    fn test_compute_next_selected_file_tag_missing() {
        let mut file_tags = BTreeMap::new();
        let path = PathBuf::from("test.md");
        file_tags.insert(path.clone(), vec!["Rust".to_string()]);
        let tag = "Go".to_string();
        
        assert_eq!(
            compute_next_selected_file(Some(&path), Some(&tag), &file_tags),
            None
        );
    }

    #[test]
    fn test_compute_next_selected_file_file_not_in_tags() {
        let file_tags = BTreeMap::new();
        let path = PathBuf::from("test.md");
        let tag = "Rust".to_string();
        
        assert_eq!(
            compute_next_selected_file(Some(&path), Some(&tag), &file_tags),
            None
        );
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet, HashSet};
    use std::sync::{Arc, Mutex};
    use crate::background::BackgroundProcessManager;

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
    fn test_show_top_panel_indexing_unfinished() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        app.indexing_finished = false;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            show_top_panel(&mut app, ctx);
        });
        assert!(!app.indexing_finished);
    }

    #[test]
    fn test_show_top_panel_indexing_finished_with_tags() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        app.indexing_finished = true;
        app.all_tags.insert("Rust".to_string());
        app.all_tags.insert("Docs".to_string());
        
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            show_top_panel(&mut app, ctx);
        });
        assert!(app.indexing_finished);
    }
}