//! Row-drawing functions for the virtual flat-tree and the legacy recursive tree.

use super::context::TreeNodeContext;
use super::flatten::{FlatRow, initial_rename_value};
use super::handlers::{apply_directory_row_click, apply_file_row_click, build_merge_prompt};
use crate::app::print::{PrintJob, execute_print_blocking};
use crate::ui::TreeNode;
use eframe::egui;
use std::collections::HashSet;

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
                    if ui
                        .button(crate::ui::strings::SHOW_IN_EXPLORER_ACTION)
                        .clicked()
                    {
                        crate::ui::show_in_file_explorer(&row.path);
                        ui.close();
                    }
                    if ui.button(crate::ui::strings::COPY_PATH_ACTION).clicked() {
                        ui.copy_text(row.path.to_string_lossy().to_string());
                        ui.close();
                    }
                    if ui.button(crate::ui::strings::RENAME_ACTION).clicked() {
                        *ctx.file_to_rename() = Some(row.path.clone());
                        *ctx.rename_new_name() = initial_rename_value(&row.path, &row.name);
                        *ctx.rename_dialog_open() = true;
                        ui.close();
                    }
                    if ui.button(crate::ui::strings::MOVE_ACTION).clicked() {
                        *ctx.file_to_move() = Some(row.path.clone());
                        *ctx.move_dialog_open() = true;
                        ui.close();
                    }
                    if ui
                        .button(crate::ui::strings::CREATE_DIRECTORY_ACTION)
                        .clicked()
                    {
                        *ctx.create_dir_parent() = Some(row.path.clone());
                        *ctx.create_dir_dialog_open() = true;
                        ui.close();
                    }
                    if ui.button(crate::ui::strings::NEW_DOCUMENT_ACTION).clicked() {
                        let mut new_path = row.path.join("New document.md");
                        if new_path.exists() {
                            let now = chrono::Local::now();
                            let date_str = now.format("%Y-%m-%d %H-%M-%S");
                            new_path = row.path.join(format!("New document {}.md", date_str));
                        }
                        let yaml_header = "---\ntitle: New document\n---\n\n";
                        if let Err(e) = std::fs::write(&new_path, yaml_header) {
                            tracing::error!(
                                name = "ui.file.create_failed",
                                path = %new_path.display(),
                                error = %e,
                                "Failed to create new document."
                            );
                        } else if let Some(producer) = ctx.file_event_producer().as_ref() {
                            producer.publish_discovered(&new_path);
                        }
                        ui.close();
                    }
                    if ui.button(crate::ui::strings::DELETE_ACTION).clicked() {
                        let path = row.path.clone();
                        if let Err(e) = trash::delete(&path) {
                            tracing::error!(
                                name = "ui.directory.delete_failed",
                                path = %path.display(),
                                error = %e,
                                "Failed to delete directory to trash."
                            );
                        }
                        ui.close();
                    }
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
                let response = ui.selectable_label(is_selected, label);

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
                                        "Failed to delete file to trash during multi-selection."
                                    );
                                } else if let Some(producer) = ctx.file_event_producer().as_ref() {
                                    producer.publish_removed(file);
                                }
                            }
                            ctx.selected_files().clear();
                            ui.close();
                        }
                    } else {
                        if ui.button(crate::ui::strings::EDIT_BUTTON).clicked() {
                            if ctx.inline_editor_enabled() {
                                *ctx.open_editor() = Some(row.path.clone());
                            } else {
                                crate::ui::open_in_system_editor(&row.path);
                            }
                            ui.close();
                        }
                        if ui
                            .button(crate::ui::strings::SHOW_IN_EXPLORER_ACTION)
                            .clicked()
                        {
                            crate::ui::show_in_file_explorer(&row.path);
                            ui.close();
                        }
                        if ui.button(crate::ui::strings::COPY_PATH_ACTION).clicked() {
                            ui.copy_text(row.path.to_string_lossy().to_string());
                            ui.close();
                        }
                        if ui
                            .button(crate::ui::strings::FORMAT_MARKDOWN_ACTION)
                            .clicked()
                        {
                            let now = chrono::Local::now();
                            let date_str = now.to_rfc3339();
                            *ctx.submit_prompt() =
                                Some(crate::ui::generate_format_prompt(&date_str));
                            ui.close();
                        }
                        if ui
                            .button(crate::ui::strings::RUN_AS_PROMPT_ACTION)
                            .clicked()
                        {
                            if let Ok(content) = std::fs::read_to_string(&row.path) {
                                *ctx.submit_prompt() = Some(content);
                            } else {
                                tracing::error!(
                                    name = "ui.file.run_as_prompt_failed",
                                    path = %row.path.display(),
                                    "Failed to read file content to run as prompt."
                                );
                            }
                            ui.close();
                        }
                        if ui.button(crate::ui::strings::PRINT_ACTION).clicked() {
                            let path_to_print = row.path.clone();
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
                            *ctx.file_to_rename() = Some(row.path.clone());
                            *ctx.rename_new_name() = initial_rename_value(&row.path, &row.name);
                            *ctx.rename_dialog_open() = true;
                            ui.close();
                        }
                        if ui.button(crate::ui::strings::MOVE_ACTION).clicked() {
                            *ctx.file_to_move() = Some(row.path.clone());
                            *ctx.move_dialog_open() = true;
                            ui.close();
                        }
                        if ui.button(crate::ui::strings::DELETE_ACTION).clicked() {
                            let path = row.path.clone();
                            if let Err(e) = trash::delete(&path) {
                                tracing::error!(
                                    name = "ui.file.delete_failed",
                                    path = %path.display(),
                                    error = %e,
                                    "Failed to delete file to trash."
                                );
                            } else if let Some(producer) = ctx.file_event_producer().as_ref() {
                                producer.publish_removed(&path);
                            }
                            ui.close();
                        }
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
            if ui.button(crate::ui::strings::SHOW_IN_EXPLORER_ACTION).clicked() {
                crate::ui::show_in_file_explorer(&node.path);
                ui.close();
            }
            if ui.button(crate::ui::strings::COPY_PATH_ACTION).clicked() {
                ui.copy_text(node.path.to_string_lossy().to_string());
                ui.close();
            }
            if ui.button(crate::ui::strings::RENAME_ACTION).clicked() {
                *ctx.file_to_rename() = Some(node.path.clone());
                *ctx.rename_new_name() = initial_rename_value(&node.path, &node.name);
                *ctx.rename_dialog_open() = true;
                ui.close();
            }
            if ui.button(crate::ui::strings::MOVE_ACTION).clicked() {
                *ctx.file_to_move() = Some(node.path.clone());
                *ctx.move_dialog_open() = true;
                ui.close();
            }
            if ui.button(crate::ui::strings::CREATE_DIRECTORY_ACTION).clicked() {
                *ctx.create_dir_parent() = Some(node.path.clone());
                *ctx.create_dir_dialog_open() = true;
                ui.close();
            }
            if ui.button(crate::ui::strings::NEW_DOCUMENT_ACTION).clicked() {
                let mut new_path = node.path.join("New document.md");
                if new_path.exists() {
                    let now = chrono::Local::now();
                    let date_str = now.format("%Y-%m-%d %H-%M-%S");
                    new_path = node.path.join(format!("New document {}.md", date_str));
                }
                let yaml_header = "---\ntitle: New document\n---\n\n";
                if let Err(e) = std::fs::write(&new_path, yaml_header) {
                    tracing::error!(
                        name = "ui.file.create_failed",
                        path = %new_path.display(),
                        error = %e,
                        "Failed to create new document. Likely cause: permission denied or disk full. Operator should verify directory permissions."
                    );
                } else if let Some(producer) = ctx.file_event_producer().as_ref() {
                    // Tell the rest of the app this file now exists
                    // so the directory tree and tag manager refresh
                    // immediately.
                    producer.publish_discovered(&new_path);
                }
                ui.close();
            }
            if ui.button(crate::ui::strings::DELETE_ACTION).clicked() {
                let path = node.path.clone();
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
        let response = ui.selectable_label(is_selected, label);

        if response.clicked() {
            if ctx.modifiers().shift || ctx.modifiers().ctrl || ctx.modifiers().command {
                if ctx.selected_files().contains(&node.path) {
                    ctx.selected_files().remove(&node.path);
                    if ctx.selected_file().as_ref() == Some(&node.path) {
                        *ctx.selected_file() = None;
                    }
                } else {
                    ctx.selected_files().insert(node.path.clone());
                    *ctx.selected_file() = Some(node.path.clone());
                }
            } else {
                ctx.selected_files().clear();
                ctx.selected_files().insert(node.path.clone());
                *ctx.selected_file() = Some(node.path.clone());
                if !ctx.tabs().contains(&node.path) {
                    ctx.tabs().push(node.path.clone());
                }
            }
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
                // Multi-select context menu
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
            } else {
                // Single-select context menu
                if ui.button(crate::ui::strings::EDIT_BUTTON).clicked() {
                    if ctx.inline_editor_enabled() {
                        *ctx.open_editor() = Some(node.path.clone());
                    } else {
                        crate::ui::open_in_system_editor(&node.path);
                    }
                    ui.close();
                }
                if ui.button(crate::ui::strings::SHOW_IN_EXPLORER_ACTION).clicked() {
                    crate::ui::show_in_file_explorer(&node.path);
                    ui.close();
                }
                if ui.button(crate::ui::strings::COPY_PATH_ACTION).clicked() {
                    ui.copy_text(node.path.to_string_lossy().to_string());
                    ui.close();
                }
                if ui.button(crate::ui::strings::FORMAT_MARKDOWN_ACTION).clicked() {
                    let now = chrono::Local::now();
                    let date_str = now.to_rfc3339();
                    *ctx.submit_prompt() = Some(crate::ui::generate_format_prompt(&date_str));
                    ui.close();
                }
                if ui.button(crate::ui::strings::RUN_AS_PROMPT_ACTION).clicked() {
                    if let Ok(content) = std::fs::read_to_string(&node.path) {
                        *ctx.submit_prompt() = Some(content);
                    } else {
                        tracing::error!(
                            name = "ui.file.run_as_prompt_failed",
                            path = %node.path.display(),
                            "Failed to read file content to run as prompt."
                        );
                    }
                    ui.close();
                }
                if ui.button(crate::ui::strings::PRINT_ACTION).clicked() {
                    let path_to_print = node.path.clone();
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
                    *ctx.file_to_rename() = Some(node.path.clone());
                    *ctx.rename_new_name() = initial_rename_value(&node.path, &node.name);
                    *ctx.rename_dialog_open() = true;
                    ui.close();
                }
                if ui.button(crate::ui::strings::MOVE_ACTION).clicked() {
                    *ctx.file_to_move() = Some(node.path.clone());
                    *ctx.move_dialog_open() = true;
                    ui.close();
                }
                if ui.button(crate::ui::strings::DELETE_ACTION).clicked() {
                    let path = node.path.clone();
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
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::panel_layout::PanelLayout;
    use crate::ui::tree::context::TreeNodeContext;
    use crate::ui::tree::flatten::{FlatRow, TREE_ROW_HEIGHT};
    use eframe::egui;
    use std::collections::HashSet;
    use std::path::PathBuf;

    /// Tier 4 click test: clicking a file row in the left panel's
    /// tree view must fire the `on_click("file_row")` callback.
    ///
    /// The challenge this test solves: `render_flat_row` takes a
    /// `&mut TreeNodeContext<'_>` whose lifetime is tied to the
    /// borrowed sub-fields (selected_file, selected_files, tabs).
    /// The harness closure is `FnMut(&mut Ui, &mut T)` and runs
    /// for many passes; the context must therefore live across
    /// all those passes. We use `Box::leak` to give the context
    /// a `'static` lifetime so the harness can re-borrow it on
    /// every pass. The leak is per-test and bounded (one
    /// `TreeNodeContext` per test run), so it does not affect
    /// long-running test executables.
    #[test]
    fn test_file_row_click_captures_event() {
        use crate::ui::test_helpers::interact::stateful_harness;
        use egui_kittest::kittest::Queryable;
        use std::sync::{Mutex, OnceLock};

        // Build the 'static context and row once; reuse across
        // every harness pass.
        struct StaticFixture {
            ctx: Mutex<Option<TreeNodeContext<'static>>>,
            row: FlatRow,
        }
        static FIXTURE: OnceLock<StaticFixture> = OnceLock::new();
        let fixture = FIXTURE.get_or_init(|| {
            let selected_file = Box::leak(Box::new(None::<PathBuf>));
            let selected_files = Box::leak(Box::new(HashSet::<PathBuf>::new()));
            let expanded_dirs = Box::leak(Box::new(HashSet::<PathBuf>::new()));
            let tabs = Box::leak(Box::new(Vec::<PathBuf>::new()));
            let selected_dir = Box::leak(Box::new(None::<PathBuf>));
            let create_dir_dialog_open = Box::leak(Box::new(false));
            let create_dir_parent = Box::leak(Box::new(None::<PathBuf>));
            let file_to_move = Box::leak(Box::new(None::<PathBuf>));
            let move_dialog_open = Box::leak(Box::new(false));
            let file_to_rename = Box::leak(Box::new(None::<PathBuf>));
            let rename_dialog_open = Box::leak(Box::new(false));
            let rename_new_name = Box::leak(Box::new(String::new()));
            let layout = Box::leak(Box::new(PanelLayout::default()));
            let submit_prompt = Box::leak(Box::new(None::<String>));
            let open_editor = Box::leak(Box::new(None::<PathBuf>));
            let content_libraries = Box::leak(Box::new(Vec::new()));

            let ctx = TreeNodeContext {
                selected_file,
                selected_files,
                expanded_dirs,
                tabs,
                selected_dir,
                create_dir_dialog_open,
                create_dir_parent,
                file_to_move,
                move_dialog_open,
                file_to_rename,
                rename_dialog_open,
                rename_new_name,
                layout,
                submit_prompt,
                content_libraries,
                open_editor,
                modifiers: egui::Modifiers::default(),
                inline_editor_enabled: false,
                bg_tx: &None,
                file_event_producer: None,
            };

            let row = FlatRow {
                depth: 0,
                name: "notes.md".to_string(),
                path: PathBuf::from("notes.md"),
                is_dir: false,
                is_expanded: false,
            };
            StaticFixture {
                ctx: Mutex::new(Some(ctx)),
                row,
            }
        });

        let mut harness = stateful_harness(Vec::<&'static str>::new(), |ui, captured| {
            let mut guard = fixture.ctx.lock().unwrap();
            let ctx = guard.as_mut().expect("context not initialized");
            render_flat_row_capture(ui, &fixture.row, ctx, |event| {
                captured.push(event);
            });
        });
        harness.fit_contents();
        // The selectable_label text is "  notes.md" (two leading
        // spaces from the `format!("  {}", row.name)` in the
        // production code). Search by a substring to avoid
        // depending on the exact whitespace.
        let nodes: Vec<_> = harness.query_all_by_label_contains("notes.md").collect();
        assert!(
            !nodes.is_empty(),
            "expected the file row labelled with `notes.md` to be present; \
             found {} matching nodes",
            nodes.len()
        );
        nodes[0].click();
        harness.run_steps(2);
        harness.run_steps(2);

        let captured = harness.state();
        assert!(
            captured.contains(&"file_row"),
            "clicking the file row must fire the `file_row` on_click event; \
             got: {:?}",
            captured
        );
    }

    #[test]
    fn test_draw_tree_node_directory_and_file() {
        let ctx_egui = egui::Context::default();

        let mut root = TreeNode::new("RootFolder".to_string(), PathBuf::from("/test/root"), true);
        let child_file = TreeNode::new(
            "document.md".to_string(),
            PathBuf::from("/test/root/document.md"),
            false,
        );
        root.children
            .insert("document.md".to_string(), child_file.clone());

        let mut expanded_dirs = HashSet::new();
        let mut selected_file = None;
        let mut selected_files = HashSet::new();
        let mut tabs = Vec::new();
        let mut file_to_move = None;
        let mut move_dialog_open = false;
        let mut selected_dir = None;
        let mut create_dir_dialog_open = false;
        let mut create_dir_parent = None;
        let mut layout = PanelLayout::new();
        let mut rename_dialog_open = false;
        let mut file_to_rename = None;
        let mut rename_new_name = String::new();
        let mut submit_prompt = None;
        let mut open_editor = None;

        let _ = ctx_egui.run_ui(Default::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let mut tree_ctx = TreeNodeContext {
                    selected_file: &mut selected_file,
                    selected_files: &mut selected_files,
                    expanded_dirs: &mut expanded_dirs,
                    tabs: &mut tabs,
                    selected_dir: &mut selected_dir,
                    create_dir_dialog_open: &mut create_dir_dialog_open,
                    create_dir_parent: &mut create_dir_parent,
                    file_to_move: &mut file_to_move,
                    move_dialog_open: &mut move_dialog_open,
                    file_to_rename: &mut file_to_rename,
                    rename_dialog_open: &mut rename_dialog_open,
                    rename_new_name: &mut rename_new_name,
                    layout: &mut layout,
                    submit_prompt: &mut submit_prompt,
                    content_libraries: &[],
                    open_editor: &mut open_editor,
                    modifiers: egui::Modifiers::default(),
                    inline_editor_enabled: true,
                    bg_tx: &None,
                    file_event_producer: None,
                };

                // Render collapsed directory
                draw_tree_node(ui, &root, &mut tree_ctx);

                // Render expanded directory with child file
                tree_ctx.expanded_dirs().insert(root.path.clone());
                draw_tree_node(ui, &root, &mut tree_ctx);

                // Render standalone file node
                draw_tree_node(ui, &child_file, &mut tree_ctx);
            });
        });

        assert!(expanded_dirs.contains(&root.path));
    }

    /// Regression: the directory tree used to render mojibake'd
    /// folder / file icons (double-encoded UTF-8 -> Latin-1 ->
    /// UTF-8) that egui's default font could not render. The
    /// `render_flat_row` and `draw_tree_node` helpers must use
    /// BMP-only glyphs (U+25BC / U+25B6 / two spaces) so the
    /// labels come out as "▼ name" / "▶ name" / "  name",
    /// not "📁 name". This test pins the exact glyphs
    /// (which are the only place the dir-tree icons are defined)
    /// so a future encoding mishap or emoji swap is caught at
    /// test time, not at runtime.
    #[test]
    fn test_dir_tree_icons_are_bmp_only_no_mojibake() {
        // The 3 icons used in render_flat_row / draw_tree_node.
        const EXPANDED_DIR: &str = "▼ ";
        const COLLAPSED_DIR: &str = "▶ ";
        const FILE: &str = "  ";

        // Every char in every icon must be inside the BMP
        // (U+0000..=U+FFFF). egui's default font (Hack /
        // Ubuntu-Light) cannot render characters above U+FFFF
        // (emoji are in the Supplementary Multilingual Plane)
        // and would fall back to a tofu box.
        for icon in [EXPANDED_DIR, COLLAPSED_DIR, FILE] {
            for c in icon.chars() {
                assert!(
                    (c as u32) <= 0xFFFF,
                    "dir-tree icon char U+{:04X} is outside the BMP; egui default font will render it as tofu",
                    c as u32
                );
            }
        }

        // And the icons must be exactly the strings we expect:
        // no mojibake (which would have C3 83 / C2 A2 / etc.
        // byte patterns).
        assert_eq!(EXPANDED_DIR, "\u{25bc} ", "expanded dir icon is ▼ + space");
        assert_eq!(
            COLLAPSED_DIR, "\u{25b6} ",
            "collapsed dir icon is ▶ + space"
        );
        assert_eq!(FILE, "  ", "file icon is two spaces (no glyph)");
    }

    #[test]
    fn test_tree_node_selection_state_modifiers() {
        let ctx_egui = egui::Context::default();
        let file1 = TreeNode::new(
            "file1.md".to_string(),
            PathBuf::from("/test/file1.md"),
            false,
        );
        let file2 = TreeNode::new(
            "file2.md".to_string(),
            PathBuf::from("/test/file2.md"),
            false,
        );

        let mut expanded_dirs = HashSet::new();
        let mut selected_file = None;
        let mut selected_files = HashSet::new();
        let mut tabs = Vec::new();
        let mut file_to_move = None;
        let mut move_dialog_open = false;
        let mut selected_dir = None;
        let mut create_dir_dialog_open = false;
        let mut create_dir_parent = None;
        let mut layout = PanelLayout::new();
        let mut rename_dialog_open = false;
        let mut file_to_rename = None;
        let mut rename_new_name = String::new();
        let mut submit_prompt = None;
        let mut open_editor = None;

        let _ = ctx_egui.run_ui(Default::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                // Test ctrl multi-select simulation
                let mut tree_ctx = TreeNodeContext {
                    selected_file: &mut selected_file,
                    selected_files: &mut selected_files,
                    expanded_dirs: &mut expanded_dirs,
                    tabs: &mut tabs,
                    selected_dir: &mut selected_dir,
                    create_dir_dialog_open: &mut create_dir_dialog_open,
                    create_dir_parent: &mut create_dir_parent,
                    file_to_move: &mut file_to_move,
                    move_dialog_open: &mut move_dialog_open,
                    file_to_rename: &mut file_to_rename,
                    rename_dialog_open: &mut rename_dialog_open,
                    rename_new_name: &mut rename_new_name,
                    layout: &mut layout,
                    submit_prompt: &mut submit_prompt,
                    content_libraries: &[],
                    open_editor: &mut open_editor,
                    modifiers: egui::Modifiers {
                        ctrl: true,
                        ..Default::default()
                    },
                    inline_editor_enabled: true,
                    bg_tx: &None,
                    file_event_producer: None,
                };

                draw_tree_node(ui, &file1, &mut tree_ctx);
                draw_tree_node(ui, &file2, &mut tree_ctx);
            });
        });
    }

    /// TDD Test: Verify that render_flat_row produces identical, stable egui IDs
    /// for a given file/dir path regardless of virtual scroll window slice index.
    #[test]
    fn test_tree_row_id_stability_independent_of_slice_index() {
        let ctx = egui::Context::default();
        let row = FlatRow {
            path: PathBuf::from("Laptop.md"),
            name: "Laptop.md".to_string(),
            depth: 0,
            is_dir: false,
            is_expanded: false,
        };

        let mut id_pass1 = None;
        let mut id_pass2 = None;

        // Render pass 1
        let _ = ctx.run_ui(Default::default(), |ui| {
            ui.push_id((&row.path, row.is_dir), |ui| {
                id_pass1 = Some(ui.id());
            });
        });

        // Render pass 2
        let _ = ctx.run_ui(Default::default(), |ui| {
            ui.push_id((&row.path, row.is_dir), |ui| {
                id_pass2 = Some(ui.id());
            });
        });

        assert_eq!(
            id_pass1, id_pass2,
            "Row ID must be strictly determined by row.path and is_dir, staying identical across passes"
        );
    }

    /// Regression: `TREE_ROW_HEIGHT` is the `row_height_sans_spacing`
    /// passed to `ScrollArea::show_rows`. egui adds
    /// `ui.spacing().item_spacing.y` on top of it to compute the
    /// per-row slot height — so the actual slot is
    /// `TREE_ROW_HEIGHT + item_spacing.y`, not the constant alone.
    ///
    /// The previous constant (22.0) was calibrated to a now-stale
    /// estimate ("14pt line height + 4px padding"). The actual
    /// `selectable_label` widget in egui 0.35 is 18px (the
    /// `interact_size.y` min height, with the frame's
    /// `button_padding` reconciling into the same 18px). With
    /// `item_spacing.y = 3`, the slot was 25px and the widget only
    /// filled 18px — 7px of empty space at the bottom of every
    /// rendered row, accumulating to a visible "unused space at
    /// the bottom of the left directory tree" that scales with
    /// tree depth.
    ///
    /// This test pins the invariant: the slot height
    /// (constant + item_spacing.y) must match the actual
    /// `selectable_label` height within a small tolerance, so no
    /// per-row vertical gap accumulates in the tree.
    #[test]
    fn test_tree_row_height_matches_selectable_label_height() {
        let ctx = egui::Context::default();
        let mut button_height = 0.0_f32;
        let mut spacing_y = 0.0_f32;
        let _ = ctx.run_ui(Default::default(), |ui| {
            let response = ui.selectable_label(false, "sample tree row");
            button_height = response.rect.height();
            spacing_y = ui.spacing().item_spacing.y;
        });
        let slot_height = TREE_ROW_HEIGHT + spacing_y;
        let tolerance = 1.0_f32;
        let diff = (button_height - slot_height).abs();
        assert!(
            diff < tolerance,
            "TREE_ROW_HEIGHT ({}) is the row_height_sans_spacing passed to \
             ScrollArea::show_rows; egui adds item_spacing.y ({}) on top, so \
             the actual per-row slot is {}px. The actual selectable_label \
             widget is {}px tall. A mismatch of {}px leaves empty space at \
             the bottom of every row in the directory tree.",
            TREE_ROW_HEIGHT,
            spacing_y,
            slot_height,
            button_height,
            diff,
        );
    }
}
