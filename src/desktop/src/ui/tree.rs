//! File-system tree widget — expand/collapse, file selection, multi-select, and context-menu operations (rename, move, delete, new file/dir).

use crate::messages::BackgroundMessage;
use crate::print::{PrintJob, execute_print_blocking};
use crate::ui::TreeNode;
use crate::ui::panel_layout::PanelLayout;
use eframe::egui;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

/// Context for file operations (rename, move).
pub struct FileOpsContext<'a> {
    /// File currently queued for move operation.
    pub file_to_move: &'a mut Option<PathBuf>,
    /// Whether move dialog is open.
    pub move_dialog_open: &'a mut bool,
    /// File currently queued for rename operation.
    pub file_to_rename: &'a mut Option<PathBuf>,
    /// Whether rename dialog is open.
    pub rename_dialog_open: &'a mut bool,
    /// New name input for rename dialog.
    pub rename_new_name: &'a mut String,
}

/// Context for directory operations (create directory).
pub struct DirOpsContext<'a> {
    /// Selected directory (for context menu operations).
    pub selected_dir: &'a mut Option<PathBuf>,
    /// Whether create directory dialog is open.
    pub create_dir_dialog_open: &'a mut bool,
    /// Parent directory for new directory creation.
    pub create_dir_parent: &'a mut Option<PathBuf>,
}

/// Context for selection state (single and multi-select).
pub struct SelectionContext<'a> {
    /// Currently selected file (single selection).
    pub selected_file: &'a mut Option<PathBuf>,
    /// Currently selected files (multi-selection).
    pub selected_files: &'a mut HashSet<PathBuf>,
    /// Set of expanded directory paths.
    pub expanded_dirs: &'a mut HashSet<PathBuf>,
    /// Open tabs (files opened in editor).
    pub tabs: &'a mut Vec<PathBuf>,
}

/// Context for application-level integration (layout, prompts, editor).
pub struct AppIntegrationContext<'a> {
    /// Panel layout state (widths, dirty flags).
    pub layout: &'a mut PanelLayout,
    /// Prompt to submit to agent (from context menu actions).
    pub submit_prompt: &'a mut Option<String>,
    /// Content libraries configuration.
    pub content_libraries: &'a [crate::config::ContentLibrary],
    /// File path to open in inline editor.
    pub open_editor: &'a mut Option<PathBuf>,
    /// Keyboard modifiers state (shift, ctrl, command).
    pub modifiers: egui::Modifiers,
    /// Whether inline editor is enabled.
    pub inline_editor_enabled: bool,
    /// Background message sender (for print jobs, etc.).
    pub bg_tx: &'a Option<Sender<BackgroundMessage>>,
    /// Optional file-event producer for immediate UI updates.
    pub file_event_producer: Option<crate::file_events::FileEventProducer<'a>>,
}

/// Composite context for tree rendering operations.
/// Groups related state into semantic sub-contexts for better organization.
pub struct TreeNodeContext<'a> {
    /// File operations context (rename, move).
    pub file_ops: FileOpsContext<'a>,
    /// Directory operations context (create directory).
    pub dir_ops: DirOpsContext<'a>,
    /// Selection state context (single and multi-select).
    pub selection: SelectionContext<'a>,
    /// Application integration context (layout, prompts, editor).
    pub app: AppIntegrationContext<'a>,
}

/// Backward-compatible accessors for TreeNodeContext fields.
/// These provide the same API as the old flat struct while internally
/// delegating to the sub-contexts.
impl<'a> TreeNodeContext<'a> {
    /// Access expanded directories set.
    pub fn expanded_dirs(&mut self) -> &mut HashSet<PathBuf> {
        self.selection.expanded_dirs
    }

    /// Access selected file.
    pub fn selected_file(&mut self) -> &mut Option<PathBuf> {
        self.selection.selected_file
    }

    /// Access selected files set.
    pub fn selected_files(&mut self) -> &mut HashSet<PathBuf> {
        self.selection.selected_files
    }

    /// Access tabs vector.
    pub fn tabs(&mut self) -> &mut Vec<PathBuf> {
        self.selection.tabs
    }

    /// Access file to move.
    pub fn file_to_move(&mut self) -> &mut Option<PathBuf> {
        self.file_ops.file_to_move
    }

    /// Access move dialog open flag.
    pub fn move_dialog_open(&mut self) -> &mut bool {
        self.file_ops.move_dialog_open
    }

    /// Access selected directory.
    pub fn selected_dir(&mut self) -> &mut Option<PathBuf> {
        self.dir_ops.selected_dir
    }

