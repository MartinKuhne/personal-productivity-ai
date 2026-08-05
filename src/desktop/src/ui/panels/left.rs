//! Left file-tree panel — builds `TreeNode` hierarchy from content libraries and discovered files, renders with tag filtering.
//!
//! Unit tests live in the sibling `left_tests.rs` sidecar.

use crate::bus::events::file::FileEventProducer;
use crate::ui::FastMdApp;
use crate::ui::TreeNode;
use crate::ui::TreeNodeContext;
use crate::ui::tree::{FlatRow, TREE_ROW_HEIGHT, flatten_tree, render_flat_row};
use eframe::egui;
use egui::RichText;
use egui::containers::Panel;
use egui::containers::panel::PanelState;

/// Recursively removes directory nodes that do not contain any child files or subdirectories.
fn prune_empty_dirs(node: &mut TreeNode) {
    node.children.retain(|_, child| {
        if child.is_dir {
            prune_empty_dirs(child);
            !child.children.is_empty()
        } else {
            true
        }
    });
}

/// Builds the `TreeNode` hierarchy from content libraries, discovered files, and tag filters.
pub fn build_workspace_tree(app: &FastMdApp) -> TreeNode {
    let filtered_files: Vec<&std::path::PathBuf> = app
        .file_processor()
        .all_files
        .iter()
        .filter(|p| {
            if let Some(active_tag) = &app.tags().selected_tag {
                if let Some(tags) = app.tags().file_tags().get(*p) {
                    tags.contains(active_tag)
                } else {
                    false
                }
            } else {
                true
            }
        })
        .collect();

    let mut root_node = TreeNode::new(
        crate::ui::strings::DEFAULT_WORKSPACE_NAME.to_string(),
        std::path::PathBuf::new(),
        true,
    );

    for lib in app.content_libraries() {
        let lib_node_name = lib.name.clone();
        let lib_root_path = std::path::PathBuf::from(&lib.root_folder);
        root_node
            .children
            .entry(lib_node_name.clone())
            .or_insert_with(|| TreeNode::new(lib_node_name.clone(), lib_root_path, true));
    }

    for path in filtered_files {
        let mut target_lib = None;
        let mut rel_path_res = None;

        for lib in app.content_libraries() {
            let lib_root = std::path::Path::new(&lib.root_folder);
            if let Ok(rel_path) = path.strip_prefix(lib_root) {
                target_lib = Some(lib);
                rel_path_res = Some(rel_path);
                break;
            }
        }

        if let (Some(lib), Some(rel_path)) = (target_lib, rel_path_res) {
            let lib_node_name = lib.name.clone();
            let Some(current_node_ref) = root_node.children.get_mut(&lib_node_name) else {
                continue;
            };
            let mut current_node = current_node_ref;
            let mut current_path = std::path::PathBuf::from(&lib.root_folder);

            let components: Vec<_> = rel_path.components().collect();
            for (i, comp) in components.iter().enumerate() {
                let name = comp.as_os_str().to_string_lossy().into_owned();
                current_path = current_path.join(&name);
                let is_last = i == components.len() - 1;
                let is_dir = !is_last;

                if !current_node.children.contains_key(&name) {
                    current_node.children.insert(
                        name.clone(),
                        TreeNode::new(name.clone(), current_path.clone(), is_dir),
                    );
                }
                match current_node.children.get_mut(&name) {
                    Some(n) => current_node = n,
                    None => break,
                }
            }
        }
    }

    if app.tags().selected_tag.is_none() {
        for dir in app.file_processor().all_dirs.iter() {
            let mut target_lib = None;
            let mut rel_path_res = None;

            for lib in app.content_libraries() {
                let lib_root = std::path::Path::new(&lib.root_folder);
                if let Ok(rel_path) = dir.strip_prefix(lib_root) {
                    target_lib = Some(lib);
                    rel_path_res = Some(rel_path);
                    break;
                }
            }

            if let (Some(lib), Some(rel_path)) = (target_lib, rel_path_res) {
                let lib_node_name = lib.name.clone();
                let Some(current_node_ref) = root_node.children.get_mut(&lib_node_name) else {
                    continue;
                };
                let mut current_node = current_node_ref;
                let mut current_path = std::path::PathBuf::from(&lib.root_folder);

                let components: Vec<_> = rel_path.components().collect();
                for comp in &components {
                    let name = comp.as_os_str().to_string_lossy().into_owned();
                    current_path = current_path.join(&name);
                    if !current_node.children.contains_key(&name) {
                        current_node.children.insert(
                            name.clone(),
                            TreeNode::new(name.clone(), current_path.clone(), true),
                        );
                    }
                    match current_node.children.get_mut(&name) {
                        Some(n) => current_node = n,
                        None => break,
                    }
                }
            }
        }
    } else {
        prune_empty_dirs(&mut root_node);
    }

    root_node
}

