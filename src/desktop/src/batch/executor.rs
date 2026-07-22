use crate::batch::types::{BatchJob, BatchJobStatus, BatchResult};
use crate::config::AppConfig;
use crate::file_events::Bus;
use crate::file_events::FileEvent;
use crate::messages::BackgroundMessage;
use chrono::Local;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Instant;
use tokio::sync::Semaphore;

pub struct BatchJobExecutor {
    app_config: AppConfig,
    file_event_bus: Bus<FileEvent>,
    tx_gui: mpsc::Sender<BackgroundMessage>,
    cancel_flag: Arc<AtomicBool>,
}

impl BatchJobExecutor {
    pub fn new(
        app_config: AppConfig,
        file_event_bus: Bus<FileEvent>,
        tx_gui: mpsc::Sender<BackgroundMessage>,
        _prompt: String,
        cancel_flag: Arc<AtomicBool>,
    ) -> Self {
        Self {
            app_config,
            file_event_bus,
            tx_gui,
            cancel_flag,
        }
    }

    pub fn execute_concurrent(
        &self,
        mut jobs: Vec<BatchJob>,
        concurrency: u8,
    ) -> BatchResult {
        let start_time = Instant::now();
        let total_jobs = jobs.len();

        let Ok(rt) = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        else {
            tracing::error!(target: "batch", "Failed to create tokio runtime for batch processing");
            return BatchResult {
                total_jobs,
                completed: 0,
                failed: 0,
                cancelled: 0,
                duration: start_time.elapsed(),
            };
        };

        let semaphore = Arc::new(Semaphore::new(concurrency as usize));
        let mut join_set = tokio::task::JoinSet::new();
        let cancel_flag = self.cancel_flag.clone();

        let mut completed: usize = 0;
        let mut failed: usize = 0;
        let mut cancelled: usize = 0;

        rt.block_on(async {
            for idx in 0..jobs.len() {
                if cancel_flag.load(Ordering::SeqCst) {
                    jobs[idx].status = BatchJobStatus::Cancelled;
                    cancelled += 1;
                    continue;
                }

                let Ok(permit) = semaphore.clone().acquire_owned().await else {
                    jobs[idx].status = BatchJobStatus::Failed;
                    failed += 1;
                    continue;
                };

                let job_id = jobs[idx].id;
                let target_path = jobs[idx].target_path.clone();
                let active_file = jobs[idx].active_file.clone();
                let active_dir = jobs[idx].active_dir.clone();
                let prompt_text = jobs[idx].prompt_text.clone();
                let app_config = self.app_config.clone();
                let file_event_bus = self.file_event_bus.clone();
                let cancel_flag = cancel_flag.clone();

                jobs[idx].status = BatchJobStatus::Running;
                jobs[idx].start_time = Some(Local::now());

                tracing::info!(target: "batch", job_id, path = ?target_path, "Starting batch job");

                join_set.spawn(async move {
                    if cancel_flag.load(Ordering::SeqCst) {
                        drop(permit);
                        return (job_id, target_path, BatchJobStatus::Cancelled, None);
                    }

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
                    (job_id, target_path, result.0, result.1)
                });
            }
        });

        while let Some(res) = rt.block_on(join_set.join_next()) {
            match res {
                Ok((job_id, target_path, status, error)) => {
                    if let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) {
                        job.status = status;
                        job.end_time = Some(Local::now());
                        if let Some(ref err) = error {
                            job.error = Some(err.clone());
                        }
                    }

                    match status {
                        BatchJobStatus::Failed => {
                            tracing::warn!(target: "batch", job_id, path = ?target_path, error = %error.unwrap_or_default(), "Batch job failed");
                            failed += 1;
                        }
                        BatchJobStatus::Cancelled => {
                            cancelled += 1;
                        }
                        _ => {
                            tracing::info!(target: "batch", job_id, path = ?target_path, "Completed batch job");
                            completed += 1;
                        }
                    }
                }
                Err(_) => {
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

        BatchResult {
            total_jobs,
            completed,
            failed,
            cancelled,
            duration: start_time.elapsed(),
        }
    }
}

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
        crate::agent::AgentContext::new(
            config,
            tx,
            file_event_bus,
            active_file,
            active_dir,
            selected_files,
            prompt,
            cancel_flag,
            history,
            current_response,
        ),
    );

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
    use std::sync::atomic::AtomicBool;

    #[test]
    fn test_execute_concurrent_empty() {
        let (tx, _rx) = mpsc::channel();
        let bus: Bus<FileEvent> = Bus::new();
        let cancel_flag = Arc::new(AtomicBool::new(false));

        let executor = BatchJobExecutor::new(
            AppConfig::default(),
            bus,
            tx,
            "test prompt".to_string(),
            cancel_flag,
        );

        let result = executor.execute_concurrent(vec![], 4);
        assert_eq!(result.total_jobs, 0);
        assert_eq!(result.completed, 0);
        assert_eq!(result.failed, 0);
        assert_eq!(result.cancelled, 0);
    }

    #[test]
    fn test_execute_concurrent_cancellation() {
        let (tx, _rx) = mpsc::channel();
        let bus: Bus<FileEvent> = Bus::new();
        let cancel_flag = Arc::new(AtomicBool::new(true));

        let executor = BatchJobExecutor::new(
            AppConfig::default(),
            bus,
            tx,
            "test prompt".to_string(),
            cancel_flag,
        );

        let jobs = vec![
            BatchJob {
                id: 0,
                target_path: PathBuf::from("/tmp/test1.md"),
                active_file: Some(PathBuf::from("/tmp/test1.md")),
                active_dir: None,
                prompt_text: "test".to_string(),
                status: BatchJobStatus::Pending,
                start_time: None,
                end_time: None,
                error: None,
            },
        ];

        let result = executor.execute_concurrent(jobs, 4);
        assert_eq!(result.total_jobs, 1);
        assert_eq!(result.cancelled, 1);
    }
}
