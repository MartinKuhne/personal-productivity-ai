//! Top toolbar panel Ã¢â‚¬â€ indexing status, tag filter dropdown, new-file/new-dir buttons, and content-library name.

use crate::ui::FastMdApp;
use eframe::egui;
use egui::RichText;
use egui::containers::Panel;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Purpose: Generates the indexing status rich text based on whether indexing is finished.
/// Inputs: indexing_finished (boolean), file_count (usize)
/// Outputs: egui::RichText with appropriate message, color, and styling.
/// Purity: Pure function.
/// Preconditions: None.
/// Postconditions: Returns green text with "Indexing finished" if true, or italicized text with "Indexing workspace" if false.
pub fn build_indexing_status_text(indexing_finished: bool, file_count: usize) -> RichText {
    if indexing_finished {
        RichText::new(crate::ui::strings::build_indexing_finished_text(file_count))
            .color(egui::Color32::from_rgb(100, 255, 100))
    } else {
        RichText::new(crate::ui::strings::build_indexing_progress_text(file_count)).italics()
    }
}

/// Purpose: Determines the display text for the tag filter combobox.
/// Inputs: selected_tag (optional string reference)
/// Outputs: String slice representing the text to show.
/// Purity: Pure function.
/// Preconditions: None.
/// Postconditions: Returns the tag name if one is selected, otherwise "Filter by Tag: All".
pub fn get_tag_filter_text(selected_tag: Option<&String>) -> &str {
    selected_tag
        .map(|s| s.as_str())
        .unwrap_or(crate::ui::strings::TAG_FILTER_DEFAULT)
}

/// Purpose: Determines the next selected file after the active tag filter changes.
/// Inputs: selected_file (current selected file path), selected_tag (currently active tag filter), file_tags (mapping of files to their tags)
/// Outputs: Option<PathBuf> representing the new selected file.
/// Purity: Pure function.
/// Preconditions: None.
/// Postconditions: Returns `None` if an active tag is selected and it is not associated with the selected file. Otherwise returns the original selected file.
pub fn compute_next_selected_file(
    selected_file: Option<&PathBuf>,
    selected_tag: Option<&String>,
    file_tags: &BTreeMap<PathBuf, Vec<String>>,
) -> Option<PathBuf> {
    let selected = selected_file?;
    if let Some(active_tag) = selected_tag {
        {
            let tags = file_tags.get(selected)?;
            if !tags.contains(active_tag) {
                return None;
            }
        }
    }
    Some(selected.clone())
}

