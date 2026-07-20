use eframe::egui;
use crate::ui::FastMdApp;
use crate::background::{LogCategory, BackgroundLogEntry};

/// Determines if a log entry matches the given category and search text.
/// Inputs: `log` (the log entry), `category` (optional filter category), `search_lower` (lowercase search string).
/// Outputs: `bool` - true if the log passes the filters.
/// Purity: Pure.
/// Preconditions: `search_lower` must be pre-lowercased for optimal performance.
/// Postconditions: Returns deterministic boolean based on exact category match and substring search match.
pub fn is_log_visible(
    log: &BackgroundLogEntry,
    category: Option<LogCategory>,
    search_lower: &str,
) -> bool {
    if let Some(cat) = category {
        if log.category != cat {
            return false;
        }
    }
    if !search_lower.is_empty() {
        if !log.message.to_lowercase().contains(search_lower) {
            return false;
        }
    }
    true
}

/// Filters an iterator of log entries based on category and search text.
/// Inputs: `logs` (iterator of log entries), `category` (optional filter), `search_text` (raw search string).
/// Outputs: `Vec<BackgroundLogEntry>` - a collection of cloned log entries that match the criteria.
/// Purity: Pure.
/// Preconditions: `logs` is a valid iterator.
/// Postconditions: Returns a freshly allocated `Vec` containing only the logs that passed `is_log_visible`.
pub fn filter_logs<'a>(
    logs: impl Iterator<Item = &'a BackgroundLogEntry>,
    category: Option<LogCategory>,
    search_text: &str,
) -> Vec<BackgroundLogEntry> {
    let search_lower = search_text.to_lowercase();
    logs.filter(|log| is_log_visible(log, category, &search_lower))
        .cloned()
        .collect()
}

pub fn show_background_logs_window(app: &mut FastMdApp, ctx: &egui::Context) {
    if !app.show_background_logs {
        return;
    }

    let mut open = app.show_background_logs;
    
    egui::Window::new("Background Processes")
        .open(&mut open)
        .resizable(true)
        .collapsible(true)
        .default_size([600.0, 400.0])
        .show(ctx, |ui| {
            let mut mgr = app.background_manager.lock().unwrap();

            ui.horizontal(|ui| {
                ui.label("Search:");
                ui.text_edit_singleline(&mut mgr.search_text);
                
                ui.label("Category:");
                egui::ComboBox::from_id_source("category_filter")
                    .selected_text(match mgr.filter_category {
                        Some(c) => c.to_string(),
                        None => "All".to_string(),
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut mgr.filter_category, None, "All");
                        ui.selectable_value(&mut mgr.filter_category, Some(LogCategory::Indexer), "Indexer");
                        ui.selectable_value(&mut mgr.filter_category, Some(LogCategory::Watcher), "Watcher");
                        ui.selectable_value(&mut mgr.filter_category, Some(LogCategory::PdfConverter), "PDF Converter");
                        ui.selectable_value(&mut mgr.filter_category, Some(LogCategory::ImageVision), "Image Vision");
                        ui.selectable_value(&mut mgr.filter_category, Some(LogCategory::LlmTools), "LLM Tools");
                    });

                ui.checkbox(&mut mgr.auto_scroll, "Auto-scroll");
                
                if ui.button("Clear").clicked() {
                    mgr.clear_logs();
                }
            });

            ui.separator();

            let logs = filter_logs(
                mgr.get_logs().iter(),
                mgr.filter_category,
                &mgr.search_text,
            );

            let row_height = ui.text_style_height(&egui::TextStyle::Body);
            
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .stick_to_bottom(mgr.auto_scroll)
                .show_rows(ui, row_height, logs.len(), |ui, row_range| {
                    for i in row_range {
                        let log = &logs[i];
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(log.timestamp.format("%H:%M:%S%.3f").to_string())
                                    .color(egui::Color32::DARK_GRAY)
                            );
                            ui.label(
                                egui::RichText::new(format!("[{}]", log.category))
                                    .color(egui::Color32::LIGHT_BLUE)
                            );
                            ui.label(&log.message);
                        });
                    }
                });
        });

    app.show_background_logs = open;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_log(category: LogCategory, message: &str) -> BackgroundLogEntry {
        BackgroundLogEntry::new(category, message.to_string())
    }

    #[test]
    fn test_is_log_visible_no_filters() {
        let log = make_log(LogCategory::Indexer, "Indexing started");
        assert!(is_log_visible(&log, None, ""));
    }

    #[test]
    fn test_is_log_visible_category_match() {
        let log = make_log(LogCategory::Watcher, "File changed");
        assert!(is_log_visible(&log, Some(LogCategory::Watcher), ""));
    }

    #[test]
    fn test_is_log_visible_category_mismatch() {
        let log = make_log(LogCategory::PdfConverter, "Converting");
        assert!(!is_log_visible(&log, Some(LogCategory::Watcher), ""));
    }

    #[test]
    fn test_is_log_visible_search_match() {
        let log = make_log(LogCategory::Indexer, "Found 42 files");
        assert!(is_log_visible(&log, None, "found"));
    }

    #[test]
    fn test_is_log_visible_search_mismatch() {
        let log = make_log(LogCategory::Indexer, "Found 42 files");
        assert!(!is_log_visible(&log, None, "missing"));
    }

    #[test]
    fn test_is_log_visible_combined_filters() {
        let log = make_log(LogCategory::ImageVision, "Processing image.jpg");
        // Matches both
        assert!(is_log_visible(&log, Some(LogCategory::ImageVision), "image"));
        // Matches search but not category
        assert!(!is_log_visible(&log, Some(LogCategory::Watcher), "image"));
        // Matches category but not search
        assert!(!is_log_visible(&log, Some(LogCategory::ImageVision), "pdf"));
    }

    #[test]
    fn test_filter_logs() {
        let logs = vec![
            make_log(LogCategory::Indexer, "Index 1"),
            make_log(LogCategory::Indexer, "Index 2"),
            make_log(LogCategory::Watcher, "Watch 1"),
        ];

        let filtered = filter_logs(logs.iter(), Some(LogCategory::Indexer), "");
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].message, "Index 1");

        let filtered_search = filter_logs(logs.iter(), None, "watch");
        assert_eq!(filtered_search.len(), 1);
        assert_eq!(filtered_search[0].message, "Watch 1");
    }
}
