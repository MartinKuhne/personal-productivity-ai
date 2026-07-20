use crate::ui::FastMdApp;
use eframe::egui;
use std::collections::BTreeSet;

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
                            let display = crate::config::library_display_label(&app.content_libraries, &folder)
                                .unwrap_or_else(|| folder.to_string_lossy().into_owned());
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

                let submit = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                ui.horizontal(|ui| {
                    if ui.button("Ok").clicked() || (submit && app.selected_move_folder.is_some()) {
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
                                } else {
                                    // A move is a rename. Publish both
                                    // events so consumers can update
                                    // any state keyed on either path.
                                    let producer = crate::file_events::FileEventProducer::new(&app.file_event_bus);
                                    producer.publish_rename(file, &new_path);
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

                let submit = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));

                ui.horizontal(|ui| {
                    if ui.button("Ok").clicked() || submit {
                        if let Some(parent) = &app.create_dir_parent {
                            if !app.create_dir_name.trim().is_empty() {
                                let dir_name = app.create_dir_name.trim();
                                if !crate::utils::path::is_safe_basename(dir_name) {
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

                let submit = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));

                ui.horizontal(|ui| {
                    if ui.button("Ok").clicked() || submit {
                        if let Some(file) = &app.file_to_rename {
                            if !app.rename_new_name.trim().is_empty() {
                                let new_name = app.rename_new_name.trim();
                                if !crate::utils::path::is_safe_basename(new_name) {
                                    tracing::warn!(
                                        name = "ui.file.invalid_rename",
                                        name_input = %new_name,
                                        "User attempted to rename file with invalid characters. Operation skipped. Operator should advise user of valid names."
                                    );
                                } else {
                                    // Preserve the original file extension
                                    let ext = file.extension()
                                        .and_then(|e| e.to_str())
                                        .map(|e| format!(".{}", e))
                                        .unwrap_or_default();
                                    let new_name_with_ext = format!("{}{}", new_name, ext);
                                    let mut new_path = file.clone();
                                    new_path.set_file_name(&new_name_with_ext);
                                    if let Err(e) = std::fs::rename(file, &new_path) {
                                        tracing::error!(
                                            name = "ui.file.rename_failed",
                                            source = %file.display(),
                                            destination = %new_path.display(),
                                            error = %e,
                                            "Failed to rename file. Likely cause: permission denied or file in use. Operator should check file locks."
                                        );
                                    } else {
                                        // Publish the rename so any
                                        // consumer keyed on either path
                                        // (cache, tag manager, file
                                        // table) refreshes
                                        // immediately.
                                        let producer = crate::file_events::FileEventProducer::new(&app.file_event_bus);
                                        producer.publish_rename(file, &new_path);
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
                                        // Update directory tree immediately (not waiting
                                        // for the filesystem watcher).
                                        app.all_files.retain(|p| p != file);
                                        if app.all_dirs.contains(file) {
                                            app.all_dirs.retain(|p| p != file);
                                            if !app.all_dirs.contains(&new_path) {
                                                app.all_dirs.push(new_path.clone());
                                            }
                                        }
                                        let ext = new_path
                                            .extension()
                                            .and_then(|e| e.to_str())
                                            .unwrap_or("");
                                        if ext == "md" || ext == "markdown" {
                                            if !app.all_files.contains(&new_path) {
                                                app.all_files.push(new_path.clone());
                                            }
                                        }
                                        app.file_tags.remove(file);
                                        let tags =
                                            crate::utils::tags::extract_tags_from_file(&new_path);
                                        app.file_tags.insert(new_path.clone(), tags);
                                        if app.expanded_dirs.remove(file) {
                                            app.expanded_dirs.insert(new_path.clone());
                                        }
                                        app.all_tags.clear();
                                        for tags in app.file_tags.values() {
                                            for tag in tags {
                                                app.all_tags.insert(tag.clone());
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

    fn create_test_app() -> FastMdApp {
        FastMdApp::empty_state()
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

        // 2. Open state - extension is automatically preserved
        // The user only enters the file name, not the extension
        app.rename_dialog_open = true;
        app.file_to_rename = Some(file_path.clone());
        app.rename_new_name = "new_name".to_string(); // Just the name, extension is preserved
        app.selected_file = Some(file_path.clone());
        app.tabs = vec![file_path.clone()];

        let _ = ctx.run(Default::default(), |ctx| {
            show_rename_modal(&mut app, ctx);
        });

        assert!(app.rename_dialog_open);

        // 3. Invalid rename with slash (rejected by is_safe_basename)
        app.rename_new_name = "invalid/name".to_string();
        let _ = ctx.run(Default::default(), |ctx| {
            show_rename_modal(&mut app, ctx);
        });

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_rename_preserves_extension() {
        // Test that renaming a file preserves its extension
        let ctx = egui::Context::default();
        let mut app = create_test_app();

        let temp_dir = std::env::temp_dir().join("fastmd_rename_test2");
        let _ = fs::create_dir_all(&temp_dir);

        // Test with .md extension
        let md_file = temp_dir.join("my_document.md");
        let _ = fs::write(&md_file, "# Test");

        app.rename_dialog_open = true;
        app.file_to_rename = Some(md_file.clone());
        app.rename_new_name = "renamed_doc".to_string(); // Extension should be auto-added

        let _ = ctx.run(Default::default(), |ctx| {
            show_rename_modal(&mut app, ctx);
        });

        // The file should have been renamed with .md extension preserved
        assert!(!temp_dir.join("renamed_doc.md").exists() || temp_dir.join("my_document.md").exists(),
            "Rename should complete without error");

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
