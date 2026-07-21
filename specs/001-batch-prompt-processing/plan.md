# Implementation Plan: Batch Prompt Processing

**Branch**: `001-batch-prompt-processing` | **Date**: 2026-07-20 | **Spec**: specs/001-batch-prompt-processing/spec.md

**Input**: Feature specification from `/specs/001-batch-prompt-processing/spec.md`

## Summary

Add a batch prompt processing feature to the FastMD desktop application. Users can open a dialog from the top navigation bar to configure and execute batch LLM prompt processing across multiple files or directories. The dialog allows selecting a directory, file pattern (wildcard), prompt (markdown files tagged "prompt"), batch mode (File/Directory), and concurrency (1-8). Processing runs asynchronously with logging to the background log window, and can be cancelled mid-execution.

## Technical Context

**Language/Version**: Rust 2024 edition (1.75+)

**Primary Dependencies**: 
- eframe/egui 0.27 (GUI framework)
- tokio 1.53 (async runtime)
- walkdir 2.4 (filesystem traversal)
- ureq 2.9 (HTTP client for LLM API)
- tracing/tracing-subscriber (logging)

**Storage**: Local filesystem (content libraries defined in config), no database

**Testing**: cargo test (unit tests in source files, integration tests in tests/)

**Target Platform**: Desktop (Windows, Linux, macOS) via eframe/egui

**Project Type**: Desktop application (Rust + egui)

**Performance Goals**: 
- Dialog opens in <1 second
- Configuration in <30 seconds
- Process N files concurrently (configurable 1-8)
- Log entries appear in real-time

**Constraints**:
- Must integrate with existing background task system (BackgroundMessage channel)
- Must reuse existing LLM agent infrastructure (run_agent)
- Must reuse existing background log manager (BackgroundProcessManager)
- Must respect existing config system (content_libraries for directories)
- Must follow existing modal dialog patterns (modals.rs)

**Scale/Scope**:
- Single modal dialog with ~6 controls
- File discovery via walkdir with glob pattern matching
- Concurrent LLM calls via tokio::task::JoinSet with semaphore
- Logging via existing BackgroundLogEntry system

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Testability | ✅ Pass | Modular dialog component, pure functions for file matching, testable batch coordinator |
| II. Security | ✅ Pass | Path validation via existing is_safe_basename, no user input in shell commands |
| III. Modularity | ✅ Pass | New batch module, reuses existing agent/logger/config modules |
| IV. Open Source Leverage | ✅ Pass | Uses glob/walkdir for pattern matching, tokio for concurrency |
| V. SDLC Best Practices | ✅ Pass | Unit tests for file matching, integration tests for dialog, follow existing patterns |

## Project Structure

### Documentation (this feature)

```text
specs/001-batch-prompt-processing/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output (UI contracts)
└── tasks.md             # Phase 2 output (NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
src/desktop/
├── src/
│   ├── batch/                    # NEW: Batch processing module
│   │   ├── mod.rs                # Public exports
│   │   ├── dialog.rs             # Batch prompt processing dialog UI
│   │   ├── coordinator.rs        # Batch execution coordinator
│   │   ├── file_matcher.rs       # File/directory discovery with glob patterns
│   │   └── types.rs              # BatchConfig, BatchMode, BatchJob, etc.
│   ├── ui/
│   │   ├── mod.rs                # Add batch_dialog module
│   │   ├── modals.rs             # Add show_batch_modal function
│   │   └── app.rs                # Add batch state to FastMdApp
│   ├── background/
│   │   ├── manager.rs            # Already has LogCategory - add Batch category
│   │   └── models.rs             # Already has BackgroundLogEntry
│   ├── agent.rs                  # Reuse run_agent for batch processing
│   ├── messages.rs               # Add BatchMessage variants if needed
│   └── lib.rs                    # Export batch module
├── Cargo.toml                    # Already has all deps
└── tests/
    ├── batch_dialog_test.rs      # NEW: Dialog UI tests
    ├── batch_coordinator_test.rs # NEW: Coordinator tests
    └── file_matcher_test.rs      # NEW: File matching tests
```

