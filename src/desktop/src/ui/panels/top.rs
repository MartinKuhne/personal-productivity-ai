//! Top toolbar panel — indexing status, tag filter dropdown, new-file/new-dir buttons, and content-library name.
//!
//! Unit tests live in the sibling `top_tests.rs` sidecar.

use crate::ui::FastMdApp;
use crate::ui::table_width::DeficitStrategy;
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
}

/// Human-readable label for a [`DeficitStrategy`] variant, used as the
/// `selected_text` of the top-bar table-width-strategy combobox and as
/// the row label inside the dropdown. The match is intentionally
/// exhaustive over every variant of `DeficitStrategy` so adding a new
/// strategy in the future is a compile error here until the label is
/// supplied.
pub fn strategy_label(strategy: DeficitStrategy) -> &'static str {
    match strategy {
        DeficitStrategy::ProportionalToSlack => {
            crate::ui::strings::TABLE_WIDTH_STRATEGY_PROPORTIONAL
        }
        DeficitStrategy::BreakpointWaterFill => crate::ui::strings::TABLE_WIDTH_STRATEGY_WATERFILL,
    }
}

/// Purpose: Applies the side effect of picking a new table-width
/// deficit strategy in the top toolbar's combobox.
///
/// Inputs: app (the application state), strategy (the newly selected
///   [`DeficitStrategy`])
/// Outputs: ()
/// Purity: Impure. Clones `app.orchestrator.config`, mutates
///   `table_width_strategy`, persists the new config to disk via
///   `save_config`, and replaces the in-memory config. The
///   markdown renderer reads `deficit_strategy()` on every frame so
///   the change takes effect on the very next paint without any
///   invalidation hook.
/// Preconditions: None.
/// Postconditions:
///   - `app.config().table_width_strategy == strategy.to_config()`
///   - `app.config().deficit_strategy() == strategy`
///   - The config file at `crate::config::get_config_path()` reflects
///     the new value (best-effort: a `save_config` failure is logged
///     via `tracing::error!` but does not panic, matching the
///     `tools_dialog::render_row` policy).
pub fn apply_table_width_strategy_change(app: &mut FastMdApp, strategy: DeficitStrategy) {
    let new_value = strategy.to_config();
    let mut new_config = app.config().clone();
    if new_config.table_width_strategy == new_value {
        // No-op: the persisted value already matches the pick.
        // Skipping the clone/save avoids redundant disk writes when
        // egui re-fires the dropdown's selected-value event across
        // frames.
        return;
    }
    new_config.table_width_strategy = new_value.to_string();
    if let Err(e) = crate::config::save_config(&new_config) {
        tracing::error!(
            error = %e,
            strategy = new_value,
            "failed to persist AppConfig after table-width-strategy change"
        );
    }
    *app.config_mut() = new_config;
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

            // Table-width deficit-strategy combobox. Always visible
            // (not gated on `indexing_finished` — the strategy is a
            // user preference unrelated to the indexer). Picked
            // strategy is persisted to `AppConfig::table_width_strategy`
            // on change; the markdown renderer re-reads it every
            // frame via `app.orchestrator.config.deficit_strategy()`,
            // so the next paint uses the new algorithm without any
            // explicit invalidation hook.
            ui.separator();
            ui.label(crate::ui::strings::TABLE_WIDTH_STRATEGY_LABEL);
            let current_strategy = app.orchestrator.config.deficit_strategy();
            let mut pending: Option<DeficitStrategy> = None;
            egui::ComboBox::from_id_salt(crate::ui::strings::TABLE_WIDTH_STRATEGY_ID_SALT)
                .selected_text(strategy_label(current_strategy))
                .show_ui(ui, |ui| {
                    for variant in [
                        DeficitStrategy::ProportionalToSlack,
                        DeficitStrategy::BreakpointWaterFill,
                    ] {
                        if ui
                            .selectable_label(
                                strategy_label(variant) == strategy_label(current_strategy),
                                strategy_label(variant),
                            )
                            .clicked()
                            && variant != current_strategy
                        {
                            pending = Some(variant);
                        }
                    }
                });
            if let Some(picked) = pending {
                apply_table_width_strategy_change(app, picked);
                on_click(crate::ui::strings::TABLE_WIDTH_STRATEGY_EVENT);
            }
        });
    });
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `top_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "top_tests.rs"]
mod tests;
