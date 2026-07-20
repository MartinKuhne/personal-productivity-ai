use crate::messages::BackgroundMessage;
use crate::print::{execute_print_blocking, PrintJob};
use crate::ui::TreeNode;
use eframe::egui;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

pub struct TreeNodeContext<'a> {
    pub expanded_dirs: &'a mut HashSet<PathBuf>,
    pub selected_file: &'a mut Option<PathBuf>,
    pub selected_files: &'a mut HashSet<PathBuf>,
    pub tabs: &'a mut Vec<PathBuf>,
    pub file_to_move: &'a mut Option<PathBuf>,
    pub move_dialog_open: &'a mut bool,
    pub selected_dir: &'a mut Option<PathBuf>,
    pub create_dir_dialog_open: &'a mut bool,
    pub create_dir_parent: &'a mut Option<PathBuf>,
    pub left_panel_reset_count: &'a mut u32,
    pub rename_dialog_open: &'a mut bool,
    pub file_to_rename: &'a mut Option<PathBuf>,
    pub rename_new_name: &'a mut String,
    pub modifiers: egui::Modifiers,
    pub submit_prompt: &'a mut Option<String>,
    pub content_libraries: &'a [crate::config::ContentLibrary],
    pub open_editor: &'a mut Option<PathBuf>,
    pub inline_editor_enabled: bool,
    pub bg_tx: &'a Option<Sender<BackgroundMessage>>,
}

