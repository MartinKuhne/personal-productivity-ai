# Contracts: Batch Prompt Processing

**Feature**: Batch Prompt Processing  
**Date**: 2026-07-20  
**Type**: Internal Rust API Contracts

---

## Overview

This feature adds batch processing capability to the existing FastMD desktop application. The contracts define the internal Rust interfaces between the UI layer (dialog), the batch orchestration logic, and the existing agent/background systems.

---

## Contract 1: Batch Dialog UI

**File**: `src/desktop/src/ui/batch_dialog.rs` (new)

### Public Interface

```rust
/// Configuration for the batch processing dialog.
#[derive(Debug, Clone)]
pub struct BatchDialogConfig {
    /// Available directories from content libraries
    pub available_dirs: Vec<PathBuf>,
    /// Discovered prompt files
    pub available_prompts: Vec<PromptInfo>,
    /// Currently selected directory (index into available_dirs)
    pub selected_dir_idx: Option<usize>,
    /// Wildcard pattern for file matching
    pub pattern: String,
    /// Currently selected prompt (index into available_prompts)
    pub selected_prompt_idx: Option<usize>,
    /// Current batch mode
    pub mode: BatchMode,
    /// Concurrency level (1-8)
    pub concurrency: u8,
}

/// Result of dialog interaction.
#[derive(Debug, Clone, PartialEq)]
pub enum BatchDialogResult {
    /// User clicked Process with valid config
    Process(BatchConfig),
    /// User clicked Cancel or closed dialog
    Cancel,
}

/// Prompt file information for selection dropdown.
#[derive(Debug, Clone)]
pub struct PromptInfo {
    pub path: PathBuf,
    pub display_name: String,  // e.g., "My Library / prompts/summarize.md"
}

/// Shows the batch prompt processing dialog.
/// Returns `Some(result)` when dialog closes, `None` if still open.
pub fn show_batch_dialog(
    app: &mut FastMdApp,
    ctx: &egui::Context,
    config: &mut BatchDialogConfig,
) -> Option<BatchDialogResult>;
```

### Behavior Contract

| Event | Expected Behavior |
|-------|-------------------|
| Dialog opens | All fields populated from `config`; Process enabled only if valid |
| Directory changed | `selected_dir_idx` updated; validation re-run |
| Pattern changed | `pattern` updated; validation re-run (File mode only) |
| Prompt changed | `selected_prompt_idx` updated; validation re-run |
| Mode toggled | `mode` updated; pattern field shown/hidden within 100ms |
| Concurrency changed | `concurrency` clamped to 1-8 |
| Process clicked (valid) | Returns `Some(BatchDialogResult::Process(config))` |
| Cancel clicked | Returns `Some(BatchDialogResult::Cancel)` |
| Window close (X) | Same as Cancel |

### Validation Rules

Process button enabled **iff**:
- `selected_dir_idx.is_some()`
- `selected_prompt_idx.is_some()`
- If `mode == File`: `!pattern.trim().is_empty()` and valid glob
- `concurrency >= 1 && concurrency <= 8`

---

## Contract 2: Batch Orchestration

**File**: `src/desktop/src/batch.rs` (new)

### Public Interface

```rust
use std::sync::{Arc, atomic::AtomicBool};
use std::path::PathBuf;
use crate::background::SharedProcessManager;
use crate::file_events::Bus;
use crate::messages::BackgroundMessage;

/// Complete configuration for a batch session.
#[derive(Debug, Clone)]
pub struct BatchConfig {
    pub directory: PathBuf,
    pub pattern: String,
    pub prompt_path: PathBuf,
    pub mode: BatchMode,
    pub concurrency: u8,
}

/// Executes a batch processing session.
/// 
/// Spawns a background thread that:
/// 1. Discovers targets (files or directories)
/// 2. Creates job queue
/// 3. Processes up to `concurrency` jobs in parallel using `run_agent`
/// 4. Logs start/end to `process_manager`
/// 5. Respects `cancel_flag` for graceful shutdown
/// 6. Sends `BackgroundMessage::LogEntry` for UI log window
///
/// Returns a handle to await completion or cancel.
pub fn execute_batch(
    config: BatchConfig,
    app_config: crate::config::AppConfig,
    process_manager: SharedProcessManager,
    file_event_bus: Bus<crate::file_events::FileEvent>,
    tx_gui: std::sync::mpsc::Sender<BackgroundMessage>,
    cancel_flag: Arc<AtomicBool>,
) -> BatchHandle;

/// Handle to a running batch session.
pub struct BatchHandle {
    /// Join handle for the batch thread
    pub thread: std::thread::JoinHandle<BatchResult>,
    /// Shared cancel flag (set true to request cancellation)
    pub cancel_flag: Arc<AtomicBool>,
}

/// Result of batch session completion.
#[derive(Debug)]
pub struct BatchResult {
    pub total_jobs: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub duration: std::time::Duration,
}
```

### Behavior Contract

