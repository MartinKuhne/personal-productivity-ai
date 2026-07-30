//! File-system tree widget — expand/collapse, file selection, multi-select, and context-menu operations (rename, move, delete, new file/dir).

use crate::app::panel_layout::PanelLayout;
use crate::app::print::{PrintJob, execute_print_blocking};
use crate::bus::events::file::FileEventProducer;
use crate::bus::events::messages::BackgroundMessage;
use crate::ui::TreeNode;
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
    pub file_event_producer: Option<FileEventProducer<'a>>,
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
    pub fn file_event_producer(&self) -> &Option<FileEventProducer<'a>> {
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

/// Sans-spacing slot height passed to `ScrollArea::show_rows`. egui
/// adds `ui.spacing().item_spacing.y` (default 3.0) on top of this
/// to compute the actual per-row slot height. The row content
/// rendered by [`render_flat_row`] is a `ui.horizontal` block whose
/// height is the max of its children: a `selectable_label`, which
/// in egui 0.35 is `interact_size.y` (18) — `button_padding.y`
/// (1 top + 1 bottom) is added inside the frame, but the text is
/// short enough that the interact_size dominates and the rendered
/// height reconciles to 18px. To keep the slot exactly matched to
/// the rendered content (and avoid empty space accumulating at the
/// bottom of every row), this constant is set so that
/// `TREE_ROW_HEIGHT + item_spacing.y` == actual `selectable_label`
/// height. The companion regression test
/// `test_tree_row_height_matches_selectable_label_height` pins
/// this invariant.
pub const TREE_ROW_HEIGHT: f32 = 15.0;

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

/// Purpose: Applies the side effect of clicking a file row in the
/// left panel's tree view.
/// Inputs: ctx (the `TreeNodeContext`; its `selected_file`,
/// `selected_files`, and `tabs` are mutated), row (the clicked
/// `FlatRow`)
/// Outputs: ()
/// Purity: Impure (mutates the selection and tab state).
/// Preconditions: `row.is_dir` should be `false` (caller filters
/// files vs directories). Modifiers are taken from `ctx.modifiers()`
/// at the moment of the call.
/// Postconditions:
///   * With any of shift/ctrl/command held: toggles the row's
///     inclusion in `ctx.selected_files()`. If it was already in
///     the set, it is removed and `ctx.selected_file()` is cleared
///     if it pointed at this path. If it was not, it is added and
///     `ctx.selected_file()` is set to it.
///   * With no modifier: replaces the selection with this single
///     file and pushes it onto `ctx.tabs()` if not already open.
///   * **Always:** updates `ctx.selected_dir()` (the "current
///     directory context" used by the bottom-panel prompt prefix
///     and by the agent session dispatch) to the file's
///     containing directory — `row.path.parent()`. If the file
///     has no parent component, `selected_dir` is cleared to
///     `None`. This runs in both the modifier and no-modifier
///     branches: the user is operating in the clicked file's
///     directory regardless of how they selected it.
///
/// Takes `&mut TreeNodeContext` rather than three separate `&mut`
/// fields because Rust's borrow checker treats three calls to
/// `ctx.selected_file()` / `ctx.selected_files()` / `ctx.tabs()` as
/// overlapping re-borrows of `*ctx` (since the accessors all return
/// `&mut` to disjoint sub-fields of the same struct, but the
/// compiler does not see through the accessor boundary). The
/// function extracts the three sub-references via split-borrow
/// inside its own body.
///
/// The file-row click in `render_flat_row` calls this function.
/// It is extracted so the modifier logic can be unit-tested
/// without driving the egui harness. The directory-row click
/// path is unchanged — it has a different effect (toggle
/// expansion + set `selected_dir`).
pub fn apply_file_row_click(ctx: &mut TreeNodeContext<'_>, row: &FlatRow) {
    let modifiers = ctx.modifiers();
    // Use split borrows through the struct's sub-fields rather than
    // calling the accessor methods three times: the accessors are
    // `&mut self` methods that return `&mut` to disjoint sub-fields,
    // but the borrow checker cannot see through the method body to
    // prove disjointness, so it treats the three calls as overlapping
    // re-borrows of `*ctx`. Field access through `ctx.selection.*`
    // lets the compiler split-borrow the inner struct directly.
    let TreeNodeContext {
        file_ops: _,
        dir_ops:
            DirOpsContext {
                selected_dir,
                create_dir_dialog_open: _,
                create_dir_parent: _,
            },
        selection:
            SelectionContext {
                selected_file,
                selected_files,
                expanded_dirs: _,
                tabs,
            },
        app: _,
    } = ctx;
    if modifiers.shift || modifiers.ctrl || modifiers.command {
        if selected_files.contains(&row.path) {
            selected_files.remove(&row.path);
            if selected_file.as_ref() == Some(&row.path) {
                **selected_file = None;
            }
        } else {
            selected_files.insert(row.path.clone());
            **selected_file = Some(row.path.clone());
        }
    } else {
        selected_files.clear();
        selected_files.insert(row.path.clone());
        **selected_file = Some(row.path.clone());
        if !tabs.contains(&row.path) {
            tabs.push(row.path.clone());
        }
    }
    // Always refresh the current directory context to the file's
    // containing directory. The bottom-panel prompt prefix and
    // the agent session dispatch both read `selected_dir`, so a
    // stale value would mislead both surfaces after a file click.
    // `Path::parent` returns `None` for bare filenames (no parent
    // component), which is the right neutral state for the bottom
    // panel's `>` prefix.
    **selected_dir = row.path.parent().map(|p| p.to_path_buf());
}