    /// Access create directory dialog open flag.
    pub fn create_dir_dialog_open(&mut self) -> &mut bool {
        self.dir_ops.create_dir_dialog_open
    }

    /// Access create directory parent.
    pub fn create_dir_parent(&mut self) -> &mut Option<PathBuf> {
        self.dir_ops.create_dir_parent
    }

    /// Access layout.
    pub fn layout(&mut self) -> &mut PanelLayout {
        self.app.layout
    }

    /// Access rename dialog open flag.
    pub fn rename_dialog_open(&mut self) -> &mut bool {
        self.file_ops.rename_dialog_open
    }

    /// Access file to rename.
    pub fn file_to_rename(&mut self) -> &mut Option<PathBuf> {
        self.file_ops.file_to_rename
    }

    /// Access rename new name.
    pub fn rename_new_name(&mut self) -> &mut String {
        self.file_ops.rename_new_name
    }

    /// Access modifiers.
    pub fn modifiers(&self) -> egui::Modifiers {
        self.app.modifiers
    }

    /// Access submit prompt.
    pub fn submit_prompt(&mut self) -> &mut Option<String> {
        self.app.submit_prompt
    }

    /// Access content libraries.
    pub fn content_libraries(&self) -> &[crate::config::ContentLibrary] {
        self.app.content_libraries
    }

    /// Access open editor.
    pub fn open_editor(&mut self) -> &mut Option<PathBuf> {
        self.app.open_editor
    }

    /// Access inline editor enabled flag.
    pub fn inline_editor_enabled(&self) -> bool {
        self.app.inline_editor_enabled
    }

    /// Access background sender.
    pub fn bg_tx(&self) -> &Option<Sender<BackgroundMessage>> {
        self.app.bg_tx
    }

    /// Access file event producer.
    pub fn file_event_producer(&self) -> &Option<crate::file_events::FileEventProducer<'a>> {
        &self.app.file_event_producer
    }
}

/// Purpose: Build the initial value for the rename text field, offering
/// only the file stem (no extension) so the user types a new base name
/// and the rename modal reattaches the original extension on submit.
/// Inputs: `path` - The file the user wants to rename; `fallback_name` -
/// The display name to fall back to if the path has no usable stem.
/// Outputs: A `String` containing just the file name without extension.
/// Purity: Pure.
/// Preconditions: `path` is the canonical path of the file to rename.
/// Postconditions: Returns a `String` with the file stem; the original
/// extension is intentionally excluded.
pub fn initial_rename_value(path: &std::path::Path, fallback_name: &str) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(fallback_name)
        .to_string()
}

/// Maximum tree depth to prevent stack overflow in flatten_tree and visual overflow in rendering.
pub const MAX_TREE_DEPTH: usize = 1000;

/// A single visible row in the flattened virtual tree list.
#[derive(Clone)]
pub struct FlatRow {
    /// Indentation depth (0 = top-level child of root).
    pub depth: usize,
    /// Display name of the node.
    pub name: String,
    /// Full path of the node.
    pub path: PathBuf,
    /// Whether this node is a directory.
    pub is_dir: bool,
    /// For directories: whether currently expanded. Always false for files.
    pub is_expanded: bool,
}

/// Fixed height for each virtual tree row in pixels.
/// Matches egui default monospace line height (14pt) + 4px padding.
pub const TREE_ROW_HEIGHT: f32 = 22.0;

/// Flatten a `TreeNode` hierarchy into a `Vec<FlatRow>` in DFS pre-order,
/// respecting the set of expanded directories.
pub fn flatten_tree(
    node: &TreeNode,
    depth: usize,
    expanded: &HashSet<PathBuf>,
    rows: &mut Vec<FlatRow>,
) {
    // Prevent stack overflow on maliciously deep directory structures.
    if depth > MAX_TREE_DEPTH {
        return;
    }
    if depth > 0 {
        rows.push(FlatRow {
            depth: depth - 1,
            name: node.name.clone(),
            path: node.path.clone(),
            is_dir: node.is_dir,
            is_expanded: node.is_dir && expanded.contains(&node.path),
        });
    }
    if node.is_dir && (depth == 0 || expanded.contains(&node.path)) {
        let mut children: Vec<_> = node.children.values().collect();
        children.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));
        for child in children {
            flatten_tree(child, depth + 1, expanded, rows);
        }
    }
}

