use crate::ui::TreeNode;
use eframe::egui;
use std::collections::HashSet;
use std::path::PathBuf;

pub struct TreeNodeContext<'a> {
    pub expanded_dirs: &'a mut HashSet<PathBuf>,
    pub selected_file: &'a mut Option<PathBuf>,
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
}

pub fn draw_tree_node(ui: &mut egui::Ui, node: &TreeNode, ctx: &mut TreeNodeContext<'_>) {
    if node.is_dir {
        let is_expanded = ctx.expanded_dirs.contains(&node.path);
        let icon = if is_expanded { "📂 " } else { "📁 " };
        let label = format!("{}{}", icon, node.name);

        let is_selected = ctx.selected_dir.as_ref() == Some(&node.path);
        let response = ui.selectable_label(is_selected || is_expanded, label);
        if response.clicked() {
            if is_expanded {
                ctx.expanded_dirs.remove(&node.path);
            } else {
                ctx.expanded_dirs.insert(node.path.clone());
            }
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
                    eprintln!("Failed to create new document: {}", e);
                }
                ui.close_menu();
            }
            if ui.button("Delete").clicked() {
                if let Err(e) = trash::delete(&node.path) {
                    eprintln!("Failed to delete: {}", e);
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
        let is_selected = ctx.selected_file.as_ref() == Some(&node.path);
        let label = format!("📄 {}", node.name);
        let response = ui.selectable_label(is_selected, label);
        if response.clicked() {
            *ctx.selected_file = Some(node.path.clone());
            if !ctx.tabs.contains(&node.path) {
                ctx.tabs.push(node.path.clone());
            }
        }
        if response.double_clicked() {
            let _ = std::process::Command::new("cmd")
                .args(["/c", "start", "", &node.path.to_string_lossy()])
                .spawn();
        }

        response.context_menu(|ui| {
            if ui.button("Show in File Explorer").clicked() {
                let _ = std::process::Command::new("explorer")
                    .arg(format!("/select,{}", node.path.to_string_lossy()))
                    .spawn();
                ui.close_menu();
            }
            if ui.button("Copy path").clicked() {
                ui.output_mut(|o| o.copied_text = node.path.to_string_lossy().to_string());
                ui.close_menu();
            }
            if ui.button("Print").clicked() {
                let path_to_print = node.path.clone();
                std::thread::spawn(move || {
                    if let Ok(_content) = std::fs::read_to_string(&path_to_print) {
                        // Print functionality would go here
                        // crate::deploy::print_markdown(&path_to_print, &content);
                        eprintln!("Print requested for: {:?}", path_to_print);
                    }
                });
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
                    eprintln!("Failed to delete: {}", e);
                }
                ui.close_menu();
            }
        });
    }
}