pub fn draw_tree_node(ui: &mut egui::Ui, node: &TreeNode, ctx: &mut TreeNodeContext<'_>) {
    if node.is_dir {
        let is_expanded = ctx.expanded_dirs.contains(&node.path);
        let icon = if is_expanded { "📂 " } else { "📁 " };
        let label = format!("{}{}", icon, node.name);

        let response = ui.selectable_label(false, label);
        if response.clicked() {
            if is_expanded {
                ctx.expanded_dirs.remove(&node.path);
            } else {
                ctx.expanded_dirs.insert(node.path.clone());
            }
            *ctx.selected_file = None;
            ctx.selected_files.clear();
            *ctx.selected_dir = Some(node.path.clone());
        }
        if response.double_clicked() {
            *ctx.left_panel_reset_count += 1;
        }

        response.context_menu(|ui| {
            if ui.button("Show in File Explorer").clicked() {
                let _ = std::process::Command::new("explorer")
                    .arg(&node.path)
                    .spawn();
                ui.close_menu();
            }
            if ui.button("Copy path").clicked() {
                ui.output_mut(|o| o.copied_text = node.path.to_string_lossy().to_string());
                ui.close_menu();
            }
            if ui.button("Rename").clicked() {
                *ctx.file_to_rename = Some(node.path.clone());
                *ctx.rename_new_name = node.name.clone();
                *ctx.rename_dialog_open = true;
                ui.close_menu();
            }
            if ui.button("Move").clicked() {
                *ctx.file_to_move = Some(node.path.clone());
                *ctx.move_dialog_open = true;
                ui.close_menu();
            }
            if ui.button("Create Directory ...").clicked() {
                *ctx.create_dir_parent = Some(node.path.clone());
                *ctx.create_dir_dialog_open = true;
                ui.close_menu();
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
                }
                ui.close_menu();
            }
            if ui.button("Delete").clicked() {
                if let Err(e) = trash::delete(&node.path) {
                    tracing::error!(
                        name = "ui.directory.delete_failed",
                        path = %node.path.display(),
                        error = %e,
                        "Failed to delete directory to trash. Likely cause: directory in use or permission denied. Operator should check file locks."
                    );
                }
                ui.close_menu();
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
        let is_selected = ctx.selected_files.contains(&node.path)
            || ctx.selected_file.as_ref() == Some(&node.path);
        let label = format!("📄 {}", node.name);
        let response = ui.selectable_label(is_selected, label);

        if response.clicked() {
            if ctx.modifiers.shift || ctx.modifiers.ctrl || ctx.modifiers.command {
                if ctx.selected_files.contains(&node.path) {
                    ctx.selected_files.remove(&node.path);
                    if ctx.selected_file.as_ref() == Some(&node.path) {
                        *ctx.selected_file = None;
                    }
                } else {
                    ctx.selected_files.insert(node.path.clone());
                    *ctx.selected_file = Some(node.path.clone());
                }
            } else {
                ctx.selected_files.clear();
                ctx.selected_files.insert(node.path.clone());
                *ctx.selected_file = Some(node.path.clone());
                if !ctx.tabs.contains(&node.path) {
                    ctx.tabs.push(node.path.clone());
                }
            }
        }

        if response.double_clicked() {
            if ctx.inline_editor_enabled {
                *ctx.open_editor = Some(node.path.clone());
            } else {
                crate::ui::open_in_system_editor(&node.path);
            }
        }

        response.context_menu(|ui| {
            if ctx.selected_files.len() > 1 && ctx.selected_files.contains(&node.path) {
                // Multi-select context menu
                if ui.button("Merge").clicked() {
                    let mut prompt = "Please merge the following documents into a new document:\n".to_string();
                    for file in ctx.selected_files.iter() {
                        let rel_str = crate::config::library_display_label(ctx.content_libraries, file)
                            .unwrap_or_else(|| file.to_string_lossy().to_string());
                        prompt.push_str(&format!("- {}\n", rel_str));
                    }
                    *ctx.submit_prompt = Some(prompt);
                    ui.close_menu();
                }
                if ui.button("Delete").clicked() {
                    for file in ctx.selected_files.iter() {
                        if let Err(e) = trash::delete(file) {
                            tracing::error!(
                                name = "ui.file.multi_delete_failed",
                                path = %file.display(),
                                error = %e,
                                "Failed to delete file to trash during multi-selection. Likely cause: file in use or permission denied. Operator should check file locks."
                            );
                        }
                    }
                    ctx.selected_files.clear();
                    ui.close_menu();
                }
            } else {
                // Single-select context menu
                if ui.button("Edit").clicked() {
                    if ctx.inline_editor_enabled {
                        *ctx.open_editor = Some(node.path.clone());
                    } else {
                        crate::ui::open_in_system_editor(&node.path);
                    }
                    ui.close_menu();
                }
                if ui.button("Show in File Explorer").clicked() {
                    crate::ui::show_in_file_explorer(&node.path);
                    ui.close_menu();
                }
                if ui.button("Copy path").clicked() {
                    ui.output_mut(|o| o.copied_text = node.path.to_string_lossy().to_string());
                    ui.close_menu();
                }
                if ui.button("Format Markdown").clicked() {
                    let now = chrono::Local::now();
                    let date_str = now.to_rfc3339();
                    *ctx.submit_prompt = Some(crate::ui::generate_format_prompt(&date_str));
                    ui.close_menu();
                }
                if ui.button("Run as prompt").clicked() {
                    if let Ok(content) = std::fs::read_to_string(&node.path) {
                        *ctx.submit_prompt = Some(content);
                    } else {
                        tracing::error!(
                            name = "ui.file.run_as_prompt_failed",
                            path = %node.path.display(),
                            "Failed to read file content to run as prompt."
                        );
                    }
                    ui.close_menu();
                }
                if ui.button("Print").clicked() {
                    let path_to_print = node.path.clone();
                    if let Some(tx) = ctx.bg_tx.clone() {
                        let job = PrintJob::new(path_to_print.clone());
                        let _ = execute_print_blocking(job, Some(tx));
                    } else {
                        tracing::warn!(
                            name = "ui.file.print_no_channel",
                            path = %path_to_print.display(),
                            "Print requested but no background channel available"
                        );
                    }
                    ui.close_menu();
                }
                if ui.button("Rename").clicked() {
                    *ctx.file_to_rename = Some(node.path.clone());
                    *ctx.rename_new_name = node.name.clone();
                    *ctx.rename_dialog_open = true;
                    ui.close_menu();
                }
                if ui.button("Move").clicked() {
                    *ctx.file_to_move = Some(node.path.clone());
                    *ctx.move_dialog_open = true;
                    ui.close_menu();
                }
                if ui.button("Delete").clicked() {
                    if let Err(e) = trash::delete(&node.path) {
                        tracing::error!(
                            name = "ui.file.delete_failed",
                            path = %node.path.display(),
                            error = %e,
                            "Failed to delete file to trash. Likely cause: file in use or permission denied. Operator should check file locks."
                        );
                    }
                    ui.close_menu();
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::PathBuf;

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
        let mut left_panel_reset_count = 0;
        let mut rename_dialog_open = false;
        let mut file_to_rename = None;
        let mut rename_new_name = String::new();
        let mut submit_prompt = None;
        let mut open_editor = None;

        let _ = ctx_egui.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut tree_ctx = TreeNodeContext {
                    expanded_dirs: &mut expanded_dirs,
                    selected_file: &mut selected_file,
                    selected_files: &mut selected_files,
                    tabs: &mut tabs,
                    file_to_move: &mut file_to_move,
                    move_dialog_open: &mut move_dialog_open,
                    selected_dir: &mut selected_dir,
                    create_dir_dialog_open: &mut create_dir_dialog_open,
                    create_dir_parent: &mut create_dir_parent,
                    left_panel_reset_count: &mut left_panel_reset_count,
                    rename_dialog_open: &mut rename_dialog_open,
                    file_to_rename: &mut file_to_rename,
                    rename_new_name: &mut rename_new_name,
                    modifiers: egui::Modifiers::default(),
                    submit_prompt: &mut submit_prompt,
                    content_libraries: &[],
                    open_editor: &mut open_editor,
                    inline_editor_enabled: true,
                    bg_tx: &None,
                };

                // Render collapsed directory
                draw_tree_node(ui, &root, &mut tree_ctx);

                // Render expanded directory with child file
                tree_ctx.expanded_dirs.insert(root.path.clone());
                draw_tree_node(ui, &root, &mut tree_ctx);

                // Render standalone file node
                draw_tree_node(ui, &child_file, &mut tree_ctx);
            });
        });

        assert!(expanded_dirs.contains(&root.path));
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
        let mut left_panel_reset_count = 0;
        let mut rename_dialog_open = false;
        let mut file_to_rename = None;
        let mut rename_new_name = String::new();
        let mut submit_prompt = None;
        let mut open_editor = None;

        let _ = ctx_egui.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                // Test ctrl multi-select simulation
                let mut tree_ctx = TreeNodeContext {
                    expanded_dirs: &mut expanded_dirs,
                    selected_file: &mut selected_file,
                    selected_files: &mut selected_files,
                    tabs: &mut tabs,
                    file_to_move: &mut file_to_move,
                    move_dialog_open: &mut move_dialog_open,
                    selected_dir: &mut selected_dir,
                    create_dir_dialog_open: &mut create_dir_dialog_open,
                    create_dir_parent: &mut create_dir_parent,
                    left_panel_reset_count: &mut left_panel_reset_count,
                    rename_dialog_open: &mut rename_dialog_open,
                    file_to_rename: &mut file_to_rename,
                    rename_new_name: &mut rename_new_name,
                    modifiers: egui::Modifiers {
                        ctrl: true,
                        ..Default::default()
                    },
                    submit_prompt: &mut submit_prompt,
                    content_libraries: &[],
                    open_editor: &mut open_editor,
                    inline_editor_enabled: true,
                    bg_tx: &None,
                };

                draw_tree_node(ui, &file1, &mut tree_ctx);
                draw_tree_node(ui, &file2, &mut tree_ctx);
            });
        });
    }
}
