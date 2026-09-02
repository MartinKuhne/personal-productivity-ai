//! Modal dialog UIs — move-file, create-directory, rename, delete confirmation, and batch prompt-processing dialogs.
//!
//! Unit tests live in the sibling `modals_tests.rs` sidecar.

use crate::config::ContentLibrary;
use crate::ui::dialogs::Dialogs;
use crate::workspace::watcher::FileEventProcessor;
use eframe::egui;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};



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
) -> Option<bool> {
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
                    action = Some(true);
                }
                if ui.button(crate::ui::strings::CANCEL_BUTTON).clicked() {
                    action = Some(false);
                }
            });
        });
    action
}

pub fn show_move_modal_dialog(
    dm: &mut Dialogs,
    content_libraries: &[ContentLibrary],
    file_processor: &FileEventProcessor,
    user_command_bus: &crate::bus::core::Bus<crate::bus::events::user_command::UserCommand>,
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
                            let display =
                                crate::config::library_display_label(content_libraries, &folder)
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

                let submit =
                    ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                ui.horizontal(|ui| {
                    if ui.button(crate::ui::strings::OK_BUTTON).clicked()
                        || (submit && dm.selected_move_folder.is_some())
                    {
                        if let (Some(file), Some(folder)) =
                            (&dm.file_to_move, &dm.selected_move_folder)
                        {
                            user_command_bus.publish(
                                crate::bus::events::user_command::UserCommand::ConfirmMove {
                                    file: file.clone(),
                                    destination: folder.clone(),
                                },
                            );
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
    dm: &mut Dialogs,
    user_command_bus: &crate::bus::core::Bus<crate::bus::events::user_command::UserCommand>,
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
        Some(true) => {
            if let Some(parent) = &dm.create_dir_parent
                && !dm.create_dir_name.trim().is_empty()
            {
                user_command_bus.publish(
                    crate::bus::events::user_command::UserCommand::ConfirmCreateDirectory {
                        parent: parent.clone(),
                        name: dm.create_dir_name.trim().to_string(),
                    },
                );
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
        Some(false) => {
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
    dm: &mut Dialogs,
    user_command_bus: &crate::bus::core::Bus<crate::bus::events::user_command::UserCommand>,
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
        Some(true) => {
            if let Some(parent) = &dm.create_document_parent
                && !dm.create_document_name.trim().is_empty()
            {
                user_command_bus.publish(
                    crate::bus::events::user_command::UserCommand::ConfirmCreateDocument {
                        parent: parent.clone(),
                        name: dm.create_document_name.trim().to_string(),
                    },
                );
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
        Some(false) => {
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
pub fn unique_document_name(entered: &str) -> String {
    if Path::new(entered).extension().is_some() {
        entered.to_owned()
    } else {
        format!("{}.md", entered)
    }
}

/// Write a new markdown document into `parent` using the
/// user-entered name (`.md` appended when the name has no extension).
/// If a file with the preferred name already exists, a
/// `<stem> <date-time><ext>` name is generated instead so the created
/// file is always unique. Returns the created file's path.
pub fn write_new_document(parent: &Path, entered: &str) -> std::io::Result<PathBuf> {
    let file_name = unique_document_name(entered);
    let mut new_path = parent.join(&file_name);
    if new_path.exists() {
        new_path = parent.join(date_suffixed_name(&file_name));
    }
    std::fs::write(&new_path, "")?;
    Ok(new_path)
}

/// Build `<stem> <timestamp><ext>` for a file name, used when the
/// preferred document name already exists.
pub fn date_suffixed_name(file_name: &str) -> String {
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
    pub dialogs: &'a mut Dialogs,
    pub user_command_bus: &'a crate::bus::core::Bus<crate::bus::events::user_command::UserCommand>,
    pub ctx: &'a egui::Context,
}

pub fn show_rename_dialog(ctx: RenameDialogCtx<'_>) {
    if !ctx.dialogs.rename_dialog_open {
        return;
    }
    let RenameDialogCtx {
        dialogs: dm,
        user_command_bus,
        ctx,
    } = ctx;
    let action = show_name_entry_window(
        ctx,
        crate::ui::strings::RENAME_WINDOW,
        crate::ui::strings::ENTER_NEW_NAME,
        &mut dm.rename_new_name,
    );
    match action {
        Some(true) => {
            if let Some(file) = &dm.file_to_rename
                && !dm.rename_new_name.trim().is_empty()
            {
                user_command_bus.publish(
                    crate::bus::events::user_command::UserCommand::ConfirmRename {
                        path: file.clone(),
                        new_name: dm.rename_new_name.trim().to_string(),
                    },
                );
            }
            dm.rename_dialog_open = false;
            dm.file_to_rename = None;
            dm.rename_new_name.clear();
            ctx.data_mut(|data| {
                data.remove_temp::<egui::Pos2>(egui::Id::new(crate::ui::strings::RENAME_WINDOW));
            });
        }
        Some(false) => {
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

// ---------------------------------------------------------------------------
// Tests live in the sibling `modals_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "modals_tests.rs"]
mod tests;
