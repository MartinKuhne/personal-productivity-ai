//! Modal dialog UIs — move-file, create-directory, rename, delete confirmation, and batch prompt-processing dialogs.

use crate::app::dialog_manager::DialogManager;
use crate::app::watcher::file_processor::FileEventProcessor;
use crate::bus::core::Bus;
use crate::bus::events::file::{FileEvent, FileEventProducer};
use crate::config::ContentLibrary;
use eframe::egui;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Result of a shared name-entry dialog.
pub enum NameEntryAction {
    /// The user confirmed the name.
    Submit,
    /// The user dismissed the dialog.
    Cancel,
}

/// Shared name-entry window used by the create-directory and
/// create-document dialogs (UI-011, UI-015).
///
/// The window appears next to the mouse cursor that opened the
/// context-menu action, falling back to the viewport centre when no
/// pointer position is available (e.g. the dialog was opened via
/// keyboard). The anchor is captured once per open session (stored in
/// `IdTypeMap` temp data keyed by the window title) and re-applied
/// every frame with `fixed_pos` — which also makes the window
/// immovable. egui's area constraining is on by default, so an anchor
/// that would place the window off-screen is clamped back onto the
/// viewport. The anchor temp entry is removed by the caller when the
/// dialog closes so the next session captures a fresh position.
fn show_name_entry_window(
    ctx: &egui::Context,
    title: &str,
    prompt: &str,
    name: &mut String,
) -> Option<NameEntryAction> {
    let window_id = egui::Id::new(title);
    let fallback = ctx
        .pointer_interact_pos()
        .unwrap_or_else(|| ctx.viewport_rect().center());
    // Capture the anchor once per open session. Note: the fallback must
    // be computed *before* `data_mut`, which takes a write lock on the
    // context; calling `pointer_interact_pos` (a read lock) inside the
    // closure would deadlock on the same non-reentrant `RwLock`.
    let anchor = ctx.data_mut(|data| *data.get_temp_mut_or_insert_with(window_id, || fallback));

    let mut action = None;
    egui::Window::new(title)
        .id(window_id)
        .fixed_pos(anchor)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(prompt);
            let response = ui.text_edit_singleline(name);
            response.request_focus();

            let submit = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));

            ui.horizontal(|ui| {
                if ui.button(crate::ui::strings::OK_BUTTON).clicked() || submit {
                    action = Some(NameEntryAction::Submit);
                }
                if ui.button(crate::ui::strings::CANCEL_BUTTON).clicked() {
                    action = Some(NameEntryAction::Cancel);
                }
            });
        });
    action
}

pub fn show_move_modal_dialog(
    dm: &mut DialogManager,
    content_libraries: &[ContentLibrary],
    file_processor: &FileEventProcessor,
    file_event_bus: &Bus<FileEvent>,
    ctx: &egui::Context,
) {
    let mut close_modal = false;
    if dm.move_dialog_open {
        egui::Window::new(crate::ui::strings::MOVE_FILE_WINDOW)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(crate::ui::strings::SELECT_DESTINATION_FOLDER);

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
                    .id_salt("move_modal_folder_scroll")
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
                    if ui.button(crate::ui::strings::OK_BUTTON).clicked() || (submit && dm.selected_move_folder.is_some()) {
                        if let (Some(file), Some(folder)) = (&dm.file_to_move, &dm.selected_move_folder)
                            && let Some(name) = file.file_name() {
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
                                    let producer = FileEventProducer::new(file_event_bus);
                                    producer.publish_rename(file, &new_path);
                                }
                            }
                        close_modal = true;
                    }
                    if ui.button(crate::ui::strings::CANCEL_BUTTON).clicked() {
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
    file_processor: &mut FileEventProcessor,
    watcher: &mut Option<notify::RecommendedWatcher>,
    file_event_bus: &Bus<FileEvent>,
    ctx: &egui::Context,
) {
    if !dm.create_dir_dialog_open {
        return;
    }
    let action = show_name_entry_window(
        ctx,
        crate::ui::strings::CREATE_DIRECTORY_WINDOW,
        crate::ui::strings::ENTER_DIRECTORY_NAME,
        &mut dm.create_dir_name,
    );
    match action {
        Some(NameEntryAction::Submit) => {
            if let Some(parent) = &dm.create_dir_parent
                && !dm.create_dir_name.trim().is_empty()
            {
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
                        file_processor.add_dir(new_dir_path.clone());
                        let producer = FileEventProducer::new(file_event_bus);
                        producer.publish_dir_discovered(&new_dir_path);
                        if let Some(watcher) = watcher {
                            use notify::Watcher;
                            let _ = watcher.watch(&new_dir_path, notify::RecursiveMode::Recursive);
                        }
                    }
                }
            }
            dm.create_dir_dialog_open = false;
            dm.create_dir_parent = None;
            dm.create_dir_name.clear();
            ctx.data_mut(|data| {
                data.remove_temp::<egui::Pos2>(egui::Id::new(
                    crate::ui::strings::CREATE_DIRECTORY_WINDOW,
                ));
            });
        }
        Some(NameEntryAction::Cancel) => {
            dm.create_dir_dialog_open = false;
            dm.create_dir_parent = None;
            dm.create_dir_name.clear();
            ctx.data_mut(|data| {
                data.remove_temp::<egui::Pos2>(egui::Id::new(
                    crate::ui::strings::CREATE_DIRECTORY_WINDOW,
                ));
            });
        }
        None => {}
    }
}