**Structure Decision**: Single project (src/desktop) with new batch module following existing patterns (tools/, background/, ui/). Reuses existing infrastructure for config, agent, logging, and file events.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| None | All principles satisfied | N/A |

---

## Phase 0: Outline & Research

### Technical Unknowns to Resolve

1. **Directory source for "available directories"**: Spec assumes existing concept. From config.rs, `content_libraries` provides root folders. Should batch dialog use all content library roots, or allow subdirectory selection within them?

2. **Prompt discovery**: Spec says "markdown files with the 'prompt' tag". The tag system extracts from YAML front matter `tags: [prompt]`. Need to scan content libraries for such files.

3. **System context for batch**: Agent's `run_agent` accepts `active_file`, `active_dir`, `selected_files`. For File mode: each file becomes `active_file`. For Directory mode: each subdirectory becomes `active_dir`. Need to verify this mapping works.

4. **Concurrency control**: Use tokio::sync::Semaphore to limit concurrent `run_agent` calls. Each call spawns a thread. Need to manage cancellation across all in-flight tasks.

5. **Log categories**: BackgroundProcessManager has LogCategory enum. Need to add `Batch` category for batch-specific log entries.

6. **Batch cancellation**: Agent uses `cancel_flag: Arc<AtomicBool>`. For batch, need one flag per job or shared flag. When user clicks Cancel, set flag to stop new jobs, wait for in-flight.

7. **Prompt content**: Prompt files are markdown with front matter. The prompt text is the file body. Need to read and pass as the user prompt to `run_agent`.

### Research Tasks

- [ ] Confirm content_libraries as directory source
- [ ] Verify prompt file discovery via tag "prompt" 
- [ ] Map batch modes to agent context parameters
- [ ] Design semaphore-based concurrency with cancellation
- [ ] Add Batch LogCategory
- [ ] Define batch job state machine (Pending → Running → Completed/Failed/Cancelled)

### Output

`research.md` with decisions for each unknown.

---

## Phase 1: Design & Contracts

### Data Model (`data-model.md`)

Entities:
- **BatchConfig**: User configuration (directory, pattern, prompt, mode, concurrency)
- **BatchMode**: Enum (File, Directory)
- **BatchJob**: Single unit of work (file or directory path, prompt text, status)
- **BatchSession**: Collection of jobs with shared config, tracks overall progress
- **BatchLogEntry**: Log entry with category Batch, job ID, phase (start/end/error)

### UI Contracts (`contracts/`)

- **BatchDialog**: Modal with controls (directory combobox, pattern text, prompt combobox, mode radio, concurrency dropdown, Process/Cancel buttons)
- **BatchDialogState**: Open/closed, validation state, processing state
- **BatchProgress**: Real-time updates (current job, completed/total, logs)

### Quickstart (`quickstart.md`)

Validation steps:
1. Open app, click "Batch..." in top nav
2. Select directory from content libraries
3. Enter wildcard pattern (e.g., "*.md")
4. Select prompt tagged "prompt"
5. Choose File mode, concurrency 3
6. Click Process → observe background logs show start/end per file
7. Click Cancel mid-processing → new jobs stop, dialog closes after in-flight complete
8. Repeat with Directory mode → wildcard hidden, processes subdirectories

---

## Phase 2: Task Generation (Future /speckit-tasks)

Tasks will include:
1. Add Batch LogCategory to background/models.rs
2. Create batch/types.rs with config, job, session structs
3. Create batch/file_matcher.rs with glob pattern matching
4. Create batch/coordinator.rs with semaphore-based concurrency
5. Create batch/dialog.rs with egui modal UI
6. Integrate dialog into ui/modals.rs and ui/app.rs
7. Add "Batch..." button to top panel
8. Wire batch processing to run_agent with proper context
9. Add unit tests for file matching, coordinator, dialog
10. Add integration test for full batch flow