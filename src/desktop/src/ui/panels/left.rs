use crate::ui::FastMdApp;
use crate::ui::{TreeNode, TreeNodeContext};
use eframe::egui;
use egui::RichText;

pub fn show_left_panel(app: &mut FastMdApp, ctx: &egui::Context) {
    let filtered_files: Vec<&std::path::PathBuf> = app
        .all_files
        .iter()
        .filter(|p| {
            if let Some(active_tag) = &app.selected_tag {
                if let Some(tags) = app.file_tags.get(*p) {
                    tags.contains(active_tag)
                } else {
                    false
                }
            } else {
                true
            }
        })
        .collect();

    let mut root_node = TreeNode::new("Workspace".to_string(), std::path::PathBuf::new(), true);

    for lib in &app.content_libraries {
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

        for lib in &app.content_libraries {
            let lib_root = std::path::Path::new(&lib.root_folder);
            if let Ok(rel_path) = path.strip_prefix(lib_root) {
                target_lib = Some(lib);
                rel_path_res = Some(rel_path);
                break;
            }
        }

        if let (Some(lib), Some(rel_path)) = (target_lib, rel_path_res) {
            let lib_node_name = lib.name.clone();
            let mut current_node = root_node.children.get_mut(&lib_node_name).unwrap();
            let mut current_path = std::path::PathBuf::from(&lib.root_folder);

            let components: Vec<_> = rel_path.components().collect();
            for (i, comp) in components.iter().enumerate() {
                let name = comp.as_os_str().to_string_lossy().into_owned();
                current_path = current_path.join(&name);
                let is_last = i == components.len() - 1;
                let is_dir = !is_last;

                current_node
                    .children
                    .entry(name.clone())
                    .or_insert_with(|| TreeNode::new(name.clone(), current_path.clone(), is_dir));
                current_node = current_node.children.get_mut(&name).unwrap();
            }
        }
    }

    if app.indexing_finished && !app.indexing_finished_handled {
        app.indexing_finished_handled = true;
        fn calc_max_width(node: &TreeNode, depth: usize, ctx: &egui::Context) -> f32 {
            let mut max_w = 0.0_f32;
            for child in node.children.values() {
                let child_w = calc_max_width(child, depth + 1, ctx);
                if child_w > max_w {
                    max_w = child_w;
                }
            }
            if depth > 0 {
                let icon = if node.is_dir { "📁 " } else { "📄 " };
                let text = format!("{}{}", icon, node.name);
                let text_w = ctx.fonts(|f| {
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
        app.left_panel_width = Some(calculated);
        app.left_panel_reset_count += 1;
    }

    let mut default_w: f32 = 280.0;
    if let Some(w) = app.left_panel_width {
        default_w = default_w.max(w);
    }
    let max_w = (ctx.available_rect().width() * 0.2).max(default_w);

    egui::SidePanel::left(egui::Id::new("left_panel").with(app.left_panel_reset_count))
        .resizable(true)
        .default_width(default_w)
        .max_width(max_w)
        .show(ctx, |ui| {
            ui.add_space(4.0);
            ui.heading(RichText::new("Workspace Files").size(16.0).strong());
            ui.add_space(4.0);

            egui::ScrollArea::vertical()
                .id_source("left_file_tree_scroll")
                .show(ui, |ui| {
                    let mut open_editor = None;
                    if root_node.children.is_empty() {
                        ui.label(
                            RichText::new("No markdown files found.")
                                .italics()
                                .color(egui::Color32::GRAY),
                        );
                    } else {
                        let mut children: Vec<_> = root_node.children.values().collect();
                        children.sort_by(|a, b| {
                            b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name))
                        });
                        let modifiers = ui.input(|i| i.modifiers);
                        for child in children {
                            let mut ctx = TreeNodeContext {
                                expanded_dirs: &mut app.expanded_dirs,
                                selected_file: &mut app.selected_file,
                                selected_files: &mut app.selected_files,
                                tabs: &mut app.tabs,
                                file_to_move: &mut app.file_to_move,
                                move_dialog_open: &mut app.move_dialog_open,
                                selected_dir: &mut app.selected_dir,
                                create_dir_dialog_open: &mut app.create_dir_dialog_open,
                                create_dir_parent: &mut app.create_dir_parent,
                                left_panel_reset_count: &mut app.left_panel_reset_count,
                                rename_dialog_open: &mut app.rename_dialog_open,
                                file_to_rename: &mut app.file_to_rename,
                                rename_new_name: &mut app.rename_new_name,
                                modifiers,
                                submit_prompt: &mut app.submit_prompt,
                                content_libraries: &app.content_libraries,
                                open_editor: &mut open_editor,
                                inline_editor_enabled: app.inline_editor_enabled,
                                bg_tx: &Some(app.tx.clone()),
                            };
                            crate::ui::tree::draw_tree_node(ui, child, &mut ctx);
                        }
                    }

                    if let Some(path) = open_editor {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            app.editor_state.open(&path, &content);
                        }
                    }
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_app() -> FastMdApp {
        FastMdApp::empty_state()
    }

    #[test]
    fn test_show_left_panel_empty() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();

        let _ = ctx.run(Default::default(), |ctx| {
            show_left_panel(&mut app, ctx);
        });

        assert_eq!(app.left_panel_reset_count, 0);
    }

    #[test]
    fn test_show_left_panel_with_libraries_and_files() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();

        let lib_dir = std::env::temp_dir().join("fastmd_left_test_lib");
        app.content_libraries.push(crate::config::ContentLibrary {
            root_folder: lib_dir.to_string_lossy().to_string(),
            name: "TestLib".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        });

        let file1 = lib_dir.join("notes.md");
        let file2 = lib_dir.join("archived.md");
        app.all_files = vec![file1.clone(), file2.clone()];
        app.file_tags
            .insert(file1.clone(), vec!["work".to_string()]);
        app.file_tags
            .insert(file2.clone(), vec!["archive".to_string()]);

        // 1. Without tag filter
        let _ = ctx.run(Default::default(), |ctx| {
            show_left_panel(&mut app, ctx);
        });

        // 2. With tag filter matching file1
        app.selected_tag = Some("work".to_string());
        let _ = ctx.run(Default::default(), |ctx| {
            show_left_panel(&mut app, ctx);
        });

        // 3. Indexing finished width calculation
        app.indexing_finished = true;
        app.indexing_finished_handled = false;
        let _ = ctx.run(Default::default(), |ctx| {
            show_left_panel(&mut app, ctx);
        });

        assert!(app.indexing_finished_handled);
        assert!(app.left_panel_width.is_some());
        assert_eq!(app.left_panel_reset_count, 1);
    }
}