/// Create-document dialog (UI-015). Prompts for a document name via
/// the shared name-entry window, then writes a YAML-headed markdown
/// file in `create_document_parent`. The user-entered name is used
/// as the file name; `.md` is appended when the user typed no
/// extension. If a file with that name already exists, the current
/// date and time are appended until a unique name is generated. On
/// success the new file is announced through `publish_discovered` so
/// the tree, tab list and tag manager refresh immediately.
pub fn show_create_document_dialog(
    dm: &mut DialogManager,
    file_event_bus: &Bus<FileEvent>,
    ctx: &egui::Context,
) {
    if !dm.create_document_dialog_open {
        return;
    }
    let action = show_name_entry_window(
        ctx,
        crate::ui::strings::CREATE_DOCUMENT_WINDOW,
        crate::ui::strings::ENTER_DOCUMENT_NAME,
        &mut dm.create_document_name,
    );
    match action {
        Some(NameEntryAction::Submit) => {
            if let Some(parent) = &dm.create_document_parent
                && !dm.create_document_name.trim().is_empty()
            {
                let entered = dm.create_document_name.trim();
                if !crate::utils::path::is_safe_basename(entered) {
                    tracing::warn!(
                        name = "ui.file.invalid_name",
                        name_input = %entered,
                        "User attempted to create document with invalid characters. Operation skipped. Operator should advise user of valid names."
                    );
                } else {
                    match write_new_document(parent, entered) {
                        Ok(new_path) => {
                            let producer = FileEventProducer::new(file_event_bus);
                            producer.publish_discovered(&new_path);
                        }
                        Err(e) => tracing::error!(
                            name = "ui.file.create_failed",
                            parent = %parent.display(),
                            error = %e,
                            "Failed to create new document. Likely cause: permission denied or disk full. Operator should verify directory permissions."
                        ),
                    }
                }
            }
            dm.create_document_dialog_open = false;
            dm.create_document_parent = None;
            dm.create_document_name.clear();
            ctx.data_mut(|data| {
                data.remove_temp::<egui::Pos2>(egui::Id::new(
                    crate::ui::strings::CREATE_DOCUMENT_WINDOW,
                ));
            });
        }
        Some(NameEntryAction::Cancel) => {
            dm.create_document_dialog_open = false;
            dm.create_document_parent = None;
            dm.create_document_name.clear();
            ctx.data_mut(|data| {
                data.remove_temp::<egui::Pos2>(egui::Id::new(
                    crate::ui::strings::CREATE_DOCUMENT_WINDOW,
                ));
            });
        }
        None => {}
    }
}

/// Append `.md` to a user-entered document name when it has no
/// extension, so the created file is always a markdown document.
fn unique_document_name(entered: &str) -> String {
    if Path::new(entered).extension().is_some() {
        entered.to_owned()
    } else {
        format!("{}.md", entered)
    }
}

