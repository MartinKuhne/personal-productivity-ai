use crate::batch::types::{
    BatchConfig, BatchHandle, BatchJob, BatchJobStatus, BatchMode, BatchResult,
};
use crate::config::AppConfig;
use crate::file_events::Bus;
use crate::file_events::FileEvent;
use crate::messages::BackgroundMessage;
use chrono::Local;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Instant;
use tokio::sync::Semaphore;

/// Coordinator for batch processing execution.
/// Manages concurrency, cancellation, and logging.
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

    /// Executes the batch processing session.
    /// Returns a handle to await completion.
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
        let mut jobs = Vec::new();

        // Discover targets based on mode
        let targets = match self.config.mode {
            BatchMode::File => {
                match crate::batch::file_matcher::find_matching_files(
                    &self.config.directory,
                    &self.config.pattern,
                ) {
                    Ok(files) => files,
                    Err(e) => {
                        self.log_error(&format!("File matching failed: {}", e));
                        return BatchResult {
                            total_jobs: 0,
                            completed: 0,
                            failed: 0,
                            cancelled: 0,
                            duration: start_time.elapsed(),
                        };
                    }
                }
            }
            BatchMode::Directory => {
                crate::batch::file_matcher::find_subdirectories(&self.config.directory)
            }
        };

        // Create jobs
        for (idx, target_path) in targets.into_iter().enumerate() {
            let (active_file, active_dir) = match self.config.mode {
                BatchMode::File => (Some(target_path.clone()), None),
                BatchMode::Directory => (None, Some(target_path.clone())),
            };

            jobs.push(BatchJob {
                id: idx,
                target_path,
                active_file,
                active_dir,
                prompt_text: self.prompt_text.clone(),
                status: BatchJobStatus::Pending,
                start_time: None,
                end_time: None,
                error: None,
            });
        }

        let total_jobs = jobs.len();

        // Log session start
        self.log_session_start(total_jobs);

        if total_jobs == 0 {
            self.log_session_end(0, 0, 0);
            return BatchResult {
                total_jobs: 0,
                completed: 0,
                failed: 0,
                cancelled: 0,
                duration: start_time.elapsed(),
            };
        }

        // Run jobs with concurrency control using tokio runtime
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to create tokio runtime for batch processing");

        let semaphore = Arc::new(Semaphore::new(self.config.concurrency as usize));
        let mut join_set = tokio::task::JoinSet::new();

        // Use indices to avoid moving jobs
        let cancel_flag = self.cancel_flag.clone();

        let mut completed: usize = 0;
        let mut failed: usize = 0;
        let mut cancelled: usize = 0;

        rt.block_on(async {
            for idx in 0..jobs.len() {
                if cancel_flag.load(Ordering::SeqCst) {
                    jobs[idx].status = BatchJobStatus::Cancelled;
                    continue;
                }

                let permit = semaphore
                    .clone()
                    .acquire_owned()
                    .await
                    .expect("batch semaphore should not be closed");
                let job_id = jobs[idx].id;
                let target_path = jobs[idx].target_path.clone();
                let active_file = jobs[idx].active_file.clone();
                let active_dir = jobs[idx].active_dir.clone();
                let prompt_text = jobs[idx].prompt_text.clone();
                let app_config = self.app_config.clone();
                let file_event_bus = self.file_event_bus.clone();
                let cancel_flag = cancel_flag.clone();

                // Mark as running
                jobs[idx].status = BatchJobStatus::Running;
                jobs[idx].start_time = Some(Local::now());

                // Log job start
                self.log_job_start(job_id, &target_path);

                join_set.spawn(async move {
                    // Check cancellation before running agent
                    if cancel_flag.load(Ordering::SeqCst) {
                        drop(permit);
                        return (job_id, BatchJobStatus::Cancelled, None);
                    }

                    // Run the agent (blocking call inside async)
                    let result = run_agent_blocking(
                        app_config,
                        active_file,
                        active_dir,
                        std::collections::HashSet::new(),
                        prompt_text,
                        cancel_flag,
                        None,
                        String::new(),
                        file_event_bus,
                    );

                    drop(permit);
                    (job_id, result.0, result.1)
                });
            }
        });

        // Collect results
        while let Some(res) = rt.block_on(join_set.join_next()) {
            match res {
                Ok((job_id, status, error)) => {
                    // Update job status
                    if let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) {
                        job.status = status;
                        job.end_time = Some(Local::now());
                        if let Some(ref err) = error {
                            job.error = Some(err.clone());
                        }
                    }

                    match status {
                        BatchJobStatus::Failed => {
                            if let Some(job) = jobs.iter().find(|j| j.id == job_id) {
                                self.log_job_error(
                                    job_id,
                                    &job.target_path,
                                    error.unwrap_or_default(),
                                );
                            }
                            failed += 1;
                        }
                        BatchJobStatus::Cancelled => {
                            cancelled += 1;
                        }
                        _ => {
                            self.log_job_end(
                                job_id,
                                &jobs
                                    .iter()
                                    .find(|j| j.id == job_id)
                                    .map(|j| j.target_path.clone())
                                    .unwrap_or_default(),
                            );
                            completed += 1;
                        }
                    }
                }
                Err(_) => {
                    // Task panicked — log and count as failed
                    failed += 1;
                    let _ = self.tx_gui.send(BackgroundMessage::LogEntry(
                        crate::background::models::BackgroundLogEntry::new(
                            crate::background::models::LogCategory::Batch,
                            "A batch job panicked and was terminated".to_string(),
                        ),
                    ));
                }
            }
        }

        // Log session end
        if self.cancel_flag.load(Ordering::SeqCst) {
            self.log_session_cancelled(completed, total_jobs);
        } else {
            self.log_session_end(completed, failed, total_jobs);
        }

        BatchResult {
            total_jobs,
            completed,
            failed,
            cancelled,
            duration: start_time.elapsed(),
        }
    }

    fn log_session_start(&self, total_jobs: usize) {
        let msg = format!(
            "Batch session started: {} mode, {} jobs, concurrency {}",
            match self.config.mode {
                BatchMode::File => "File",
                BatchMode::Directory => "Directory",
            },
            total_jobs,
            self.config.concurrency
        );
        let entry = crate::background::models::BackgroundLogEntry::new(
            crate::background::models::LogCategory::Batch,
            msg,
        );
        let _ = self.tx_gui.send(BackgroundMessage::LogEntry(entry));
    }

    fn log_job_start(&self, job_id: usize, target_path: &PathBuf) {
        let msg = format!("Starting batch job {}: {}", job_id, target_path.display());
        let entry = crate::background::models::BackgroundLogEntry::new(
            crate::background::models::LogCategory::Batch,
            msg,
        );
        let _ = self.tx_gui.send(BackgroundMessage::LogEntry(entry));
    }

    fn log_job_end(&self, job_id: usize, target_path: &PathBuf) {
        let msg = format!("Completed batch job {}: {}", job_id, target_path.display());
        let entry = crate::background::models::BackgroundLogEntry::new(
            crate::background::models::LogCategory::Batch,
            msg,
        );
        let _ = self.tx_gui.send(BackgroundMessage::LogEntry(entry));
    }

    fn log_job_error(&self, job_id: usize, target_path: &PathBuf, error: String) {
        let msg = format!(
            "Error in batch job {}: {}: {}",
            job_id,
            target_path.display(),
            error
        );
        let entry = crate::background::models::BackgroundLogEntry::new(
            crate::background::models::LogCategory::Batch,
            msg,
        );
        let _ = self.tx_gui.send(BackgroundMessage::LogEntry(entry));
    }

    fn log_error(&self, msg: &str) {
        let entry = crate::background::models::BackgroundLogEntry::new(
            crate::background::models::LogCategory::Batch,
            msg.to_string(),
        );
        let _ = self.tx_gui.send(BackgroundMessage::LogEntry(entry));
    }

    fn log_session_end(&self, completed: usize, failed: usize, total: usize) {
        let msg = format!(
            "Batch session completed: {}/{} jobs ({} failed)",
            completed, total, failed
        );
        let entry = crate::background::models::BackgroundLogEntry::new(
            crate::background::models::LogCategory::Batch,
            msg,
        );
        let _ = self.tx_gui.send(BackgroundMessage::LogEntry(entry));
    }

    fn log_session_cancelled(&self, completed: usize, total: usize) {
        let msg = format!("Batch session cancelled: {}/{} jobs done", completed, total);
        let entry = crate::background::models::BackgroundLogEntry::new(
            crate::background::models::LogCategory::Batch,
            msg,
        );
        let _ = self.tx_gui.send(BackgroundMessage::LogEntry(entry));
    }
}

