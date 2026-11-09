//! Tests for [`BatchCoordinator`] (`agent::batch::coordinator`).

use super::*;
use crate::bus::events::typed::BackgroundEventSender;
use crate::config::AppConfig;
use tokio::sync::broadcast;

#[test]
fn test_coordinator_new_and_execute() {
    let dir = tempfile::tempdir().unwrap();
    let (tx, _rx) = broadcast::channel(16);
    let tx = BackgroundEventSender::new(tx);
    let bus: Bus<FileChanged> = Bus::new();

    let config = BatchConfig {
        directory: dir.path().to_path_buf(),
        pattern: "*.md".to_string(),
        prompt_path: dir.path().join("prompt.md"),
        mode: BatchMode::File,
        concurrency: 4,
    };

    let app_config = AppConfig::default();
    let (coordinator, cancel_flag) = BatchCoordinator::new(
        config,
        app_config,
        tx,
        bus,
        "test prompt".to_string(),
        Arc::new(crate::utils::clock::SystemClock),
    );

    assert!(!cancel_flag.load(Ordering::SeqCst));
    let handle = coordinator.execute();
    let result = handle.join();
    assert_eq!(result.total_jobs, 0);
    assert_eq!(result.completed, 0);
    assert_eq!(result.failed, 0);
}

#[test]
fn test_coordinator_empty_directory() {
    let dir = tempfile::tempdir().unwrap();
    let (tx, _rx) = broadcast::channel(16);
    let tx = BackgroundEventSender::new(tx);
    let bus: Bus<FileChanged> = Bus::new();

    let config = BatchConfig {
        directory: dir.path().to_path_buf(),
        pattern: "*.md".to_string(),
        prompt_path: dir.path().join("prompt.md"),
        mode: BatchMode::File,
        concurrency: 4,
    };

    let app_config = AppConfig::default();
    let (coordinator, _cancel_flag) = BatchCoordinator::new(
        config,
        app_config,
        tx,
        bus,
        "test prompt".to_string(),
        Arc::new(crate::utils::clock::SystemClock),
    );

    let handle = coordinator.execute();
    let result = handle.join();
    assert_eq!(result.total_jobs, 0);
    assert_eq!(result.completed, 0);
    assert_eq!(result.failed, 0);
}

#[test]
fn test_coordinator_with_matching_files() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("test1.md"), "").unwrap();
    std::fs::write(dir.path().join("test2.md"), "").unwrap();
    std::fs::write(dir.path().join("test3.md"), "").unwrap();

    let (tx, _rx) = broadcast::channel(16);
    let tx = BackgroundEventSender::new(tx);
    let bus: Bus<FileChanged> = Bus::new();

    let config = BatchConfig {
        directory: dir.path().to_path_buf(),
        pattern: "*.md".to_string(),
        prompt_path: dir.path().join("prompt.md"),
        mode: BatchMode::File,
        concurrency: 4,
    };

    let app_config = AppConfig::default();
    let (coordinator, _) = BatchCoordinator::new(
        config,
        app_config,
        tx,
        bus,
        "test prompt".to_string(),
        Arc::new(crate::utils::clock::SystemClock),
    );

    let handle = coordinator.execute();
    let result = handle.join();
    assert!(result.total_jobs > 0);
    assert!(result.duration.as_secs() < 60);
}