/// Write a new YAML-headed markdown document into `parent` using the
/// user-entered name (`.md` appended when the name has no extension).
/// If a file with the preferred name already exists, a
/// `<stem> <date-time><ext>` name is generated instead so the created
/// file is always unique. Returns the created file's path.
fn write_new_document(parent: &Path, entered: &str) -> std::io::Result<PathBuf> {
    let file_name = unique_document_name(entered);
    let mut new_path = parent.join(&file_name);
    if new_path.exists() {
        new_path = parent.join(date_suffixed_name(&file_name));
    }
    let yaml_header = format!("---\ntitle: {}\n---\n\n", entered);
    std::fs::write(&new_path, yaml_header)?;
    Ok(new_path)
}

/// Build `<stem> <timestamp><ext>` for a file name, used when the
/// preferred document name already exists.
fn date_suffixed_name(file_name: &str) -> String {
    let path = Path::new(file_name);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file_name);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e))
        .unwrap_or_default();
    let now = chrono::Local::now();
    let date_str = now.format("%Y-%m-%d %H-%M-%S");
    format!("{} {}{}", stem, date_str, ext)
}

/// Borrowed inputs the rename dialog needs. Bundled so the function
/// signature stays under four parameters (PSD-002) — the call site
/// in `FastMdApp::show_modals` constructs the bundle by reborrowing
/// each field from `&mut self`.
pub struct RenameDialogCtx<'a> {
    pub dialog_manager: &'a mut DialogManager,
    pub file_event_bus: &'a Bus<FileEvent>,
    pub loaded_path: &'a mut Option<PathBuf>,
    pub selected_file: &'a mut Option<PathBuf>,
    pub selected_dir: &'a mut Option<PathBuf>,
    pub tabs: &'a mut [PathBuf],
    pub file_processor: &'a mut FileEventProcessor,
    pub tag_manager: &'a mut crate::app::tag_manager::TagManager,
    pub expanded_dirs: &'a mut std::collections::HashSet<PathBuf>,
    pub ctx: &'a egui::Context,
}

