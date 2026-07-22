//! Batch coordinator — discovers targets, spawns the executor on a background thread, polls progress, and reports results.

use crate::batch::discoverer::JobDiscoverer;
use crate::batch::executor::BatchJobExecutor;
use crate::batch::types::{
    BatchConfig, BatchHandle, BatchJob, BatchJobStatus, BatchMode, BatchResult,
};
use crate::config::AppConfig;
use crate::file_events::Bus;
use crate::file_events::FileEvent;
use crate::messages::BackgroundMessage;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Instant;

pub struct BatchCoordinator {
    config: BatchConfig,
    app_config: AppConfig,
    tx_gui: mpsc::Sender<BackgroundMessage>,
    file_event_bus: Bus<FileEvent>,
    prompt_text: String,
    cancel_flag: Arc<AtomicBool>,
}

impl BatchCoordinator {
    pub fn new(
        config: BatchConfig,
        app_config: AppConfig,
        tx_gui: mpsc::Sender<BackgroundMessage>,
        file_event_bus: Bus<FileEvent>,
        prompt_text: String,
    ) -> (Self, Arc<AtomicBool>) {
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let coordinator = Self {
            config,
            app_config,
            tx_gui,
            file_event_bus,
            prompt_text,
            cancel_flag: cancel_flag.clone(),
        };
        (coordinator, cancel_flag)
    }

    pub fn execute(self) -> BatchHandle {
        let cancel_flag = self.cancel_flag.clone();
        let thread = thread::spawn(move || self.run());
        BatchHandle {
            thread,
            cancel_flag,
        }
    }

    fn run(self) -> BatchResult {
        let start_time = Instant::now();
        let discoverer: Box<dyn JobDiscoverer> = <dyn JobDiscoverer>::from_config(&self.config);
        let targets = match discoverer.discover() {
            Ok(t) => t,
            Err(e) => {
                tracing::error!(target: "batch", error = %e, "Discovery failed");
                return BatchResult {
                    total_jobs: 0,
                    completed: 0,
                    failed: 0,
                    cancelled: 0,
                    duration: start_time.elapsed(),
                };
            }
        };

        if targets.is_empty() {
            tracing::info!(target: "batch", "No targets found");
            return BatchResult {
                total_jobs: 0,
                completed: 0,
                failed: 0,
                cancelled: 0,
                duration: start_time.elapsed(),
            };
        }

        let jobs: Vec<BatchJob> = targets
            .into_iter()
            .enumerate()
            .map(|(idx, target_path)| {
                let (active_file, active_dir) = match self.config.mode {
                    BatchMode::File => (Some(target_path.clone()), None),
                    BatchMode::Directory => (None, Some(target_path.clone())),
                };
                BatchJob {
                    id: idx,
                    target_path,
                    active_file,
                    active_dir,
                    prompt_text: self.prompt_text.clone(),
                    status: BatchJobStatus::Pending,
                    start_time: None,
                    end_time: None,
                    error: None,
                }
            })
            .collect();

        tracing::info!(
            target: "batch",
            total = jobs.len(),
            concurrency = self.config.concurrency,
            "Batch session started"
        );

        let executor = BatchJobExecutor::new(
            self.app_config,
            self.file_event_bus,
            self.tx_gui,
            self.prompt_text,
            self.cancel_flag.clone(),
        );
        let mut result = executor.execute_concurrent(jobs, self.config.concurrency);
        result.duration = start_time.elapsed();

        if self.cancel_flag.load(Ordering::SeqCst) {
            tracing::info!(
                target: "batch",
                completed = result.completed,
                total = result.total_jobs,
                "Batch session cancelled"
            );
        } else {
            tracing::info!(
                target: "batch",
                completed = result.completed,
                failed = result.failed,
                total = result.total_jobs,
                "Batch session ended"
            );
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;

    #[test]
    fn test_coordinator_new_and_execute() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, _rx) = mpsc::channel();
        let bus: Bus<FileEvent> = Bus::new();

        let config = BatchConfig {
            directory: dir.path().to_path_buf(),
            pattern: "*.md".to_string(),
            prompt_path: dir.path().join("prompt.md"),
            mode: BatchMode::File,
            concurrency: 4,
        };

        let app_config = AppConfig::default();
        let (coordinator, cancel_flag) =
            BatchCoordinator::new(config, app_config, tx, bus, "test prompt".to_string());

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
        let (tx, _rx) = mpsc::channel();
        let bus: Bus<FileEvent> = Bus::new();

        let config = BatchConfig {
            directory: dir.path().to_path_buf(),
            pattern: "*.md".to_string(),
            prompt_path: dir.path().join("prompt.md"),
            mode: BatchMode::File,
            concurrency: 4,
        };

        let app_config = AppConfig::default();
        let (coordinator, _cancel_flag) =
            BatchCoordinator::new(config, app_config, tx, bus, "test prompt".to_string());

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

        let (tx, _rx) = mpsc::channel();
        let bus: Bus<FileEvent> = Bus::new();

        let config = BatchConfig {
            directory: dir.path().to_path_buf(),
            pattern: "*.md".to_string(),
            prompt_path: dir.path().join("prompt.md"),
            mode: BatchMode::File,
            concurrency: 4,
        };

        let app_config = AppConfig::default();
        let (coordinator, _) =
            BatchCoordinator::new(config, app_config, tx, bus, "test prompt".to_string());

        let handle = coordinator.execute();
        let result = handle.join();
        assert!(result.total_jobs > 0);
        assert!(result.duration.as_secs() < 60);
    }
}
