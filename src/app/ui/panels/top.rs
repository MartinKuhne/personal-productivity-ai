//! Top toolbar panel — indexing status, tag filter dropdown, new-file/new-dir buttons, and content-library name.
//!
//! Unit tests live in the sibling `top_tests.rs` sidecar.

use crate::config::AppConfig;
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

/// Purpose: Applies the side effect of toggling the background logs window from the Windows menu.
///
/// Inputs: app (the application state), `show`: the new boolean state
/// Outputs: ()
/// Purity: Impure (mutates `app.orchestrator.background_manager.show_background_logs`).
/// Preconditions: None.
/// Postconditions: `show_background_logs` matches `show`.
pub fn apply_background_logs_toggle(app: &mut FastMdApp, show: bool) {
    app.orchestrator
        .background_manager
        .lock()
        .unwrap()
        .show_background_logs = show;
}

/// Purpose: Applies the side effect of toggling the agent debug window from the Windows menu.
///
/// Inputs: app (the application state), `show`: the new boolean state
/// Outputs: ()
/// Purity: Impure (mutates `app.orchestrator.agent_panel_state.show_debug_window`).
/// Preconditions: None.
/// Postconditions: `show_debug_window` matches `show`.
pub fn apply_agent_debug_toggle(app: &mut FastMdApp, show: bool) {
    app.orchestrator.agent_panel_state.show_debug_window = show;
}

/// Purpose: Applies the side effect of picking a new chat model from the Chat Models menu.
///
/// Inputs: `app` (application state), `model_name` (selected model name), `persist` (config saver callback).
/// Outputs: ()
/// Purity: Impure (mutates the app and agent configuration).
/// Preconditions: None.
/// Postconditions: Both app and agent configuration select `model_name`, and config is saved.
pub fn apply_chat_model_selection<F>(app: &mut FastMdApp, model_name: String, persist: &mut F)
where
    F: FnMut(&AppConfig) -> Result<PathBuf, String>,
{
    let mut new_config = app.config().clone();
    if new_config.selected_chat_model.as_deref() == Some(&model_name) {
        return;
    }
    new_config.selected_chat_model = Some(model_name.clone());
    if let Err(e) = persist(&new_config) {
        tracing::error!(
            error = %e,
            model = %model_name,
            "failed to persist AppConfig after chat-model selection change"
        );
    }
    app.agent_mut()
        .set_agent_config(new_config.to_agent_config());
    *app.config_mut() = new_config;
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
        DeficitStrategy::WaterFillRatio => crate::ui::strings::TABLE_WIDTH_STRATEGY_RATIO,
        DeficitStrategy::LagrangePenalty => crate::ui::strings::TABLE_WIDTH_STRATEGY_LAGRANGE,
        DeficitStrategy::HybridMinPenaltyWaterFill => {
            crate::ui::strings::TABLE_WIDTH_STRATEGY_HYBRID
        }
    }
}

