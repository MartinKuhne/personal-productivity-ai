# Research: Batch Prompt Processing

**Feature**: Batch Prompt Processing  
**Date**: 2026-07-20  
**Status**: Complete

---

## Research Summary

All technical unknowns have been resolved by analyzing the existing codebase. Decisions are documented below with rationale.

---

## 1. Directory Source for "Available Directories"

**Decision**: Use `config.content_libraries` root folders as the available directories. Users can select any content library root folder.

**Rationale**: 
- `AppConfig.content_libraries` is the existing concept of "available directories" in the app
- Each library has a `root_folder` path that is already watched/indexed
- This aligns with the existing file discovery and tagging infrastructure
- Subdirectory selection within a library is not needed for MVP; users select the library root

**Alternatives Considered**:
- Allow browsing subdirectories: Rejected for MVP - adds UI complexity (tree picker). Can be added later.
- Use `app.all_dirs` (all discovered directories): Too many, includes non-library folders.

---

## 2. Prompt Discovery (Markdown Files Tagged "prompt")

**Decision**: Scan all content libraries for `.md`/`.markdown` files with `tags: [prompt]` in YAML front matter. Use existing `extract_tags_from_file` utility.

**Rationale**:
- `utils/tags.rs::extract_tags_from_file` already parses front matter tags
- Tags are normalized to lowercase, so "prompt", "Prompt", "PROMPT" all work
- Content libraries are the authoritative file source (already indexed)
- Can reuse the file event bus for real-time updates if prompts are added/removed

**Implementation**: 
- On dialog open, scan `config.content_libraries` roots for markdown files
- Filter by `tags.contains(&"prompt".to_string())`
- Display as `library_name / relative_path` in combobox
- Store full path for reading prompt content

---

## 3. System Context Mapping for Batch Modes

**Decision**: Map batch modes to `run_agent` parameters as follows:

| Batch Mode | run_agent Parameter | Context Added to System Prompt |
|------------|---------------------|--------------------------------|
| File       | `active_file = Some(file_path)` | "The user is currently viewing the file: {path}" |
| Directory  | `active_dir = Some(dir_path)` | "The user has selected the directory context: {path}" |

**Rationale**:
- `run_agent` already accepts `active_file: Option<PathBuf>` and `active_dir: Option<PathBuf>`
- The system prompt builder (lines 178-190 in agent.rs) injects these into the system prompt
- This is exactly the "system context" concept referenced in the spec
- No changes to `run_agent` needed - just pass appropriate parameters

**Note**: `selected_files` is not used for batch (that's for multi-file selection in UI).

---

## 4. Concurrency Control with Cancellation

**Decision**: Use `tokio::sync::Semaphore` with `Arc<AtomicBool>` cancellation flag shared across all jobs.

**Architecture**:
```rust
struct BatchCoordinator {
    semaphore: Arc<Semaphore>,
    cancel_flag: Arc<AtomicBool>,
    tx_gui: Sender<BackgroundMessage>,
    config: AppConfig,
    file_event_bus: Bus<FileEvent>,
}

async fn run_batch_job(&self, job: BatchJob) {
    let _permit = self.semaphore.acquire().await;
    if self.cancel_flag.load(Ordering::SeqCst) { return; }
    
    // Log start
    self.tx_gui.send(BackgroundMessage::LogEntry(batch_log_start(&job)));
    
    // Run agent with appropriate context
    run_agent(
        self.config.clone(),
        self.tx_gui.clone(),
        job.active_file,  // File mode
        job.active_dir,   // Directory mode
        HashSet::new(),
        job.prompt_text,
        self.cancel_flag.clone(),  // Shared flag
        None,
        String::new(),
        self.file_event_bus.clone(),
    );
    
    // Log end (agent sends AgentFinished, but we also log batch-level)
    self.tx_gui.send(BackgroundMessage::LogEntry(batch_log_end(&job)));
}
```

**Cancellation Behavior**:
- User clicks Cancel → `cancel_flag.store(true, SeqCst)`
- Coordinator stops acquiring new permits (no new jobs start)
- In-flight jobs check flag at loop start in `run_agent` (already implemented at lines 236, 293)
- Dialog closes after all permits released (in-flight complete)
- Graceful shutdown, no forced thread termination

---

## 5. Log Category for Batch Processing

**Decision**: Add `Batch` variant to `LogCategory` enum in `background/models.rs`.

**Rationale**:
- Existing categories: `Indexer`, `Watcher`, `PdfConverter`, `VisionProcessor`, `Agent`
- Batch is a distinct user-initiated operation category
- Allows filtering batch logs in background log window
- Consistent with existing logging architecture

**Change**: 
```rust
pub enum LogCategory {
    Indexer,
    Watcher,
    PdfConverter,
    VisionProcessor,
    Agent,
    Batch,  // NEW
}
```

---

## 6. Batch Job State Machine

**Decision**: Simple state tracked in coordinator, not persisted.

**States**:
- `Pending` - Job queued, waiting for semaphore
- `Running` - Permit acquired, agent started
- `Completed` - Agent finished successfully
- `Failed` - Agent returned error
- `Cancelled` - Cancel flag set before job started

**Tracking**: Coordinator maintains `Vec<BatchJobStatus>` for UI progress display.

---

## 7. Prompt Content Extraction

**Decision**: Read entire markdown file body (after front matter) as the prompt text.

**Rationale**:
- Prompts are markdown files with front matter for tags
- The prompt template is the markdown content
- `parse_front_matter` in `utils/markdown.rs` already separates front matter from body
- Use the body as-is for the user prompt to `run_agent`

---

## 8. Integration Points Summary

| Component | Integration Method |
|-----------|-------------------|
| Top Navigation | Add button in `ui/panels/top.rs` → sets `app.batch_dialog_open = true` |
| Modal Dialog | Add `show_batch_modal` in `ui/modals.rs` following `show_move_modal` pattern |
| App State | Add `BatchDialogState` to `FastMdApp` in `ui/app.rs` |
| Background Logs | Use existing `BackgroundProcessManager` via `BackgroundMessage::LogEntry` |
| LLM Processing | Call existing `run_agent` with mapped context parameters |
| File Discovery | Use `walkdir` with `glob` pattern matching in new `batch/file_matcher.rs` |
| Config | Read `config.content_libraries` for directories and prompt files |
| Cancellation | Reuse `Arc<AtomicBool>` pattern from `run_agent` |

---

## 9. Testing Strategy

**Unit Tests** (in source files):
- `file_matcher.rs`: Glob pattern matching, directory traversal, edge cases
- `coordinator.rs`: Semaphore limiting, cancellation propagation, job state transitions
- `dialog.rs`: UI state validation (disabled buttons, mode switching hides pattern)

**Integration Tests** (in tests/):
- Full batch flow: config → dialog → process → logs → cancel
- File mode with multiple files
- Directory mode with subdirectories
- Error handling (no prompts, no matches, LLM failure)

---

## Conclusion

All technical unknowns resolved. The feature integrates cleanly with existing architecture:
- No new external dependencies needed
- Reuses `run_agent`, `BackgroundProcessManager`, `config`, `tags`, `file_events`
- Follows existing modal dialog patterns
- Adds one new module (`batch/`) with clear separation of concerns

Ready for Phase 1 design artifacts.