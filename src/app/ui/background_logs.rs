//! Background-logs viewer panel — category filter, text search, auto-scroll, and copy-to-clipboard.

use crate::background::{BackgroundLogEntry, LogCategory};
use crate::ui::FastMdApp;
use eframe::egui;

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
    if let Some(cat) = category
        && log.category != cat
    {
        return false;
    }
    if !search_lower.is_empty() && !log.message.to_lowercase().contains(search_lower) {
        return false;
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
    if !app
        .orchestrator
        .background_manager
        .lock()
        .unwrap()
        .show_background_logs
    {
        return;
    }

    let mut open = app
        .orchestrator
        .background_manager
        .lock()
        .unwrap()
        .show_background_logs;

    egui::Window::new(crate::ui::strings::BACKGROUND_PROCESSES_WINDOW)
        .open(&mut open)
        .resizable(true)
        .collapsible(true)
        .default_size([600.0, 400.0])
        .show(ctx, |ui| {
            let Ok(mut mgr) = app.orchestrator.background_manager.lock() else {
                ui.label(crate::ui::strings::BACKGROUND_MGR_ACCESS_ERROR);
                return;
            };

            ui.horizontal(|ui| {
                ui.label(crate::ui::strings::SEARCH_LABEL);
                ui.text_edit_singleline(&mut mgr.search_text);

                ui.label(crate::ui::strings::CATEGORY_LABEL);
                egui::ComboBox::from_id_salt("category_filter")
                    .selected_text(match mgr.filter_category {
                        Some(c) => c.to_string(),
                        None => crate::ui::strings::LOG_CATEGORY_ALL.to_string(),
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut mgr.filter_category,
                            None,
                            crate::ui::strings::LOG_CATEGORY_ALL,
                        );
                        ui.selectable_value(
                            &mut mgr.filter_category,
                            Some(LogCategory::Indexer),
                            crate::ui::strings::LOG_CATEGORY_INDEXER,
                        );
                        ui.selectable_value(
                            &mut mgr.filter_category,
                            Some(LogCategory::Watcher),
                            crate::ui::strings::LOG_CATEGORY_WATCHER,
                        );
                        ui.selectable_value(
                            &mut mgr.filter_category,
                            Some(LogCategory::PdfConverter),
                            crate::ui::strings::LOG_CATEGORY_PDF_CONVERTER,
                        );
                        ui.selectable_value(
                            &mut mgr.filter_category,
                            Some(LogCategory::ImageVision),
                            crate::ui::strings::LOG_CATEGORY_IMAGE_VISION,
                        );
                        ui.selectable_value(
                            &mut mgr.filter_category,
                            Some(LogCategory::LlmTools),
                            crate::ui::strings::LOG_CATEGORY_LLM_TOOLS,
                        );
                    });

                ui.checkbox(
                    &mut mgr.auto_scroll,
                    crate::ui::strings::AUTO_SCROLL_CHECKBOX,
                );

                if ui.button(crate::ui::strings::CLEAR_BUTTON).clicked() {
                    mgr.clear_logs();
                }
            });

            ui.separator();

            let logs = filter_logs(mgr.get_logs().iter(), mgr.filter_category, &mgr.search_text);

            let row_height = ui.text_style_height(&egui::TextStyle::Body);

            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .stick_to_bottom(mgr.auto_scroll)
                .show_rows(ui, row_height, logs.len(), |ui, row_range| {
                    for i in row_range {
                        let log = &logs[i];
                        ui.push_id(i, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(
                                        log.timestamp.format("%H:%M:%S%.3f").to_string(),
                                    )
                                    .color(egui::Color32::DARK_GRAY),
                                );
                                ui.label(
                                    egui::RichText::new(format!("[{}]", log.category))
                                        .color(egui::Color32::LIGHT_BLUE),
                                );
                                ui.label(&log.message);
                            })
                        });
                    }
                });
        });

    app.orchestrator
        .background_manager
        .lock()
        .unwrap()
        .show_background_logs = open;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::test_helpers::run_ui_test;

    fn make_log(category: LogCategory, message: &str) -> BackgroundLogEntry {
        BackgroundLogEntry::new(category, message.to_string())
    }

    /// Id-stability: the background-logs window is a `egui::Window` that
    /// combines a `ui.horizontal` filter row with a `ScrollArea::show_rows`
    /// over a `Vec<BackgroundLogEntry>`. Both are common sources of the
    /// "rect changed id between passes" warning class documented in
    /// `AGENTS.md` §"Conditional rendering" — the filter row is rebuilt
    /// every frame and the scroll area can balloon between passes if the
    /// `len()` or the available rect changes. Render twice through the
    /// same `ctx` and assert no red-stroke rect appears in the second
    /// pass.
    #[test]
    fn test_show_background_logs_window_no_id_change_warnings() {
        use crate::ui::test_helpers::assert::assert_no_id_change_in_shapes;
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        app.orchestrator
            .background_manager
            .lock()
            .unwrap()
            .show_background_logs = true;
        {
            let mut mgr = app.orchestrator.background_manager.lock().unwrap();
            for i in 0..60 {
                mgr.push_log(BackgroundLogEntry::new(
                    LogCategory::Indexer,
                    format!("log {i}"),
                ));
            }
        }

        // Pass 1: prime previous-pass state.
        let _ = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
            show_background_logs_window(&mut app, ui.ctx());
        });
        // Pass 2: surface red-stroke rect if the widget tree shifted.
        let output = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
            show_background_logs_window(&mut app, ui.ctx());
        });

        let shapes: Vec<egui::Shape> = output.shapes.into_iter().map(|cs| cs.shape).collect();
        assert_no_id_change_in_shapes(&shapes);
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
        assert!(is_log_visible(
            &log,
            Some(LogCategory::ImageVision),
            "image"
        ));
        // Matches search but not category
        assert!(!is_log_visible(&log, Some(LogCategory::Watcher), "image"));
        // Matches category but not search
        assert!(!is_log_visible(&log, Some(LogCategory::ImageVision), "pdf"));
    }

    #[test]
    fn test_filter_logs() {
        let logs = [
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

    // --- UI / window tests (R-7: merged from `mod ui_tests`) ---

    use crate::ui::test_helpers::app::test_app as create_test_app;

    #[test]
    fn test_show_background_logs_window_closed() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        app.orchestrator
            .background_manager
            .lock()
            .unwrap()
            .show_background_logs = false;

        let _ = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
            show_background_logs_window(&mut app, ui.ctx());
        });
        assert!(
            !app.orchestrator
                .background_manager
                .lock()
                .unwrap()
                .show_background_logs
        );
    }

    #[test]
    fn test_show_background_logs_window_open_with_logs() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        app.orchestrator
            .background_manager
            .lock()
            .unwrap()
            .show_background_logs = true;

        {
            let mut mgr = app.orchestrator.background_manager.lock().unwrap();
            mgr.push_log(BackgroundLogEntry::new(
                LogCategory::Indexer,
                "Indexing workspace...".to_string(),
            ));
            mgr.push_log(BackgroundLogEntry::new(
                LogCategory::Watcher,
                "File modified".to_string(),
            ));
        }

        let _output = run_ui_test(&ctx, egui::RawInput::default(), |ui| {
            show_background_logs_window(&mut app, ui.ctx());
        });
        assert!(
            app.orchestrator
                .background_manager
                .lock()
                .unwrap()
                .show_background_logs
        );
    }
}
