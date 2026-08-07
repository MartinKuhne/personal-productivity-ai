//! Batch prompt-processing dialog UI — directory/prompt/concurrency selection, progress display, and results summary.
//!
//! This is the egui presentation layer for the batch subsystem. The domain
//! logic and data types live in [`crate::app::batch`]; this module only renders
//! the dialog and translates UI state into a [`BatchDialogResult`].

use crate::app::batch::prompts::resolve_prompts;
use crate::app::batch::types::{
    BatchConfig, BatchDialogConfig, BatchDialogResult, BatchMode, validate_batch_params,
};
use crate::ui::FastMdApp;
use crate::ui::strings;
use eframe::egui;

/// Shows the batch prompt processing dialog.
/// Returns `Some(result)` when dialog closes, `None` if still open.
pub fn show_batch_modal(
    app: &mut FastMdApp,
    ctx: &egui::Context,
    config: &mut BatchDialogConfig,
) -> Option<BatchDialogResult> {
    let mut result = None;
    let is_running = app.dialogs().batch_handle.is_some();
    let mut dialog_open = app.dialogs().batch_dialog_open;

    if dialog_open {
        config.available_prompts =
            resolve_prompts(app.tags().prompt_paths(), &app.config().content_libraries);
        // Clamp the previously-selected prompt index to the new list length.
        // When the prompt list shrinks or is reordered the stale index would
        // otherwise be out-of-bounds (see issue: selected_prompt_idx not
        // adjusted after resolve_prompts replaces the list).
        if let Some(idx) = config.selected_prompt_idx
            && idx >= config.available_prompts.len()
        {
            config.selected_prompt_idx = None;
        }
    }

    egui::Window::new(strings::BATCH_DIALOG_WINDOW)
        .open(&mut dialog_open)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.set_min_width(500.0);
            ui.spacing_mut().item_spacing.y = 10.0;

            if is_running {
                show_running_view(ui, app, &mut result);
            } else {
                show_config_view(ui, app, config, &mut result);
            }
        });

    app.dialogs_mut().batch_dialog_open = dialog_open;

    if !dialog_open && result.is_none() {
        if let Some(handle) = &app.dialogs().batch_handle {
            handle.cancel();
        }
        result = Some(BatchDialogResult::Cancel);
    }

    result
}

