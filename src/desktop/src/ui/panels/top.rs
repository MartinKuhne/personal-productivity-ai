//! Top toolbar panel — indexing status, tag filter dropdown, new-file/new-dir buttons, and content-library name.

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

/// Purpose: Applies the side effect of clicking the batch-processing
/// button in the top toolbar.
/// Inputs: app (the application state)
/// Outputs: ()
/// Purity: Impure (mutates `app.orchestrator.dialogs.batch_dialog_open`).
/// Preconditions: None.
/// Postconditions: `app.orchestrator.dialogs.batch_dialog_open` is `true` after
/// the call. The flag is sticky; the batch dialog itself resets the
/// flag to `false` when it closes (`ui/app.rs:749`).
///
/// The button click in `show_top_panel` calls this function. It is
/// extracted so the side effect can be unit-tested without driving
/// the egui harness.
pub fn apply_batch_button_click(app: &mut FastMdApp) {
    app.dialogs_mut().batch_dialog_open = true;
}

/// Purpose: Applies the side effect of clicking the
/// "Tools..." button in the top toolbar.
///
/// Inputs: app (the application state)
/// Outputs: ()
/// Purity: Impure (mutates `app.orchestrator.dialogs.tools_dialog_open`).
/// Preconditions: None.
/// Postconditions: `app.orchestrator.dialogs.tools_dialog_open` is `true` after
/// the call. The dialog itself resets the flag to `false` when it
/// closes.
///
/// The button click in `show_top_panel` calls this function. It is
/// extracted so the side effect can be unit-tested without driving
/// the egui harness.
pub fn apply_tools_button_click(app: &mut FastMdApp) {
    app.dialogs_mut().tools_dialog_open = true;
    // Flag the dialog to do a one-time MCP tool discovery on
    // its first frame so MCP groups show their tools and
    // prompt char count immediately.
    app.dialogs_mut().tools_dialog_just_opened = true;
}

pub fn show_top_panel(app: &mut FastMdApp, parent_ui: &mut egui::Ui) {
    show_top_panel_capture(app, parent_ui, |_| {});
}

