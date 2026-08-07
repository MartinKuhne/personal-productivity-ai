//! Tests for `ui/batch_dialog.rs`

use super::*;
use crate::ui::test_helpers::text::extract_text;
use eframe::egui;

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

// ---------------------------------------------------------------------------
// UI Rendering tests
// ---------------------------------------------------------------------------

fn render_dialog_once(app: &mut crate::ui::FastMdApp, config: &mut BatchDialogConfig) -> egui::FullOutput {
    let ctx = egui::Context::default();
    app.dialogs_mut().batch_dialog_open = true; // Ensure dialog renders
    
    let raw_input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1920.0, 1080.0))),
        ..Default::default()
    };
    
    // First frame initializes the window
    let _ = ctx.run_ui(raw_input.clone(), |ui| {
        let _ = show_batch_modal(app, ui.ctx(), config);
    });
    
    // Second frame actually renders the contents
    app.dialogs_mut().batch_dialog_open = true;
    ctx.run_ui(raw_input, |ui| {
        let _ = show_batch_modal(app, ui.ctx(), config);
    })
}

#[test]
fn test_batch_dialog_renders_config_view() {
    let mut app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    let mut config = BatchDialogConfig::default();
    let output = render_dialog_once(&mut app, &mut config);
    
    let texts = extract_text(&output.shapes);
    
    // In config view, we expect Directory, Pattern, Prompt, Mode, and Concurrency labels
    let has_dir_label = texts.iter().any(|t| t.contains(crate::ui::strings::BATCH_DIALOG_DIRECTORY_LABEL));
    assert!(has_dir_label, "Dialog must render directory label in config view. Texts: {:?}", texts);
    
    let has_mode_file = texts.iter().any(|t| t.contains(crate::ui::strings::BATCH_MODE_FILE));
    assert!(has_mode_file, "Dialog must render file mode radio. Texts: {:?}", texts);
}

#[test]
fn test_batch_dialog_renders_running_view() {
    let mut app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    // Simulate running state by attaching a dummy handle
    let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let dummy_thread = std::thread::spawn(|| crate::app::batch::types::BatchResult {
        total_jobs: 0,
        completed: 0,
        failed: 0,
        cancelled: 0,
        duration: std::time::Duration::ZERO,
    }); // Finished immediately
    let handle = crate::app::batch::types::BatchHandle {
        cancel_flag,
        thread: dummy_thread,
    };
    app.dialogs_mut().batch_handle = Some(handle);
    
    let mut config = BatchDialogConfig::default();
    let output = render_dialog_once(&mut app, &mut config);
    
    let texts = extract_text(&output.shapes);
    
    // In running view (with finished thread), we expect "Completed." text
    let has_completed = texts.iter().any(|t| t.contains(crate::ui::strings::BATCH_COMPLETED_TEXT));
    assert!(has_completed, "Dialog must render completed text when task finishes. Texts: {:?}", texts);
    
    let has_close = texts.iter().any(|t| t.contains(crate::ui::strings::BATCH_CLOSE_BUTTON));
    assert!(has_close, "Dialog must render close button when finished. Texts: {:?}", texts);
}
