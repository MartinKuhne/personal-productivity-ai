//! Row-drawing functions for the virtual flat-tree and the legacy recursive tree.
//!
//! Unit tests live in the sibling `render_tests.rs` sidecar.

use super::context::TreeNodeContext;
use super::flatten::FlatRow;
use super::handlers::{apply_directory_row_click, apply_file_row_click};
use crate::export::pdf::{SaveAsPdfJob, execute_save_as_pdf_blocking};
use crate::export::print::{PrintJob, execute_print_blocking};
use crate::ui::TreeNode;
use crate::ui::theme::PaletteExt;
use crate::ui::widgets::{InteractionSink, menu_item, no_op_sink};
use eframe::egui;
use std::collections::HashSet;

/// Show context menu for a directory row.
/// Used by both `render_flat_row` and `draw_tree_node`.
fn show_dir_context_menu(
    ui: &mut egui::Ui,
    path: &std::path::Path,
    _name: &str,
    ctx: &mut TreeNodeContext,
) {
    if menu_item(ui, crate::ui::strings::SHOW_IN_EXPLORER_ACTION) {
        ctx.user_command_bus.publish(
            crate::bus::events::user_command::UserCommand::ShowInExplorer(path.to_path_buf()),
        );
    }
    if menu_item(ui, crate::ui::strings::COPY_PATH_ACTION) {
        ctx.user_command_bus
            .publish(crate::bus::events::user_command::UserCommand::CopyPath(
                path.to_path_buf(),
            ));
    }
    if menu_item(ui, crate::ui::strings::RENAME_ACTION) {
        ctx.user_command_bus
            .publish(crate::bus::events::user_command::UserCommand::Rename(
                path.to_path_buf(),
            ));
    }
    if menu_item(ui, crate::ui::strings::MOVE_ACTION) {
        ctx.user_command_bus
            .publish(crate::bus::events::user_command::UserCommand::Move(
                path.to_path_buf(),
            ));
    }
    if menu_item(ui, crate::ui::strings::CREATE_DIRECTORY_ACTION) {
        ctx.user_command_bus.publish(
            crate::bus::events::user_command::UserCommand::CreateDirectory {
                parent: path.to_path_buf(),
            },
        );
    }
    if menu_item(ui, crate::ui::strings::NEW_DOCUMENT_ACTION) {
        ctx.user_command_bus.publish(
            crate::bus::events::user_command::UserCommand::CreateDocument {
                parent: path.to_path_buf(),
            },
        );
    }
    if menu_item(ui, crate::ui::strings::DELETE_ACTION) {
        ctx.user_command_bus
            .publish(crate::bus::events::user_command::UserCommand::Delete(
                path.to_path_buf(),
            ));
    }

    // Folder skills (VFS-122)
    let folder_skills = {
        let folder_dir = ctx
            .content_libraries()
            .iter()
            .find(|lib| lib.name == "System")
            .map(|lib| {
                std::path::PathBuf::from(&lib.root_folder)
                    .join("Skills")
                    .join("Folder")
            })
            .unwrap_or_else(crate::config::AppConfig::get_skills_folder_dir);
        crate::config::AppConfig::list_skill_files_in(&folder_dir)
    };
    if !folder_skills.is_empty() {
        ui.separator();
        for skill in folder_skills {
            if menu_item(ui, skill.name.as_str()) {
                publish_skill(
                    ctx,
                    &skill.path,
                    "ui.dir.skill_prompt_failed",
                    Some(path.to_path_buf()),
                    Some(path.to_path_buf()),
                );
            }
        }
    }
}

/// Show multi-select file context menu (merge, delete).
/// Used by both `render_flat_row` and `draw_tree_node`.
fn show_multi_select_file_context_menu(ui: &mut egui::Ui, ctx: &mut TreeNodeContext) {
    if menu_item(ui, crate::ui::strings::MERGE_ACTION) {
        let files: HashSet<_> = ctx.selected_files().iter().cloned().collect();
        ctx.user_command_bus
            .publish(crate::bus::events::user_command::UserCommand::MergePrompt(
                files.into_iter().collect(),
            ));
    }
    if menu_item(ui, crate::ui::strings::DELETE_ACTION) {
        let files: Vec<_> = ctx.selected_files().iter().cloned().collect();
        for file in files {
            ctx.user_command_bus
                .publish(crate::bus::events::user_command::UserCommand::Delete(file));
        }
        ctx.selected_files().clear();
    }
}