/// Purpose: Applies the side effect of picking a new table-width
/// deficit strategy in the top toolbar's combobox.
///
/// Inputs:
///   - `app`: the application state
///   - `strategy`: the newly selected [`DeficitStrategy`]
///   - `persist`: a callback invoked exactly once when the strategy
///     actually changes. Receives a reference to the post-mutation
///     `AppConfig` (with the new `table_width_strategy` set) and is
///     expected to persist it. The signature matches
///     [`crate::config::save_config`] / [`crate::config::save_config_to_path`]
///     so production can pass those as function pointers without an
///     extra closure wrapper. A `Result::Err` is logged via
///     `tracing::error!` but does not propagate — the in-memory
///     config is still updated (matches the `tools_dialog::render_row`
///     policy).
///
/// Outputs: ()
///
/// Purity: Impure. Clones `app.orchestrator.config`, mutates
///   `table_width_strategy`, invokes `persist` on the new config, and
///   replaces the in-memory config. The markdown renderer reads
///   `deficit_strategy()` on every frame so the change takes effect
///   on the very next paint without any invalidation hook.
///
/// Preconditions: None.
///
/// Postconditions:
///   - `app.config().table_width_strategy == strategy.to_config()`
///   - `app.config().deficit_strategy() == strategy`
///   - `persist` is called exactly once with the post-mutation config
///     iff the value actually changed; otherwise it is not called at
///     all (so a no-op re-pick from egui's re-fired dropdown events
///     does not trigger a redundant write).
///
/// # Why a callback
///
/// Decoupling persistence from the in-memory mutation keeps the
/// function testable without a filesystem: tests pass a closure that
/// captures the saved config (or panics, to assert it wasn't called).
/// Production passes `crate::config::save_config` directly. An
/// earlier version hard-coded `save_config` (the APPDATA-path
/// version) and a test was silently overwriting the user's real
/// `config.yaml` at `%APPDATA%\fastmd\config.yaml` on every test run.
/// A previous attempt split the function into a `_to_path` variant;
/// the callback form is cleaner because the persistence choice lives
/// at the call site (production = "write to APPDATA", test = "do
/// nothing" or "write to a tempdir").
pub fn apply_table_width_strategy_change<F>(
    app: &mut FastMdApp,
    strategy: DeficitStrategy,
    persist: &mut F,
) where
    F: FnMut(&AppConfig) -> Result<PathBuf, String>,
{
    let new_value = strategy.to_config();
    let mut new_config = app.config().clone();
    if new_config.table_width_strategy == new_value {
        // No-op: the persisted value already matches the pick.
        // Skipping the persist call avoids redundant disk writes when
        // egui re-fires the dropdown's selected-value event across
        // frames.
        return;
    }
    new_config.table_width_strategy = new_value.to_string();
    if let Err(e) = persist(&new_config) {
        tracing::error!(
            error = %e,
            strategy = new_value,
            "failed to persist AppConfig after table-width-strategy change"
        );
    }
    *app.config_mut() = new_config;
}

pub fn show_top_panel(app: &mut FastMdApp, parent_ui: &mut egui::Ui) {
    show_top_panel_capture_with_persist(app, parent_ui, crate::config::save_config, |_| {});
}

