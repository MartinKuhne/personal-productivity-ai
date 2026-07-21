use chrono::{DateTime, Local};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
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
    if concurrency < 1 || concurrency > 8 {
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
#[derive(Debug, Clone)]
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
                eprintln!("Batch thread panicked");
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
    /// Available directories from content libraries
    pub available_dirs: Vec<PathBuf>,
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
            available_prompts: Vec::new(),
            selected_dir_idx: None,
            pattern: "*.md".to_string(),
            selected_prompt_idx: None,
            mode: BatchMode::File,
            concurrency: 4,
        }
    }
}

/// Result of dialog interaction
#[derive(Debug, Clone, PartialEq)]
pub enum BatchDialogResult {
    /// User clicked Process with valid config
    Process(BatchConfig),
    /// User clicked Cancel or closed dialog
    Cancel,
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
mod tests {
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
}