pub fn show_top_panel(app: &mut FastMdApp, parent_ui: &mut egui::Ui) {
    // egui 0.35 unified `TopBottomPanel` into `Panel`.
    Panel::top("top_panel").show(parent_ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading(
                RichText::new(crate::ui::strings::APP_TITLE)
                    .strong()
                    .color(egui::Color32::from_rgb(100, 200, 255)),
            );
            ui.separator();
            // Single lock acquisition for the read-modify-write of
            // `show_background_logs`. The previous revision locked
            // twice (read + write) with an `unwrap()` on each — two
            // panic-on-poison sites per frame plus a lost-update
            // window between the locks (render-audit P1-8).
            {
                let mut bg = app.background_manager.lock().unwrap();
                let mut show_bg = bg.show_background_logs;
                if ui
                    .checkbox(&mut show_bg, crate::ui::strings::SHOW_LOG_CHECKBOX)
                    .changed()
                {
                    bg.show_background_logs = show_bg;
                }
            }
            ui.separator();

            if ui.button(crate::ui::strings::BATCH_BUTTON).clicked() {
                app.dialogs_mut().batch_dialog_open = true;
            }
            ui.separator();

            // Spinner and tag combobox must always allocate, even
            // when invisible, so their widget ids stay stable across
            // the indexing-finished transition. The previous
            // revision rendered the spinner only while indexing and
            // the combobox only after indexing; the conditional
            // add/remove swapped different widgets into the same
            // rect on successive passes and triggered
            // `WARN egui::context: Widget rect ... changed id
            // between passes` for the whole toolbar row.
            ui.add_visible(
                !app.file_processor().indexing_finished,
                egui::Spinner::new(),
            );

            ui.label(build_indexing_status_text(
                app.file_processor().indexing_finished,
                app.file_processor().all_files.len(),
            ));

            // Allocate the tag combobox unconditionally so its id is
            // stable across the indexing transition, then hide its
            // content with `set_visible(false)` while we are still
            // indexing. The previous revision put both the separator
            // and the combobox inside `if indexing_finished`, which
            // was the direct cause of the per-frame id-clash log
            // spam on the toolbar row.
            ui.scope(|ui| {
                if !app.file_processor().indexing_finished {
                    ui.set_invisible();
                }
                ui.add_visible(
                    app.file_processor().indexing_finished,
                    egui::Separator::default(),
                );
                egui::ComboBox::from_id_salt(crate::ui::strings::TAG_FILTER_ID_SALT)
                    .selected_text(get_tag_filter_text(app.tags().selected_tag.as_ref()))
                    .show_ui(ui, |ui| {
                        let mut changed = ui
                            .selectable_value(
                                &mut app.tags_mut().selected_tag,
                                None,
                                crate::ui::strings::TAG_FILTER_ALL,
                            )
                            .changed();
                        let all_tags: Vec<String> = app.tags().all_tags().iter().cloned().collect();
                        for tag in all_tags {
                            changed |= ui
                                .selectable_value(
                                    &mut app.tags_mut().selected_tag,
                                    Some(tag.clone()),
                                    &tag,
                                )
                                .changed();
                        }
                        if changed {
                            let next = compute_next_selected_file(
                                app.selection().selected_file(),
                                app.tags().selected_tag.as_ref(),
                                app.tags().file_tags(),
                            );
                            *app.selection_mut().selected_file_mut() = next;
                        }
                    });
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_indexing_status_text_finished() {
        let text = build_indexing_status_text(true, 42);
        assert_eq!(text.text(), "Indexing finished (42 files)");
    }

    #[test]
    fn test_build_indexing_status_text_unfinished() {
        let text = build_indexing_status_text(false, 10);
        assert_eq!(text.text(), "Indexing workspace (found 10 files)...");
    }

    #[test]
    fn test_get_tag_filter_text() {
        assert_eq!(get_tag_filter_text(None), "Filter by Tag: All");
        let tag = "Rust".to_string();
        assert_eq!(get_tag_filter_text(Some(&tag)), "Rust");
    }

    #[test]
    fn test_compute_next_selected_file_no_selected_file() {
        let file_tags = BTreeMap::new();
        assert_eq!(compute_next_selected_file(None, None, &file_tags), None);
    }

    #[test]
    fn test_compute_next_selected_file_no_tag() {
        let mut file_tags = BTreeMap::new();
        let path = PathBuf::from("test.md");
        file_tags.insert(path.clone(), vec!["Rust".to_string()]);

        assert_eq!(
            compute_next_selected_file(Some(&path), None, &file_tags),
            Some(path)
        );
    }

    #[test]
    fn test_compute_next_selected_file_tag_matches() {
        let mut file_tags = BTreeMap::new();
        let path = PathBuf::from("test.md");
        file_tags.insert(path.clone(), vec!["Rust".to_string()]);
        let tag = "Rust".to_string();

        assert_eq!(
            compute_next_selected_file(Some(&path), Some(&tag), &file_tags),
            Some(path)
        );
    }

    #[test]
    fn test_compute_next_selected_file_tag_missing() {
        let mut file_tags = BTreeMap::new();
        let path = PathBuf::from("test.md");
        file_tags.insert(path.clone(), vec!["Rust".to_string()]);
        let tag = "Go".to_string();

        assert_eq!(
            compute_next_selected_file(Some(&path), Some(&tag), &file_tags),
            None
        );
    }

    #[test]
    fn test_compute_next_selected_file_file_not_in_tags() {
        let file_tags = BTreeMap::new();
        let path = PathBuf::from("test.md");
        let tag = "Rust".to_string();

        assert_eq!(
            compute_next_selected_file(Some(&path), Some(&tag), &file_tags),
            None
        );
    }
}

#[cfg(test)]
mod ui_tests {
    use super::*;

    fn create_test_app() -> FastMdApp {
        FastMdApp::empty_state(crate::config::AppConfig::default())
    }

    #[test]
    fn test_show_top_panel_indexing_unfinished() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        app.file_processor_mut().indexing_finished = false;
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            show_top_panel(&mut app, ui);
        });
        assert!(!app.file_processor().indexing_finished);
    }

    #[test]
    fn test_show_top_panel_indexing_finished_with_tags() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        app.file_processor_mut().indexing_finished = true;
        app.tags_mut().add_tags(
            PathBuf::from("dummy.md"),
            vec!["Rust".to_string(), "Docs".to_string()],
        );

        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            show_top_panel(&mut app, ui);
        });
        assert!(app.file_processor().indexing_finished);
    }

    /// Regression: the production UI logged
    /// `WARN egui::context: Widget rect ... changed id between passes`
    /// for the toolbar row on every frame around the
    /// `indexing_finished` transition. The previous revision put the
    /// spinner and the tag combobox under mutually-exclusive
    /// `if`/`else if` blocks keyed on the bool, so the moment
    /// indexing finished a different widget (combobox) replaced the
    /// previous one (spinner) at the same rect and egui flagged the
    /// whole row. After the fix, both widgets always allocate (via
    /// `add_visible` / `set_invisible`) so their ids are stable
    /// across the bool flip. The test simulates the transition by
    /// flipping `indexing_finished` between two passes and asserts
    /// no `changed id between passes` warning is emitted.
    #[test]
    fn test_show_top_panel_no_id_change_warnings_on_indexing_finished_transition() {
        use std::sync::{Mutex, OnceLock};
        struct Capture {
            msgs: Mutex<Vec<String>>,
        }
        impl log::Log for Capture {
            fn enabled(&self, _: &log::Metadata) -> bool {
                true
            }
            fn log(&self, record: &log::Record) {
                self.msgs
                    .lock()
                    .unwrap()
                    .push(format!("[{}] {}", record.level(), record.args()));
            }
            fn flush(&self) {}
        }
        static LOGGER: OnceLock<Capture> = OnceLock::new();
        static INSTALLED: OnceLock<()> = OnceLock::new();
        let cap = LOGGER.get_or_init(|| Capture {
            msgs: Mutex::new(Vec::new()),
        });
        INSTALLED.get_or_init(|| {
            let _ = log::set_logger(cap);
            log::set_max_level(log::LevelFilter::Trace);
        });
        cap.msgs.lock().unwrap().clear();

        let ctx = egui::Context::default();
        let mut app = create_test_app();
        app.file_processor_mut().indexing_finished = false;
        app.tags_mut().add_tags(
            PathBuf::from("dummy.md"),
            vec!["Rust".to_string(), "Docs".to_string()],
        );

        // Pre-finish: spinner is visible, combobox is hidden but
        // still allocated.
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            show_top_panel(&mut app, ui);
        });
        // Flip the bool and render again — the rects the spinner
        // and combobox live at must stay the same; only their
        // visibility changes.
        app.file_processor_mut().indexing_finished = true;
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            show_top_panel(&mut app, ui);
        });
        // Stabilise on the finished side.
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            show_top_panel(&mut app, ui);
        });

        let msgs = cap.msgs.lock().unwrap().clone();
        let id_change_count = msgs
            .iter()
            .filter(|m| m.contains("changed id between passes"))
            .count();
        assert!(
            !msgs.is_empty(),
            "log capture is empty — the test is not actually running under the installed log::Log impl"
        );
        assert_eq!(
            id_change_count,
            0,
            "top panel must produce a stable widget tree across the indexing_finished transition, but egui emitted {} 'changed id' warning(s): {:?}",
            id_change_count,
            msgs.iter()
                .filter(|m| m.contains("changed id between passes"))
                .collect::<Vec<_>>()
        );
    }
}