/// Shows the configuration view (idle state).
fn show_config_view(
    ui: &mut egui::Ui,
    _app: &mut FastMdApp,
    config: &mut BatchDialogConfig,
    result: &mut Option<BatchDialogResult>,
) {
    // Directory selector
    ui.horizontal(|ui| {
        ui.label(strings::BATCH_DIALOG_DIRECTORY_LABEL);
        egui::ComboBox::from_id_salt("batch_dir_combo")
            .selected_text(
                config
                    .selected_dir_idx
                    .and_then(|i| config.available_dirs.get(i))
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| strings::BATCH_DIALOG_SELECT_DIRECTORY.to_string()),
            )
            .show_ui(ui, |ui| {
                for (idx, dir) in config.available_dirs.iter().enumerate() {
                    let label = dir.display().to_string();
                    if ui
                        .selectable_label(config.selected_dir_idx == Some(idx), label)
                        .clicked()
                    {
                        config.selected_dir_idx = Some(idx);
                    }
                }
            });
    });

    // Wildcard pattern (hidden in Directory mode)
    if config.mode == BatchMode::File {
        ui.horizontal(|ui| {
            ui.label(strings::BATCH_DIALOG_PATTERN_LABEL);
            ui.add(egui::TextEdit::singleline(&mut config.pattern).hint_text("*.md"));
        });
    }

    // Prompt selector
    ui.horizontal(|ui| {
        ui.label(strings::BATCH_DIALOG_PROMPT_LABEL);
        egui::ComboBox::from_id_salt("batch_prompt_combo")
            .selected_text(
                config
                    .selected_prompt_idx
                    .and_then(|i| config.available_prompts.get(i))
                    .map(|p| p.display_name.clone())
                    .unwrap_or_else(|| strings::BATCH_DIALOG_SELECT_PROMPT.to_string()),
            )
            .show_ui(ui, |ui| {
                for (idx, prompt) in config.available_prompts.iter().enumerate() {
                    if ui
                        .selectable_label(
                            config.selected_prompt_idx == Some(idx),
                            &prompt.display_name,
                        )
                        .clicked()
                    {
                        config.selected_prompt_idx = Some(idx);
                    }
                }
            });
    });

    // Batch mode selector
    ui.horizontal(|ui| {
        ui.label(strings::BATCH_DIALOG_MODE_LABEL);
        ui.radio_value(&mut config.mode, BatchMode::File, strings::BATCH_MODE_FILE);
        ui.radio_value(
            &mut config.mode,
            BatchMode::Directory,
            strings::BATCH_MODE_DIRECTORY,
        );
    });

    // Concurrency selector
    ui.horizontal(|ui| {
        ui.label(strings::BATCH_DIALOG_CONCURRENCY_LABEL);
        egui::ComboBox::from_id_salt("batch_concurrency_combo")
            .selected_text(config.concurrency.to_string())
            .show_ui(ui, |ui| {
                for n in 1..=8 {
                    if ui
                        .selectable_label(config.concurrency == n, n.to_string())
                        .clicked()
                    {
                        config.concurrency = n;
                    }
                }
            });
    });

    ui.separator();

    // Process and Cancel buttons
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let process_enabled = is_config_valid(config);

            if ui
                .add_enabled(
                    process_enabled,
                    egui::Button::new(strings::BATCH_PROCESS_BUTTON),
                )
                .clicked()
                && let (Some(dir_idx), Some(prompt_idx)) =
                    (config.selected_dir_idx, config.selected_prompt_idx)
                && let (Some(directory), Some(prompt)) = (
                    config.available_dirs.get(dir_idx),
                    config.available_prompts.get(prompt_idx),
                )
            {
                let batch_config = BatchConfig {
                    directory: directory.clone(),
                    pattern: config.pattern.clone(),
                    prompt_path: prompt.path.clone(),
                    mode: config.mode,
                    concurrency: config.concurrency,
                };
                *result = Some(BatchDialogResult::Process(batch_config));
            }

            if ui.button(strings::CANCEL_BUTTON).clicked() {
                *result = Some(BatchDialogResult::Cancel);
            }
        });
    });
}

/// Shows the running/progress view during batch processing.
fn show_running_view(
    ui: &mut egui::Ui,
    app: &mut FastMdApp,
    result: &mut Option<BatchDialogResult>,
) {
    let is_finished = app
        .dialogs()
        .batch_handle
        .as_ref()
        .map(|h| h.thread.is_finished())
        .unwrap_or(true);

    ui.heading(strings::BATCH_RUNNING_HEADING);
    ui.separator();

    if is_finished {
        ui.label(strings::BATCH_COMPLETED_TEXT);
        if let Some(cancel_flag) = &app.dialogs().batch_cancel_flag
            && cancel_flag.load(std::sync::atomic::Ordering::SeqCst)
        {
            ui.colored_label(egui::Color32::YELLOW, strings::BATCH_CANCELLED_TEXT);
        }
    } else {
        ui.label(strings::BATCH_PROCESSING_TEXT);
        ui.add(egui::Spinner::new());
        ui.label(strings::BATCH_RUNNING_CANCEL_HINT);
    }

    ui.separator();

    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if is_finished {
                if ui.button(strings::BATCH_CLOSE_BUTTON).clicked() {
                    *result = Some(BatchDialogResult::Cancel);
                }
            } else {
                if ui.button(strings::CANCEL_BUTTON).clicked()
                    && let Some(handle) = &app.dialogs().batch_handle
                {
                    handle.cancel();
                }
            }
        });
    });
}

/// Validates the current dialog configuration.
fn is_config_valid(config: &BatchDialogConfig) -> bool {
    if config.selected_dir_idx.is_none() {
        return false;
    }
    if config.selected_prompt_idx.is_none() {
        return false;
    }
    validate_batch_params(config.mode, &config.pattern, config.concurrency).is_ok()
}

#[cfg(test)]
#[path = "batch_dialog_tests.rs"]
mod batch_dialog_tests;