/// Show single-select file context menu.
/// Used by both `render_flat_row` and `draw_tree_node`.
fn show_file_context_menu(
    ui: &mut egui::Ui,
    path: &std::path::Path,
    _name: &str,
    ctx: &mut TreeNodeContext,
) {
    if menu_item(ui, crate::ui::strings::EDIT_BUTTON) {
        ctx.user_command_bus
            .publish(crate::bus::events::user_command::UserCommand::OpenInEditor(
                path.to_path_buf(),
            ));
    }
    if menu_item(ui, crate::ui::strings::SHOW_IN_EXPLORER_ACTION) {
        ctx.user_command_bus.publish(
            crate::bus::events::user_command::UserCommand::ShowInExplorer(path.to_path_buf()),
        );
    }
    if menu_item(ui, crate::ui::strings::COPY_PATH_ACTION) {
        ctx.user_command_bus
            .publish(crate::bus::events::user_command::UserCommand::CopyPath(
                path.to_path_buf(),
            ));
    }
    if menu_item(ui, crate::ui::strings::RUN_AS_PROMPT_ACTION) {
        if let Ok(raw_content) = std::fs::read_to_string(path) {
            let content = crate::markdown::DocumentContent::parse(&raw_content).body;
            ctx.user_command_bus.publish(
                crate::bus::events::user_command::UserCommand::RunSkillPrompt {
                    content,
                    target_dir: None,
                    target_file: Some(path.to_path_buf()),
                },
            );
        } else {
            tracing::error!(
                name = "ui.file.run_as_prompt_failed",
                path = %path.display(),
                "Failed to read file content to run as prompt."
            );
        }
    }
    if menu_item(ui, crate::ui::strings::PRINT_ACTION) {
        let path_to_print = path.to_path_buf();
        if let Some(tx) = ctx.bg_tx().cloned() {
            let job = PrintJob::new(path_to_print.clone());
            let _ = execute_print_blocking(job, Some(tx));
        } else {
            tracing::warn!(
                name = "ui.file.print_no_channel",
                path = %path_to_print.display(),
                "Print requested but no background channel available"
            );
        }
    }
    // "Save as PDF..." — show a native "Save as" dialog first so the
    // user picks the destination. The previous behaviour was to
    // silently save next to the source `.md` and open the PDF
    // immediately, which surprised users who expected the standard
    // "Save as" prompt. The new flow:
    //
    //   1. rfd shows a native file picker, defaulted to the source
    //      file's directory and stem with a `.pdf` extension, with
    //      a PDF filter so the platform dialog narrows by type.
    //   2. The user picks (or cancels). The cancel path is silent
    //      — no log entry, no error.
    //   3. The picked path is wrapped in a `SaveAsPdfJob` with an
    //      explicit `output_path` (not the default next-to-source).
    //   4. The compile+write runs on a background thread so the UI
    //      stays responsive. On success the PDF is opened in the
    //      user's default viewer (matches the prior behaviour for
    //      the "I just saved a PDF" feedback loop).
    //
    // The "Save as PDF..." menu item is only displayed when the official
    // `typst` binary is available in PATH.
    if fastmd_pdf::is_typst_available() && menu_item(ui, crate::ui::strings::SAVE_AS_PDF_ACTION) {
        let path_to_export = path.to_path_buf();
        // Default the dialog to the source file's directory and
        // stem — most users want to save next to the `.md` they
        // just exported, just with an explicit name. The `rfd`
        // API takes a `&Path` for the directory and a `&str` for
        // the file name separately.
        let default_dir = path_to_export.parent().map(|p| p.to_path_buf());
        let default_name = path_to_export
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| format!("{s}.pdf"));
        let mut dialog = rfd::FileDialog::new()
            .set_title(crate::ui::strings::SAVE_AS_PDF_DIALOG_TITLE)
            .add_filter(
                crate::ui::strings::SAVE_AS_PDF_FILTER_LABEL,
                &[crate::ui::strings::SAVE_AS_PDF_FILTER_EXTENSION],
            );
        if let Some(dir) = default_dir.as_ref() {
            dialog = dialog.set_directory(dir);
        }
        if let Some(name) = default_name.as_ref() {
            dialog = dialog.set_file_name(name);
        }
        let chosen = dialog.save_file();
        // User pressed Cancel or closed the dialog — chosen is
        // `None`. Fall through to menu close without spawning any
        // background work. We do NOT log this as a failure; cancel
        // is a valid user choice.
        if let Some(target) = chosen {
            if let Some(tx) = ctx.bg_tx().cloned() {
                // Build the job with the chosen `output_path`. The
                // markdown source path is still the first arg so the
                // translator reads from the right file; `output_path`
                // overrides the default next-to-source destination.
                let mut job = SaveAsPdfJob::from_path(path_to_export.clone());
                job.output_path = Some(target.clone());
                let target_for_log = target.clone();
                std::thread::Builder::new()
                    .name("pdf-export".into())
                    .stack_size(fastmd_pdf::TYPST_THREAD_STACK_SIZE)
                    .spawn(move || {
                        if let Err(e) = execute_save_as_pdf_blocking(job, Some(tx)) {
                            tracing::error!(
                                name = "ui.file.save_as_pdf_failed",
                                source = %path_to_export.display(),
                                target = %target_for_log.display(),
                                error = %e,
                                "Save as PDF failed."
                            );
                        }
                    })
                    .expect("failed to spawn pdf-export thread");
            } else {
                tracing::warn!(
                    name = "ui.file.save_as_pdf_no_channel",
                    path = %path_to_export.display(),
                    "Save as PDF requested but no background channel available"
                );
            }
        }
    }
    if menu_item(ui, crate::ui::strings::RENAME_ACTION) {
        ctx.user_command_bus
            .publish(crate::bus::events::user_command::UserCommand::Rename(
                path.to_path_buf(),
            ));
    }
    if menu_item(ui, crate::ui::strings::MOVE_ACTION) {
        ctx.user_command_bus
            .publish(crate::bus::events::user_command::UserCommand::Move(
                path.to_path_buf(),
            ));
    }
    if menu_item(ui, crate::ui::strings::DELETE_ACTION) {
        ctx.user_command_bus
            .publish(crate::bus::events::user_command::UserCommand::Delete(
                path.to_path_buf(),
            ));
    }

    // Note skills (VFS-121)
    let note_skills = {
        let note_dir = ctx
            .content_libraries()
            .iter()
            .find(|lib| lib.name == "System")
            .map(|lib| {
                std::path::PathBuf::from(&lib.root_folder)
                    .join("Skills")
                    .join("Note")
            })
            .unwrap_or_else(crate::config::AppConfig::get_skills_note_dir);
        crate::config::AppConfig::list_skill_files_in(&note_dir)
    };
    if !note_skills.is_empty() {
        ui.separator();
        for skill in note_skills {
            if menu_item(ui, skill.name.as_str()) {
                publish_skill(
                    ctx,
                    &skill.path,
                    "ui.file.skill_prompt_failed",
                    None,
                    Some(path.to_path_buf()),
                );
            }
        }
    }
}