pub fn show_left_panel(app: &mut FastMdApp, parent_ui: &mut egui::Ui) {
    let ctx = parent_ui.ctx();
    let panel_id = parent_ui.make_persistent_id("left_panel");
    let indexing_just_finished =
        app.file_processor().indexing_finished && !app.file_processor().indexing_finished_handled;
    if indexing_just_finished || app.layout().left_panel_dirty {
        ctx.data_mut(|d| d.remove::<PanelState>(panel_id));
        app.file_processor_mut().indexing_finished_handled = true;
        let root_node = build_workspace_tree(app);
        fn calc_max_width(node: &TreeNode, depth: usize, ctx: &egui::Context) -> f32 {
            let mut max_w = 0.0_f32;
            for child in node.children.values() {
                let child_w = calc_max_width(child, depth + 1, ctx);
                if child_w > max_w {
                    max_w = child_w;
                }
            }
            if depth > 0 {
                let icon = if node.is_dir { "▶ " } else { "  " };
                let text = format!("{}{}", icon, node.name);
                // egui 0.35: `FontsView::layout_no_wrap` requires
                // `&mut self`, so we need `fonts_mut` rather than `fonts`.
                let text_w = ctx.fonts_mut(|f| {
                    f.layout_no_wrap(text, egui::FontId::proportional(14.0), egui::Color32::WHITE)
                        .size()
                        .x
                });
                let indent = (depth - 1) as f32 * 18.0;
                let my_w = indent + text_w + 40.0;
                if my_w > max_w {
                    max_w = my_w;
                }
            }
            max_w
        }
        let calculated = calc_max_width(&root_node, 0, ctx);
        // egui 0.35: `Context::available_rect` was removed. Use
        // `viewport_rect` (the full area available to egui, equivalent
        // to the old `available_rect` for this purpose) to size the
        // left panel.
        let max_allowed = ctx.viewport_rect().width() * 0.2;
        app.layout_mut().left_panel_width = Some(calculated.min(max_allowed));
        app.layout_mut().left_panel_dirty = false;
    }

    let max_w = ctx.viewport_rect().width() * 0.2;
    let default_w = app
        .layout()
        .left_panel_width
        .unwrap_or(280.0)
        .max(180.0)
        .min(max_w);

    // Rebuild tree rows only when dirty
    let tree_rows: Vec<FlatRow> = if app.selection().tree_dirty() {
        #[cfg(feature = "profiling")]
        puffin::profile_scope!("build_workspace_tree");
        let root_node = build_workspace_tree(app);
        let mut rows = Vec::new();
        if !root_node.children.is_empty() {
            flatten_tree(&root_node, 0, &app.selection().expanded_dirs, &mut rows);
        }
        app.cached_tree_rows = Some(rows.clone());
        *app.selection_mut().tree_dirty_mut() = false;
        rows
    } else if let Some(cached) = app.cached_tree_rows.take() {
        let rows = cached.clone();
        app.cached_tree_rows = Some(cached);
        rows
    } else {
        #[cfg(feature = "profiling")]
        puffin::profile_scope!("build_workspace_tree");
        let root_node = build_workspace_tree(app);
        let mut rows = Vec::new();
        if !root_node.children.is_empty() {
            flatten_tree(&root_node, 0, &app.selection().expanded_dirs, &mut rows);
        }
        app.cached_tree_rows = Some(rows.clone());
        rows
    };

    // egui 0.35 unified `SidePanel`/`TopBottomPanel` into `Panel`,
    // and panels now allocate within a parent `&mut Ui`.
    // `default_width` / `max_width` are now `default_size` / `max_size`.
    let panel_response = Panel::left("left_panel")
        .resizable(true)
        .default_size(default_w)
        .max_size(max_w)
        .show(parent_ui, |ui| {
            ui.add_space(4.0);
            ui.heading(
                RichText::new(crate::ui::strings::WORKSPACE_HEADER)
                    .size(16.0)
                    .strong(),
            );
            ui.add_space(4.0);

            // Single virtual-scroll container for the file tree.
            //
            // The previous revision wrapped this `ScrollArea::show_rows`
            // in another `ScrollArea` ("left_file_tree_scroll") which
            // is a known egui anti-pattern: the outer scroll area gives
            // the inner one *infinite* available height, so the inner
            // one balloons to the height of all rows, and any tiny
            // content-height drift between passes shifts every row's
            // rect. egui's `warn_if_rect_changes_id` then logs a
            // `WARN egui::context: Widget rect ... changed id between
            // passes` line for every shifted row on every frame —
            // dozens of warnings per frame in production. Removing
            // the redundant outer scroll area lets `show_rows` size
            // itself to the panel's available height, so the rects
            // are stable across passes.
            let mut open_editor = None;
            let _modifiers = ui.input(|i| i.modifiers);
            let pdf_backing_tracker = app.pdf_backing_tracker().clone();
            let selection = &mut app.orchestrator.selection;
            let tab_manager = &mut app.orchestrator.tab_manager;
            let dialogs = &mut app.orchestrator.dialogs;
            let layout = &mut app.layout;
            let submit_prompt = &mut app.orchestrator.submit_prompt;
            let content_libraries = &app.orchestrator.content_libraries;
            let inline_editor_enabled = app.orchestrator.inline_editor_enabled;
            let file_event_bus = &app.orchestrator.file_event_bus;
            let tx = app.orchestrator.tx.clone();

            egui::ScrollArea::vertical()
                .id_salt("virtual_tree_rows")
                .auto_shrink([false, false])
                .show_rows(ui, TREE_ROW_HEIGHT, tree_rows.len(), |ui, row_range| {
                    let mut ctx = TreeNodeContext {
                        selected_file: &mut selection.selected_file,
                        selected_files: &mut selection.selected_files,
                        expanded_dirs: &mut selection.expanded_dirs,
                        tabs: &mut tab_manager.tabs,
                        selected_dir: &mut selection.selected_dir,
                        create_dir_dialog_open: &mut dialogs.create_dir_dialog_open,
                        create_dir_parent: &mut dialogs.create_dir_parent,
                        file_to_move: &mut dialogs.file_to_move,
                        move_dialog_open: &mut dialogs.move_dialog_open,
                        file_to_rename: &mut dialogs.file_to_rename,
                        rename_dialog_open: &mut dialogs.rename_dialog_open,
                        rename_new_name: &mut dialogs.rename_new_name,
                        create_document_dialog_open: &mut dialogs.create_document_dialog_open,
                        create_document_parent: &mut dialogs.create_document_parent,
                        layout,
                        submit_prompt,
                        content_libraries,
                        open_editor: &mut open_editor,
                        modifiers: ui.input(|i| i.modifiers),
                        inline_editor_enabled,
                        bg_tx: &Some(tx.clone()),
                        file_event_producer: Some(FileEventProducer::new(file_event_bus)),
                        tree_dirty: &mut selection.tree_dirty,
                        pdf_backing_tracker: pdf_backing_tracker.clone(),
                    };

                    for i in row_range {
                        let row = &tree_rows[i];
                        render_flat_row(ui, row, &mut ctx);
                    }
                });

            // Empty-state placeholder must be rendered outside the
            // virtual-scroll `show_rows` (which would allocate zero
            // rows for an empty tree) and outside any conditional
            // that would add/remove a widget at the same rect on
            // successive passes — that conditional was itself a
            // source of id-clash warnings before the fix.
            if tree_rows.is_empty() {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(crate::ui::strings::NO_MARKDOWN_FILES)
                        .italics()
                        .color(egui::Color32::GRAY),
                );
            }

            if let Some(ref path) = open_editor
                && let Ok(content) = std::fs::read_to_string(path)
            {
                let is_pdf_backed = app.pdf_backing_tracker().is_pdf_backed(path);
                if !is_pdf_backed {
                    app.editor_mut().open(path, &content, None);
                }
            }
        });
    // Capture the panel's actual width after user interaction
    let rect = panel_response.response.rect;
    app.layout_mut().left_panel_width = Some(rect.width());
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `left_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "left_tests.rs"]
mod tests;
