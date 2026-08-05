//! Row-drawing functions for the virtual flat-tree and the legacy recursive tree.
//!
//! Unit tests live in the sibling `render_tests.rs` sidecar.

use super::context::TreeNodeContext;
use super::flatten::{FlatRow, initial_rename_value};
use super::handlers::{apply_directory_row_click, apply_file_row_click, build_merge_prompt};
use crate::app::print::{PrintJob, execute_print_blocking};
use crate::ui::TreeNode;
use eframe::egui;
use std::collections::HashSet;

/// Show context menu for a directory row.
/// Used by both `render_flat_row` and `draw_tree_node`.
fn show_dir_context_menu(
    ui: &mut egui::Ui,
    path: &std::path::Path,
    name: &str,
    ctx: &mut TreeNodeContext<'_>,
) {
    if ui
        .button(crate::ui::strings::SHOW_IN_EXPLORER_ACTION)
        .clicked()
    {
        crate::ui::show_in_file_explorer(path);
        ui.close();
    }
    if ui.button(crate::ui::strings::COPY_PATH_ACTION).clicked() {
        ui.copy_text(path.to_string_lossy().to_string());
        ui.close();
    }
    if ui.button(crate::ui::strings::RENAME_ACTION).clicked() {
        *ctx.file_to_rename() = Some(path.to_path_buf());
        *ctx.rename_new_name() = initial_rename_value(path, name);
        *ctx.rename_dialog_open() = true;
        ui.close();
    }
    if ui.button(crate::ui::strings::MOVE_ACTION).clicked() {
        *ctx.file_to_move() = Some(path.to_path_buf());
        *ctx.move_dialog_open() = true;
        ui.close();
    }
    if ui
        .button(crate::ui::strings::CREATE_DIRECTORY_ACTION)
        .clicked()
    {
        *ctx.create_dir_parent() = Some(path.to_path_buf());
        *ctx.create_dir_dialog_open() = true;
        ui.close();
    }
    if ui.button(crate::ui::strings::NEW_DOCUMENT_ACTION).clicked() {
        *ctx.create_document_parent() = Some(path.to_path_buf());
        *ctx.create_document_dialog_open() = true;
        ui.close();
    }
    if ui.button(crate::ui::strings::DELETE_ACTION).clicked() {
        let path = path.to_path_buf();
        if let Err(e) = trash::delete(&path) {
            tracing::error!(
                name = "ui.directory.delete_failed",
                path = %path.display(),
                error = %e,
                "Failed to delete directory to trash. Likely cause: directory in use or permission denied. Operator should check file locks."
            );
        }
        ui.close();
    }
}

/// Show multi-select file context menu (merge, delete).
/// Used by both `render_flat_row` and `draw_tree_node`.
fn show_multi_select_file_context_menu(ui: &mut egui::Ui, ctx: &mut TreeNodeContext<'_>) {
    if ui.button(crate::ui::strings::MERGE_ACTION).clicked() {
        let files: HashSet<_> = ctx.selected_files().iter().cloned().collect();
        let prompt = build_merge_prompt(ctx.content_libraries(), &files);
        *ctx.submit_prompt() = Some(prompt);
        ui.close();
    }
    if ui.button(crate::ui::strings::DELETE_ACTION).clicked() {
        let files: Vec<_> = ctx.selected_files().iter().cloned().collect();
        for file in files.iter() {
            if let Err(e) = trash::delete(file) {
                tracing::error!(
                    name = "ui.file.multi_delete_failed",
                    path = %file.display(),
                    error = %e,
                    "Failed to delete file to trash during multi-selection. Likely cause: file in use or permission denied. Operator should check file locks."
                );
            } else if let Some(producer) = ctx.file_event_producer().as_ref() {
                producer.publish_removed(file);
            }
        }
        ctx.selected_files().clear();
        ui.close();
    }
}