/// Reads a skill file, parses its body, and publishes a `RunSkillPrompt`
/// command targeting `target_dir`/`target_file`.
///
/// The folder-skill menu and the note-skill menu were duplicating this
/// read → parse → publish dance; the only differences are the target and the
/// error-log name. A read failure is logged as an `ERROR` (NFR-001).
fn publish_skill(
    ctx: &mut TreeNodeContext,
    skill_path: &std::path::Path,
    error_name: &'static str,
    target_dir: Option<std::path::PathBuf>,
    target_file: Option<std::path::PathBuf>,
) {
    match std::fs::read_to_string(skill_path) {
        Ok(raw_content) => {
            let content = crate::markdown::DocumentContent::parse(&raw_content).body;
            ctx.user_command_bus.publish(
                crate::bus::events::user_command::UserCommand::RunSkillPrompt {
                    content,
                    target_dir,
                    target_file,
                },
            );
        }
        Err(_) => {
            tracing::error!(
                name = error_name,
                path = %skill_path.display(),
                "Failed to read skill file content to run as prompt."
            );
        }
    }
}

/// Builds the selectable-label content for a directory row: the expansion
/// caret followed by the directory name.
fn dir_row_label(is_expanded: bool, name: &str) -> egui::RichText {
    let icon = if is_expanded {
        crate::ui::strings::ICON_CARET_DOWN
    } else {
        crate::ui::strings::ICON_CARET_RIGHT
    };
    egui::RichText::new(format!("{icon} {name}"))
}

/// Builds the selectable-label content for a file row: an indented file name,
/// tinted with the palette's `error` token when the file is PDF-backed.
fn file_row_label(ui: &egui::Ui, name: &str, is_pdf_backed: bool) -> egui::RichText {
    let mut text = egui::RichText::new(format!("  {name}"));
    if is_pdf_backed {
        text = text.color(ui.palette().error);
    }
    text
}

