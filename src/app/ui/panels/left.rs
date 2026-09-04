//! Left file-tree panel — builds `TreeNode` hierarchy from content libraries and discovered files, renders with tag filtering.
//!
//! Unit tests live in the sibling `left_tests.rs` sidecar.

use std::sync::Arc;

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
#[tracing::instrument(skip_all, name = "ui.left.build_workspace_tree", level = "debug")]
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

#[tracing::instrument(skip_all, name = "ui.panel.left", level = "debug")]
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
                let icon = if node.is_dir {
                    egui_phosphor::regular::CARET_RIGHT
                } else {
                    " "
                };
                let text = format!("{} {}", icon, node.name);
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

    // Rebuild tree rows only when dirty or not yet cached
    let tree_rows: Arc<Vec<FlatRow>> = if !app.selection().tree_dirty()
        && let Some(cached) = &app.cached_tree_rows
    {
        Arc::clone(cached)
    } else {
        let root_node = build_workspace_tree(app);
        let mut rows = Vec::new();
        if !root_node.children.is_empty() {
            flatten_tree(&root_node, 0, &app.selection().expanded_dirs, &mut rows);
        }
        let rows_arc = Arc::new(rows);
        app.cached_tree_rows = Some(Arc::clone(&rows_arc));
        *app.selection_mut().tree_dirty_mut() = false;
        rows_arc
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

            // Workspace content search box with magnifying glass trigger (UI-050)
            ui.horizontal(|ui| {
                let text_resp = ui.add(
                    egui::TextEdit::singleline(app.search_mut().query_mut())
                        .hint_text(crate::ui::strings::SEARCH_PLACEHOLDER)
                        .desired_width((ui.available_width() - 56.0).max(80.0)),
                );
                let enter_pressed = text_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let search_clicked = ui
                    .button(egui_phosphor::regular::MAGNIFYING_GLASS)
                    .on_hover_text(crate::ui::strings::SEARCH_TRIGGER_TOOLTIP)
                    .clicked();

                if (search_clicked || enter_pressed) && !app.search().query().trim().is_empty() {
                    let files = app.file_processor().all_files.clone();
                    let libs = app.content_libraries().to_vec();
                    app.search_mut().apply(&files, &libs);
                }

                if app.search().is_searching()
                    && ui
                        .button(egui_phosphor::regular::X)
                        .on_hover_text(crate::ui::strings::SEARCH_CLEAR_TOOLTIP)
                        .clicked()
                {
                    app.search_mut().clear();
                }
            });
            ui.add_space(6.0);

            if app.search().is_searching() {
                let results = app.search().results().to_vec();
                let active_query = app.search().active_filter().unwrap_or("").to_string();

                if results.is_empty() {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(crate::ui::strings::SEARCH_NO_RESULTS)
                            .italics()
                            .color(egui::Color32::GRAY),
                    );
                } else {
                    const SEARCH_ROW_HEIGHT: f32 = 52.0;
                    egui::ScrollArea::vertical()
                        .id_salt("search_results_scroll")
                        .auto_shrink([false, false])
                        .show_rows(ui, SEARCH_ROW_HEIGHT, results.len(), |ui, row_range| {
                            for i in row_range {
                                let entry = &results[i];
                                let is_selected = app.selection().selected_file() == Some(&entry.path);

                                ui.push_id((&entry.path, "search_result_entry"), |ui| {
                                    let bg_color = if is_selected {
                                        ui.visuals().selection.bg_fill
                                    } else {
                                        egui::Color32::from_rgb(18, 18, 20)
                                    };

                                    let frame_resp = egui::Frame::NONE
                                        .fill(bg_color)
                                        .corner_radius(4.0)
                                        .inner_margin(egui::Margin::symmetric(6, 4))
                                        .show(ui, |ui| {
                                            ui.set_width(ui.available_width());
                                            ui.vertical(|ui| {
                                                ui.horizontal(|ui| {
                                                    ui.label(RichText::new(&entry.file_name).strong());
                                                    if entry.match_count > 1 {
                                                        ui.label(
                                                            RichText::new(format!("({})", entry.match_count))
                                                                .size(11.0)
                                                                .color(egui::Color32::from_gray(140)),
                                                        );
                                                    }
                                                });
                                                if !entry.relative_path.is_empty() && entry.relative_path != entry.file_name {
                                                    ui.label(
                                                        RichText::new(&entry.relative_path)
                                                            .size(11.0)
                                                            .color(egui::Color32::from_gray(140)),
                                                    );
                                                }
                                                if !entry.snippet.is_empty() {
                                                    ui.label(
                                                        RichText::new(&entry.snippet)
                                                            .size(11.0)
                                                            .italics()
                                                            .color(egui::Color32::from_rgb(160, 190, 220)),
                                                    );
                                                }
                                            });
                                        });

                                    let interact = ui.interact(
                                        frame_resp.response.rect,
                                        frame_resp.response.id,
                                        egui::Sense::click(),
                                    );
                                    if interact.clicked() {
                                        app.orchestrator.user_command_bus.publish(
                                            crate::bus::events::user_command::UserCommand::SelectFile {
                                                path: entry.path.clone(),
                                                multi: false,
                                            },
                                        );
                                        app.orchestrator.tabs.scroll_to_search = Some(active_query.clone());
                                    }
                                });
                                ui.add_space(2.0);
                            }
                        });
                }
            } else {
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
                // Build the per-frame view object once, render all
                // rows into it, then write the (possibly modified)
                // fields back to the orchestrator. Owning the
                // fields in `TreeNodeContext` (no `&'a mut T`)
                // means the per-row closure doesn't have to re-borrow
                // from `selection` / `dialogs` / etc. on every pass,
                // and tests can construct a `TreeNodeContext` directly
                // without any `Box::leak`.
                //
                let mut ctx = TreeNodeContext::from_app_state(
                    &app.orchestrator.selection,
                    &app.orchestrator.tabs,
                    &app.layout,
                    &app.orchestrator.content_libraries,
                    Some(app.orchestrator.tx.clone()),
                    app.orchestrator.file_event_bus.clone(),
                    app.orchestrator.inline_editor_enabled,
                    ui.input(|i| i.modifiers),
                    app.pdf_backing_tracker().clone(),
                    app.orchestrator.user_command_bus.clone(),
                );

                egui::ScrollArea::vertical()
                    .id_salt("virtual_tree_rows")
                    .auto_shrink([false, false])
                    .show_rows(ui, TREE_ROW_HEIGHT, tree_rows.len(), |ui, row_range| {
                        for i in row_range {
                            let row = &tree_rows[i];
                            render_flat_row(ui, row, &mut ctx);
                        }
                    });

                // Commit the (possibly mutated) view object back to
                // the orchestrator. `bg_tx`, `file_event_producer`,
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
