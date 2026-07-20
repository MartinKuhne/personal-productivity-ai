use crate::ui::FastMdApp;
use eframe::egui;
use std::collections::BTreeSet;
use std::path::Path;

pub fn show_move_modal(app: &mut FastMdApp, ctx: &egui::Context) {
    let mut close_modal = false;
    if app.move_dialog_open {
        egui::Window::new("Move File")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Select destination folder:");

                let mut folders = BTreeSet::new();
                for lib in &app.content_libraries {
                    folders.insert(std::path::PathBuf::from(&lib.root_folder));
                }
                for dir in &app.all_dirs {
                    folders.insert(dir.clone());
                }
                for file in &app.all_files {
                    if let Some(parent) = file.parent() {
                        folders.insert(parent.to_path_buf());
                    }
                }

                egui::ScrollArea::vertical()
                    .id_source("move_modal_folder_scroll")
                    .max_height(200.0)
                    .show(ui, |ui| {
                        for folder in folders {
                            let mut label = folder.to_string_lossy().into_owned();
                            for lib in &app.content_libraries {
                                if let Ok(rel) = folder.strip_prefix(std::path::Path::new(&lib.root_folder)) {
                                    label = std::path::Path::new(&lib.name).join(rel).to_string_lossy().into_owned();
                                    break;
                                }
                            }
                            let display = label;
                            if ui
                                .selectable_label(
                                    app.selected_move_folder.as_ref() == Some(&folder),
                                    display,
                                )
                                .clicked()
                            {
                                app.selected_move_folder = Some(folder.clone());
                            }
                        }
                    });

                ui.horizontal(|ui| {
                    if ui.button("Ok").clicked() {
                        if let (Some(file), Some(folder)) = (&app.file_to_move, &app.selected_move_folder) {
                            if let Some(name) = file.file_name() {
                                let new_path = folder.join(name);
                                if let Err(e) = std::fs::rename(file, &new_path) {
                                    tracing::error!(
                                        name = "ui.file.move_failed",
                                        source = %file.display(),
                                        destination = %new_path.display(),
                                        error = %e,
                                        "Failed to move file to new destination. Likely cause: permission denied or file in use. Operator should check file locks."
                                    );
                                }
                            }
                        }
                        close_modal = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close_modal = true;
                    }
                });
            });

        if close_modal {
            app.move_dialog_open = false;
            app.file_to_move = None;
            app.selected_move_folder = None;
        }
    }
}

pub fn show_create_dir_modal(app: &mut FastMdApp, ctx: &egui::Context) {
    let mut close_create_modal = false;
    if app.create_dir_dialog_open {
        egui::Window::new("Create Directory")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Enter directory name:");
                let response = ui.text_edit_singleline(&mut app.create_dir_name);
                response.request_focus();

                let mut submit = false;
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit = true;
                }

                ui.horizontal(|ui| {
                    if ui.button("Ok").clicked() || submit {
                        if let Some(parent) = &app.create_dir_parent {
                            if !app.create_dir_name.trim().is_empty() {
                                let dir_name = app.create_dir_name.trim();
                                if Path::new(dir_name).components().any(|c| c == std::path::Component::ParentDir) || dir_name.contains('/') || dir_name.contains('\\') {
                                    tracing::warn!(
                                        name = "ui.directory.invalid_name",
                                        name_input = %dir_name,
                                        "User attempted to create directory with invalid characters. Operation skipped. Operator should advise user of valid names."
                                    );
                                } else {
                                    let new_dir_path = parent.join(dir_name);
                                    if let Err(e) = std::fs::create_dir_all(&new_dir_path) {
                                        tracing::error!(
                                            name = "ui.directory.create_failed",
                                            path = %new_dir_path.display(),
                                            error = %e,
                                            "Failed to create new directory. Likely cause: permission denied or invalid path. Operator should verify permissions on parent directory."
                                        );
                                    } else {
                                        if !app.all_dirs.contains(&new_dir_path) {
                                            app.all_dirs.push(new_dir_path.clone());
                                        }
                                        if let Some(watcher) = &mut app._watcher {
                                            use notify::Watcher;
                                            let _ = watcher.watch(&new_dir_path, notify::RecursiveMode::Recursive);
                                        }
                                    }
                                }
                            }
                        }
                        close_create_modal = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close_create_modal = true;
                    }
                });
            });

        if close_create_modal {
            app.create_dir_dialog_open = false;
            app.create_dir_parent = None;
            app.create_dir_name.clear();
        }
    }
}

