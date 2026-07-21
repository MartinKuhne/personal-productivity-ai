# Tasks: Batch Prompt Processing

**Input**: Design documents from `/specs/001-batch-prompt-processing/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

**Tests**: Tests are OPTIONAL - only include them if explicitly requested in the feature specification.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **Single project**: `src/desktop/src/` at repository root
- Paths shown below assume single project structure per plan.md

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and basic structure

- [x] T001 Create batch module directory structure at `src/desktop/src/batch/`
- [x] T002 Add `batch` module to `src/desktop/src/lib.rs` exports
- [x] T003 [P] Add `glob` crate dependency to `src/desktop/Cargo.toml` for pattern matching (version 0.3)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [x] T004 Add `Batch` variant to `LogCategory` enum in `src/desktop/src/background/models.rs`
- [x] T005 [P] T006 Create `BatchMode` enum in `src/desktop/src/batch/types.rs`
- [x] [P] T007 Create `BatchConfig` struct in `src/desktop/src/batch/types.rs`
- [x] [P] T008 Create `PromptInfo` struct in `src/desktop/src/batch/types.rs`
- [x] [P] T009 Create `BatchJob` and `BatchJobStatus` structs in `src/desktop/src/batch/types.rs`
- [x] [P] T010 Create `BatchSession` struct in `src/desktop/src/batch/types.rs`
- [x] [P] T011 Create `BatchLogPhase` enum in `src/desktop/src/batch/types.rs`
- [x] T012 Export all types from `src/desktop/src/batch/mod.rs`
- [x] T013 Create `discover_prompts` function in `src/desktop/src/batch/prompts.rs` (new file)
- [x] T014 Create `read_prompt_content` function in `src/desktop/src/batch/prompts.rs`
- [x] T015 Create `find_matching_files` function in `src/desktop/src/batch/file_matcher.rs` (File mode)
- [x] T016 Create `find_subdirectories` function in `src/desktop/src/batch/file_matcher.rs` (Directory mode)
- [x] T017 Export file matcher functions from `src/desktop/src/batch/mod.rs`

**Checkpoint**: Foundation ready - user story implementation can now begin

---

## Phase 3: User Story 1 - Open Batch Processing Dialog (Priority: P1) 🎯 MVP

**Goal**: User can open the batch prompt processing dialog from the top navigation and see all configuration options

**Independent Test**: Click "Batch ..." button in top navigation → dialog opens with all controls visible and interactive

### Implementation for User Story 1

- [x] T018 [US1] Add `batch_dialog_open: bool` and `batch_dialog_config: BatchDialogConfig` fields to `FastMdApp` in `src/desktop/src/ui/app.rs`
- [x] T019 [US1] Create `BatchDialogConfig` struct in `src/desktop/src/batch/dialog.rs` with fields for available_dirs, available_prompts, selected_dir_idx, pattern, selected_prompt_idx, mode, concurrency
- [x] T020 [US1] Create `show_batch_modal` function in `src/desktop/src/ui/modals.rs` following `show_move_modal` pattern
- [x] T021 [US1] Implement dialog UI in `show_batch_modal`: directory ComboBox, pattern TextEdit, prompt ComboBox, mode Radio buttons, concurrency ComboBox (1-8), Process/Cancel buttons
- [x] T022 [US1] Add validation logic: Process button enabled only when directory selected, prompt selected, pattern valid (File mode), concurrency 1-8
- [x] T023 [US1] Implement mode switching: hide/show pattern field when switching between File/Directory mode
- [x] T024 [US1] Add "Batch ..." button to top panel in `src/desktop/src/ui/panels/top.rs` that sets `app.batch_dialog_open = true`
- [x] T025 [US1] Wire dialog open/close in `FastMdApp::update()` in `src/desktop/src/ui/app.rs` - call `show_batch_modal` when `batch_dialog_open`
- [x] T026 [US1] Handle dialog result: on `Process` → store config for US2; on `Cancel` → close dialog, clear state

**Checkpoint**: At this point, User Story 1 should be fully functional - dialog opens from top nav with all controls

---

## Phase 4: User Story 2 - Configure and Execute File Mode Batch Processing (Priority: P1)

**Goal**: User can configure and run a batch job in File mode to process multiple files with a selected prompt

**Independent Test**: Select directory, pattern, prompt, File mode, concurrency → click Process → system processes matching files concurrently → logs show start/end per file → Process disabled during processing → Cancel stops new processing

### Implementation for User Story 2

- [x] T027 [US2] Create `BatchCoordinator` struct in `src/desktop/src/batch/coordinator.rs` with semaphore, cancel_flag, config, channels
- [x] T028 [US2] Implement `execute_batch` function in `src/desktop/src/batch/coordinator.rs` that:
  - Discovers files using `find_matching_files` (File mode)
  - Creates `BatchJob` for each file with `active_file` set
  - Spawns tokio tasks with semaphore limiting concurrency
  - Calls `run_agent` for each job with `active_file` context
  - Logs JobStart/JobEnd via `BackgroundMessage::LogEntry` with `LogCategory::Batch`
  - Respects `cancel_flag` for graceful shutdown
- [x] T029 [US2] Implement `BatchHandle` with thread join handle and cancel_flag for external cancellation
- [x] T030 [US2] Wire `execute_batch` call in `show_batch_modal` result handler (when Process clicked in File mode) in `src/desktop/src/ui/modals.rs`
- [x] T031 [US2] Store `BatchHandle` and `cancel_flag` in `FastMdApp` for cancellation during processing
- [x] T032 [US2] Disable Process button and all config controls in dialog while batch is running (in `show_batch_modal`)
- [x] T033 [US2] Implement background log entries for session start, each job start/end, session end with timestamps
- [x] T034 [US2] Poll `BatchHandle` in `FastMdApp::update()` to detect completion and re-enable UI

**Checkpoint**: At this point, User Story 2 should be fully functional - File mode batch processing works end-to-end

---

## Phase 5: User Story 3 - Configure and Execute Directory Mode Batch Processing (Priority: P1)

**Goal**: User can configure and run a batch job in Directory mode to process subdirectories with a selected prompt

**Independent Test**: Select directory, Directory mode (pattern hidden), prompt, concurrency → click Process → system processes subdirectories → logs show start/end per directory

### Implementation for User Story 3

- [x] T035 [US3] Extend `execute_batch` in `src/desktop/src/batch/coordinator.rs` to handle Directory mode:
  - Discovers subdirectories using `find_subdirectories`
  - Creates `BatchJob` for each subdirectory with `active_dir` set
  - Same concurrency and logging as File mode
- [x] T036 [US3] Ensure pattern field is ignored in Directory mode (validation already hides it from US1)
- [x] T037 [US3] Test Directory mode end-to-end: subdirectory discovery, agent context with `active_dir`, logging

**Checkpoint**: At this point, User Stories 1, 2, AND 3 should all work independently

---

## Phase 6: User Story 4 - Cancel Batch Processing Dialog (Priority: P2)

**Goal**: User can cancel the dialog before or during processing without any side effects

**Independent Test**: Open dialog, click Cancel → dialog closes immediately, no processing. During processing, click Cancel → new jobs stop, dialog closes after in-flight complete

### Implementation for User Story 4

- [x] T038 [US4] Implement pre-processing Cancel in `show_batch_modal`: close dialog, clear config, no batch started
- [x] T039 [US4] Implement during-processing Cancel in `FastMdApp::update()` or dialog:
  - Set `cancel_flag.store(true, SeqCst)` on stored flag
  - Show progress dialog with Cancel button during processing
  - Wait for `BatchHandle` thread to complete (in-flight jobs finish)
  - Log "Batch session cancelled" with counts
- [x] T040 [US4] Handle window close (X button) same as Cancel button
- [x] T041 [US4] Ensure no files modified on cancel (agent reads only, no write tools in batch)

**Checkpoint**: At this point, User Stories 1-4 should all work independently

---

## Phase 7: User Story 5 - Concurrency Control (Priority: P2)

**Goal**: User controls how many prompts are processed simultaneously (1-8)

**Independent Test**: Set concurrency to 1, 4, 8 → observe at most N prompts process concurrently

### Implementation for User Story 5

- [x] T042 [US5] Verify concurrency dropdown in dialog has options 1-8 (already in US1)
- [x] T043 [US5] Verify semaphore in `BatchCoordinator` uses `concurrency` from config
- [x] T044 [US5] Add default concurrency value (4) in `BatchDialogConfig` initialization
- [ ] T045 [US5] Test concurrency limits: run with 1, 4, 8 and verify log timestamps show correct overlap

**Checkpoint**: All 5 user stories should now be independently functional

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [x] T046 [P] Add unit tests for `file_matcher.rs` in `src/desktop/src/batch/file_matcher.rs` (tests module)
- [x] T047 [P] Add unit tests for `prompts.rs` in `src/desktop/src/batch/prompts.rs` (tests module)
- [x] T048 [P] Add unit tests for `coordinator.rs` in `src/desktop/src/batch/coordinator.rs` (tests module)
- [ ] T049 [P] Add unit tests for dialog validation in `src/desktop/src/ui/modals.rs` (tests module)
- [ ] T050 [P] Add integration test file `src/desktop/tests/batch_integration_test.rs` for full flow
- [x] T051 Run `cargo test` to ensure no regressions
- [ ] T052 Run quickstart.md validation scenarios manually
- [ ] T053 [P] Update any documentation if needed
- [x] T054 Code cleanup and formatting (`cargo fmt`)

## Completion Note (Jul 20, 2026)

**45 of 54 tasks completed** (was 37 before this session). Added:
- Phase 5 (US3): 3 tasks - Directory mode fully implemented and tested
- Phase 6 (US4): 4 tasks - Cancel during processing with progress dialog, window X handler
- Phase 7 (US5): 3 of 4 tasks - Concurrency plumbing verified, manual test remaining
- Phase 8: 4 of 9 tasks - Unit tests for file_matcher, prompts, coordinator; cargo test+fmt passed

**Bugs fixed**: Duplicate batch handle poll blocks in app.rs; unconditional glob validation in Directory mode; tokio runtime context for JoinSet spawning.

**Remaining**: T045 (manual concurrency verification), T049 (dialog validation tests), T050 (integration test), T052 (manual validation), T053 (documentation).

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phases 3-7)**: All depend on Foundational phase completion
  - US1 (Phase 3) can start after Phase 2
  - US2 (Phase 4) depends on US1 (dialog must exist)
  - US3 (Phase 5) depends on US1 + US2 (coordinator must exist)
  - US4 (Phase 6) depends on US1 + US2 (cancel during processing)
  - US5 (Phase 7) depends on US1 + US2 (concurrency in coordinator)
- **Polish (Phase 8)**: Depends on all desired user stories being complete

### User Story Dependencies

| Story | Priority | Depends On | Notes |
|-------|----------|------------|-------|
| US1 | P1 | Phase 2 | MVP - dialog + top nav button |
| US2 | P1 | US1 | File mode processing |
| US3 | P1 | US1, US2 | Directory mode (extends coordinator) |
| US4 | P2 | US1, US2 | Cancel handling |
| US5 | P2 | US1, US2 | Concurrency validation |

### Within Each User Story

- Types/structs before functions
- Core logic before UI integration
- Integration before polish

### Parallel Opportunities

- All Setup tasks marked [P] can run in parallel
- All Foundational type definitions (T006-T011) marked [P] can run in parallel
- T013-T017 (prompt discovery + file matching) can run in parallel
- All Polish unit tests (T046-T050) marked [P] can run in parallel
- Different user stories can be worked on in parallel once their dependencies are met

---

## Parallel Example: Foundational Phase

```bash
# Launch all type definitions together:
Task: T006 Create BatchMode enum in src/desktop/src/batch/types.rs
Task: T007 Create BatchConfig struct in src/desktop/src/batch/types.rs
Task: T008 Create PromptInfo struct in src/desktop/src/batch/types.rs
Task: T009 Create BatchJob struct in src/desktop/src/batch/types.rs
Task: T010 Create BatchSession struct in src/desktop/src/batch/types.rs
Task: T011 Create BatchLogPhase enum in src/desktop/src/batch/types.rs