/// Wires the shared click, double-click and context-menu behaviour for a tree
/// row whose `selectable_label` has already been rendered.
///
/// Both the virtual flat tree ([`render_flat_row_capture`]) and the legacy
/// recursive tree ([`draw_tree_node`]) put an identical interaction on top of
/// the label; this helper is that interaction. Only `path` reaches the
/// handlers, so the synthetic [`FlatRow`] is built with zero depth and only
/// on click (to keep the per-frame row loop allocation-free).
fn wire_row_response(
    response: &egui::Response,
    path: &std::path::Path,
    name: &str,
    is_dir: bool,
    is_expanded: bool,
    ctx: &mut TreeNodeContext,
    on_click: &mut impl InteractionSink,
) {
    if is_dir {
        if response.clicked() {
            let row = FlatRow {
                depth: 0,
                name: name.to_string(),
                path: path.to_path_buf(),
                is_dir: true,
                is_expanded,
            };
            let cmd = apply_directory_row_click(ctx, &row);
            ctx.user_command_bus.publish(cmd);
            on_click(crate::ui::strings::DIR_ROW_EVENT);
            // Do NOT call mark_dirty() here: it would trigger a full
            // calc_max_width re-shaping pass and, before the P1-4 fix,
            // discard the user's manual panel resize on every directory
            // click (see render-audit P1-4).
        }
        // Double-click on a directory is a deliberate no-op: egui fires
        // `clicked()` AND `double_clicked()` for a double-click and the
        // single-click handler above already toggled expansion, so a second
        // toggle would undo it (render-audit P1-9).
        response.context_menu(|ui| {
            show_dir_context_menu(ui, path, name, ctx);
        });
    } else {
        if response.clicked() {
            let row = FlatRow {
                depth: 0,
                name: name.to_string(),
                path: path.to_path_buf(),
                is_dir: false,
                is_expanded,
            };
            let cmd = apply_file_row_click(ctx, &row);
            ctx.user_command_bus.publish(cmd);
            on_click(crate::ui::strings::FILE_ROW_EVENT);
        }
        if response.double_clicked() {
            ctx.user_command_bus.publish(
                crate::bus::events::user_command::UserCommand::OpenInEditor(path.to_path_buf()),
            );
        }
        response.context_menu(|ui| {
            if ctx.selected_files().len() > 1 && ctx.selected_files().contains(path) {
                show_multi_select_file_context_menu(ui, ctx);
            } else {
                show_file_context_menu(ui, path, name, ctx);
            }
        });
    }
}

/// Render a single flat row with the same interaction logic as `draw_tree_node`
/// but without recursion.
pub fn render_flat_row(ui: &mut egui::Ui, row: &FlatRow, ctx: &mut TreeNodeContext) {
    render_flat_row_capture(ui, row, ctx, no_op_sink);
}

/// Tier 4 test variant of [`render_flat_row`]. The `on_click`
/// callback is invoked after every row click (both directory and
/// file), with a stable event name. The production caller
/// ([`render_flat_row`]) passes a no-op closure; the test caller
/// in `tests::test_file_row_click_captures_event` passes a
/// closure that pushes the event into the harness's persistent
/// state.
pub fn render_flat_row_capture(
    ui: &mut egui::Ui,
    row: &FlatRow,
    ctx: &mut TreeNodeContext,
    mut on_click: impl InteractionSink,
) {
    ui.push_id((&row.path, row.is_dir), |ui| {
        // Clamp depth to prevent visual overflow on deeply nested paths.
        let clamped_depth = row.depth.min(50);
        ui.horizontal(|ui| {
            ui.add_space(clamped_depth as f32 * 18.0);
            let response = if row.is_dir {
                ui.selectable_label(false, dir_row_label(row.is_expanded, &row.name))
            } else {
                let is_selected = ctx.selected_files().contains(&row.path)
                    || ctx.selected_file().as_ref() == Some(&row.path);
                let label = file_row_label(
                    ui,
                    &row.name,
                    ctx.pdf_backing_tracker.is_pdf_backed(&row.path),
                );
                ui.selectable_label(is_selected, label)
            };
            wire_row_response(
                &response,
                &row.path,
                &row.name,
                row.is_dir,
                row.is_expanded,
                ctx,
                &mut on_click,
            );
        });
    });
}

pub fn draw_tree_node(ui: &mut egui::Ui, node: &TreeNode, ctx: &mut TreeNodeContext) {
    if node.is_dir {
        let is_expanded = ctx.expanded_dirs().contains(&node.path);
        let response = ui.selectable_label(false, dir_row_label(is_expanded, &node.name));
        wire_row_response(
            &response,
            &node.path,
            &node.name,
            true,
            is_expanded,
            ctx,
            &mut |_| {},
        );

        if is_expanded {
            ui.indent(node.path.to_string_lossy().to_string(), |ui| {
                let mut children: Vec<_> = node.children.values().collect();
                children.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));
                for child in children {
                    draw_tree_node(ui, child, ctx);
                }
            });
        }
    } else {
        let is_selected = ctx.selected_files().contains(&node.path)
            || ctx.selected_file().as_ref() == Some(&node.path);
        let label = file_row_label(
            ui,
            &node.name,
            ctx.pdf_backing_tracker.is_pdf_backed(&node.path),
        );
        let response = ui.selectable_label(is_selected, label);
        wire_row_response(
            &response,
            &node.path,
            &node.name,
            false,
            false,
            ctx,
            &mut |_| {},
        );
    }
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `render_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