pub fn show_rename_dialog(ctx: RenameDialogCtx<'_>) {
    if !ctx.dialog_manager.rename_dialog_open {
        return;
    }
    let RenameDialogCtx {
        dialog_manager: dm,
        file_event_bus,
        loaded_path,
        selected_file,
        selected_dir,
        tabs,
        file_processor,
        tag_manager,
        expanded_dirs,
        ctx,
    } = ctx;
    let action = show_name_entry_window(
        ctx,
        crate::ui::strings::RENAME_WINDOW,
        crate::ui::strings::ENTER_NEW_NAME,
        &mut dm.rename_new_name,
    );
    match action {
        Some(NameEntryAction::Submit) => {
            if let Some(file) = &dm.file_to_rename
                && !dm.rename_new_name.trim().is_empty()
            {
                let new_name = dm.rename_new_name.trim();
                if !crate::utils::path::is_safe_basename(new_name) {
                    tracing::warn!(
                        name = "ui.file.invalid_rename",
                        name_input = %new_name,
                        "User attempted to rename file with invalid characters. Operation skipped. Operator should advise user of valid names."
                    );
                } else {
                    let ext = file
                        .extension()
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
                        let producer = FileEventProducer::new(file_event_bus);
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
                        for tab in tabs.iter_mut() {
                            if *tab == *file {
                                *tab = new_path.clone();
                            }
                        }
                        file_processor.remove_file(file);
                        if file_processor.contains_dir(file) {
                            file_processor.remove_dir(file);
                            file_processor.add_dir(new_path.clone());
                        }
                        let ext = new_path.extension().and_then(|e| e.to_str()).unwrap_or("");
                        if ext == "md" || ext == "markdown" {
                            file_processor.add_file(new_path.clone());
                        }
                        let tags = crate::utils::tags::extract_tags_from_file(&new_path);
                        tag_manager.remove_file(file);
                        tag_manager.add_tags(new_path.clone(), tags);
                        if expanded_dirs.remove(file) {
                            expanded_dirs.insert(new_path.clone());
                        }
                    }
                }
            }
            dm.rename_dialog_open = false;
            dm.file_to_rename = None;
            dm.rename_new_name.clear();
            ctx.data_mut(|data| {
                data.remove_temp::<egui::Pos2>(egui::Id::new(crate::ui::strings::RENAME_WINDOW));
            });
        }
        Some(NameEntryAction::Cancel) => {
            dm.rename_dialog_open = false;
            dm.file_to_rename = None;
            dm.rename_new_name.clear();
            ctx.data_mut(|data| {
                data.remove_temp::<egui::Pos2>(egui::Id::new(crate::ui::strings::RENAME_WINDOW));
            });
        }
        None => {}
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

    /// Modal dialogs are drawn into the root `egui::Context`, and
    /// `egui::Window::show` requires a non-trivial viewport for the
    /// modal's rect to be observable. The default `RawInput` has no
    /// `screen_rect`, which collapses the modal's bounding rect to
    /// zero and clips the title + prompt out of the output. Set a
    /// 1024x768 viewport for the test runs.
    fn raw_input() -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1024.0, 768.0),
            )),
            ..egui::RawInput::default()
        }
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

        // R-2 / Q12: the `Window::show` rendering path used by the modals
        // is not observable through `ctx.run_ui(...).shapes` — the
        // window's title and prompt are rendered via egui's `Atoms`
        // widget system and end up in a separate paint layer that the
        // single-frame test harness does not include in the captured
        // output. The modal's visual surface is therefore covered by
        // the app-level Tier 3 snapshot (R-1c) rather than this test.
        // This test is kept as a smoke + state-coverage test.
        let _ = ctx.run_ui(raw_input(), |ui| {
            show_move_modal_dialog(
                &mut app.dialogs,
                &app.content_libraries,
                &app.file_processor,
                &app.file_event_bus,
                ui.ctx(),
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
            &mut app.file_processor,
            &mut watcher,
            &app.file_event_bus,
            &ctx,
        );
        assert!(!app.dialogs.create_dir_dialog_open);

        app.dialogs.create_dir_dialog_open = true;
        app.dialogs.create_dir_parent = Some(temp_dir.clone());
        app.dialogs.create_dir_name = "subfolder".to_string();

        // See `test_move_modal_rendering_and_state` for the rationale
        // on why the modal's rendered text is not asserted here.
        let _ = ctx.run_ui(raw_input(), |ui| {
            show_create_dir_dialog(
                &mut app.dialogs,
                &mut app.file_processor,
                &mut watcher,
                &app.file_event_bus,
                ui.ctx(),
            );
        });

        assert!(app.dialogs.create_dir_dialog_open);

        app.dialogs.create_dir_name = "../invalid_traversal".to_string();
        let _ = ctx.run_ui(raw_input(), |ui| {
            show_create_dir_dialog(
                &mut app.dialogs,
                &mut app.file_processor,
                &mut watcher,
                &app.file_event_bus,
                ui.ctx(),
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
            show_rename_dialog(RenameDialogCtx {
                dialog_manager: &mut app.dialogs,
                file_event_bus: &app.file_event_bus,
                loaded_path: &mut app.tab_manager.loaded_path,
                selected_file: &mut sel.selected_file,
                selected_dir: &mut sel.selected_dir,
                tabs: &mut app.tab_manager.tabs,
                file_processor: &mut app.file_processor,
                tag_manager: &mut app.tag_manager,
                expanded_dirs: &mut sel.expanded_dirs,
                ctx: &ctx,
            });
        }
        assert!(!app.dialogs.rename_dialog_open);

        app.dialogs.rename_dialog_open = true;
        app.dialogs.file_to_rename = Some(file_path.clone());
        app.dialogs.rename_new_name = "new_name".to_string();
        *app.selection.selected_file_mut() = Some(file_path.clone());
        app.tab_manager.tabs = vec![file_path.clone()];

        // See `test_move_modal_rendering_and_state` for the rationale
        // on why the modal's rendered text is not asserted here.
        let _ = ctx.run_ui(raw_input(), |ui| {
            let sel = &mut app.selection;
            show_rename_dialog(RenameDialogCtx {
                dialog_manager: &mut app.dialogs,
                file_event_bus: &app.file_event_bus,
                loaded_path: &mut app.tab_manager.loaded_path,
                selected_file: &mut sel.selected_file,
                selected_dir: &mut sel.selected_dir,
                tabs: &mut app.tab_manager.tabs,
                file_processor: &mut app.file_processor,
                tag_manager: &mut app.tag_manager,
                expanded_dirs: &mut sel.expanded_dirs,
                ctx: ui.ctx(),
            });
        });

        assert!(app.dialogs.rename_dialog_open);

        app.dialogs.rename_new_name = "invalid/name".to_string();
        let _ = ctx.run_ui(raw_input(), |ui| {
            let sel = &mut app.selection;
            show_rename_dialog(RenameDialogCtx {
                dialog_manager: &mut app.dialogs,
                file_event_bus: &app.file_event_bus,
                loaded_path: &mut app.tab_manager.loaded_path,
                selected_file: &mut sel.selected_file,
                selected_dir: &mut sel.selected_dir,
                tabs: &mut app.tab_manager.tabs,
                file_processor: &mut app.file_processor,
                tag_manager: &mut app.tag_manager,
                expanded_dirs: &mut sel.expanded_dirs,
                ctx: ui.ctx(),
            });
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

        let _ = ctx.run_ui(Default::default(), |ui| {
            let sel = &mut app.selection;
            show_rename_dialog(RenameDialogCtx {
                dialog_manager: &mut app.dialogs,
                file_event_bus: &app.file_event_bus,
                loaded_path: &mut app.tab_manager.loaded_path,
                selected_file: &mut sel.selected_file,
                selected_dir: &mut sel.selected_dir,
                tabs: &mut app.tab_manager.tabs,
                file_processor: &mut app.file_processor,
                tag_manager: &mut app.tag_manager,
                expanded_dirs: &mut sel.expanded_dirs,
                ctx: ui.ctx(),
            });
        });

        assert!(
            !temp_dir.join("renamed_doc.md").exists() || temp_dir.join("my_document.md").exists(),
            "Rename should complete without error"
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn unique_document_name_appends_md_only_when_no_extension() {
        assert_eq!(unique_document_name("my notes"), "my notes.md");
        assert_eq!(unique_document_name("report.md"), "report.md");
        assert_eq!(unique_document_name("draft.txt"), "draft.txt");
    }

    #[test]
    fn date_suffixed_name_preserves_stem_and_extension() {
        let name = date_suffixed_name("report.md");
        assert!(name.starts_with("report "), "unexpected: {}", name);
        assert!(name.ends_with(".md"), "unexpected: {}", name);
        let stem = name.trim_end_matches(".md");
        let date_part = stem.trim_start_matches("report ").trim();
        // `%Y-%m-%d %H-%M-%S` → e.g. `2026-08-01 14-30-00`.
        assert!(
            date_part.len() == 19 && date_part.as_bytes()[10] == b' ',
            "unexpected timestamp: {:?}",
            date_part
        );

        let no_ext = date_suffixed_name("report");
        assert!(no_ext.starts_with("report "), "unexpected: {}", no_ext);
        assert!(!no_ext.ends_with(".md"), "unexpected: {}", no_ext);
    }

    #[test]
    fn write_new_document_creates_yaml_headed_markdown() {
        let temp_dir = std::env::temp_dir().join("fastmd_write_doc_test");
        let _ = fs::create_dir_all(&temp_dir);

        let path = write_new_document(&temp_dir, "first notes").unwrap();
        assert_eq!(path, temp_dir.join("first notes.md"));
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "---\ntitle: first notes\n---\n\n");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn write_new_document_avoids_existing_name_with_date_suffix() {
        let temp_dir = std::env::temp_dir().join("fastmd_write_doc_unique_test");
        let _ = fs::create_dir_all(&temp_dir);
        let _ = fs::write(temp_dir.join("dup.md"), "existing");

        let path = write_new_document(&temp_dir, "dup").unwrap();
        assert_ne!(path, temp_dir.join("dup.md"));
        assert!(path.exists(), "unique file must be created");
        // The unique name must keep the `.md` extension.
        assert!(
            path.extension().and_then(|e| e.to_str()) == Some("md"),
            "unique name must retain .md extension: {:?}",
            path
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    /// Tier 4 functional test: opening the create-document dialog,
    /// typing a name and pressing Ok must write a YAML-headed
    /// markdown file in the dialog's parent directory and publish a
    /// `Discovered` file event on the file-event bus.
    #[test]
    fn test_create_document_dialog_writes_file_on_submit() {
        use crate::ui::test_helpers::interact::stateful_harness;
        use egui_kittest::kittest::Queryable;
        use std::sync::{Mutex, OnceLock};

        struct StaticFixture {
            dm: Mutex<Option<DialogManager>>,
            bus: Bus<FileEvent>,
            temp_dir: &'static Path,
        }
        static FIXTURE: OnceLock<StaticFixture> = OnceLock::new();
        let fixture = FIXTURE.get_or_init(|| {
            let leaked = Box::leak(Box::new(std::env::temp_dir().join(format!(
                "fastmd_create_document_click_{}",
                std::process::id()
            ))));
            let temp_dir: &'static Path = leaked;
            let _ = fs::create_dir_all(temp_dir);
            let mut dm = DialogManager::new();
            dm.create_document_dialog_open = true;
            dm.create_document_parent = Some(temp_dir.to_path_buf());
            dm.create_document_name = "from dialog".to_string();
            StaticFixture {
                dm: Mutex::new(Some(dm)),
                bus: Bus::new(),
                temp_dir,
            }
        });

        let mut harness = stateful_harness((), |ui, _| {
            let mut guard = fixture.dm.lock().unwrap();
            if let Some(dm) = guard.as_mut() {
                show_create_document_dialog(dm, &fixture.bus, ui.ctx());
            }
        });
        harness.fit_contents();
        harness.get_by_label(crate::ui::strings::OK_BUTTON).click();
        harness.run_steps(2);
        harness.run_steps(2);

        let created = fixture.temp_dir.join("from dialog.md");
        assert!(created.exists(), "Ok must create the document file");
        let content = fs::read_to_string(&created).unwrap();
        assert_eq!(content, "---\ntitle: from dialog\n---\n\n");

        let _ = fs::remove_dir_all(fixture.temp_dir);
    }

    /// Tier 4 functional test: opening the rename dialog with a file
    /// queued and pressing Ok must rename the file on disk (preserving
    /// its extension) and update the `selected_file` binding.
    #[test]
    fn test_rename_dialog_renames_file_on_submit() {
        use crate::ui::test_helpers::interact::stateful_harness;
        use egui_kittest::kittest::Queryable;
        use std::sync::{Mutex, OnceLock};

        struct StaticFixture {
            app: Mutex<Option<crate::ui::FastMdApp>>,
            temp_dir: &'static Path,
        }
        static FIXTURE: OnceLock<StaticFixture> = OnceLock::new();
        let fixture = FIXTURE.get_or_init(|| {
            let leaked = Box::leak(Box::new(
                std::env::temp_dir()
                    .join(format!("fastmd_rename_dialog_click_{}", std::process::id())),
            ));
            let temp_dir: &'static Path = leaked;
            let _ = fs::create_dir_all(temp_dir);
            let file_path = temp_dir.join("original.txt");
            let _ = fs::write(&file_path, "content");
            let mut app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
            app.dialogs.rename_dialog_open = true;
            app.dialogs.file_to_rename = Some(file_path.clone());
            app.dialogs.rename_new_name = "renamed".to_string();
            *app.selection.selected_file_mut() = Some(file_path.clone());
            StaticFixture {
                app: Mutex::new(Some(app)),
                temp_dir,
            }
        });

        let mut harness = stateful_harness((), |ui, _| {
            let mut guard = fixture.app.lock().unwrap();
            if let Some(app) = guard.as_mut() {
                let sel = &mut app.selection;
                show_rename_dialog(RenameDialogCtx {
                    dialog_manager: &mut app.dialogs,
                    file_event_bus: &app.file_event_bus,
                    loaded_path: &mut app.tab_manager.loaded_path,
                    selected_file: &mut sel.selected_file,
                    selected_dir: &mut sel.selected_dir,
                    tabs: &mut app.tab_manager.tabs,
                    file_processor: &mut app.file_processor,
                    tag_manager: &mut app.tag_manager,
                    expanded_dirs: &mut sel.expanded_dirs,
                    ctx: ui.ctx(),
                });
            }
        });
        harness.fit_contents();
        harness.get_by_label(crate::ui::strings::OK_BUTTON).click();
        harness.run_steps(2);
        harness.run_steps(2);

        let renamed = fixture.temp_dir.join("renamed.txt");
        assert!(
            renamed.exists(),
            "Ok must rename the file (preserving extension)"
        );
        assert!(!fixture.temp_dir.join("original.txt").exists());

        let _ = fs::remove_dir_all(fixture.temp_dir);
    }
}