/// Show single-select file context menu.
/// Used by both `render_flat_row` and `draw_tree_node`.
fn show_file_context_menu(
    ui: &mut egui::Ui,
    path: &std::path::Path,
    name: &str,
    ctx: &mut TreeNodeContext<'_>,
) {
    if ui.button(crate::ui::strings::EDIT_BUTTON).clicked() {
        if ctx.inline_editor_enabled() {
            *ctx.open_editor() = Some(path.to_path_buf());
        } else {
            crate::ui::open_in_system_editor(path);
        }
        ui.close();
    }
    if ui
        .button(crate::ui::strings::SHOW_IN_EXPLORER_ACTION)
        .clicked()
    {
        crate::ui::show_in_file_explorer(path);
        ui.close();
    }
    if ui.button(crate::ui::strings::COPY_PATH_ACTION).clicked() {
        ui.copy_text(path.to_string_lossy().to_string());
        ui.close();
    }
    if ui
        .button(crate::ui::strings::FORMAT_MARKDOWN_ACTION)
        .clicked()
    {
        let now = chrono::Local::now();
        let date_str = now.to_rfc3339();
        *ctx.submit_prompt() = Some(crate::ui::generate_format_prompt(&date_str));
        ui.close();
    }
    if ui
        .button(crate::ui::strings::RUN_AS_PROMPT_ACTION)
        .clicked()
    {
        if let Ok(content) = std::fs::read_to_string(path) {
            *ctx.submit_prompt() = Some(content);
        } else {
            tracing::error!(
                name = "ui.file.run_as_prompt_failed",
                path = %path.display(),
                "Failed to read file content to run as prompt."
            );
        }
        ui.close();
    }
    if ui.button(crate::ui::strings::PRINT_ACTION).clicked() {
        let path_to_print = path.to_path_buf();
        if let Some(tx) = ctx.bg_tx().clone() {
            let job = PrintJob::new(path_to_print.clone());
            let _ = execute_print_blocking(job, Some(tx));
        } else {
            tracing::warn!(
                name = "ui.file.print_no_channel",
                path = %path_to_print.display(),
                "Print requested but no background channel available"
            );
        }
        ui.close();
    }
    if ui.button(crate::ui::strings::RENAME_ACTION).clicked() {
        *ctx.file_to_rename() = Some(path.to_path_buf());
        *ctx.rename_new_name() = initial_rename_value(path, name);
        *ctx.rename_dialog_open() = true;
        ui.close();
    }
    if ui.button(crate::ui::strings::MOVE_ACTION).clicked() {
        *ctx.file_to_move() = Some(path.to_path_buf());
        *ctx.move_dialog_open() = true;
        ui.close();
    }
    if ui.button(crate::ui::strings::DELETE_ACTION).clicked() {
        let path = path.to_path_buf();
        if let Err(e) = trash::delete(&path) {
            tracing::error!(
                name = "ui.file.delete_failed",
                path = %path.display(),
                error = %e,
                "Failed to delete file to trash. Likely cause: file in use or permission denied. Operator should check file locks."
            );
        } else if let Some(producer) = ctx.file_event_producer().as_ref() {
            producer.publish_removed(&path);
        }
        ui.close();
    }
}