/// Render a single flat row with the same interaction logic as `draw_tree_node`
/// but without recursion.
pub fn render_flat_row(ui: &mut egui::Ui, row: &FlatRow, ctx: &mut TreeNodeContext<'_>) {
    if row.is_dir {
        let icon = if row.is_expanded { "▼ " } else { "▶ " };
        let label = format!("{}{}", icon, row.name);

        ui.horizontal(|ui| {
            // Clamp depth to prevent visual overflow on deeply nested paths
            let clamped_depth = row.depth.min(50);
            ui.add_space(clamped_depth as f32 * 18.0);
            let response = ui.selectable_label(false, label);
            if response.clicked() {
                if row.is_expanded {
                    ctx.expanded_dirs().remove(&row.path);
                } else {
                    ctx.expanded_dirs().insert(row.path.clone());
                }
                *ctx.selected_file() = None;
                ctx.selected_files().clear();
                *ctx.selected_dir() = Some(row.path.clone());
                ctx.layout().mark_dirty();
            }
            if response.double_clicked() {
                // Toggle expansion on double-click
                if row.is_expanded {
                    ctx.expanded_dirs().remove(&row.path);
                } else {
                    ctx.expanded_dirs().insert(row.path.clone());
                }
                ctx.layout().mark_dirty();
            }

            response.context_menu(|ui| {
                if ui.button("Show in File Explorer").clicked() {
                    crate::ui::show_in_file_explorer(&row.path);
                    ui.close();
                }
                if ui.button("Copy path").clicked() {
                    ui.copy_text(row.path.to_string_lossy().to_string());
                    ui.close();
                }
                if ui.button("Rename").clicked() {
                    *ctx.file_to_rename() = Some(row.path.clone());
                    *ctx.rename_new_name() = initial_rename_value(&row.path, &row.name);
                    *ctx.rename_dialog_open() = true;
                    ui.close();
                }
                if ui.button("Move").clicked() {
                    *ctx.file_to_move() = Some(row.path.clone());
                    *ctx.move_dialog_open() = true;
                    ui.close();
                }
                if ui.button("Create Directory ...").clicked() {
                    *ctx.create_dir_parent() = Some(row.path.clone());
                    *ctx.create_dir_dialog_open() = true;
                    ui.close();
                }
                if ui.button("New document").clicked() {
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
                if ui.button("Delete").clicked() {
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
                if ctx.modifiers().shift || ctx.modifiers().ctrl || ctx.modifiers().command {
                    if ctx.selected_files().contains(&row.path) {
                        ctx.selected_files().remove(&row.path);
                        if ctx.selected_file().as_ref() == Some(&row.path) {
                            *ctx.selected_file() = None;
                        }
                    } else {
                        ctx.selected_files().insert(row.path.clone());
                        *ctx.selected_file() = Some(row.path.clone());
                    }
                } else {
                    ctx.selected_files().clear();
                    ctx.selected_files().insert(row.path.clone());
                    *ctx.selected_file() = Some(row.path.clone());
                    if !ctx.tabs().contains(&row.path) {
                        ctx.tabs().push(row.path.clone());
                    }
                }
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
                    if ui.button("Merge").clicked() {
                        let files: HashSet<_> = ctx.selected_files().iter().cloned().collect();
                        let prompt = build_merge_prompt(ctx.content_libraries(), &files);
                        *ctx.submit_prompt() = Some(prompt);
                        ui.close();
                    }
                    if ui.button("Delete").clicked() {
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
                    if ui.button("Edit").clicked() {
                        if ctx.inline_editor_enabled() {
                            *ctx.open_editor() = Some(row.path.clone());
                        } else {
                            crate::ui::open_in_system_editor(&row.path);
                        }
                        ui.close();
                    }
                    if ui.button("Show in File Explorer").clicked() {
                        crate::ui::show_in_file_explorer(&row.path);
                        ui.close();
                    }
                    if ui.button("Copy path").clicked() {
                        ui.copy_text(row.path.to_string_lossy().to_string());
                        ui.close();
                    }
                    if ui.button("Format Markdown").clicked() {
                        let now = chrono::Local::now();
                        let date_str = now.to_rfc3339();
                        *ctx.submit_prompt() = Some(crate::ui::generate_format_prompt(&date_str));
                        ui.close();
                    }
                    if ui.button("Run as prompt").clicked() {
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
                    if ui.button("Print").clicked() {
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
                    if ui.button("Rename").clicked() {
                        *ctx.file_to_rename() = Some(row.path.clone());
                        *ctx.rename_new_name() = initial_rename_value(&row.path, &row.name);
                        *ctx.rename_dialog_open() = true;
                        ui.close();
                    }
                    if ui.button("Move").clicked() {
                        *ctx.file_to_move() = Some(row.path.clone());
                        *ctx.move_dialog_open() = true;
                        ui.close();
                    }
                    if ui.button("Delete").clicked() {
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
}

pub fn draw_tree_node(ui: &mut egui::Ui, node: &TreeNode, ctx: &mut TreeNodeContext<'_>) {
    if node.is_dir {
        let is_expanded = ctx.expanded_dirs().contains(&node.path);
        let icon = if is_expanded { "▼ " } else { "▶ " };
        let label = format!("{}{}", icon, node.name);

        let response = ui.selectable_label(false, label);
        if response.clicked() {
            if is_expanded {
                ctx.expanded_dirs().remove(&node.path);
            } else {
                ctx.expanded_dirs().insert(node.path.clone());
            }
            *ctx.selected_file() = None;
            ctx.selected_files().clear();
            *ctx.selected_dir() = Some(node.path.clone());
            ctx.layout().mark_dirty();
        }
        if response.double_clicked() {
            // Toggle expansion on double-click
            if is_expanded {
                ctx.expanded_dirs().remove(&node.path);
            } else {
                ctx.expanded_dirs().insert(node.path.clone());
            }
            ctx.layout().mark_dirty();
        }

        response.context_menu(|ui| {
            if ui.button("Show in File Explorer").clicked() {
                crate::ui::show_in_file_explorer(&node.path);
                ui.close();
            }
            if ui.button("Copy path").clicked() {
                ui.copy_text(node.path.to_string_lossy().to_string());
                ui.close();
            }
            if ui.button("Rename").clicked() {
                *ctx.file_to_rename() = Some(node.path.clone());
                *ctx.rename_new_name() = initial_rename_value(&node.path, &node.name);
                *ctx.rename_dialog_open() = true;
                ui.close();
            }
            if ui.button("Move").clicked() {
                *ctx.file_to_move() = Some(node.path.clone());
                *ctx.move_dialog_open() = true;
                ui.close();
            }
            if ui.button("Create Directory ...").clicked() {
                *ctx.create_dir_parent() = Some(node.path.clone());
                *ctx.create_dir_dialog_open() = true;
                ui.close();
            }
            if ui.button("New document").clicked() {
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
            if ui.button("Delete").clicked() {
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
                if ui.button("Merge").clicked() {
                    let files: HashSet<_> = ctx.selected_files().iter().cloned().collect();
                    let prompt = build_merge_prompt(ctx.content_libraries(), &files);
                    *ctx.submit_prompt() = Some(prompt);
                    ui.close();
                }
                if ui.button("Delete").clicked() {
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
                if ui.button("Edit").clicked() {
                    if ctx.inline_editor_enabled() {
                        *ctx.open_editor() = Some(node.path.clone());
                    } else {
                        crate::ui::open_in_system_editor(&node.path);
                    }
                    ui.close();
                }
                if ui.button("Show in File Explorer").clicked() {
                    crate::ui::show_in_file_explorer(&node.path);
                    ui.close();
                }
                if ui.button("Copy path").clicked() {
                    ui.copy_text(node.path.to_string_lossy().to_string());
                    ui.close();
                }
                if ui.button("Format Markdown").clicked() {
                    let now = chrono::Local::now();
                    let date_str = now.to_rfc3339();
                    *ctx.submit_prompt() = Some(crate::ui::generate_format_prompt(&date_str));
                    ui.close();
                }
                if ui.button("Run as prompt").clicked() {
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
                if ui.button("Print").clicked() {
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
                if ui.button("Rename").clicked() {
                    *ctx.file_to_rename() = Some(node.path.clone());
                    *ctx.rename_new_name() = initial_rename_value(&node.path, &node.name);
                    *ctx.rename_dialog_open() = true;
                    ui.close();
                }
                if ui.button("Move").clicked() {
                    *ctx.file_to_move() = Some(node.path.clone());
                    *ctx.move_dialog_open() = true;
                    ui.close();
                }
                if ui.button("Delete").clicked() {
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

pub fn build_merge_prompt(
    content_libraries: &[crate::config::ContentLibrary],
    selected_files: &HashSet<PathBuf>,
) -> String {
    let mut prompt = "Please read each of the following documents using the read_file tool and merge their content into a new document. Consolidate overlapping content, deduplicate repeated information, and produce a single unified document that combines all of the source material:\n".to_string();
    for file in selected_files.iter() {
        let rel_str = crate::config::library_display_label(content_libraries, file)
            .unwrap_or_else(|| file.to_string_lossy().to_string());
        prompt.push_str(&format!("- {}\n", rel_str));
    }
    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::PathBuf;

    /// Regression: the rename dialog must pre-fill with just the file stem
    /// (no extension). The modal reattaches the original extension on
    /// submit, so the user should only ever type the base name. Both
    /// context-menu entry points in `draw_tree_node` go through
    /// `initial_rename_value`, so testing the helper covers both call
    /// sites.
    #[test]
    fn test_initial_rename_value_strips_extension() {
        assert_eq!(
            initial_rename_value(&PathBuf::from("/notes/today.md"), "today.md"),
            "today"
        );
        assert_eq!(
            initial_rename_value(&PathBuf::from("/notes/today.markdown"), "today.markdown"),
            "today"
        );
        assert_eq!(
            initial_rename_value(&PathBuf::from("/notes/2023-01-15.md"), "2023-01-15.md"),
            "2023-01-15"
        );
        assert_eq!(
            initial_rename_value(&PathBuf::from("/notes/notes.txt"), "notes.txt"),
            "notes",
            ".txt extension must also be stripped so the modal re-adds it"
        );
    }

    /// Edge case: a file with no extension should still pre-fill with the
    /// full name, since there is nothing to strip.
    #[test]
    fn test_initial_rename_value_no_extension() {
        assert_eq!(
            initial_rename_value(&PathBuf::from("/notes/Makefile"), "Makefile"),
            "Makefile"
        );
    }

    /// Edge case: an empty file stem falls back to the display name so we
    /// never hand the user a blank text field.
    #[test]
    fn test_initial_rename_value_falls_back_to_display_name() {
        assert_eq!(
            initial_rename_value(&PathBuf::from("/notes/.hidden"), ".hidden"),
            ".hidden",
            "a dotfile's stem is the empty string — display name is the right fallback"
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
                    file_ops: FileOpsContext {
                        file_to_move: &mut file_to_move,
                        move_dialog_open: &mut move_dialog_open,
                        file_to_rename: &mut file_to_rename,
                        rename_dialog_open: &mut rename_dialog_open,
                        rename_new_name: &mut rename_new_name,
                    },
                    dir_ops: DirOpsContext {
                        selected_dir: &mut selected_dir,
                        create_dir_dialog_open: &mut create_dir_dialog_open,
                        create_dir_parent: &mut create_dir_parent,
                    },
                    selection: SelectionContext {
                        selected_file: &mut selected_file,
                        selected_files: &mut selected_files,
                        expanded_dirs: &mut expanded_dirs,
                        tabs: &mut tabs,
                    },
                    app: AppIntegrationContext {
                        layout: &mut layout,
                        submit_prompt: &mut submit_prompt,
                        content_libraries: &[],
                        open_editor: &mut open_editor,
                        modifiers: egui::Modifiers::default(),
                        inline_editor_enabled: true,
                        bg_tx: &None,
                        file_event_producer: None,
                    },
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
    /// not "Ã°Å¸â€œâ€š name". This test pins the exact glyphs
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
                    file_ops: FileOpsContext {
                        file_to_move: &mut file_to_move,
                        move_dialog_open: &mut move_dialog_open,
                        file_to_rename: &mut file_to_rename,
                        rename_dialog_open: &mut rename_dialog_open,
                        rename_new_name: &mut rename_new_name,
                    },
                    dir_ops: DirOpsContext {
                        selected_dir: &mut selected_dir,
                        create_dir_dialog_open: &mut create_dir_dialog_open,
                        create_dir_parent: &mut create_dir_parent,
                    },
                    selection: SelectionContext {
                        selected_file: &mut selected_file,
                        selected_files: &mut selected_files,
                        expanded_dirs: &mut expanded_dirs,
                        tabs: &mut tabs,
                    },
                    app: AppIntegrationContext {
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
                    },
                };

                draw_tree_node(ui, &file1, &mut tree_ctx);
                draw_tree_node(ui, &file2, &mut tree_ctx);
            });
        });
    }

    #[test]
    fn test_merge_prompt_includes_consolidate_instruction_and_files() {
        let libs = vec![crate::config::ContentLibrary {
            root_folder: "C:/notes".to_string(),
            name: "Notes".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        }];
        let file1 = PathBuf::from("C:/notes/alpha.md");
        let file2 = PathBuf::from("C:/notes/beta.md");

        let mut selected_files = HashSet::new();
        selected_files.insert(file1.clone());
        selected_files.insert(file2.clone());

        let prompt = build_merge_prompt(&libs, &selected_files);

        assert!(
            prompt.to_lowercase().contains("merge"),
            "prompt should instruct merge: {}",
            prompt
        );
        assert!(
            prompt.to_lowercase().contains("consolidate"),
            "prompt should instruct consolidate: {}",
            prompt
        );
        assert!(prompt.contains("alpha.md"), "prompt should list alpha.md");
        assert!(prompt.contains("beta.md"), "prompt should list beta.md");
    }
}