/// Purpose: Applies the side effect of clicking a directory row in
/// the left panel's tree view.
/// Inputs: ctx (the `TreeNodeContext`; its `expanded_dirs` and
/// `selected_dir` are mutated), row (the clicked `FlatRow`;
/// `row.is_dir` is `true`).
/// Outputs: ()
/// Purity: Impure (mutates the tree's expand/collapse state and the
/// current-directory context).
/// Preconditions: `row.is_dir` is `true`.
/// Postconditions:
///   * Toggles `ctx.expanded_dirs()` for `row.path` — adds it if
///     the folder was collapsed, removes it if it was already
///     expanded.
///   * Updates `ctx.selected_dir()` to `Some(row.path.clone())`
///     so the bottom-panel prompt prefix and the agent session
///     reflect the folder the user just browsed to.
///   * **Does NOT touch** `ctx.selected_file()`, `ctx.selected_files()`,
///     or `ctx.tabs()`. Expanding/collapsing a folder is a
///     tree-navigation action, orthogonal to which file is open
///     in the editor. Clearing the file selection here would
///     hide the center panel body (the file header, YAML table,
///     and rendered markdown inside `ScrollArea`, all guarded by
///     `if let Some(selected_path) = app.selection().selected_file()`)
///     and the right (TOC) panel (`should_show_panel` requires a
///     selected file), even though `tab_manager.current_markdown`
///     and `loaded_path` are still set. The user would have to
///     click the file again to restore the preview. The unit
///     test `test_apply_directory_row_click_preserves_selected_file`
///     pins this invariant.
///
/// The directory-row click in `render_flat_row` and
/// `draw_tree_node` (legacy recursive path) calls this function.
/// It is extracted so the state mutation can be unit-tested
/// without driving the egui harness, mirroring the
/// `apply_file_row_click` pattern.
pub fn apply_directory_row_click(ctx: &mut TreeNodeContext<'_>, row: &FlatRow) {
    // Split-borrow through the struct's sub-fields rather than
    // calling the accessor methods: the accessors are `&mut self`
    // methods that return `&mut` to disjoint sub-fields, but the
    // borrow checker cannot see through the method body to prove
    // disjointness, so it treats the calls as overlapping
    // re-borrows of `*ctx`. Field access through `ctx.selection.*`
    // lets the compiler split-borrow the inner struct directly.
    let TreeNodeContext {
        file_ops: _,
        dir_ops:
            DirOpsContext {
                selected_dir,
                create_dir_dialog_open: _,
                create_dir_parent: _,
            },
        selection:
            SelectionContext {
                selected_file: _,
                selected_files: _,
                expanded_dirs,
                tabs: _,
            },
        app: _,
    } = ctx;
    if expanded_dirs.contains(&row.path) {
        expanded_dirs.remove(&row.path);
    } else {
        expanded_dirs.insert(row.path.clone());
    }
    **selected_dir = Some(row.path.clone());
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

pub fn build_merge_prompt(
    content_libraries: &[crate::config::ContentLibrary],
    selected_files: &HashSet<PathBuf>,
) -> String {
    let mut prompt = "Please read each of the following documents using the read_file tool and merge their content into a new document. Consolidate overlapping content, deduplicate repeated information, and produce a single unified document that combines all of the source material:\n".to_string();
    let mut sorted_files: Vec<&PathBuf> = selected_files.iter().collect();
    sorted_files.sort();
    for file in sorted_files {
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

    /// Tier 1 test: a file row click with no modifier replaces
    /// the single selection with the clicked file and pushes it
    /// onto `tabs` if not already there.
    #[test]
    fn test_apply_file_row_click_no_modifier_replaces_selection_and_opens_tab() {
        let mut tabs: Vec<PathBuf> = vec![PathBuf::from("a.md")];
        let mut selected_file = Some(PathBuf::from("a.md"));
        let mut selected_files = HashSet::new();
        selected_files.insert(PathBuf::from("a.md"));
        let mut expanded_dirs = HashSet::new();
        let row = FlatRow {
            depth: 0,
            name: "b.md".to_string(),
            path: PathBuf::from("b.md"),
            is_dir: false,
            is_expanded: false,
        };
        let mut ctx = TreeNodeContext {
            file_ops: FileOpsContext {
                file_to_move: &mut None,
                move_dialog_open: &mut false,
                file_to_rename: &mut None,
                rename_dialog_open: &mut false,
                rename_new_name: &mut String::new(),
            },
            dir_ops: DirOpsContext {
                selected_dir: &mut None,
                create_dir_dialog_open: &mut false,
                create_dir_parent: &mut None,
            },
            selection: SelectionContext {
                selected_file: &mut selected_file,
                selected_files: &mut selected_files,
                expanded_dirs: &mut expanded_dirs,
                tabs: &mut tabs,
            },
            app: AppIntegrationContext {
                layout: &mut PanelLayout::default(),
                submit_prompt: &mut None,
                content_libraries: &[],
                open_editor: &mut None,
                modifiers: egui::Modifiers::default(),
                inline_editor_enabled: false,
                bg_tx: &None,
                file_event_producer: None,
            },
        };

        apply_file_row_click(&mut ctx, &row);

        assert_eq!(selected_file, Some(PathBuf::from("b.md")));
        assert!(selected_files.contains(&PathBuf::from("b.md")));
        assert_eq!(
            selected_files.len(),
            1,
            "previous selection must be cleared"
        );
        assert_eq!(
            tabs,
            vec![PathBuf::from("a.md"), PathBuf::from("b.md")],
            "clicked file must be pushed onto tabs"
        );
    }

    /// Tier 1 test: a file row click with shift held toggles the
    /// row's membership in `selected_files`. Toggling an
    /// already-selected file removes it and clears `selected_file`
    /// if it pointed at that file.
    #[test]
    fn test_apply_file_row_click_shift_toggles_off() {
        let mut tabs: Vec<PathBuf> = vec![];
        let mut selected_file = Some(PathBuf::from("b.md"));
        let mut selected_files = HashSet::new();
        selected_files.insert(PathBuf::from("b.md"));
        let mut expanded_dirs = HashSet::new();
        let row = FlatRow {
            depth: 0,
            name: "b.md".to_string(),
            path: PathBuf::from("b.md"),
            is_dir: false,
            is_expanded: false,
        };
        let mut ctx = TreeNodeContext {
            file_ops: FileOpsContext {
                file_to_move: &mut None,
                move_dialog_open: &mut false,
                file_to_rename: &mut None,
                rename_dialog_open: &mut false,
                rename_new_name: &mut String::new(),
            },
            dir_ops: DirOpsContext {
                selected_dir: &mut None,
                create_dir_dialog_open: &mut false,
                create_dir_parent: &mut None,
            },
            selection: SelectionContext {
                selected_file: &mut selected_file,
                selected_files: &mut selected_files,
                expanded_dirs: &mut expanded_dirs,
                tabs: &mut tabs,
            },
            app: AppIntegrationContext {
                layout: &mut PanelLayout::default(),
                submit_prompt: &mut None,
                content_libraries: &[],
                open_editor: &mut None,
                modifiers: egui::Modifiers {
                    shift: true,
                    ..Default::default()
                },
                inline_editor_enabled: false,
                bg_tx: &None,
                file_event_producer: None,
            },
        };

        apply_file_row_click(&mut ctx, &row);

        assert!(
            !selected_files.contains(&PathBuf::from("b.md")),
            "shift-click on a selected file must remove it from selected_files"
        );
        assert!(
            selected_file.is_none(),
            "selected_file must be cleared when the toggled-off file was the selected one"
        );
    }

    /// Tier 1 test: shift-clicking a file that is NOT in
    /// `selected_files` adds it to the set and makes it the
    /// `selected_file` without touching `tabs` (multi-select does
    /// not auto-open tabs).
    #[test]
    fn test_apply_file_row_click_shift_adds_to_selection_without_opening_tab() {
        let mut tabs: Vec<PathBuf> = vec![];
        let mut selected_file = Some(PathBuf::from("a.md"));
        let mut selected_files = HashSet::new();
        selected_files.insert(PathBuf::from("a.md"));
        let mut expanded_dirs = HashSet::new();
        let row = FlatRow {
            depth: 0,
            name: "b.md".to_string(),
            path: PathBuf::from("b.md"),
            is_dir: false,
            is_expanded: false,
        };
        let mut ctx = TreeNodeContext {
            file_ops: FileOpsContext {
                file_to_move: &mut None,
                move_dialog_open: &mut false,
                file_to_rename: &mut None,
                rename_dialog_open: &mut false,
                rename_new_name: &mut String::new(),
            },
            dir_ops: DirOpsContext {
                selected_dir: &mut None,
                create_dir_dialog_open: &mut false,
                create_dir_parent: &mut None,
            },
            selection: SelectionContext {
                selected_file: &mut selected_file,
                selected_files: &mut selected_files,
                expanded_dirs: &mut expanded_dirs,
                tabs: &mut tabs,
            },
            app: AppIntegrationContext {
                layout: &mut PanelLayout::default(),
                submit_prompt: &mut None,
                content_libraries: &[],
                open_editor: &mut None,
                modifiers: egui::Modifiers {
                    shift: true,
                    ..Default::default()
                },
                inline_editor_enabled: false,
                bg_tx: &None,
                file_event_producer: None,
            },
        };

        apply_file_row_click(&mut ctx, &row);

        assert!(selected_files.contains(&PathBuf::from("b.md")));
        assert_eq!(
            selected_file,
            Some(PathBuf::from("b.md")),
            "shift-click must set selected_file to the clicked file"
        );
        assert!(
            tabs.is_empty(),
            "shift-click must NOT auto-open the clicked file as a tab (multi-select mode)"
        );
    }

    /// Tier 1 test: clicking a file that is already open in a tab
    /// does NOT push a duplicate. Tab list is the unique set of
    /// open paths.
    #[test]
    fn test_apply_file_row_click_no_duplicate_tab() {
        let mut tabs: Vec<PathBuf> = vec![PathBuf::from("a.md")];
        let mut selected_file = Some(PathBuf::from("a.md"));
        let mut selected_files = HashSet::new();
        selected_files.insert(PathBuf::from("a.md"));
        let mut expanded_dirs = HashSet::new();
        let row = FlatRow {
            depth: 0,
            name: "a.md".to_string(),
            path: PathBuf::from("a.md"),
            is_dir: false,
            is_expanded: false,
        };
        let mut ctx = TreeNodeContext {
            file_ops: FileOpsContext {
                file_to_move: &mut None,
                move_dialog_open: &mut false,
                file_to_rename: &mut None,
                rename_dialog_open: &mut false,
                rename_new_name: &mut String::new(),
            },
            dir_ops: DirOpsContext {
                selected_dir: &mut None,
                create_dir_dialog_open: &mut false,
                create_dir_parent: &mut None,
            },
            selection: SelectionContext {
                selected_file: &mut selected_file,
                selected_files: &mut selected_files,
                expanded_dirs: &mut expanded_dirs,
                tabs: &mut tabs,
            },
            app: AppIntegrationContext {
                layout: &mut PanelLayout::default(),
                submit_prompt: &mut None,
                content_libraries: &[],
                open_editor: &mut None,
                modifiers: egui::Modifiers::default(),
                inline_editor_enabled: false,
                bg_tx: &None,
                file_event_producer: None,
            },
        };

        apply_file_row_click(&mut ctx, &row);

        assert_eq!(
            tabs,
            vec![PathBuf::from("a.md")],
            "clicking an already-open tab must not push a duplicate"
        );
    }

    /// TDD regression: clicking a file row in the left directory
    /// tree must update `selected_dir` (the "current directory
    /// context" used by the bottom-panel prompt prefix and the
    /// agent session) to the file's containing directory.
    ///
    /// Before the fix, `apply_file_row_click` only updated
    /// `selected_file` / `selected_files` / `tabs` — `selected_dir`
    /// kept whatever value the previous directory click (or app
    /// start) had set, so the bottom panel would keep showing a
    /// stale directory prefix and the agent would receive the
    /// wrong context once the user opened a file.
    #[test]
    fn test_apply_file_row_click_updates_selected_dir_to_parent() {
        let mut tabs: Vec<PathBuf> = vec![];
        let mut selected_file: Option<PathBuf> = None;
        let mut selected_files: HashSet<PathBuf> = HashSet::new();
        let mut expanded_dirs: HashSet<PathBuf> = HashSet::new();
        // Pre-existing stale value to prove the click overwrites it.
        let mut selected_dir: Option<PathBuf> = Some(PathBuf::from("C:/old/dir"));
        let file_path = PathBuf::from("C:/notes/folder/file.md");
        let expected_parent = Some(PathBuf::from("C:/notes/folder"));
        let row = FlatRow {
            depth: 1,
            name: "file.md".to_string(),
            path: file_path.clone(),
            is_dir: false,
            is_expanded: false,
        };
        let mut ctx = TreeNodeContext {
            file_ops: FileOpsContext {
                file_to_move: &mut None,
                move_dialog_open: &mut false,
                file_to_rename: &mut None,
                rename_dialog_open: &mut false,
                rename_new_name: &mut String::new(),
            },
            dir_ops: DirOpsContext {
                selected_dir: &mut selected_dir,
                create_dir_dialog_open: &mut false,
                create_dir_parent: &mut None,
            },
            selection: SelectionContext {
                selected_file: &mut selected_file,
                selected_files: &mut selected_files,
                expanded_dirs: &mut expanded_dirs,
                tabs: &mut tabs,
            },
            app: AppIntegrationContext {
                layout: &mut PanelLayout::default(),
                submit_prompt: &mut None,
                content_libraries: &[],
                open_editor: &mut None,
                modifiers: egui::Modifiers::default(),
                inline_editor_enabled: false,
                bg_tx: &None,
                file_event_producer: None,
            },
        };

        apply_file_row_click(&mut ctx, &row);

        assert_eq!(
            selected_dir, expected_parent,
            "clicking a file row must update selected_dir to the file's containing directory"
        );
    }

    /// TDD regression: even with a multi-select modifier (shift),
    /// clicking a file row must still update `selected_dir` to
    /// the file's containing directory. The user is operating
    /// in that directory and the bottom-panel prefix / agent
    /// context should reflect it.
    #[test]
    fn test_apply_file_row_click_shift_updates_selected_dir_to_parent() {
        let mut tabs: Vec<PathBuf> = vec![];
        let mut selected_file: Option<PathBuf> = None;
        let mut selected_files: HashSet<PathBuf> = HashSet::new();
        let mut expanded_dirs: HashSet<PathBuf> = HashSet::new();
        let mut selected_dir: Option<PathBuf> = None;
        let file_path = PathBuf::from("C:/notes/folder/file.md");
        let expected_parent = Some(PathBuf::from("C:/notes/folder"));
        let row = FlatRow {
            depth: 1,
            name: "file.md".to_string(),
            path: file_path.clone(),
            is_dir: false,
            is_expanded: false,
        };
        let mut ctx = TreeNodeContext {
            file_ops: FileOpsContext {
                file_to_move: &mut None,
                move_dialog_open: &mut false,
                file_to_rename: &mut None,
                rename_dialog_open: &mut false,
                rename_new_name: &mut String::new(),
            },
            dir_ops: DirOpsContext {
                selected_dir: &mut selected_dir,
                create_dir_dialog_open: &mut false,
                create_dir_parent: &mut None,
            },
            selection: SelectionContext {
                selected_file: &mut selected_file,
                selected_files: &mut selected_files,
                expanded_dirs: &mut expanded_dirs,
                tabs: &mut tabs,
            },
            app: AppIntegrationContext {
                layout: &mut PanelLayout::default(),
                submit_prompt: &mut None,
                content_libraries: &[],
                open_editor: &mut None,
                modifiers: egui::Modifiers {
                    shift: true,
                    ..Default::default()
                },
                inline_editor_enabled: false,
                bg_tx: &None,
                file_event_producer: None,
            },
        };

        apply_file_row_click(&mut ctx, &row);

        assert_eq!(
            selected_dir, expected_parent,
            "shift-clicking a file row must also update selected_dir to the file's containing directory"
        );
    }

    /// Edge case: a file with no parent component (a bare
    /// filename like `file.md`) must refresh `selected_dir` away
    /// from any stale prior value. `Path::parent("file.md")`
    /// returns `Some(Path::new(""))` (an empty path), not `None`,
    /// because the OS-level "containing directory" of a bare
    /// filename is the empty path. The downstream
    /// `compute_prompt_prefix` already handles this case — an
    /// empty path falls through to its `is_empty()` branch and
    /// renders the bare `">"` prefix, matching the `None` case.
    #[test]
    fn test_apply_file_row_click_bare_filename_sets_empty_parent() {
        let mut tabs: Vec<PathBuf> = vec![];
        let mut selected_file: Option<PathBuf> = None;
        let mut selected_files: HashSet<PathBuf> = HashSet::new();
        let mut expanded_dirs: HashSet<PathBuf> = HashSet::new();
        let mut selected_dir: Option<PathBuf> = Some(PathBuf::from("C:/stale/dir"));
        let row = FlatRow {
            depth: 0,
            name: "file.md".to_string(),
            path: PathBuf::from("file.md"),
            is_dir: false,
            is_expanded: false,
        };
        let mut ctx = TreeNodeContext {
            file_ops: FileOpsContext {
                file_to_move: &mut None,
                move_dialog_open: &mut false,
                file_to_rename: &mut None,
                rename_dialog_open: &mut false,
                rename_new_name: &mut String::new(),
            },
            dir_ops: DirOpsContext {
                selected_dir: &mut selected_dir,
                create_dir_dialog_open: &mut false,
                create_dir_parent: &mut None,
            },
            selection: SelectionContext {
                selected_file: &mut selected_file,
                selected_files: &mut selected_files,
                expanded_dirs: &mut expanded_dirs,
                tabs: &mut tabs,
            },
            app: AppIntegrationContext {
                layout: &mut PanelLayout::default(),
                submit_prompt: &mut None,
                content_libraries: &[],
                open_editor: &mut None,
                modifiers: egui::Modifiers::default(),
                inline_editor_enabled: false,
                bg_tx: &None,
                file_event_producer: None,
            },
        };

        apply_file_row_click(&mut ctx, &row);

        // `Path::parent("file.md")` is `Some(Path::new(""))`,
        // not `None`. Verify the click refreshes the stale
        // value to that canonical empty-parent form, and that
        // the resulting bottom-panel prefix renders as the bare
        // ">" (same surface as `selected_dir == None`).
        assert_eq!(
            selected_dir,
            Some(PathBuf::new()),
            "clicking a bare-filename row must set selected_dir to Some(Path::new(\"\"))"
        );
        let prefix = crate::ui::panels::bottom::compute_prompt_prefix(selected_dir.as_deref(), &[]);
        assert_eq!(
            prefix, ">",
            "an empty-path selected_dir must render as the bare `>` prefix in the bottom panel"
        );
    }

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
                file_ops: FileOpsContext {
                    file_to_move,
                    move_dialog_open,
                    file_to_rename,
                    rename_dialog_open,
                    rename_new_name,
                },
                dir_ops: DirOpsContext {
                    selected_dir,
                    create_dir_dialog_open,
                    create_dir_parent,
                },
                selection: SelectionContext {
                    selected_file,
                    selected_files,
                    expanded_dirs,
                    tabs,
                },
                app: AppIntegrationContext {
                    layout,
                    submit_prompt,
                    content_libraries,
                    open_editor,
                    modifiers: egui::Modifiers::default(),
                    inline_editor_enabled: false,
                    bg_tx: &None,
                    file_event_producer: None,
                },
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

    /// TDD regression: clicking a directory row in the left
    /// directory tree must NOT clear the currently selected file
    /// or the multi-selection set.
    ///
    /// **Why this matters.** `render_tabs_and_content` in the
    /// center panel guards its body on
    /// `if let Some(selected_path) = app.selection().selected_file()`.
    /// If a directory click cleared `selected_file`, the body —
    /// the file's header, the YAML front-matter table, and the
    /// rendered markdown inside its `ScrollArea` — would be
    /// skipped on the next frame. The tab strip would still be
    /// visible, but the preview area would go blank. The right
    /// (TOC) panel would also disappear, because
    /// `should_show_panel(has_toc, has_selected_file)` requires
    /// a selected file. The user would have to click the file
    /// again to restore the preview, even though
    /// `tab_manager.current_markdown` / `current_yaml` /
    /// `loaded_path` were never touched.
    ///
    /// **The bug.** The directory-click branch in
    /// `render_flat_row` and `draw_tree_node` (legacy) used to
    /// unconditionally run `*ctx.selected_file() = None` and
    /// `ctx.selected_files().clear()`, conflating "expand this
    /// folder" with "deselect the open file." The two helpers
    /// now route through `apply_directory_row_click`, which
    /// only toggles `expanded_dirs` and refreshes `selected_dir`.
    ///
    /// **The contract pinned by this test.** After
    /// `apply_directory_row_click`:
    ///   * `selected_file` is unchanged.
    ///   * `selected_files` is unchanged.
    ///   * `tabs` is unchanged.
    ///   * `expanded_dirs` is toggled for `row.path`.
    ///   * `selected_dir` is set to `Some(row.path.clone())`
    ///     (the "current directory context" used by the
    ///     bottom-panel prompt prefix and the agent session).
    #[test]
    fn test_apply_directory_row_click_preserves_selected_file() {
        let mut tabs: Vec<PathBuf> = vec![PathBuf::from("doc.md")];
        let mut selected_file: Option<PathBuf> = Some(PathBuf::from("doc.md"));
        let mut selected_files: HashSet<PathBuf> = HashSet::new();
        selected_files.insert(PathBuf::from("doc.md"));
        let mut expanded_dirs: HashSet<PathBuf> = HashSet::new();
        // Pre-existing stale value to prove the click overwrites it
        // (mirrors the `apply_file_row_click` `selected_dir` test).
        let mut selected_dir: Option<PathBuf> = Some(PathBuf::from("C:/old/dir"));
        let dir_path = PathBuf::from("C:/notes/folder");
        let row = FlatRow {
            depth: 0,
            name: "folder".to_string(),
            path: dir_path.clone(),
            is_dir: true,
            is_expanded: false,
        };
        let mut ctx = TreeNodeContext {
            file_ops: FileOpsContext {
                file_to_move: &mut None,
                move_dialog_open: &mut false,
                file_to_rename: &mut None,
                rename_dialog_open: &mut false,
                rename_new_name: &mut String::new(),
            },
            dir_ops: DirOpsContext {
                selected_dir: &mut selected_dir,
                create_dir_dialog_open: &mut false,
                create_dir_parent: &mut None,
            },
            selection: SelectionContext {
                selected_file: &mut selected_file,
                selected_files: &mut selected_files,
                expanded_dirs: &mut expanded_dirs,
                tabs: &mut tabs,
            },
            app: AppIntegrationContext {
                layout: &mut PanelLayout::default(),
                submit_prompt: &mut None,
                content_libraries: &[],
                open_editor: &mut None,
                modifiers: egui::Modifiers::default(),
                inline_editor_enabled: false,
                bg_tx: &None,
                file_event_producer: None,
            },
        };

        apply_directory_row_click(&mut ctx, &row);

        // The contract: file selection and tabs are preserved.
        assert_eq!(
            selected_file,
            Some(PathBuf::from("doc.md")),
            "directory row click must NOT clear selected_file; clearing it \
             hides the center panel body and the right (TOC) panel"
        );
        assert!(
            selected_files.contains(&PathBuf::from("doc.md")),
            "directory row click must NOT clear selected_files"
        );
        assert_eq!(
            tabs,
            vec![PathBuf::from("doc.md")],
            "directory row click must NOT touch the open tabs"
        );
        // The actual purpose: expand the folder and refresh the
        // current-directory context.
        assert!(
            expanded_dirs.contains(&dir_path),
            "directory row click must add the folder to expanded_dirs"
        );
        assert_eq!(
            selected_dir,
            Some(dir_path.clone()),
            "directory row click must update selected_dir to the folder's path"
        );
    }

    /// TDD regression (companion to
    /// `test_apply_directory_row_click_preserves_selected_file`):
    /// the second click on an already-expanded directory must
    /// collapse it. The same invariant holds — the open file
    /// selection and the open tabs are NOT touched.
    ///
    /// This is a separate test rather than a follow-up call in
    /// the previous test, because the borrow checker treats two
    /// sequential `&mut ctx` calls as overlapping re-borrows of
    /// `ctx`'s inner fields; splitting the test lets each
    /// assertion set live independently of the next call.
    #[test]
    fn test_apply_directory_row_click_collapses_expanded_folder_preserves_selection() {
        let mut tabs: Vec<PathBuf> = vec![PathBuf::from("doc.md")];
        let mut selected_file: Option<PathBuf> = Some(PathBuf::from("doc.md"));
        let mut selected_files: HashSet<PathBuf> = HashSet::new();
        selected_files.insert(PathBuf::from("doc.md"));
        let mut expanded_dirs: HashSet<PathBuf> = HashSet::new();
        let dir_path = PathBuf::from("C:/notes/folder");
        expanded_dirs.insert(dir_path.clone());
        let mut selected_dir: Option<PathBuf> = Some(dir_path.clone());
        let row = FlatRow {
            depth: 0,
            name: "folder".to_string(),
            path: dir_path.clone(),
            is_dir: true,
            is_expanded: true,
        };
        let mut ctx = TreeNodeContext {
            file_ops: FileOpsContext {
                file_to_move: &mut None,
                move_dialog_open: &mut false,
                file_to_rename: &mut None,
                rename_dialog_open: &mut false,
                rename_new_name: &mut String::new(),
            },
            dir_ops: DirOpsContext {
                selected_dir: &mut selected_dir,
                create_dir_dialog_open: &mut false,
                create_dir_parent: &mut None,
            },
            selection: SelectionContext {
                selected_file: &mut selected_file,
                selected_files: &mut selected_files,
                expanded_dirs: &mut expanded_dirs,
                tabs: &mut tabs,
            },
            app: AppIntegrationContext {
                layout: &mut PanelLayout::default(),
                submit_prompt: &mut None,
                content_libraries: &[],
                open_editor: &mut None,
                modifiers: egui::Modifiers::default(),
                inline_editor_enabled: false,
                bg_tx: &None,
                file_event_producer: None,
            },
        };

        apply_directory_row_click(&mut ctx, &row);

        // Collapse: the folder is removed from `expanded_dirs`.
        assert!(
            !expanded_dirs.contains(&dir_path),
            "clicking an already-expanded directory must collapse it"
        );
        // Same invariant as the expand test: file selection and
        // tabs are untouched.
        assert_eq!(
            selected_file,
            Some(PathBuf::from("doc.md")),
            "collapsing a directory must NOT clear selected_file"
        );
        assert!(
            selected_files.contains(&PathBuf::from("doc.md")),
            "collapsing a directory must NOT clear selected_files"
        );
        assert_eq!(
            tabs,
            vec![PathBuf::from("doc.md")],
            "collapsing a directory must NOT touch the open tabs"
        );
        // `selected_dir` is refreshed to the directory's path
        // regardless of whether the click expanded or collapsed it.
        assert_eq!(
            selected_dir,
            Some(dir_path),
            "collapsing a directory must still update selected_dir to its path"
        );
    }
}
