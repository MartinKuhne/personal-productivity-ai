# Tasks: PDF to Markdown Conversion

**Input**: Design documents from `/specs/002-pdf-markdown-conversion/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, quickstart.md

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and basic structure

- [x] T001 Update AppConfig to include `pdf_converter_command` in `src/config.rs`
- [x] T002 [P] Define `LogCategory` and `BackgroundLogEntry` entities in `src/background/models.rs`
- [x] T003 [P] Create the `src/background/mod.rs` module structure and expose models

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [x] T004 Implement `BackgroundProcessManager` (state, VecDeque capped at 10,000, filter state) in `src/background/manager.rs`
- [x] T005 [P] Write unit tests for `BackgroundProcessManager` (limit, insertion) in `tests/background_manager_test.rs`
- [x] T006 Initialize the background tokio channels and shared state in `src/app.rs`

**Checkpoint**: Foundation ready - user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - Automatic PDF to Markdown Conversion (Priority: P1) 🎯 MVP

**Goal**: Automatically convert any PDF files in configured text libraries to Markdown so they can be analyzed without manual intervention, keeping PDFs hidden from UI/LLMs.

**Independent Test**: Can be fully tested by placing a dummy PDF in a watched folder, ensuring a corresponding Markdown file is generated, and verifying no PDF files appear in the UI.

### Tests for User Story 1 ⚠️

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [x] T007 [P] [US1] Unit test for PDF conversion job execution in `tests/pdf_converter_test.rs`
- [x] T008 [P] [US1] Unit test for PDF exclusion logic in file discovery in `tests/discovery_test.rs`

### Implementation for User Story 1

- [x] T009 [P] [US1] Create `PdfConversionJob` model and logic in `src/background/pdf_converter.rs` to run `tokio::process::Command` asynchronously and send stdout/stderr via channel
- [x] T010 [US1] Integrate `PdfConversionJob` launching into the file watcher loop in `src/background/watcher.rs`
- [x] T011 [US1] Update file discovery/indexing logic to skip `.pdf` files in `src/background/watcher.rs` (or wherever discovery lives)
- [x] T012 [US1] Emit progress log entries from the watcher and indexer to the channel in `src/background/watcher.rs`

**Checkpoint**: At this point, User Story 1 should be fully functional and testable independently

---

## Phase 4: User Story 2 - Background Process Visibility (Priority: P2)

**Goal**: Provide a "Background Processes" tab displaying real-time output from background tasks to allow users to monitor operations and debug failures.

**Independent Test**: Can be tested by verifying the new "Background Processes" tab appears on startup, auto-scrolls with new log entries, and correctly persists to logs/background-process.log on exit.

### Implementation for User Story 2

- [x] T013 [P] [US2] Unit test for log serialization and persistence in `tests/log_persistence_test.rs`
- [x] T014 [US2] Create the "Background Processes" tab UI component in `src/ui/background_logs.rs` using `egui`
- [x] T015 [US2] Integrate the `background_logs` UI panel into the main application layout and menu in `src/app.rs`
- [x] T016 [US2] Add the application exit hook in `src/app.rs` to write the `BackgroundProcessManager` logs to `logs/background-process.log`

**Checkpoint**: At this point, User Stories 1 AND 2 should both work independently

---

## Phase 5: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [ ] T017 Code cleanup and ensure `cargo check` and `cargo test` run perfectly clean with no warnings
- [ ] T018 Run quickstart.md validation manually

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3+)**: All depend on Foundational phase completion
  - User stories can then proceed sequentially in priority order (P1 → P2) or parallel
- **Polish (Final Phase)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2) - No dependencies on other stories
- **User Story 2 (P2)**: Can start after Foundational (Phase 2) - No dependencies on other stories

### Within Each User Story

- Tests MUST be written and FAIL before implementation
- Models before services
- Services before endpoints/UI
- Core implementation before integration
- Story complete before moving to next priority

### Parallel Opportunities

- All Setup tasks marked [P] can run in parallel
- All Foundational tasks marked [P] can run in parallel
- Once Foundational phase completes, US1 and US2 can start in parallel
- All tests marked [P] can run in parallel

---

## Parallel Example: User Story 1

```bash
# Launch all tests for User Story 1 together:
Task: "Unit test for PDF conversion job execution in tests/pdf_converter_test.rs"
Task: "Unit test for PDF exclusion logic in file discovery in tests/discovery_test.rs"

# Once models are done, logic implementation can run:
Task: "Create PdfConversionJob model and logic in src/background/pdf_converter.rs..."
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL - blocks all stories)
3. Complete Phase 3: User Story 1
4. **STOP and VALIDATE**: Test User Story 1 independently using dummy config.
5. Deploy/demo if ready

### Incremental Delivery

1. Complete Setup + Foundational → Foundation ready
2. Add User Story 1 → Test independently → Deploy/Demo (MVP!)
3. Add User Story 2 → Test independently → Deploy/Demo
4. Each story adds value without breaking previous stories

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- Verify tests fail before implementing
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- Avoid: vague tasks, same file conflicts, cross-story dependencies that break independence