# Launch prompt/file discovery together:
Task: T013 Create discover_prompts in src/desktop/src/batch/prompts.rs
Task: T015 Create find_matching_files in src/desktop/src/batch/file_matcher.rs
Task: T016 Create find_subdirectories in src/desktop/src/batch/file_matcher.rs
```

---

## Parallel Example: User Story 1

```bash
# UI components can be developed in parallel once types exist:
Task: T019 Create BatchDialogConfig in src/desktop/src/batch/dialog.rs
Task: T020 Create show_batch_modal in src/desktop/src/ui/modals.rs
Task: T024 Add Batch button in src/desktop/src/ui/panels/top.rs
```

---

## Parallel Example: User Story 2

```bash
# Coordinator and integration:
Task: T027 Create BatchCoordinator in src/desktop/src/batch/coordinator.rs
Task: T028 Implement execute_batch in src/desktop/src/batch/coordinator.rs
Task: T030 Wire execute_batch in src/desktop/src/ui/modals.rs
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL - blocks all stories)
3. Complete Phase 3: User Story 1
4. **STOP and VALIDATE**: Test User Story 1 independently - dialog opens from top nav
5. Deploy/demo if ready

### Incremental Delivery

1. Complete Setup + Foundational → Foundation ready
2. Add User Story 1 → Test independently → Deploy/Demo (MVP!)
3. Add User Story 2 → Test independently → Deploy/Demo
4. Add User Story 3 → Test independently → Deploy/Demo
5. Add User Story 4 → Test independently → Deploy/Demo
6. Add User Story 5 → Test independently → Deploy/Demo
7. Each story adds value without breaking previous stories

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: User Story 1 (dialog + top nav)
   - Developer B: User Story 2 (coordinator + File mode)
   - Developer C: User Story 3 (Directory mode extension)
3. Stories complete and integrate independently
4. Developer D: User Stories 4-5 (cancel + concurrency polish)
5. All: Phase 8 Polish

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- Verify tests fail before implementing (if tests included)
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- Avoid: vague tasks, same file conflicts, cross-story dependencies that break independence

---

## Task Summary

| Phase | Tasks | User Story | Done |
|-------|-------|------------|------|
| Phase 1: Setup | 3 | - | 3/3 |
| Phase 2: Foundational | 14 | - | 14/14 |
| Phase 3: US1 | 9 | P1 - Open Dialog | 9/9 |
| Phase 4: US2 | 8 | P1 - File Mode | 8/8 |
| Phase 5: US3 | 3 | P1 - Directory Mode | 3/3 |
| Phase 6: US4 | 4 | P2 - Cancel | 4/4 |
| Phase 7: US5 | 4 | P2 - Concurrency | 3/4 |
| Phase 8: Polish | 9 | - | 4/9 |
| **Total** | **54** | | **45/54 (83%)** |

**MVP Scope**: Phases 1-3 (Setup + Foundational + US1) = 26 tasks

**Full Feature**: All 54 tasks across 8 phases