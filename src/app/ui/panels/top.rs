//! Top toolbar panel — indexing status, tag filter dropdown, new-file/new-dir buttons, and content-library name.
//!
//! Unit tests live in the sibling `top_tests.rs` sidecar.

use crate::bus::events::user_command::UserCommand;
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
/// Outputs: `Option<PathBuf>` representing the new selected file.
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

/// Returns the command for clicking the batch-processing button.
pub fn apply_batch_button_click() -> UserCommand {
    UserCommand::OpenBatchDialog
}

/// Returns the command for clicking the "Tools..." button.
pub fn apply_tools_button_click() -> UserCommand {
    UserCommand::OpenToolsDialog
}

/// Returns the command for toggling the background logs window.
pub fn apply_background_logs_toggle(show: bool) -> UserCommand {
    UserCommand::ToggleBackgroundLogs(show)
}

/// Returns the command for toggling the agent debug window.
pub fn apply_agent_debug_toggle(show: bool) -> UserCommand {
    UserCommand::ToggleAgentDebugWindow(show)
}

/// Returns the command for picking a new chat model.
pub fn apply_chat_model_selection(model_name: String) -> UserCommand {
    UserCommand::SelectChatModel(model_name)
}

/// Returns the command for changing the table-width deficit strategy.
pub fn apply_table_width_strategy_change(strategy: DeficitStrategy) -> UserCommand {
    UserCommand::ChangeTableWidthStrategy(strategy)
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

pub fn show_top_panel(app: &mut FastMdApp, parent_ui: &mut egui::Ui) {
    show_top_panel_capture(app, parent_ui, |_| {});
}

/// Tier 4 test variant of [`show_top_panel`]. The `on_click` callback
/// is invoked after every button click in the toolbar, with a stable
/// event name.
#[tracing::instrument(skip_all, name = "ui.panel.top", level = "debug")]
pub fn show_top_panel_capture(
    app: &mut FastMdApp,
    parent_ui: &mut egui::Ui,
    mut on_click: impl FnMut(&'static str),
) {
    // egui 0.35 unified `TopBottomPanel` into `Panel`.
    Panel::top("top_panel").show(parent_ui, |ui| {
        ui.horizontal(|ui| {
            // Document + Bolt logo badge (20×20) — vector painted so no texture dependency.
            let logo_rect = {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
                crate::ui::logo::paint_logo(ui, rect);
                rect
            };
            // Keep the heading accessible; `APP_TITLE` is plain text and the logo conveys the mark.
            let _ = logo_rect;
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

                    // Direct action items ?" open dialogs immediately on click.
                    if ui.button(crate::ui::strings::MENU_BATCH).clicked() {
                        app.orchestrator
                            .user_command_bus
                            .publish(apply_batch_button_click());
                        on_click("batch_button");
                        ui.close();
                    }
                    if ui.button(crate::ui::strings::MENU_TOOLS).clicked() {
                        app.orchestrator
                            .user_command_bus
                            .publish(apply_tools_button_click());
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
                        // egui's `ui.checkbox` draws the checkmark as vector
                        // strokes, so it renders correctly without relying on a
                        // glyph (e.g. U+2713) that is absent from egui's bundled
                        // default fonts. On click egui flips `background_checked`
                        // before returning, so `apply_background_logs_toggle`
                        // receives the post-click value.
                        let mut background_checked = show_bg;
                        if ui
                            .checkbox(
                                &mut background_checked,
                                crate::ui::strings::MENU_BACKGROUND_OPERATIONS,
                            )
                            .clicked()
                        {
                            app.orchestrator
                                .user_command_bus
                                .publish(apply_background_logs_toggle(background_checked));
                            on_click(crate::ui::strings::BACKGROUND_OPERATIONS_EVENT);
                            ui.close();
                        }

                        let show_debug = app.orchestrator.agent_panel_state.show_debug_window;
                        let mut debug_checked = show_debug;
                        if ui
                            .checkbox(&mut debug_checked, crate::ui::strings::MENU_AGENT_DEBUG)
                            .clicked()
                        {
                            app.orchestrator
                                .user_command_bus
                                .publish(apply_agent_debug_toggle(debug_checked));
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
                                // `ui.checkbox` renders the selected indicator
                                // as vector strokes rather than a `✓` glyph that
                                // is absent from egui's bundled default fonts
                                // (see the Chat models / Windows / Table wrap
                                // submenus). The local `checked` flip on click
                                // is invisible because the menu closes
                                // immediately.
                                let mut checked = is_current;
                                let label = crate::ui::strings::format_chat_model_menu_label(
                                    name,
                                    model_cfg.get_cost(),
                                );
                                if ui.checkbox(&mut checked, label).clicked() {
                                    if !is_current {
                                        pending_model = Some(name.clone());
                                    }
                                    ui.close();
                                }
                            }

                            if let Some(picked) = pending_model {
                                app.orchestrator
                                    .user_command_bus
                                    .publish(apply_chat_model_selection(picked));
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
                            let mut checked = is_selected;
                            if ui.checkbox(&mut checked, strategy_label(variant)).clicked() {
                                if !is_selected {
                                    pending = Some(variant);
                                }
                                ui.close();
                            }
                        }

                        if let Some(picked) = pending {
                            app.orchestrator
                                .user_command_bus
                                .publish(apply_table_width_strategy_change(picked));
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