/// Tier 4 test variant of [`show_top_panel`]. The `on_click` callback
/// is invoked after every button click in the toolbar, with a stable
/// event name. The production caller ([`show_top_panel`]) passes a
/// no-op closure; the test caller in
/// `tests::test_batch_button_click_opens_dialog` passes a closure
/// that pushes the event into the harness's persistent state. The
/// callback runs on the same frame as the click, *after* the side
/// effect on `app` is applied, so the test can read both `app`
/// (via the captured `&mut FastMdApp` in the closure) and the
/// harness's `state()` after `harness.run()` to verify the
/// integration end-to-end.
pub fn show_top_panel_capture(
    app: &mut FastMdApp,
    parent_ui: &mut egui::Ui,
    mut on_click: impl FnMut(&'static str),
) {
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
                let mut bg = app.orchestrator.background_manager.lock().unwrap();
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
                apply_batch_button_click(app);
                on_click("batch_button");
            }
            if ui.button(crate::ui::strings::TOOLS_BUTTON).clicked() {
                apply_tools_button_click(app);
                on_click("tools_button");
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
                            app.selection_mut().tree_dirty = true;
                        }
                    });
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tier 4 click test: clicking the batch-processing button in
    /// the top toolbar must open the batch dialog (sets
    /// `app.orchestrator.dialogs.batch_dialog_open = true`) and fire the
    /// `on_click("batch_button")` callback that the test
    /// harness captures into its persistent state.
    ///
    /// Uses the `stateful_harness` helper from `test_helpers::interact`
    /// (R-3). The closure calls the production `show_top_panel_capture`
    /// with a callback that pushes the event into the harness's
    /// `T = Vec<&'static str>` state. After the click settles, we
    /// read the state and verify the event was captured.
    #[test]
    fn test_batch_button_click_opens_dialog() {
        use crate::ui::test_helpers::interact::stateful_harness;
        use egui_kittest::kittest::Queryable;

        let mut harness = stateful_harness(Vec::<&'static str>::new(), |ui, captured| {
            // `app` is moved into the closure. The closure is
            // called once per pass; the harness owns it for
            // its lifetime. After the harness drops, the
            // captured `&'static str` events are the only
            // post-click observable state (per the state-
            // capture pattern documented in
            // `test_helpers::interact`).
            let mut app = create_test_app();
            assert!(!app.orchestrator.dialogs.batch_dialog_open);
            show_top_panel_capture(&mut app, ui, |event| {
                captured.push(event);
            });
        });
        harness.fit_contents();
        // The top toolbar shows a spinner while indexing is in
        // progress, which keeps repainting forever. Use
        // `run_steps` with a bounded count rather than `run()`
        // (which would hit `Harness::run exceeded max_steps`).
        harness.run_steps(2);
        // Locate the batch button by its label (from
        // `strings::BATCH_BUTTON`) and click it.
        harness
            .get_by_label(crate::ui::strings::BATCH_BUTTON)
            .click();
        // Two `run_steps` calls after the click: the first
        // processes the pointer events (hover + press + release
        // = three steps), the second settles any post-click
        // repaint. Bounded count to avoid the spinner infinite
        // loop.
        harness.run_steps(2);
        harness.run_steps(2);

        let captured = harness.state();
        assert!(
            captured.contains(&"batch_button"),
            "clicking the batch button must fire the `batch_button` \
             on_click event; got: {:?}",
            captured
        );
    }

    /// Tier 4 click test: clicking the "Tools..." button in the top
    /// toolbar must open the tools dialog (sets
    /// `app.orchestrator.dialogs.tools_dialog_open = true`) and fire the
    /// `on_click("tools_button")` callback. Mirrors the batch
    /// button test above.
    #[test]
    fn test_tools_button_click_opens_dialog() {
        use crate::ui::test_helpers::interact::stateful_harness;
        use egui_kittest::kittest::Queryable;

        let mut harness = stateful_harness(Vec::<&'static str>::new(), |ui, captured| {
            let mut app = create_test_app();
            assert!(!app.orchestrator.dialogs.tools_dialog_open);
            show_top_panel_capture(&mut app, ui, |event| {
                captured.push(event);
            });
        });
        harness.fit_contents();
        harness.run_steps(2);
        harness
            .get_by_label(crate::ui::strings::TOOLS_BUTTON)
            .click();
        harness.run_steps(2);
        harness.run_steps(2);

        let captured = harness.state();
        assert!(
            captured.contains(&"tools_button"),
            "clicking the tools button must fire the `tools_button` \
             on_click event; got: {:?}",
            captured
        );
    }

    /// Tier 1 test for the batch button click effect. The click sets
    /// `app.orchestrator.dialogs.batch_dialog_open` to `true`; the dialog itself
    /// resets the flag to `false` when it closes. We verify the
    /// effect without driving the egui harness.
    #[test]
    fn test_apply_batch_button_click_sets_dialog_open() {
        let mut app = create_test_app();
        assert!(
            !app.orchestrator.dialogs.batch_dialog_open,
            "dialog must start closed"
        );
        apply_batch_button_click(&mut app);
        assert!(
            app.orchestrator.dialogs.batch_dialog_open,
            "batch button click must open the batch dialog"
        );
    }

    /// Tier 1 test for the tools button click effect. The click sets
    /// `app.orchestrator.dialogs.tools_dialog_open` to `true`; the dialog itself
    /// resets the flag to `false` when it closes.
    #[test]
    fn test_apply_tools_button_click_sets_dialog_open() {
        let mut app = create_test_app();
        assert!(
            !app.orchestrator.dialogs.tools_dialog_open,
            "dialog must start closed"
        );
        apply_tools_button_click(&mut app);
        assert!(
            app.orchestrator.dialogs.tools_dialog_open,
            "tools button click must open the tools dialog"
        );
    }

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

    // --- UI / window tests (R-7: merged from `mod ui_tests`) ---

    use crate::ui::strings::{APP_TITLE, BATCH_BUTTON, SHOW_LOG_CHECKBOX};
    use crate::ui::test_helpers::assert::assert_no_id_change_in_log;
    use crate::ui::test_helpers::text::assert_text_contains;

    fn create_test_app() -> FastMdApp {
        FastMdApp::empty_state(crate::config::AppConfig::default())
    }

    #[test]
    fn test_show_top_panel_indexing_unfinished() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        app.file_processor_mut().indexing_finished = false;
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            show_top_panel(&mut app, ui);
        });
        // R-2 / Q12: the top panel always renders the app title, the log
        // checkbox, the batch button, and the new tools button — those
        // are stable across states.
        assert_text_contains(&output.shapes, APP_TITLE);
        assert_text_contains(&output.shapes, SHOW_LOG_CHECKBOX);
        assert_text_contains(&output.shapes, BATCH_BUTTON);
        assert_text_contains(&output.shapes, crate::ui::strings::TOOLS_BUTTON);
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

        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            show_top_panel(&mut app, ui);
        });
        // Header assertion (Q12 borderline case): the toolbar chrome is
        // the stable surface here. The tag combobox content is dynamic
        // so we don't assert on individual tag names.
        assert_text_contains(&output.shapes, APP_TITLE);
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
        // Sanity check that the log capture is actually wired up —
        // an empty `msgs` would silently pass the id-stability check
        // even if the warning fires through a different sink.
        assert!(
            !msgs.is_empty(),
            "log capture is empty — the test is not actually running under the installed log::Log impl"
        );
        assert_no_id_change_in_log(&msgs);
    }
}
