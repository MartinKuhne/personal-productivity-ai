//! Right-pane file viewer. Renders a markdown file's text content with
//! the file name as a header, or a placeholder when nothing is selected.

use eframe::egui::{self, RichText, ScrollArea};

use crate::file_node::FileNode;

const PLACEHOLDER: &str = "Select a markdown file to view";

/// Render the file viewer pane. `selection` is the currently selected file
/// (`(name, content)`); `None` renders the placeholder.
pub fn render(ui: &mut egui::Ui, selection: &Option<(String, String)>) {
    match selection {
        Some((name, content)) => {
            ui.label(
                RichText::new(name)
                    .size(22.0)
                    .strong()
                    .color(egui::Color32::from_rgb(0xE0, 0xE0, 0xE6)),
            );
            ui.add_space(12.0);
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    // A future iteration will pipe this through pulldown-cmark
                    // (already a dep of the desktop crate) and render markdown
                    // properly. The Kotlin app is plain text too, so this
                    // matches current behaviour.
                    ui.label(
                        RichText::new(content)
                            .monospace()
                            .color(egui::Color32::from_rgb(0xD0, 0xD0, 0xD6)),
                    );
                });
        }
        None => {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.3);
                ui.label(
                    RichText::new(PLACEHOLDER)
                        .size(16.0)
                        .color(egui::Color32::from_rgb(0x80, 0x80, 0x88)),
                );
            });
        }
    }
}

/// Suppress the unused-import lint when no widget helpers need `FileNode`
/// yet. Future iterations will use the type here (e.g. for a "loading"
/// indicator that shows the file's metadata before content arrives).
#[allow(dead_code)]
pub(crate) fn _type_anchor(_: &FileNode) {}