pub fn show_rename_modal(app: &mut FastMdApp, ctx: &egui::Context) {
    let mut close_rename_modal = false;
    if app.rename_dialog_open {
        egui::Window::new("Rename")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Enter new name:");
                let response = ui.text_edit_singleline(&mut app.rename_new_name);
                response.request_focus();

                let mut submit = false;
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit = true;
                }

                ui.horizontal(|ui| {
                    if ui.button("Ok").clicked() || submit {
                        if let Some(file) = &app.file_to_rename {
                            if !app.rename_new_name.trim().is_empty() {
                                let new_name = app.rename_new_name.trim();
                                if Path::new(new_name).components().any(|c| c == std::path::Component::ParentDir) || new_name.contains('/') || new_name.contains('\\') {
                                    tracing::warn!(
                                        name = "ui.file.invalid_rename",
                                        name_input = %new_name,
                                        "User attempted to rename file with invalid characters. Operation skipped. Operator should advise user of valid names."
                                    );
                                } else {
                                    let mut new_path = file.clone();
                                    new_path.set_file_name(new_name);
                                    if let Err(e) = std::fs::rename(file, &new_path) {
                                        tracing::error!(
                                            name = "ui.file.rename_failed",
                                            source = %file.display(),
                                            destination = %new_path.display(),
                                            error = %e,
                                            "Failed to rename file. Likely cause: permission denied or file in use. Operator should check file locks."
                                        );
                                    } else {
                                        if app.loaded_path.as_ref() == Some(file) {
                                            app.loaded_path = Some(new_path.clone());
                                        }
                                        if app.selected_file.as_ref() == Some(file) {
                                            app.selected_file = Some(new_path.clone());
                                        }
                                        if app.selected_dir.as_ref() == Some(file) {
                                            app.selected_dir = Some(new_path.clone());
                                        }
                                        for i in 0..app.tabs.len() {
                                            if app.tabs[i] == *file {
                                                app.tabs[i] = new_path.clone();
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        close_rename_modal = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close_rename_modal = true;
                    }
                });
            });

        if close_rename_modal {
            app.rename_dialog_open = false;
            app.file_to_rename = None;
            app.rename_new_name.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Arc;

    fn create_test_app() -> FastMdApp {
        let (tx, rx) = std::sync::mpsc::channel();
        FastMdApp {
            content_libraries: vec![],
            rx,
            tx,
            all_files: vec![],
            all_dirs: vec![],
            file_tags: std::collections::BTreeMap::new(),
            all_tags: std::collections::BTreeSet::new(),
            selected_tag: None,
            indexing_finished: true,
            indexing_finished_handled: true,
            left_panel_width: None,
            selected_file: None,
            selected_files: std::collections::HashSet::new(),
            selected_dir: None,
            expanded_dirs: std::collections::HashSet::new(),
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
            background_manager: Arc::new(std::sync::Mutex::new(crate::background::BackgroundProcessManager::new())),
            show_background_logs: false,
            config: crate::config::AppConfig::default(),
        }
    }

    #[test]
    fn test_move_modal_rendering_and_state() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();

        // 1. Closed state
        show_move_modal(&mut app, &ctx);
        assert!(!app.move_dialog_open);

        // 2. Open state with content
        let temp_dir = std::env::temp_dir().join("fastmd_move_test");
        let dest_dir = temp_dir.join("dest");
        let _ = fs::create_dir_all(&dest_dir);

        let src_file = temp_dir.join("move_me.txt");
        let _ = fs::write(&src_file, "content");

        app.move_dialog_open = true;
        app.file_to_move = Some(src_file.clone());
        app.all_dirs.push(dest_dir.clone());
        app.selected_move_folder = Some(dest_dir.clone());

        let _ = ctx.run(Default::default(), |ctx| {
            show_move_modal(&mut app, ctx);
        });

        assert!(app.move_dialog_open);

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_create_dir_modal() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();

        let temp_dir = std::env::temp_dir().join("fastmd_create_dir_test");
        let _ = fs::create_dir_all(&temp_dir);

        // 1. Closed state
        show_create_dir_modal(&mut app, &ctx);
        assert!(!app.create_dir_dialog_open);

        // 2. Open state with parent
        app.create_dir_dialog_open = true;
        app.create_dir_parent = Some(temp_dir.clone());
        app.create_dir_name = "subfolder".to_string();

        let _ = ctx.run(Default::default(), |ctx| {
            show_create_dir_modal(&mut app, ctx);
        });

        assert!(app.create_dir_dialog_open);

        // 3. Test invalid directory name
        app.create_dir_name = "../invalid_traversal".to_string();
        let _ = ctx.run(Default::default(), |ctx| {
            show_create_dir_modal(&mut app, ctx);
        });

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_rename_modal() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();

        let temp_dir = std::env::temp_dir().join("fastmd_rename_test");
        let _ = fs::create_dir_all(&temp_dir);

        let file_path = temp_dir.join("old_name.txt");
        let _ = fs::write(&file_path, "sample text");

        // 1. Closed state
        show_rename_modal(&mut app, &ctx);
        assert!(!app.rename_dialog_open);

        // 2. Open state with valid rename target
        app.rename_dialog_open = true;
        app.file_to_rename = Some(file_path.clone());
        app.rename_new_name = "new_name.txt".to_string();
        app.selected_file = Some(file_path.clone());
        app.tabs = vec![file_path.clone()];

        let _ = ctx.run(Default::default(), |ctx| {
            show_rename_modal(&mut app, ctx);
        });

        assert!(app.rename_dialog_open);

        // 3. Invalid rename with slash
        app.rename_new_name = "invalid/name.txt".to_string();
        let _ = ctx.run(Default::default(), |ctx| {
            show_rename_modal(&mut app, ctx);
        });

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }
}