/// Render a single flat row with the same interaction logic as `draw_tree_node`
/// but without recursion.
pub fn render_flat_row(ui: &mut egui::Ui, row: &FlatRow, ctx: &mut TreeNodeContext<'_>) {
    render_flat_row_capture(ui, row, ctx, |_| {});
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
    ctx: &mut TreeNodeContext<'_>,
    mut on_click: impl FnMut(&'static str),
) {
    ui.push_id((&row.path, row.is_dir), |ui| {
        if row.is_dir {
            let icon = if row.is_expanded { "▼ " } else { "▶ " };
            let label = format!("{}{}", icon, row.name);

            ui.horizontal(|ui| {
                // Clamp depth to prevent visual overflow on deeply nested paths
                let clamped_depth = row.depth.min(50);
                ui.add_space(clamped_depth as f32 * 18.0);
                let response = ui.selectable_label(false, label);
                if response.clicked() {
                    apply_directory_row_click(ctx, row);
                    on_click("dir_row");
                    // Do NOT call mark_dirty() here: it would trigger a
                    // full calc_max_width re-shaping pass and, before the
                    // P1-4 fix, discard the user's manual panel resize
                    // on every directory click (see render-audit P1-4).
                }
                if response.double_clicked() {
                    // Toggle expansion on double-click.
                    // NOTE: egui fires `clicked()` AND `double_clicked()`
                    // for a double-click, so both branches run. The
                    // second toggle undoes the first → net no-op
                    // expansion (see render-audit P1-9). We intentionally
                    // do NOT toggle here; the single-click handler above
                    // already toggled. Double-click on a directory is a
                    // no-op for expansion — it does not also clear the
                    // file selection, matching the common file-explorer
                    // convention where double-click opens (for files)
                    // and is inert for folders.
                }

                response.context_menu(|ui| {
                    show_dir_context_menu(ui, &row.path, &row.name, ctx);
                });
            });
        } else {
            let is_selected = ctx.selected_files().contains(&row.path)
                || ctx.selected_file().as_ref() == Some(&row.path);
            let label = format!("  {}", row.name);

            ui.horizontal(|ui| {
                // Clamp depth to prevent visual overflow on deeply nested paths
                let clamped_depth = row.depth.min(50);
                ui.add_space(clamped_depth as f32 * 18.0);
                let mut text = egui::RichText::new(label);
                if ctx.pdf_backing_tracker.is_pdf_backed(&row.path) {
                    text = text.color(egui::Color32::from_rgb(244, 15, 2));
                }

                let response = ui.selectable_label(is_selected, text);

                if response.clicked() {
                    apply_file_row_click(ctx, row);
                    on_click("file_row");
                }

                if response.double_clicked() {
                    if ctx.inline_editor_enabled() {
                        *ctx.open_editor() = Some(row.path.clone());
                    } else {
                        crate::ui::open_in_system_editor(&row.path);
                    }
                }

                response.context_menu(|ui| {
                    if ctx.selected_files().len() > 1 && ctx.selected_files().contains(&row.path) {
                        show_multi_select_file_context_menu(ui, ctx);
                    } else {
                        show_file_context_menu(ui, &row.path, &row.name, ctx);
                    }
                });
            });
        }
    });
}

pub fn draw_tree_node(ui: &mut egui::Ui, node: &TreeNode, ctx: &mut TreeNodeContext<'_>) {
    if node.is_dir {
        let is_expanded = ctx.expanded_dirs().contains(&node.path);
        let icon = if is_expanded { "▼ " } else { "▶ " };
        let label = format!("{}{}", icon, node.name);

        let response = ui.selectable_label(false, label);
        if response.clicked() {
            apply_directory_row_click(
                ctx,
                &FlatRow {
                    depth: 0,
                    name: node.name.clone(),
                    path: node.path.clone(),
                    is_dir: true,
                    is_expanded,
                },
            );
            // Do NOT call mark_dirty() here — see render_flat_row
            // for the rationale (render-audit P1-4/P1-9).
        }
        if response.double_clicked() {
            // See render_flat_row: the single-click handler already
            // toggled expansion. A second toggle here would undo it
            // (render-audit P1-9), so double-click is a no-op for
            // directory expansion.
        }

        response.context_menu(|ui| {
            show_dir_context_menu(ui, &node.path, &node.name, ctx);
        });

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
        let label = format!("  {}", node.name);
        let mut text = egui::RichText::new(label);
        if ctx.pdf_backing_tracker.is_pdf_backed(&node.path) {
            text = text.color(egui::Color32::from_rgb(244, 15, 2));
        }

        let response = ui.selectable_label(is_selected, text);

        if response.clicked() {
            let is_expanded = ctx.expanded_dirs().contains(&node.path);
            apply_file_row_click(
                ctx,
                &FlatRow {
                    depth: 0,
                    name: node.name.clone(),
                    path: node.path.clone(),
                    is_dir: false,
                    is_expanded,
                },
            );
        }

        if response.double_clicked() {
            if ctx.inline_editor_enabled() {
                *ctx.open_editor() = Some(node.path.clone());
            } else {
                crate::ui::open_in_system_editor(&node.path);
            }
        }

        response.context_menu(|ui| {
            if ctx.selected_files().len() > 1 && ctx.selected_files().contains(&node.path) {
                show_multi_select_file_context_menu(ui, ctx);
            } else {
                show_file_context_menu(ui, &node.path, &node.name, ctx);
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `render_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
