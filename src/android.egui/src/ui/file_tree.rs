//! Collapsible file tree. Direct port of `FileTreeView` in `MainActivity.kt`.
//!
//! Each row is a clickable button with a folder/file glyph; directories
//! toggle expansion, files are returned to the caller via the `Vec` return
//! value so the App can fetch their content. State (which nodes are
//! expanded) lives in the `expanded` set passed in by the caller — the App
//! owns it so the tree remembers its state across rebuilds.

use std::collections::HashSet;

use eframe::egui::{self, Color32, RichText, Ui};

use crate::file_node::FileNode;

const INDENT_PX: f32 = 16.0;

/// Render the entire tree under `root`. The root itself is shown as the
/// header row. Returns the list of files the user clicked this frame; the
/// caller is responsible for fetching their content.
pub fn render(
    ui: &mut Ui,
    root: &FileNode,
    expanded: &mut HashSet<String>,
) -> Vec<FileNode> {
    let mut clicked = Vec::new();
    render_node(ui, root, 0, expanded, &mut clicked);
    clicked
}

fn render_node(
    ui: &mut Ui,
    node: &FileNode,
    depth: usize,
    expanded: &mut HashSet<String>,
    clicked: &mut Vec<FileNode>,
) {
    let indent = depth as f32 * INDENT_PX;
    ui.horizontal(|ui| {
        ui.add_space(indent);
        let is_expanded = expanded.contains(&node.id);
        let (icon, label_color) = if node.is_directory {
            (
                if is_expanded { "📂" } else { "📁" },
                Color32::from_rgb(0xE0, 0xC8, 0x70),
            )
        } else {
            ("📄", Color32::from_rgb(0xE0, 0xE0, 0xE6))
        };
        let label = format!("{icon} {}", node.name);
        let response = ui.add(
            egui::Button::new(RichText::new(label).color(label_color))
                .frame(false)
                .min_size(egui::vec2(0.0, 22.0)),
        );
        if response.clicked() {
            if node.is_directory {
                if is_expanded {
                    expanded.remove(&node.id);
                } else {
                    expanded.insert(node.id.clone());
                }
            } else {
                clicked.push(node.clone());
            }
        }
    });

    if node.is_directory && expanded.contains(&node.id) {
        for child in &node.children {
            render_node(ui, child, depth + 1, expanded, clicked);
        }
    }
}
