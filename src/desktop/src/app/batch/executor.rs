//! Batch job executor — runs the LLM agent against each discovered unit (file or directory) with configurable concurrency.

use crate::app::batch::types::{BatchJob, BatchJobStatus, BatchResult};
use crate::bus::core::Bus;
use crate::bus::events::file::FileEvent;
use crate::bus::events::typed::BackgroundEvent;
use crate::config::AppConfig;
use crate::utils::clock::Clock;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use tokio::sync::Semaphore;

pub struct BatchJobExecutor {
    app_config: AppConfig,
    file_event_bus: Bus<FileEvent>,
    tx_gui: mpsc::Sender<BackgroundEvent>,
    cancel_flag: Arc<AtomicBool>,
    clock: Arc<dyn Clock>,
}

impl BatchJobExecutor {
    pub fn new(
        app_config: AppConfig,
        file_event_bus: Bus<FileEvent>,
        tx_gui: mpsc::Sender<BackgroundEvent>,
        _prompt: String,
        cancel_flag: Arc<AtomicBool>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            app_config,
            file_event_bus,
            tx_gui,
            cancel_flag,
            clock,
        }
    }

    pub fn execute_concurrent(&self, mut jobs: Vec<BatchJob>, concurrency: u8) -> BatchResult {
        let start_time = self.clock.now();
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
                duration: (self.clock.now() - start_time).to_std().unwrap_or_default(),
            };
        };

        // Round-robin across all models tied for the minimum cost for "chat".
        let min_cost_models: Vec<String> = self
            .app_config
            .models_for_use_case_min_cost("chat")
            .into_iter()
            .map(|(key, _)| key.clone())
            .collect();
        let model_count = min_cost_models.len();
        let rr_counter = Arc::new(AtomicUsize::new(0));

        let semaphore = Arc::new(Semaphore::new(concurrency as usize));
        let mut join_set = tokio::task::JoinSet::new();
        let cancel_flag = self.cancel_flag.clone();

        let mut completed: usize = 0;
        let mut failed: usize = 0;
        let mut cancelled: usize = 0;

        rt.block_on(async {
            for job in &mut jobs {
                if cancel_flag.load(Ordering::SeqCst) {
                    job.status = BatchJobStatus::Cancelled;
                    cancelled += 1;
                    continue;
                }

                let Ok(permit) = semaphore.clone().acquire_owned().await else {
                    job.status = BatchJobStatus::Failed;
                    failed += 1;
                    continue;
                };

                let job_id = job.id;
                let target_path = job.target_path.clone();
                let active_file = job.active_file.clone();
                let active_dir = job.active_dir.clone();
                let prompt_text = job.prompt_text.clone();
                let app_config = self.app_config.clone();
                let file_event_bus = self.file_event_bus.clone();
                let cancel_flag = cancel_flag.clone();

                // Assign model round-robin when multiple min-cost models exist.
                let model_name = if model_count > 1 {
                    let i = rr_counter.fetch_add(1, Ordering::Relaxed) % model_count;
                    Some(min_cost_models[i].clone())
                } else {
                    None
                };

                job.status = BatchJobStatus::Running;
                job.start_time = Some(self.clock.now());

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
                        file_event_bus,
                        model_name,
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
                        job.end_time = Some(self.clock.now());
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
                    let _ = self.tx_gui.send(BackgroundEvent::from(
                        crate::app::background::models::BackgroundLogEntry::new(
                            crate::app::background::models::LogCategory::Batch,
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
            duration: (self.clock.now() - start_time).to_std().unwrap_or_default(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn run_agent_blocking(
    config: AppConfig,
    active_file: Option<PathBuf>,
    active_dir: Option<PathBuf>,
    selected_files: std::collections::HashSet<PathBuf>,
    prompt: String,
    cancel_flag: Arc<AtomicBool>,
    history: Option<Vec<serde_json::Value>>,
    file_event_bus: Bus<FileEvent>,
    model_name: Option<String>,
) -> (BatchJobStatus, Option<String>) {
    use crate::agent::events::AgentEvent as SeamAgentEvent;
    use crate::agent::run_agent;

    let agent_event_bus = crate::bus::core::Bus::new();
    let reader = agent_event_bus.subscribe();

    let ctx = crate::agent::AgentContext {
        config,
        file_event_bus,
        agent_event_bus,
        active_file,
        active_dir,
        selected_files,
        prompt,
        cancel_flag,
        history,
        model_name,
        session_id: uuid::Uuid::new_v4(),
        browser_session: std::sync::Arc::new(crate::app::session::BrowserSession::new(
            &crate::config::AppConfig::default(),
        )),
        pdf_backing: std::sync::Arc::new(crate::app::session::PdfBackingTracker::new()),
        tool_manager: std::sync::Arc::new(std::sync::RwLock::new(
            crate::agent::tools::manager::ToolManager::new(),
        )),
        uuid_gen: std::sync::Arc::new(crate::utils::uuid::SystemUuidGenerator),
    };
    run_agent(ctx);

    let mut status = BatchJobStatus::Completed;
    let mut error = None;

    // Drain the agent event bus until SessionFinished or Failed is seen.
    // `BusReader::recv` spin-waits; the bus stays alive as long as the
    // `Bus` handle exists (it was moved into the context and dropped when
    // the agent thread finished, which disconnects the broadcast sender).
    while let Ok(ev) = reader.recv() {
        match ev {
            SeamAgentEvent::SessionFinished { .. } => break,
            SeamAgentEvent::Failed { error: err, .. } => {
                status = BatchJobStatus::Failed;
                error = Some(err);
            }
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
            Arc::new(crate::utils::clock::SystemClock),
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
            Arc::new(crate::utils::clock::SystemClock),
        );

        let jobs = vec![BatchJob {
            id: 0,
            target_path: PathBuf::from("/tmp/test1.md"),
            active_file: Some(PathBuf::from("/tmp/test1.md")),
            active_dir: None,
            prompt_text: "test".to_string(),
            status: BatchJobStatus::Pending,
            start_time: None,
            end_time: None,
            error: None,
        }];

        let result = executor.execute_concurrent(jobs, 4);
        assert_eq!(result.total_jobs, 1);
        assert_eq!(result.cancelled, 1);
    }
}