/// Tier 4 test variant of [`show_top_panel`]. The `on_click` callback
/// is invoked after every button click in the toolbar, with a stable
/// event name. The test persister is a no-op so tests do not touch disk.
#[cfg(test)]
pub fn show_top_panel_capture(
    app: &mut FastMdApp,
    parent_ui: &mut egui::Ui,
    on_click: impl FnMut(&'static str),
) {
    show_top_panel_capture_with_persist(
        app,
        parent_ui,
        |_| Ok(std::path::PathBuf::new()),
        on_click,
    );
}

/// Variant of [`show_top_panel_capture`] allowing custom persistence handler.
#[tracing::instrument(skip_all, name = "ui.panel.top", level = "debug")]
pub fn show_top_panel_capture_with_persist<F>(
    app: &mut FastMdApp,
    parent_ui: &mut egui::Ui,
    mut persist: F,
    mut on_click: impl FnMut(&'static str),
) where
    F: FnMut(&AppConfig) -> Result<PathBuf, String>,
{
    // egui 0.35 unified `TopBottomPanel` into `Panel`.
    Panel::top("top_panel").show(parent_ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading(
                RichText::new(crate::ui::strings::APP_TITLE)
                    .strong()
                    .color(egui::Color32::from_rgb(100, 200, 255)),
            );
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

            // Hamburger menu placed at the far right of the top toolbar.
            // Clicking the hamburger button opens a dropdown containing
            // Batch, Tools, Windows, Chat models, and Table wrap algorithm entries.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.menu_button(crate::ui::strings::HAMBURGER_MENU_BUTTON, |ui| {
                    ui.set_min_width(180.0);

                    // Direct action items — open dialogs immediately on click.
                    if ui.button(crate::ui::strings::MENU_BATCH).clicked() {
                        apply_batch_button_click(app);
                        on_click("batch_button");
                        ui.close();
                    }
                    if ui.button(crate::ui::strings::MENU_TOOLS).clicked() {
                        apply_tools_button_click(app);
                        on_click("tools_button");
                        ui.close();
                    }

                    ui.separator();

                    // Windows submenu offering window visibility toggles.
                    ui.menu_button(crate::ui::strings::MENU_WINDOWS, |ui| {
                        let show_bg = {
                            app.orchestrator
                                .background_manager
                                .lock()
                                .unwrap()
                                .show_background_logs
                        };
                        let background_operations_label =
                            crate::ui::strings::format_menu_selection_label(
                                crate::ui::strings::MENU_BACKGROUND_OPERATIONS,
                                show_bg,
                            );
                        if ui.button(background_operations_label).clicked() {
                            apply_background_logs_toggle(app, !show_bg);
                            on_click(crate::ui::strings::BACKGROUND_OPERATIONS_EVENT);
                            ui.close();
                        }

                        let show_debug = app.orchestrator.agent_panel_state.show_debug_window;
                        let agent_debug_label = crate::ui::strings::format_menu_selection_label(
                            crate::ui::strings::MENU_AGENT_DEBUG,
                            show_debug,
                        );
                        if ui.button(agent_debug_label).clicked() {
                            apply_agent_debug_toggle(app, !show_debug);
                            on_click(crate::ui::strings::AGENT_DEBUG_EVENT);
                            ui.close();
                        }
                    });

                    // Chat models submenu offering chat model selection.
                    ui.menu_button(crate::ui::strings::MENU_CHAT_MODELS, |ui| {
                        let current_model = app.config().current_chat_model_key();
                        let mut chat_models: Vec<(&String, &crate::config::LlmConfig)> = app
                            .config()
                            .models
                            .iter()
                            .filter(|(_, cfg)| cfg.has_use_case("chat"))
                            .collect();
                        if chat_models.is_empty() {
                            chat_models = app.config().models.iter().collect();
                        }
                        chat_models.sort_by_key(|(name, _)| (*name).clone());

                        if chat_models.is_empty() {
                            ui.add_enabled(
                                false,
                                egui::Button::new(crate::ui::strings::NO_CHAT_MODELS_CONFIGURED),
                            );
                        } else {
                            let mut pending_model: Option<String> = None;
                            for (name, model_cfg) in chat_models {
                                let is_current = current_model.as_deref() == Some(name.as_str());
                                let checkmark = if is_current { "✓ " } else { "   " };
                                let label = format!(
                                    "{}{}",
                                    checkmark,
                                    crate::ui::strings::format_chat_model_menu_label(
                                        name,
                                        model_cfg.get_cost()
                                    )
                                );
                                if ui.button(label).clicked() {
                                    if !is_current {
                                        pending_model = Some(name.clone());
                                    }
                                    ui.close();
                                }
                            }

                            if let Some(picked) = pending_model {
                                apply_chat_model_selection(app, picked, &mut persist);
                                on_click(crate::ui::strings::CHAT_MODEL_SELECTION_EVENT);
                            }
                        }
                    });

                    // Table wrap algorithm submenu offering deficit strategy selection.
                    ui.menu_button(crate::ui::strings::MENU_TABLE_WRAP_ALGORITHM, |ui| {
                        let current_strategy = app.orchestrator.config.deficit_strategy();
                        let mut pending: Option<DeficitStrategy> = None;

                        // Order matters: default first (HybridMinPenaltyWaterFill,
                        // best G1/G2 trade-off), then the two original FTWA
                        // strategies, then the three survey algorithms from
                        // `doc/planning/table-column-width-algorithm.md` §2.10
                        // / §2.13 / §2.14. Future strategies should be appended
                        // here AND in `DeficitStrategy` in
                        // `src/markdown/table_width/mod.rs` (exhaustive match).
                        for variant in [
                            DeficitStrategy::HybridMinPenaltyWaterFill,
                            DeficitStrategy::ProportionalToSlack,
                            DeficitStrategy::BreakpointWaterFill,
                            DeficitStrategy::WaterFillRatio,
                            DeficitStrategy::LagrangePenalty,
                        ] {
                            let is_selected = variant == current_strategy;
                            let checkmark = if is_selected { "✓ " } else { "   " };
                            let label = format!("{}{}", checkmark, strategy_label(variant));
                            if ui.button(label).clicked() {
                                if !is_selected {
                                    pending = Some(variant);
                                }
                                ui.close();
                            }
                        }

                        if let Some(picked) = pending {
                            apply_table_width_strategy_change(app, picked, &mut persist);
                            on_click(crate::ui::strings::TABLE_WIDTH_STRATEGY_EVENT);
                        }
                    });
                });
            });
        });
    });
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `top_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "top_tests.rs"]
mod tests;
