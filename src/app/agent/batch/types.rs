//! Batch-processing data types - `BatchMode`, `BatchConfig`, `BatchJob`, `BatchResult`, `BatchHandle`, and validation logic.
//!
//! Unit tests live in the sibling `types_tests.rs` sidecar.

use chrono::{DateTime, Local};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread;
use std::time::Duration;

/// Batch processing mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BatchMode {
    /// Process each matching file individually
    File,
    /// Process each immediate subdirectory as a unit
    Directory,
}

/// User configuration for a batch processing session
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct BatchConfig {
    /// Root directory to process (from content libraries)
    pub directory: PathBuf,
    /// Glob pattern for file matching (File mode only)
    pub pattern: String,
    /// Path to prompt markdown file
    pub prompt_path: PathBuf,
    /// Processing mode
    pub mode: BatchMode,
    /// Number of concurrent LLM calls (1-8)
    pub concurrency: u8,
}

impl BatchConfig {
    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if !self.directory.exists() {
            return Err("Directory does not exist".to_string());
        }
        if !self.prompt_path.exists() {
            return Err("Prompt file does not exist".to_string());
        }
        validate_batch_params(self.mode, &self.pattern, self.concurrency)
    }
}

/// Validate parameters shared by `BatchConfig` and the dialog UI.
pub(crate) fn validate_batch_params(
    mode: BatchMode,
    pattern: &str,
    concurrency: u8,
) -> Result<(), String> {
    if !(1..=8).contains(&concurrency) {
        return Err("Concurrency must be 1-8".to_string());
    }
    if mode == BatchMode::File {
        if pattern.trim().is_empty() {
            return Err("Pattern is required for File mode".to_string());
        }
        if let Err(e) = glob::Pattern::new(pattern) {
            return Err(format!("Invalid glob pattern: {}", e));
        }
    }
    Ok(())
}

/// Discovered prompt file available for selection
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptInfo {
    /// Full path to prompt markdown file
    pub path: PathBuf,
    /// User-friendly display name (library / relative path)
    pub display_name: String,
    /// Source content library name
    pub library_name: String,
    /// Prompt text content (body only, no front matter)
    pub content: String,
}

/// A single unit of work in a batch session
#[derive(Debug, Clone)]
pub struct BatchJob {
    /// Sequential job index (0-based)
    pub id: usize,
    /// File path (File mode) or directory path (Directory mode)
    pub target_path: PathBuf,
    /// Passed to run_agent as active_file (File mode)
    pub active_file: Option<PathBuf>,
    /// Passed to run_agent as active_dir (Directory mode)
    pub active_dir: Option<PathBuf>,
    /// The prompt content to execute
    pub prompt_text: String,
    /// Current execution status
    pub status: BatchJobStatus,
    /// When job started
    pub start_time: Option<DateTime<Local>>,
    /// When job completed
    pub end_time: Option<DateTime<Local>>,
    /// Error message if failed
    pub error: Option<String>,
}

/// Job execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BatchJobStatus {
    /// Queued, waiting for semaphore
    Pending,
    /// Permit acquired, agent executing
    Running,
    /// Agent finished successfully
    Completed,
    /// Agent returned error
    Failed,
    /// Cancelled before starting
    Cancelled,
}

/// Aggregates all jobs for a batch execution
#[derive(Debug, Clone)]
pub struct BatchSession {
    /// User configuration
    pub config: BatchConfig,
    /// All jobs in this session
    pub jobs: Vec<BatchJob>,
    /// Shared cancellation signal
    pub cancel_flag: Arc<AtomicBool>,
    /// Session start timestamp
    pub start_time: DateTime<Local>,
}

impl BatchSession {
    /// Total number of jobs
    pub fn total_jobs(&self) -> usize {
        self.jobs.len()
    }

    /// Number of completed (success) jobs
    pub fn completed_jobs(&self) -> usize {
        self.jobs
            .iter()
            .filter(|j| j.status == BatchJobStatus::Completed)
            .count()
    }

    /// Number of failed jobs
    pub fn failed_jobs(&self) -> usize {
        self.jobs
            .iter()
            .filter(|j| j.status == BatchJobStatus::Failed)
            .count()
    }