/// Blocking version of run_agent for use in batch coordinator
/// Returns (status, error_message)
pub fn run_agent_blocking(
    config: AppConfig,
    active_file: Option<PathBuf>,
    active_dir: Option<PathBuf>,
    selected_files: std::collections::HashSet<PathBuf>,
    prompt: String,
    cancel_flag: Arc<AtomicBool>,
    history: Option<Vec<serde_json::Value>>,
    current_response: String,
    file_event_bus: Bus<FileEvent>,
) -> (BatchJobStatus, Option<String>) {
    use crate::agent::run_agent;
    use std::sync::mpsc::channel;

    let (tx, rx) = channel();

    run_agent(
        config,
        tx,
        active_file,
        active_dir,
        selected_files,
        prompt,
        cancel_flag,
        history,
        current_response,
        file_event_bus,
    );

    // Wait for agent to complete
    let mut status = BatchJobStatus::Completed;
    let mut error = None;

    while let Ok(msg) = rx.recv() {
        match msg {
            BackgroundMessage::AgentFailed(err) => {
                status = BatchJobStatus::Failed;
                error = Some(err);
            }
            BackgroundMessage::AgentFinished(_) => {}
            _ => {}
        }
    }

    (status, error)
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
        // Jobs will be created but agent calls will fail (no API key etc.)
        // We just verify the process ran and produced a result
        assert!(result.total_jobs > 0);
        // At least the session ran through completion
        assert!(result.duration.as_secs() < 60);
    }
}
