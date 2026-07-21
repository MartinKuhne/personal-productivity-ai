# Data Model: Batch Prompt Processing

**Feature**: Batch Prompt Processing  
**Date**: 2026-07-20  
**Status**: Draft

---

## Entities

### BatchConfig
User configuration for a batch processing session.

| Field | Type | Description | Validation |
|-------|------|-------------|------------|
| `directory` | `PathBuf` | Root directory to process (from content libraries) | Must exist, must be within a content library root |
| `pattern` | `String` | Glob pattern for file matching (File mode only) | Valid glob syntax, default `"*.md"` |
| `prompt_path` | `PathBuf` | Path to prompt markdown file (tagged "prompt") | Must exist, must have "prompt" tag |
| `mode` | `BatchMode` | Processing mode: File or Directory | Required |
| `concurrency` | `u8` | Number of concurrent LLM calls | 1-8, default 4 |

### BatchMode
Enumeration of batch processing modes.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BatchMode {
    File,       // Process each matching file individually
    Directory,  // Process each immediate subdirectory as a unit
}
```

**Behavior**:
- **File**: Walk `directory`, match files against `pattern`, run prompt once per file
- **Directory**: List immediate subdirectories of `directory`, run prompt once per subdirectory (pattern ignored)

### PromptInfo
Discovered prompt file available for selection.

| Field | Type | Description |
|-------|------|-------------|
| `path` | `PathBuf` | Full path to prompt markdown file |
| `display_name` | `String` | User-friendly name: "library_name / relative/path.md" |
| `library_name` | `String` | Source content library name |
| `content` | `String` | Prompt text (file body, excluding front matter) |

### BatchJob
A single unit of work in a batch session.

| Field | Type | Description |
|-------|------|-------------|
| `id` | `usize` | Sequential job index (0-based) |
| `target_path` | `PathBuf` | File path (File mode) or directory path (Directory mode) |
| `active_file` | `Option<PathBuf>` | Passed to `run_agent` as `active_file` (File mode) |
| `active_dir` | `Option<PathBuf>` | Passed to `run_agent` as `active_dir` (Directory mode) |
| `prompt_text` | `String` | The prompt content to execute |
| `status` | `BatchJobStatus` | Current execution status |
| `start_time` | `Option<DateTime<Local>>` | When job started |
| `end_time` | `Option<DateTime<Local>>` | When job completed |
| `error` | `Option<String>` | Error message if failed |

### BatchJobStatus
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchJobStatus {
    Pending,     // Queued, waiting for semaphore
    Running,     // Permit acquired, agent executing
    Completed,   // Agent finished successfully
    Failed,      // Agent returned error
    Cancelled,   // Cancelled before starting
}
```

### BatchSession
Aggregates all jobs for a batch execution.

| Field | Type | Description |
|-------|------|-------------|
| `config` | `BatchConfig` | User configuration |
| `jobs` | `Vec<BatchJob>` | All jobs in this session |
| `total_jobs` | `usize` | `jobs.len()` |
| `completed_jobs` | `usize` | Count of Completed + Failed + Cancelled |
| `running_jobs` | `usize` | Count of Running |
| `cancel_flag` | `Arc<AtomicBool>` | Shared cancellation signal |
| `start_time` | `DateTime<Local>` | Session start timestamp |

### BatchLogEntry
Extension of `BackgroundLogEntry` for batch-specific logging.

| Field | Type | Description |
|-------|------|-------------|
| `timestamp` | `DateTime<Local>` | Log timestamp |
| `category` | `LogCategory::Batch` | Log category |
| `message` | `String` | Human-readable message |
| `job_id` | `Option<usize>` | Associated job index |
| `phase` | `BatchLogPhase` | Start/End/Error |

### BatchLogPhase
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchLogPhase {
    JobStart,   // "Starting batch job N: {path}"
    JobEnd,     // "Completed batch job N: {path}"
    JobError,   // "Error in batch job N: {path}: {error}"
    SessionStart, // "Batch session started: {mode} mode, {N} jobs, concurrency {C}"
    SessionEnd,   // "Batch session completed: {success}/{total} jobs"
}
```

---

## Relationships

```
BatchConfig (1) ──► (1) BatchSession
BatchSession (1) ──► (N) BatchJob
BatchJob (1) ──► (1) PromptInfo (shared prompt_text)
BatchSession (1) ──► (N) BatchLogEntry (via BackgroundProcessManager)
```

---

## Validation Rules

1. **Directory**: Must be a content library root folder (from `config.content_libraries`)
2. **Pattern**: Required for File mode; ignored (but must be valid glob) for Directory mode
3. **Prompt**: Must be a `.md`/`.markdown` file with `tags: [prompt]` in front matter
4. **Concurrency**: Integer 1-8 inclusive
5. **Mode**: Must be File or Directory

---

## State Transitions

### BatchJob
```
Pending → Running → Completed
                ↘ Failed
Pending → Cancelled (if cancel before permit acquired)
Running → Cancelled (if cancel during execution - agent checks flag)
```

### BatchSession
```
Created → Running → Completed (all jobs done)
                  ↘ Cancelled (user cancelled)
```

---

## Serialization

- `BatchConfig`: Serde for potential future persistence
- `BatchJobStatus`, `BatchMode`: Serde for logging/debugging
- `PromptInfo`: Not persisted (discovered at dialog open)