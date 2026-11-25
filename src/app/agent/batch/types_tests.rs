//! Tests for the batch types (`agent::batch::types`).

use super::*;
use std::sync::atomic::Ordering;

#[test]
fn test_batch_config_validate_valid_file_mode() {
    let dir = tempfile::tempdir().unwrap();
    let prompt = dir.path().join("prompt.md");
    std::fs::write(&prompt, "test").unwrap();

    let config = BatchConfig {
        directory: dir.path().to_path_buf(),
        pattern: "*.md".to_string(),
        prompt_path: prompt,
        mode: BatchMode::File,
        concurrency: 4,
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_batch_config_validate_valid_directory_mode() {
    let dir = tempfile::tempdir().unwrap();
    let prompt = dir.path().join("prompt.md");
    std::fs::write(&prompt, "test").unwrap();

    let config = BatchConfig {
        directory: dir.path().to_path_buf(),
        pattern: "".to_string(),
        prompt_path: prompt,
        mode: BatchMode::Directory,
        concurrency: 4,
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_batch_config_validate_invalid_concurrency() {
    let dir = tempfile::tempdir().unwrap();
    let prompt = dir.path().join("prompt.md");
    std::fs::write(&prompt, "test").unwrap();

    let config = BatchConfig {
        directory: dir.path().to_path_buf(),
        pattern: "*.md".to_string(),
        prompt_path: prompt,
        mode: BatchMode::File,
        concurrency: 0,
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_batch_config_validate_missing_directory() {
    let prompt = PathBuf::from("prompt.md");
    std::fs::write(&prompt, "test").unwrap();

    let config = BatchConfig {
        directory: PathBuf::from("/nonexistent"),
        pattern: "*.md".to_string(),
        prompt_path: prompt.clone(),
        mode: BatchMode::File,
        concurrency: 4,
    };
    assert!(config.validate().is_err());

    let _ = std::fs::remove_file(&prompt);
}

#[test]
fn test_batch_config_validate_missing_prompt_path() {
    let dir = tempfile::tempdir().unwrap();

    let config = BatchConfig {
        directory: dir.path().to_path_buf(),
        pattern: "*.md".to_string(),
        prompt_path: PathBuf::from("/nonexistent/prompt.md"),
        mode: BatchMode::File,
        concurrency: 4,
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_batch_config_validate_file_mode_empty_pattern() {
    let dir = tempfile::tempdir().unwrap();
    let prompt = dir.path().join("prompt.md");
    std::fs::write(&prompt, "test").unwrap();

    let config = BatchConfig {
        directory: dir.path().to_path_buf(),
        pattern: "".to_string(),
        prompt_path: prompt,
        mode: BatchMode::File,
        concurrency: 4,
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_batch_session_progress() {
    let config = BatchConfig {
        directory: PathBuf::from("/tmp"),
        pattern: "*.md".to_string(),
        prompt_path: PathBuf::from("/tmp/prompt.md"),
        mode: BatchMode::File,
        concurrency: 4,
    };

    let mut session = BatchSession {
        config,
        jobs: (0..4)
            .map(|i| BatchJob {
                id: i,
                target_path: PathBuf::from(format!("/tmp/file{}.md", i)),
                active_file: None,
                active_dir: None,
                prompt_text: "test".to_string(),
                status: BatchJobStatus::Pending,
                start_time: None,
                end_time: None,
                error: None,
            })
            .collect(),
        cancel_flag: Arc::new(AtomicBool::new(false)),
        start_time: Local::now(),
    };

    assert_eq!(session.total_jobs(), 4);
    assert_eq!(session.progress(), 0.0);

    session.jobs[0].status = BatchJobStatus::Completed;
    session.jobs[1].status = BatchJobStatus::Failed;
    assert_eq!(session.progress(), 0.5);

    session.jobs[2].status = BatchJobStatus::Completed;
    assert_eq!(session.progress(), 0.75);

    assert_eq!(session.completed_jobs(), 2);
    assert_eq!(session.failed_jobs(), 1);
    assert_eq!(session.running_jobs(), 0);
}

#[test]
fn test_batch_handle_cancel() {
    let cancel_flag = Arc::new(AtomicBool::new(false));
    assert!(!cancel_flag.load(Ordering::SeqCst));

    let handle = BatchHandle {
        thread: std::thread::spawn(|| BatchResult {
            total_jobs: 0,
            completed: 0,
            failed: 0,
            cancelled: 0,
            duration: Duration::ZERO,
        }),
        cancel_flag: cancel_flag.clone(),
    };

    handle.cancel();
    assert!(cancel_flag.load(Ordering::SeqCst));
    let _ = handle.join();
}

#[test]
fn test_batch_dialog_config_default() {
    let config = BatchDialogConfig::default();
    assert!(config.available_dirs.is_empty());
    assert!(
        config.available_dir_labels.is_empty(),
        "no virtual labels are known until a config arrival populates them"
    );
    assert!(config.available_prompts.is_empty());
    assert_eq!(config.pattern, "*.md");
    assert_eq!(config.mode, BatchMode::File);
    assert_eq!(config.concurrency, 4);
    assert!(config.selected_dir_idx.is_none());
    assert!(config.selected_prompt_idx.is_none());
}

#[test]
fn test_validate_batch_params_valid() {
    assert!(validate_batch_params(BatchMode::File, "*.md", 4).is_ok());
    assert!(validate_batch_params(BatchMode::Directory, "", 1).is_ok());
    assert!(validate_batch_params(BatchMode::Directory, "", 8).is_ok());
}

#[test]
fn test_validate_batch_params_concurrency_bounds() {
    assert!(validate_batch_params(BatchMode::File, "*.md", 0).is_err());
    assert!(validate_batch_params(BatchMode::File, "*.md", 9).is_err());
}

#[test]
fn test_validate_batch_params_empty_pattern_file_mode() {
    assert!(validate_batch_params(BatchMode::File, "", 4).is_err());
    assert!(validate_batch_params(BatchMode::File, "  ", 4).is_err());
}

#[test]
fn test_validate_batch_params_invalid_pattern() {
    assert!(validate_batch_params(BatchMode::File, "[invalid", 4).is_err());
}

#[test]
fn test_validate_batch_params_empty_pattern_directory_mode() {
    // Directory mode does not require a pattern
    assert!(validate_batch_params(BatchMode::Directory, "", 4).is_ok());
}