| Requirement | Implementation |
|-------------|----------------|
| Target discovery | File mode: `walkdir` with glob match; Directory mode: `read_dir` immediate children |
| Job creation | One `BatchJob` per target with `active_file` or `active_dir` set |
| Concurrency | `tokio::sync::Semaphore` with `concurrency` permits |
| Agent execution | Call `crate::agent::run_agent` per job with appropriate context |
| Logging | `process_manager.push_log()` for each JobStart/JobEnd with timestamps |
| Cancellation | Set `cancel_flag=true`; `run_agent` checks flag each loop iteration |
| Graceful shutdown | Wait for in-flight jobs (semaphore permits released) before exiting |
| Error handling | Job failure logged, session continues with remaining jobs |

### Job Context Mapping

| Mode | `run_agent` Parameters |
|------|------------------------|
| File | `active_file=Some(target_path)`, `active_dir=None` |
| Directory | `active_file=None`, `active_dir=Some(target_path)` |

Both: `selected_files=empty`, `prompt=prompt_content`, `history=None`, `current_response=""`

---

## Contract 3: Prompt Discovery

**File**: `src/desktop/src/batch.rs` (or `utils/prompts.rs`)

### Public Interface

```rust
use std::path::PathBuf;

/// Discovers all prompt files in content libraries.
/// A prompt file is a .md/.markdown file with `tags: [prompt]` in front matter.
pub fn discover_prompts(config: &crate::config::AppConfig) -> Vec<PromptInfo>;

/// Reads prompt content from file (body only, no front matter).
pub fn read_prompt_content(path: &PathBuf) -> Result<String, std::io::Error>;
```

### Behavior Contract

| Requirement | Implementation |
|-------------|----------------|
| Discovery | Walk each content library root, filter `.md`/`.markdown`, parse front matter for `tags` containing `prompt` |
| Display name | Format: `"{library_name} / {relative_path}"` |
| Content reading | Strip YAML front matter (`---`...`---`), return remainder |
| Caching | Not required; re-discover on each dialog open |

---

## Contract 4: Background Log Integration

**File**: Uses existing `BackgroundProcessManager` and `LogCategory`

### Extension to LogCategory

```rust
// In src/desktop/src/background/models.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum LogCategory {
    // ... existing variants ...
    Batch,  // NEW: for batch processing logs
}
```

### Log Message Format

| Phase | Message Template |
|-------|------------------|
| SessionStart | `Batch session started: {mode} mode, {count} jobs, concurrency {N}` |
| JobStart | `Starting batch job {idx}: {target_path}` |
| JobEnd | `Completed batch job {idx}: {target_path}` |
| JobError | `Error in batch job {idx}: {target_path}: {error}` |
| SessionEnd | `Batch session completed: {success}/{total} jobs` |
| SessionCancelled | `Batch session cancelled: {completed}/{total} jobs done, {in_flight} in flight` |

---

## Contract 5: App State Integration

**File**: `src/desktop/src/ui/app.rs` (modifications to `FastMdApp`)

### New Fields

```rust
pub struct FastMdApp {
    // ... existing fields ...
    
    // Batch dialog state
    pub batch_dialog_open: bool,
    pub batch_dialog_config: BatchDialogConfig,
    // Active batch session (for cancel during processing)
    pub batch_cancel_flag: Option<Arc<AtomicBool>>,
    pub batch_handle: Option<BatchHandle>,
}
```

### Event Loop Integration

In `FastMdApp::update()` or main loop:
- Call `show_batch_dialog()` when `batch_dialog_open`
- On `BatchDialogResult::Process(config)`:
  - Create `cancel_flag = Arc::new(AtomicBool::new(false))`
  - Call `execute_batch()` with config, store handle
  - Set `batch_cancel_flag = Some(cancel_flag)`
- On `BatchDialogResult::Cancel` or dialog close:
  - Set `batch_dialog_open = false`
  - If batch running: set cancel flag, await handle (or detach)
- Poll `batch_handle` for completion; on done, clear handle, re-enable UI

---

## Error Handling Contract

All functions return `Result<T, BatchError>` where appropriate:

```rust
#[derive(Debug, thiserror::Error)]
pub enum BatchError {
    #[error("No content libraries configured")]
    NoLibraries,
    #[error("Directory not in content libraries: {0}")]
    InvalidDirectory(PathBuf),
    #[error("Invalid glob pattern: {0}")]
    InvalidPattern(String),
    #[error("No prompts found")]
    NoPrompts,
    #[error("Prompt not found: {0}")]
    PromptNotFound(PathBuf),
    #[error("Concurrency must be 1-8, got {0}")]
    InvalidConcurrency(u8),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Agent execution failed: {0}")]
    AgentFailed(String),
}
```

---

## Testing Contracts

Each contract must have unit tests verifying:

1. **Dialog**: Validation logic, mode switching, field visibility
2. **Orchestration**: Target discovery, job creation, concurrency limiting, cancellation
3. **Prompt Discovery**: Front matter parsing, tag detection, content extraction
4. **Logging**: Correct log entries emitted at each phase
5. **Integration**: End-to-end batch with mock agent

See `quickstart.md` for manual validation scenarios.