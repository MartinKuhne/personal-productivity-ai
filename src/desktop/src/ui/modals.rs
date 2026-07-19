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
                                    eprintln!("Failed to move: {}", e);
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
                                    eprintln!("Invalid directory name");
                                } else {
                                    let new_dir_path = parent.join(dir_name);
                                    if let Err(e) = std::fs::create_dir_all(&new_dir_path) {
                                        eprintln!("Failed to create directory: {}", e);
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
                                    eprintln!("Invalid file name");
                                } else {
                                    let mut new_path = file.clone();
                                    new_path.set_file_name(new_name);
                                    if let Err(e) = std::fs::rename(file, &new_path) {
                                        eprintln!("Failed to rename: {}", e);
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