    /// Number of cancelled jobs
    pub fn cancelled_jobs(&self) -> usize {
        self.jobs
            .iter()
            .filter(|j| j.status == BatchJobStatus::Cancelled)
            .count()
    }

    /// Number of currently running jobs
    pub fn running_jobs(&self) -> usize {
        self.jobs
            .iter()
            .filter(|j| j.status == BatchJobStatus::Running)
            .count()
    }

    /// Overall progress (0.0 to 1.0)
    pub fn progress(&self) -> f32 {
        if self.total_jobs() == 0 {
            return 1.0;
        }
        (self.completed_jobs() + self.failed_jobs() + self.cancelled_jobs()) as f32
            / self.total_jobs() as f32
    }
}

/// Result of batch session completion
#[derive(Debug, Clone)]
pub struct BatchResult {
    pub total_jobs: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub duration: Duration,
}

/// Handle to a running batch session
pub struct BatchHandle {
    /// Thread join handle
    pub thread: thread::JoinHandle<BatchResult>,
    /// Shared cancellation flag
    pub cancel_flag: Arc<AtomicBool>,
}

impl BatchHandle {
    /// Request cancellation
    pub fn cancel(&self) {
        self.cancel_flag
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Wait for completion and get result
    pub fn join(self) -> BatchResult {
        match self.thread.join() {
            Ok(result) => result,
            Err(_) => {
                tracing::error!(target: "batch", "Batch thread panicked");
                BatchResult {
                    total_jobs: 0,
                    completed: 0,
                    failed: 0,
                    cancelled: 0,
                    duration: Duration::ZERO,
                }
            }
        }
    }
}

/// Dialog configuration state
#[derive(Debug, Clone)]
pub struct BatchDialogConfig {
    /// Candidate batch roots, one per content library.
    ///
    /// Drift correction ([RUST-041]): this doc used to claim "all
    /// discovered directories, not just top-level libraries". That never
    /// matched the implementation — every population site
    /// (`orchestrator.rs`'s config drain, `ui/app/init.rs`,
    /// `ui_iced/app.rs`'s state builder and `ui_iced/update.rs`'s
    /// config-arrival projection) supplies exactly one entry per content
    /// library, taken from `ContentLibrary::root_folder`. Nothing here
    /// walks the workspace for nested directories.
    pub available_dirs: Vec<PathBuf>,
    /// Virtual label for each entry in [`Self::available_dirs`], shown to the
    /// user in place of the real filesystem path.
    ///
    /// Index-aligned: `available_dir_labels[i]` labels `available_dirs[i]`.
    /// A missing or empty entry means "no virtual name known" and the caller
    /// falls back to the real path.
    ///
    /// The label is the content library's `name`, i.e. the virtual path the
    /// VFS addresses that library by (`ContentLibraryExt::display_label_for`
    /// maps a library's own root to exactly that). The real path stays in
    /// [`Self::available_dirs`] so the batch runner can still glob the
    /// filesystem.
    pub available_dir_labels: Vec<String>,
    /// Discovered prompt files
    pub available_prompts: Vec<PromptInfo>,
    /// Currently selected directory index
    pub selected_dir_idx: Option<usize>,
    /// Wildcard pattern for file matching
    pub pattern: String,
    /// Currently selected prompt index
    pub selected_prompt_idx: Option<usize>,
    /// Current batch mode
    pub mode: BatchMode,
    /// Concurrency level (1-8)
    pub concurrency: u8,
}

impl Default for BatchDialogConfig {
    fn default() -> Self {
        Self {
            available_dirs: Vec::new(),
            available_dir_labels: Vec::new(),
            available_prompts: Vec::new(),
            selected_dir_idx: None,
            pattern: "*.md".to_string(),
            selected_prompt_idx: None,
            mode: BatchMode::File,
            concurrency: 4,
        }
    }
}

/// Log phase for batch logging
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BatchLogPhase {
    SessionStart,
    JobStart,
    JobEnd,
    JobError,
    SessionEnd,
    SessionCancelled,
}

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
