use crate::batch::types::{
    validate_batch_params, BatchConfig, BatchDialogConfig, BatchDialogResult, BatchMode,
};
use eframe::egui;

/// Shows the batch prompt processing dialog.
/// Returns `Some(result)` when dialog closes, `None` if still open.
pub fn show_batch_modal(
    app: &mut crate::ui::FastMdApp,
    ctx: &egui::Context,
    config: &mut BatchDialogConfig,
) -> Option<BatchDialogResult> {
    let mut result = None;
    let is_running = app.dialogs.batch_handle.is_some();
    let mut dialog_open = app.dialogs.batch_dialog_open;

    // Refresh prompts from the tag manager on every dialog open so
    // the list stays in sync with the tag index (prompt discovery no
    // longer does a separate filesystem walk).
    if dialog_open {
        config.available_prompts = crate::batch::prompts::resolve_prompts(
            app.tag_manager.prompt_paths(),
            &app.config.content_libraries,
        );
    }

    egui::Window::new("Batch Prompt Processing")
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

    app.dialogs.batch_dialog_open = dialog_open;

    // Handle dialog close via window X button
    if !dialog_open && result.is_none() {
        // If running, cancel before closing
        if let Some(handle) = &app.dialogs.batch_handle {
            handle.cancel();
        }
        result = Some(BatchDialogResult::Cancel);
    }

    result
}

/// Shows the configuration view (idle state).
fn show_config_view(
    ui: &mut egui::Ui,
    _app: &mut crate::ui::FastMdApp,
    config: &mut BatchDialogConfig,
    result: &mut Option<BatchDialogResult>,
) {
    // Directory selector
    ui.horizontal(|ui| {
        ui.label("Directory:");
        egui::ComboBox::from_id_source("batch_dir_combo")
            .selected_text(
                config
                    .selected_dir_idx
                    .and_then(|i| config.available_dirs.get(i))
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "Select directory...".to_string()),
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
            ui.label("Pattern:");
            ui.add(egui::TextEdit::singleline(&mut config.pattern).hint_text("*.md"));
        });
    }

    // Prompt selector
    ui.horizontal(|ui| {
        ui.label("Prompt:");
        egui::ComboBox::from_id_source("batch_prompt_combo")
            .selected_text(
                config
                    .selected_prompt_idx
                    .and_then(|i| config.available_prompts.get(i))
                    .map(|p| p.display_name.clone())
                    .unwrap_or_else(|| "Select prompt...".to_string()),
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
        ui.label("Mode:");
        ui.radio_value(&mut config.mode, BatchMode::File, "File");
        ui.radio_value(&mut config.mode, BatchMode::Directory, "Directory");
    });

    // Concurrency selector
    ui.horizontal(|ui| {
        ui.label("Concurrency:");
        egui::ComboBox::from_id_source("batch_concurrency_combo")
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
                .add_enabled(process_enabled, egui::Button::new("Process"))
                .clicked()
            {
                if let (Some(dir_idx), Some(prompt_idx)) =
                    (config.selected_dir_idx, config.selected_prompt_idx)
                {
                    if let (Some(directory), Some(prompt)) = (
                        config.available_dirs.get(dir_idx),
                        config.available_prompts.get(prompt_idx),
                    ) {
                        let batch_config = BatchConfig {
                            directory: directory.clone(),
                            pattern: config.pattern.clone(),
                            prompt_path: prompt.path.clone(),
                            mode: config.mode,
                            concurrency: config.concurrency,
                        };
                        *result = Some(BatchDialogResult::Process(batch_config));
                    }
                }
            }

            if ui.button("Cancel").clicked() {
                *result = Some(BatchDialogResult::Cancel);
            }
        });
    });
}

/// Shows the running/progress view during batch processing.
fn show_running_view(
    ui: &mut egui::Ui,
    app: &mut crate::ui::FastMdApp,
    result: &mut Option<BatchDialogResult>,
) {
    let is_finished = app
        .dialogs
        .batch_handle
        .as_ref()
        .map(|h| h.thread.is_finished())
        .unwrap_or(true);

    ui.heading("Batch Processing");
    ui.separator();

    if is_finished {
        ui.label("Batch processing completed.");
        if let Some(cancel_flag) = &app.dialogs.batch_cancel_flag {
            if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
                ui.colored_label(egui::Color32::YELLOW, "Batch was cancelled by user.");
            }
        }
    } else {
        ui.label("Processing...");
        ui.add(egui::Spinner::new());
        ui.label("Click Cancel to stop processing. In-flight jobs will finish.");
    }

    ui.separator();

    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if is_finished {
                if ui.button("Close").clicked() {
                    *result = Some(BatchDialogResult::Cancel);
                }
            } else {
                if ui.button("Cancel").clicked() {
                    if let Some(handle) = &app.dialogs.batch_handle {
                        handle.cancel();
                    }
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
mod tests {
    use super::*;

    #[test]
    fn test_is_config_valid_valid() {
        let config = BatchDialogConfig {
            selected_dir_idx: Some(0),
            selected_prompt_idx: Some(0),
            pattern: "*.md".to_string(),
            mode: BatchMode::File,
            concurrency: 4,
            ..Default::default()
        };
        assert!(is_config_valid(&config));
    }

    #[test]
    fn test_is_config_valid_no_directory() {
        let config = BatchDialogConfig {
            selected_dir_idx: None,
            selected_prompt_idx: Some(0),
            pattern: "*.md".to_string(),
            mode: BatchMode::File,
            concurrency: 4,
            ..Default::default()
        };
        assert!(!is_config_valid(&config));
    }

    #[test]
    fn test_is_config_valid_no_prompt() {
        let config = BatchDialogConfig {
            selected_dir_idx: Some(0),
            selected_prompt_idx: None,
            pattern: "*.md".to_string(),
            mode: BatchMode::File,
            concurrency: 4,
            ..Default::default()
        };
        assert!(!is_config_valid(&config));
    }

    #[test]
    fn test_is_config_valid_empty_pattern_file_mode() {
        let config = BatchDialogConfig {
            selected_dir_idx: Some(0),
            selected_prompt_idx: Some(0),
            pattern: "".to_string(),
            mode: BatchMode::File,
            concurrency: 4,
            ..Default::default()
        };
        assert!(!is_config_valid(&config));
    }

    #[test]
    fn test_is_config_valid_directory_mode_no_pattern() {
        let config = BatchDialogConfig {
            selected_dir_idx: Some(0),
            selected_prompt_idx: Some(0),
            pattern: "".to_string(),
            mode: BatchMode::Directory,
            concurrency: 4,
            ..Default::default()
        };
        assert!(is_config_valid(&config));
    }

    #[test]
    fn test_is_config_valid_concurrency_zero() {
        let config = BatchDialogConfig {
            selected_dir_idx: Some(0),
            selected_prompt_idx: Some(0),
            pattern: "*.md".to_string(),
            mode: BatchMode::File,
            concurrency: 0,
            ..Default::default()
        };
        assert!(!is_config_valid(&config));
    }

    #[test]
    fn test_is_config_valid_concurrency_too_high() {
        let config = BatchDialogConfig {
            selected_dir_idx: Some(0),
            selected_prompt_idx: Some(0),
            pattern: "*.md".to_string(),
            mode: BatchMode::File,
            concurrency: 9,
            ..Default::default()
        };
        assert!(!is_config_valid(&config));
    }
}
