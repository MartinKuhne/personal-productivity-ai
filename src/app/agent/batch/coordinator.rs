//! Batch coordinator — discovers targets, spawns the executor on a background thread, polls progress, and reports results.
//!
//! Unit tests live in the sibling `coordinator_tests.rs` sidecar.

use super::discoverer::Discoverer;
use super::executor::BatchJobExecutor;
use super::types::{BatchConfig, BatchHandle, BatchJob, BatchJobStatus, BatchMode, BatchResult};
use crate::bus::core::Bus;
use crate::bus::events::file::FileChanged;
use crate::config::AppConfig;
use crate::utils::clock::Clock;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

pub struct BatchCoordinator {
    config: BatchConfig,
    app_config: AppConfig,
    tx_gui: crate::bus::events::typed::BackgroundEventSender,
    file_event_bus: Bus<FileChanged>,
    prompt_text: String,
    cancel_flag: Arc<AtomicBool>,
    clock: Arc<dyn Clock>,
}

impl BatchCoordinator {
    pub fn new(
        config: BatchConfig,
        app_config: AppConfig,
        tx_gui: crate::bus::events::typed::BackgroundEventSender,
        file_event_bus: Bus<FileChanged>,
        prompt_text: String,
        clock: Arc<dyn Clock>,
    ) -> (Self, Arc<AtomicBool>) {
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let coordinator = Self {
            config,
            app_config,
            tx_gui,
            file_event_bus,
            prompt_text,
            cancel_flag: cancel_flag.clone(),
            clock,
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
        let start_time = self.clock.now();
        let discoverer = Discoverer::from_config(&self.config);
        let targets = match discoverer.discover() {
            Ok(t) => t,
            Err(e) => {
                tracing::error!(target: "batch", error = %e, "Discovery failed");
                return BatchResult {
                    total_jobs: 0,
                    completed: 0,
                    failed: 0,
                    cancelled: 0,
                    duration: (self.clock.now() - start_time).to_std().unwrap_or_default(),
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
                duration: (self.clock.now() - start_time).to_std().unwrap_or_default(),
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
            std::sync::Arc::new(crate::agent::session::bus_observer::AppFileObserver::new(
                self.file_event_bus.clone(),
            )),
            self.tx_gui,
            self.prompt_text,
            self.cancel_flag.clone(),
            self.clock.clone(),
        );
        let mut result = executor.execute_concurrent(jobs, self.config.concurrency);
        result.duration = (self.clock.now() - start_time).to_std().unwrap_or_default();

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
#[path = "coordinator_tests.rs"]
mod tests;
