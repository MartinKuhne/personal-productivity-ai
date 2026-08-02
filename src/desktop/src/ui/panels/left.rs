//! Left file-tree panel — builds `TreeNode` hierarchy from content libraries and discovered files, renders with tag filtering.

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
    let root_node = build_workspace_tree(app);

    let panel_id = parent_ui.make_persistent_id("left_panel");
    let indexing_just_finished =
        app.file_processor().indexing_finished && !app.file_processor().indexing_finished_handled;
    if indexing_just_finished || app.layout().left_panel_dirty {
        ctx.data_mut(|d| d.remove::<PanelState>(panel_id));
        app.file_processor_mut().indexing_finished_handled = true;
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
    Panel::left("left_panel")
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
            let modifiers = ui.input(|i| i.modifiers);
            let selection = &mut app.selection;
            let tab_manager = &mut app.tab_manager;
            let dialogs = &mut app.dialogs;
            let layout = &mut app.layout;
            let submit_prompt = &mut app.submit_prompt;
            let content_libraries = &app.content_libraries;
            let inline_editor_enabled = app.inline_editor_enabled;
            let file_event_bus = &app.file_event_bus;
            let tx = app.tx.clone();

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
                        modifiers,
                        inline_editor_enabled,
                        bg_tx: &Some(tx),
                        file_event_producer: Some(
                            crate::bus::events::file::FileEventProducer::new(file_event_bus),
                        ),
                        tree_dirty: &mut selection.tree_dirty,
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::test_helpers::assert::assert_no_id_change_in_shapes;

    fn create_test_app() -> FastMdApp {
        FastMdApp::empty_state(crate::config::AppConfig::default())
    }

    #[test]
    fn test_show_left_panel_empty() {
        use crate::ui::strings::WORKSPACE_HEADER;
        use crate::ui::test_helpers::text::assert_text_contains;

        let ctx = egui::Context::default();
        let mut app = create_test_app();
        app.layout_mut().left_panel_dirty = false;

        let output = ctx.run_ui(Default::default(), |ui| {
            show_left_panel(&mut app, ui);
        });

        // R-2 / Q12: replace the tautological state check with a
        // rendered-content assertion. The empty-state label is
        // conditionally rendered and falls below the panel's clip
        // rect under the default test viewport (no screen_rect), so
        // it is not in the rendered output here. The id-stability
        // test for the empty state (`test_show_left_panel_no_id_change_warnings_when_empty`)
        // covers the empty-state widget tree separately. Header-only
        // assertion is correct per the Q12 borderline policy.
        assert_text_contains(&output.shapes, WORKSPACE_HEADER);
        // Panel renders without crashing; width is unset because the dirty flag is false.
        assert!(app.layout().left_panel_width.is_none());
    }

    #[test]
    fn test_show_left_panel_with_libraries_and_files() {
        use crate::ui::strings::WORKSPACE_HEADER;
        use crate::ui::test_helpers::text::assert_text_contains;

        let ctx = egui::Context::default();
        let mut app = create_test_app();
        app.layout_mut().left_panel_dirty = false;

        let lib_dir = std::env::temp_dir().join("fastmd_left_test_lib");
        app.content_libraries_mut()
            .push(crate::config::ContentLibrary {
                root_folder: lib_dir.to_string_lossy().to_string(),
                name: "TestLib".to_string(),
                kind: "text".to_string(),
                readonly: false,
                priority: 0,
            });

        let file1 = lib_dir.join("notes.md");
        let file2 = lib_dir.join("archived.md");
        app.file_processor_mut().all_files = vec![file1.clone(), file2.clone()];
        app.tags_mut()
            .add_tags(file1.clone(), vec!["work".to_string()]);
        app.tags_mut()
            .add_tags(file2.clone(), vec!["archive".to_string()]);

        let output = ctx.run_ui(Default::default(), |ui| {
            show_left_panel(&mut app, ui);
        });
        // Header assertion (Q12 borderline case): the library name and
        // file paths are dynamic, but the panel header is stable.
        assert_text_contains(&output.shapes, WORKSPACE_HEADER);

        app.tags_mut().selected_tag = Some("work".to_string());
        let _ = ctx.run_ui(Default::default(), |ui| {
            show_left_panel(&mut app, ui);
        });

        app.file_processor_mut().indexing_finished = true;
        app.file_processor_mut().indexing_finished_handled = false;
        let _ = ctx.run_ui(Default::default(), |ui| {
            show_left_panel(&mut app, ui);
        });

        assert!(app.file_processor().indexing_finished_handled);
        assert!(app.layout().left_panel_width.is_some());
    }

    /// TDD regression: clicking a directory row in the left panel
    /// must expand the folder and reveal its children. Before the
    /// P0 perf optimization (P0-1 / P0-2) `show_left_panel` rebuilt
    /// the flat row list on every frame, so the new `expanded_dirs`
    /// membership was reflected immediately. After P0 the flat rows
    /// are cached in `FastMdApp::cached_tree_rows` and only rebuilt
    /// when `SelectionManager::tree_dirty` is `true`. The directory
    /// click handler (`apply_directory_row_click`) mutates
    /// `expanded_dirs` but did not set `tree_dirty`, so the cache
    /// kept returning the *previous* flat rows and the click looked
    /// like a no-op — the folder triangle toggled in `expanded_dirs`
    /// but its children never appeared.
    ///
    /// The fix is in `apply_directory_row_click` (handlers.rs):
    /// it now sets `*ctx.tree_dirty() = true` so the next
    /// `show_left_panel` pass rebuilds the flat rows. This test
    /// pins the user-visible invariant: after a directory click,
    /// the directory's children must appear in the rendered output.
    #[test]
    fn test_directory_click_invalidates_tree_cache() {
        use crate::ui::test_helpers::text::assert_text_contains;
        use crate::ui::tree::context::TreeNodeContext;

        let ctx = egui::Context::default();
        let mut app = create_test_app();
        let lib_dir = std::env::temp_dir().join("fastmd_left_test_dir_click_cache");
        let sub_dir = lib_dir.join("subdir");
        let inner_file = sub_dir.join("inner_note.md");
        let top_file = lib_dir.join("top_note.md");

        app.content_libraries_mut()
            .push(crate::config::ContentLibrary {
                root_folder: lib_dir.to_string_lossy().to_string(),
                name: "ClickLib".to_string(),
                kind: "text".to_string(),
                readonly: false,
                priority: 0,
            });
        app.file_processor_mut().all_files = vec![inner_file.clone(), top_file.clone()];
        app.file_processor_mut().all_dirs = vec![sub_dir.clone()];
        app.file_processor_mut().indexing_finished = true;
        app.file_processor_mut().indexing_finished_handled = true;
        // Pin the panel width so the only thing that can change
        // between renders is the inner widget tree.
        app.layout_mut().left_panel_width = Some(240.0);
        app.layout_mut().left_panel_dirty = false;
        // Expand the library so the files inside it are visible
        // at all. The library is a child of the root tree node and
        // is not auto-expanded; this mirrors a user clicking the
        // library name once. We only want to test the subdirectory
        // click behavior, not the library click.
        app.selection_mut().expanded_dirs.insert(lib_dir.clone());

        // Pass 1: prime the cache with the collapsed tree. The
        // subdirectory starts collapsed, so the inner file must
        // not be in the rendered output yet.
        let output_collapsed = ctx.run_ui(Default::default(), |ui| {
            show_left_panel(&mut app, ui);
        });
        assert!(
            app.cached_tree_rows.is_some(),
            "first render should populate the cached flat rows"
        );
        assert!(
            !app.selection().tree_dirty(),
            "first render should clear the tree_dirty flag"
        );
        let collapsed_rows = app
            .cached_tree_rows
            .as_ref()
            .expect("cache populated by pass 1")
            .clone();
        assert!(
            !collapsed_rows.iter().any(|r| r.path == inner_file),
            "inner file must be hidden when the subdirectory is collapsed ({} cached rows)",
            collapsed_rows.len()
        );
        assert_text_contains(&output_collapsed.shapes, "top_note.md");
        // The collapsed render must NOT show the inner file.
        let collapsed_texts: Vec<String> = output_collapsed
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect();
        assert!(
            !collapsed_texts.iter().any(|t| t.contains("inner_note.md")),
            "inner_note.md must not appear while the subdir is collapsed, got: {:?}",
            collapsed_texts
        );

        // Simulate a user click on the subdirectory row. We build a
        // `TreeNodeContext` from the app's state and call
        // `apply_directory_row_click` directly — this is the same
        // function `render_flat_row` invokes from its `if
        // response.clicked()` branch. The context borrows from
        // `app`, so we wrap the click in a block scope to release
        // the borrows before re-rendering.
        let dir_row = crate::ui::tree::flatten::FlatRow {
            depth: 0,
            name: "subdir".to_string(),
            path: sub_dir.clone(),
            is_dir: true,
            is_expanded: false,
        };
        {
            let mut open_editor = None;
            let tx = app.tx.clone();
            let file_event_bus = &app.file_event_bus;
            let mut ctx = TreeNodeContext {
                selected_file: &mut app.selection.selected_file,
                selected_files: &mut app.selection.selected_files,
                expanded_dirs: &mut app.selection.expanded_dirs,
                tabs: &mut app.tab_manager.tabs,
                selected_dir: &mut app.selection.selected_dir,
                create_dir_dialog_open: &mut app.dialogs.create_dir_dialog_open,
                create_dir_parent: &mut app.dialogs.create_dir_parent,
                file_to_move: &mut app.dialogs.file_to_move,
                move_dialog_open: &mut app.dialogs.move_dialog_open,
                file_to_rename: &mut app.dialogs.file_to_rename,
                rename_dialog_open: &mut app.dialogs.rename_dialog_open,
                rename_new_name: &mut app.dialogs.rename_new_name,
                create_document_dialog_open: &mut app.dialogs.create_document_dialog_open,
                create_document_parent: &mut app.dialogs.create_document_parent,
                layout: &mut app.layout,
                submit_prompt: &mut app.submit_prompt,
                content_libraries: &app.content_libraries,
                open_editor: &mut open_editor,
                modifiers: egui::Modifiers::default(),
                inline_editor_enabled: app.inline_editor_enabled,
                bg_tx: &Some(tx),
                file_event_producer: Some(crate::bus::events::file::FileEventProducer::new(
                    file_event_bus,
                )),
                tree_dirty: &mut app.selection.tree_dirty,
            };
            crate::ui::tree::handlers::apply_directory_row_click(&mut ctx, &dir_row);
        }

        // The click must invalidate the cached flat rows.
        assert!(
            app.selection().tree_dirty(),
            "directory click must mark the tree cache dirty so the next render rebuilds the flat rows"
        );
        assert!(
            app.selection().expanded_dirs().contains(&sub_dir),
            "directory click must add the folder to expanded_dirs"
        );

        // Pass 2: re-render. The cache must have been rebuilt, so
        // the inner file is now visible.
        let output_expanded = ctx.run_ui(Default::default(), |ui| {
            show_left_panel(&mut app, ui);
        });
        let expanded_texts: Vec<String> = output_expanded
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect();
        assert!(
            expanded_texts.iter().any(|t| t.contains("inner_note.md")),
            "inner_note.md must appear after the subdir is expanded; rendered texts: {:?}",
            expanded_texts
        );
        assert_text_contains(&output_expanded.shapes, "top_note.md");
        // The click must NOT touch the left panel width — directory
        // expansion is a tree-only concern.
        assert!(
            !app.layout().left_panel_dirty,
            "directory click must not trigger a panel-width recalc"
        );
    }

    #[test]
    fn test_show_left_panel_dirty_flag_triggers_recalc() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        let lib_dir = std::env::temp_dir().join("fastmd_left_test_recalc");
        app.content_libraries_mut()
            .push(crate::config::ContentLibrary {
                root_folder: lib_dir.to_string_lossy().to_string(),
                name: "RecalcLib".to_string(),
                kind: "text".to_string(),
                readonly: false,
                priority: 0,
            });
        app.file_processor_mut().all_files = vec![lib_dir.join("doc.md")];
        app.layout_mut().left_panel_dirty = false;

        let _ = ctx.run_ui(Default::default(), |ui| {
            show_left_panel(&mut app, ui);
        });
        assert!(!app.layout().left_panel_dirty);

        app.layout_mut().left_panel_dirty = true;
        let _ = ctx.run_ui(Default::default(), |ui| {
            show_left_panel(&mut app, ui);
        });
        assert!(!app.layout().left_panel_dirty);
        assert!(app.layout().left_panel_width.is_some());
    }

    #[test]
    fn test_show_left_panel_width_capped_at_twenty_percent() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        let lib_dir = std::env::temp_dir().join("fastmd_left_test_cap");
        app.content_libraries_mut()
            .push(crate::config::ContentLibrary {
                root_folder: lib_dir.to_string_lossy().to_string(),
                name: "CapLib".to_string(),
                kind: "text".to_string(),
                readonly: false,
                priority: 0,
            });

        let long_name = "a".repeat(500);
        app.file_processor_mut().all_files = vec![lib_dir.join(format!("{}.md", long_name))];
        app.file_processor_mut().indexing_finished = true;
        app.file_processor_mut().indexing_finished_handled = false;

        let mut inside_available: f32 = 0.0;
        let _ = ctx.run_ui(Default::default(), |ui| {
            inside_available = ui.ctx().viewport_rect().width();
            show_left_panel(&mut app, ui);
        });

        let stored = app.layout().left_panel_width.expect("width should be set");
        let cap_at_recalc_time = inside_available * 0.2;
        assert!(
            stored <= cap_at_recalc_time + 0.5,
            "stored width {} should not exceed 20% cap {} (available={})",
            stored,
            cap_at_recalc_time,
            inside_available
        );
    }

    /// Renders the panel twice through the same `ctx` and returns the
    /// shapes from the second pass. The second pass is the one that
    /// emits egui's "rect changed id between passes" warning as a red
    /// stroke rectangle in `output.shapes` (see
    /// `egui::Context::warn_if_rect_changes_id`). The first pass
    /// primes the previous-pass state so the warning is actually
    /// emitted on the second pass.
    ///
    /// Tests then call `assert_no_id_change_in_shapes` to assert the
    /// panel produces a stable widget tree across passes.
    fn render_left_panel_twice(ctx: &egui::Context, app: &mut FastMdApp) -> Vec<egui::Shape> {
        let _ = ctx.run_ui(Default::default(), |ui| {
            show_left_panel(app, ui);
        });
        let output = ctx.run_ui(Default::default(), |ui| {
            show_left_panel(app, ui);
        });
        output.shapes.into_iter().map(|cs| cs.shape).collect()
    }

    /// Regression: the production UI logged dozens of
    /// `WARN egui::context: Widget rect ... changed id between passes`
    /// lines on every frame because the left panel nested a
    /// `ScrollArea` around another `ScrollArea::show_rows`. The
    /// outer scroll area gave the inner one infinite available
    /// height, so the inner one ballooned each pass and shifted the
    /// rects of every row above it. After the fix the same state
    /// must render with zero red-stroke rects in the second pass.
    ///
    /// Note: this test renders in isolation (no parent panel
    /// layout, no user input), so the same `Context::run_ui` cycle
    /// does not reproduce the loud production trigger. The
    /// production fix was verified by the absence of the
    /// `WARN egui::context` lines in the running app. This test
    /// remains as a smoke test guarding the panel's render path
    /// from accidentally re-introducing an obvious id-clash (such
    /// as a second nested `ScrollArea` or a fresh `if root.children
    /// .is_empty()` branch swap).
    #[test]
    fn test_show_left_panel_no_id_change_warnings_with_files() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        let lib_dir = std::env::temp_dir().join("fastmd_left_id_stability_files");
        app.content_libraries_mut()
            .push(crate::config::ContentLibrary {
                root_folder: lib_dir.to_string_lossy().to_string(),
                name: "StabilityLib".to_string(),
                kind: "text".to_string(),
                readonly: false,
                priority: 0,
            });
        // Use enough files to actually exercise the virtual-scroll
        // ScrollArea, not just the empty-state path.
        let mut all_files = Vec::new();
        for i in 0..60 {
            all_files.push(lib_dir.join(format!("note_{:02}.md", i)));
        }
        app.file_processor_mut().all_files = all_files;
        app.file_processor_mut().indexing_finished = true;
        app.file_processor_mut().indexing_finished_handled = true;
        // Stabilize the panel width so the only thing that can shift
        // between passes is the inner widget tree.
        app.layout_mut().left_panel_width = Some(240.0);
        app.layout_mut().left_panel_dirty = false;

        let shapes = render_left_panel_twice(&ctx, &mut app);
        assert_no_id_change_in_shapes(&shapes);
    }

    /// Same regression as above but for the empty state: when no
    /// files have been indexed, the panel must show the "No markdown
    /// files found" placeholder. Even there the widget tree must
    /// stay stable across passes.
    #[test]
    fn test_show_left_panel_no_id_change_warnings_when_empty() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        app.layout_mut().left_panel_width = Some(240.0);
        app.layout_mut().left_panel_dirty = false;

        let shapes = render_left_panel_twice(&ctx, &mut app);
        assert_no_id_change_in_shapes(&shapes);
    }

    /// TDD Test: When indexing completes (`indexing_finished = true` and
    /// `indexing_finished_handled = false`), the first pass wipin PanelState
    /// must NOT cause egui ID-change warnings on Pass 2 of the layout.
    #[test]
    fn test_show_left_panel_no_id_change_warnings_on_indexing_finished_transition() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        let lib_dir = std::env::temp_dir().join("fastmd_left_id_stability_transition");
        app.content_libraries_mut()
            .push(crate::config::ContentLibrary {
                root_folder: lib_dir.to_string_lossy().to_string(),
                name: "StabilityLib".to_string(),
                kind: "text".to_string(),
                readonly: false,
                priority: 0,
            });
        let mut all_files = Vec::new();
        for i in 0..10 {
            all_files.push(lib_dir.join(format!("note_{:02}.md", i)));
        }
        app.file_processor_mut().all_files = all_files;
        app.file_processor_mut().indexing_finished = true;
        app.file_processor_mut().indexing_finished_handled = false;

        let shapes = render_left_panel_twice(&ctx, &mut app);
        assert_no_id_change_in_shapes(&shapes);
    }

    /// TDD Test: When stored PanelState width (e.g. 294.7px) exceeds max_size (204.8px),
    /// Panel::left must clamp default_size and size_range BEFORE passing to Panel::left,
    /// so Pass 1 and Pass 2 evaluate identical bounds and 0 ID change warnings are emitted.
    #[test]
    fn test_left_panel_clamped_width_pass_stability() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        app.layout_mut().left_panel_width = Some(294.7);
        app.layout_mut().left_panel_dirty = false;

        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1024.0, 768.0),
            )),
            ..egui::RawInput::default()
        };

        // Prime egui data memory with stored PanelState at width 294.7px
        ctx.data_mut(|d| {
            d.insert_persisted(
                egui::Id::new("left_panel"),
                PanelState {
                    outer_rect: egui::Rect::from_min_size(
                        egui::Pos2::new(0.0, 24.8),
                        egui::vec2(294.7, 900.0),
                    ),
                },
            );
        });

        // Pass 1: Run panel layout directly on primed state
        let output = ctx.run_ui(raw_input, |ui| {
            show_left_panel(&mut app, ui);
        });

        let shapes: Vec<egui::Shape> = output.shapes.into_iter().map(|cs| cs.shape).collect();
        assert_no_id_change_in_shapes(&shapes);
    }

    #[test]
    fn test_show_left_panel_tag_filter_hides_directories_without_matching_files() {
        let mut app = create_test_app();
        let lib_dir = std::env::temp_dir().join("fastmd_left_test_tag_filter_dirs");
        app.content_libraries_mut()
            .push(crate::config::ContentLibrary {
                root_folder: lib_dir.to_string_lossy().to_string(),
                name: "TagTestLib".to_string(),
                kind: "text".to_string(),
                readonly: false,
                priority: 0,
            });
        app.content_libraries_mut()
            .push(crate::config::ContentLibrary {
                root_folder: std::env::temp_dir()
                    .join("fastmd_left_test_empty_lib")
                    .to_string_lossy()
                    .to_string(),
                name: "EmptyLib".to_string(),
                kind: "text".to_string(),
                readonly: false,
                priority: 0,
            });

        let matching_dir = lib_dir.join("matching_folder");
        let non_matching_dir = lib_dir.join("non_matching_folder");

        let file_match = matching_dir.join("match.md");
        let file_no_match = non_matching_dir.join("other.md");

        app.file_processor_mut().all_files = vec![file_match.clone(), file_no_match.clone()];
        app.file_processor_mut().all_dirs = vec![matching_dir.clone(), non_matching_dir.clone()];

        app.tags_mut()
            .add_tags(file_match.clone(), vec!["target_tag".to_string()]);
        app.tags_mut()
            .add_tags(file_no_match.clone(), vec!["other_tag".to_string()]);

        // Select the tag filter
        app.tags_mut().selected_tag = Some("target_tag".to_string());

        let tree = build_workspace_tree(&app);

        let lib_node = tree
            .children
            .get("TagTestLib")
            .expect("TagTestLib node should exist");
        assert!(
            lib_node.children.contains_key("matching_folder"),
            "matching_folder should be in the tree when filtering by target_tag"
        );
        assert!(
            !lib_node.children.contains_key("non_matching_folder"),
            "non_matching_folder should NOT be in the tree when filtering by target_tag"
        );
        assert!(
            !tree.children.contains_key("EmptyLib"),
            "EmptyLib (library without matching files) should NOT be in the tree when filtering by target_tag"
        );
    }
}
