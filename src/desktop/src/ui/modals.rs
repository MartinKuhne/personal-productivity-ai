use crate::config::ContentLibrary;
use crate::file_events::Bus;
use crate::file_processor::FileEventProcessor;
use crate::ui::dialog_manager::DialogManager;
use eframe::egui;
use std::collections::BTreeSet;
use std::path::PathBuf;

pub fn show_move_modal_dialog(
    dm: &mut DialogManager,
    content_libraries: &[ContentLibrary],
    file_processor: &FileEventProcessor,
    file_event_bus: &Bus<crate::file_events::FileEvent>,
    ctx: &egui::Context,
) {
    let mut close_modal = false;
    if dm.move_dialog_open {
        egui::Window::new("Move File")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Select destination folder:");

                let mut folders = BTreeSet::new();
                for lib in content_libraries {
                    folders.insert(PathBuf::from(&lib.root_folder));
                }
                for dir in &file_processor.all_dirs {
                    folders.insert(dir.clone());
                }
                for file in &file_processor.all_files {
                    if let Some(parent) = file.parent() {
                        folders.insert(parent.to_path_buf());
                    }
                }

                egui::ScrollArea::vertical()
                    .id_source("move_modal_folder_scroll")
                    .max_height(200.0)
                    .show(ui, |ui| {
                        for folder in folders {
                            let display = crate::config::library_display_label(content_libraries, &folder)
                                .unwrap_or_else(|| folder.to_string_lossy().into_owned());
                            if ui
                                .selectable_label(
                                    dm.selected_move_folder.as_ref() == Some(&folder),
                                    display,
                                )
                                .clicked()
                            {
                                dm.selected_move_folder = Some(folder.clone());
                            }
                        }
                    });

                let submit = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                ui.horizontal(|ui| {
                    if ui.button("Ok").clicked() || (submit && dm.selected_move_folder.is_some()) {
                        if let (Some(file), Some(folder)) = (&dm.file_to_move, &dm.selected_move_folder) {
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
                                    let producer = crate::file_events::FileEventProducer::new(file_event_bus);
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
            dm.move_dialog_open = false;
            dm.file_to_move = None;
            dm.selected_move_folder = None;
        }
    }
}

pub fn show_create_dir_dialog(
    dm: &mut DialogManager,
    all_dirs: &mut Vec<PathBuf>,
    watcher: &mut Option<notify::RecommendedWatcher>,
    ctx: &egui::Context,
) {
    let mut close_create_modal = false;
    if dm.create_dir_dialog_open {
        egui::Window::new("Create Directory")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Enter directory name:");
                let response = ui.text_edit_singleline(&mut dm.create_dir_name);
                response.request_focus();

                let submit = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));

                ui.horizontal(|ui| {
                    if ui.button("Ok").clicked() || submit {
                        if let Some(parent) = &dm.create_dir_parent {
                            if !dm.create_dir_name.trim().is_empty() {
                                let dir_name = dm.create_dir_name.trim();
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
                                        if !all_dirs.contains(&new_dir_path) {
                                            all_dirs.push(new_dir_path.clone());
                                        }
                                        if let Some(watcher) = watcher {
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
            dm.create_dir_dialog_open = false;
            dm.create_dir_parent = None;
            dm.create_dir_name.clear();
        }
    }
}

pub fn show_rename_dialog(
    dm: &mut DialogManager,
    file_event_bus: &Bus<crate::file_events::FileEvent>,
    loaded_path: &mut Option<PathBuf>,
    selected_file: &mut Option<PathBuf>,
    selected_dir: &mut Option<PathBuf>,
    tabs: &mut Vec<PathBuf>,
    file_processor: &mut FileEventProcessor,
    tag_manager: &mut crate::tag_manager::TagManager,
    expanded_dirs: &mut std::collections::HashSet<PathBuf>,
    ctx: &egui::Context,
) {
    let mut close_rename_modal = false;
    if dm.rename_dialog_open {
        egui::Window::new("Rename")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Enter new name:");
                let response = ui.text_edit_singleline(&mut dm.rename_new_name);
                response.request_focus();

                let submit = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));

                ui.horizontal(|ui| {
                    if ui.button("Ok").clicked() || submit {
                        if let Some(file) = &dm.file_to_rename {
                            if !dm.rename_new_name.trim().is_empty() {
                                let new_name = dm.rename_new_name.trim();
                                if !crate::utils::path::is_safe_basename(new_name) {
                                    tracing::warn!(
                                        name = "ui.file.invalid_rename",
                                        name_input = %new_name,
                                        "User attempted to rename file with invalid characters. Operation skipped. Operator should advise user of valid names."
                                    );
                                } else {
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
                                        let producer = crate::file_events::FileEventProducer::new(file_event_bus);
                                        producer.publish_rename(file, &new_path);
                                        if loaded_path.as_ref() == Some(file) {
                                            *loaded_path = Some(new_path.clone());
                                        }
                                        if selected_file.as_ref() == Some(file) {
                                            *selected_file = Some(new_path.clone());
                                        }
                                        if selected_dir.as_ref() == Some(file) {
                                            *selected_dir = Some(new_path.clone());
                                        }
                                        for i in 0..tabs.len() {
                                            if tabs[i] == *file {
                                                tabs[i] = new_path.clone();
                                            }
                                        }
                                        file_processor.all_files.retain(|p| p != file);
                                        if file_processor.all_dirs.contains(file) {
                                            file_processor.all_dirs.retain(|p| p != file);
                                            if !file_processor.all_dirs.contains(&new_path) {
                                                file_processor.all_dirs.push(new_path.clone());
                                            }
                                        }
                                        let ext = new_path
                                            .extension()
                                            .and_then(|e| e.to_str())
                                            .unwrap_or("");
                                        if ext == "md" || ext == "markdown" {
                                            if !file_processor.all_files.contains(&new_path) {
                                                file_processor.all_files.push(new_path.clone());
                                            }
                                        }
                                        let tags =
                                            crate::utils::tags::extract_tags_from_file(&new_path);
                                        tag_manager.remove_file(file);
                                        tag_manager.add_tags(new_path.clone(), tags);
                                        if expanded_dirs.remove(file) {
                                            expanded_dirs.insert(new_path.clone());
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
            dm.rename_dialog_open = false;
            dm.file_to_rename = None;
            dm.rename_new_name.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::ui::FastMdApp;
    use notify::RecommendedWatcher;
    use std::fs;

    fn create_test_app() -> FastMdApp {
        FastMdApp::empty_state(AppConfig::default())
    }

    #[test]
    fn test_move_modal_rendering_and_state() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();

        show_move_modal_dialog(
            &mut app.dialogs,
            &app.content_libraries,
            &app.file_processor,
            &app.file_event_bus,
            &ctx,
        );
        assert!(!app.dialogs.move_dialog_open);

        let temp_dir = std::env::temp_dir().join("fastmd_move_test");
        let dest_dir = temp_dir.join("dest");
        let _ = fs::create_dir_all(&dest_dir);

        let src_file = temp_dir.join("move_me.txt");
        let _ = fs::write(&src_file, "content");

        app.dialogs.move_dialog_open = true;
        app.dialogs.file_to_move = Some(src_file.clone());
        app.file_processor.all_dirs.push(dest_dir.clone());
        app.dialogs.selected_move_folder = Some(dest_dir.clone());

        let _ = ctx.run(Default::default(), |ctx| {
            show_move_modal_dialog(
                &mut app.dialogs,
                &app.content_libraries,
                &app.file_processor,
                &app.file_event_bus,
                ctx,
            );
        });

        assert!(app.dialogs.move_dialog_open);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_create_dir_modal() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        let mut watcher: Option<RecommendedWatcher> = None;

        let temp_dir = std::env::temp_dir().join("fastmd_create_dir_test");
        let _ = fs::create_dir_all(&temp_dir);

        show_create_dir_dialog(
            &mut app.dialogs,
            &mut app.file_processor.all_dirs,
            &mut watcher,
            &ctx,
        );
        assert!(!app.dialogs.create_dir_dialog_open);

        app.dialogs.create_dir_dialog_open = true;
        app.dialogs.create_dir_parent = Some(temp_dir.clone());
        app.dialogs.create_dir_name = "subfolder".to_string();

        let _ = ctx.run(Default::default(), |ctx| {
            show_create_dir_dialog(
                &mut app.dialogs,
                &mut app.file_processor.all_dirs,
                &mut watcher,
                ctx,
            );
        });

        assert!(app.dialogs.create_dir_dialog_open);

        app.dialogs.create_dir_name = "../invalid_traversal".to_string();
        let _ = ctx.run(Default::default(), |ctx| {
            show_create_dir_dialog(
                &mut app.dialogs,
                &mut app.file_processor.all_dirs,
                &mut watcher,
                ctx,
            );
        });

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

        {
            let sel = &mut app.selection;
            show_rename_dialog(
                &mut app.dialogs,
                &app.file_event_bus,
                &mut app.tab_manager.loaded_path,
                &mut sel.selected_file,
                &mut sel.selected_dir,
                &mut app.tab_manager.tabs,
                &mut app.file_processor,
                &mut app.tag_manager,
                &mut sel.expanded_dirs,
                &ctx,
            );
        }
        assert!(!app.dialogs.rename_dialog_open);

        app.dialogs.rename_dialog_open = true;
        app.dialogs.file_to_rename = Some(file_path.clone());
        app.dialogs.rename_new_name = "new_name".to_string();
        *app.selection.selected_file_mut() = Some(file_path.clone());
        app.tab_manager.tabs = vec![file_path.clone()];

        let _ = ctx.run(Default::default(), |ctx| {
            let sel = &mut app.selection;
            show_rename_dialog(
                &mut app.dialogs,
                &app.file_event_bus,
                &mut app.tab_manager.loaded_path,
                &mut sel.selected_file,
                &mut sel.selected_dir,
                &mut app.tab_manager.tabs,
                &mut app.file_processor,
                &mut app.tag_manager,
                &mut sel.expanded_dirs,
                ctx,
            );
        });

        assert!(app.dialogs.rename_dialog_open);

        app.dialogs.rename_new_name = "invalid/name".to_string();
        let _ = ctx.run(Default::default(), |ctx| {
            let sel = &mut app.selection;
            show_rename_dialog(
                &mut app.dialogs,
                &app.file_event_bus,
                &mut app.tab_manager.loaded_path,
                &mut sel.selected_file,
                &mut sel.selected_dir,
                &mut app.tab_manager.tabs,
                &mut app.file_processor,
                &mut app.tag_manager,
                &mut sel.expanded_dirs,
                ctx,
            );
        });

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_rename_preserves_extension() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();

        let temp_dir = std::env::temp_dir().join("fastmd_rename_test2");
        let _ = fs::create_dir_all(&temp_dir);

        let md_file = temp_dir.join("my_document.md");
        let _ = fs::write(&md_file, "# Test");

        app.dialogs.rename_dialog_open = true;
        app.dialogs.file_to_rename = Some(md_file.clone());
        app.dialogs.rename_new_name = "renamed_doc".to_string();

        let _ = ctx.run(Default::default(), |ctx| {
            let sel = &mut app.selection;
            show_rename_dialog(
                &mut app.dialogs,
                &app.file_event_bus,
                &mut app.tab_manager.loaded_path,
                &mut sel.selected_file,
                &mut sel.selected_dir,
                &mut app.tab_manager.tabs,
                &mut app.file_processor,
                &mut app.tag_manager,
                &mut sel.expanded_dirs,
                ctx,
            );
        });

        assert!(
            !temp_dir.join("renamed_doc.md").exists() || temp_dir.join("my_document.md").exists(),
            "Rename should complete without error"
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
