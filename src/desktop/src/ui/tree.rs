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
                let _ = std::process::Command::new("cmd")
                    .args(["/c", "start", "", &node.path.to_string_lossy()])
                    .spawn();
            }
        }

        response.context_menu(|ui| {
            if ctx.selected_files.len() > 1 && ctx.selected_files.contains(&node.path) {
                // Multi-select context menu
                if ui.button("Merge").clicked() {
                    let mut prompt = "Please merge the following documents into a new document:\n".to_string();
                    for file in ctx.selected_files.iter() {
                        let mut rel_str = file.to_string_lossy().to_string();
                        for lib in ctx.content_libraries {
                            if let Ok(rel) = file.strip_prefix(std::path::Path::new(&lib.root_folder)) {
                                let lib_path = std::path::Path::new(&lib.name).join(rel);
                                rel_str = lib_path.to_string_lossy().to_string();
                                break;
                            }
                        }
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
                        let _ = std::process::Command::new("cmd")
                            .args(["/c", "start", "", &node.path.to_string_lossy()])
                            .spawn();
                    }
                    ui.close_menu();
                }
                if ui.button("Show in File Explorer").clicked() {
                    #[cfg(target_os = "windows")]
                    {
                        use std::os::windows::process::CommandExt;
                        let _ = std::process::Command::new("explorer")
                            .raw_arg(format!("/select,\"{}\"", node.path.to_string_lossy()))
                            .spawn();
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        let _ = std::process::Command::new("explorer")
                            .arg(&node.path)
                            .spawn();
                    }
                    ui.close_menu();
                }
                if ui.button("Copy path").clicked() {
                    ui.output_mut(|o| o.copied_text = node.path.to_string_lossy().to_string());
                    ui.close_menu();
                }
                if ui.button("Format Markdown").clicked() {
                    let now = chrono::Local::now();
                    let date_str = now.to_rfc3339();
                    let prompt = format!("Format the current document into correct markdown and use this template for the yaml front matter. Focus ONLY on the currently active file, and DO NOT use list_files or search for other files.\n```yaml\n---\ntitle: A brief title\nsummary: A three sentence summary of the contents\ntags: [\"tag1\",\"tag2\"]\nheader-date: {}\n---\n```", date_str);
                    *ctx.submit_prompt = Some(prompt);
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
