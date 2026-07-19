# Tasks: Image Support (Vision)

**Input**: Design documents from `/specs/003-image-support-vision/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/vision_api.md

**Tests**: Tests are OPTIONAL - only include them if explicitly requested in the feature specification.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and basic structure

- [X] T001 Add `base64` crate to `src/desktop/Cargo.toml` dependencies

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T002 Implement `has_vision` helper method in `src/desktop/src/config.rs` for `LlmConfig`.
- [X] T003 [P] Define `ImageJob` struct in `src/desktop/src/background/models.rs`.

**Checkpoint**: Foundation ready - user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - Initial Image Discovery & Analysis (Priority: P1) 🎯 MVP

**Goal**: When the system indexes the library, it should find all images and trigger vision analysis for them, creating markdown files that describe the images.

**Independent Test**: Can be tested by adding a directory with an image, running initial indexing, and verifying a markdown file is created containing the vision analysis.

### Implementation for User Story 1

- [X] T004 [US1] Create new module `src/desktop/src/background/vision_processor.rs` to handle async vision API requests using `ureq` and the OpenAI-compatible payload format.
- [X] T005 [P] [US1] Export `vision_processor` in `src/desktop/src/background/mod.rs`.
- [X] T006 [US1] Update `src/desktop/src/background_task.rs` initial indexing logic to detect image extensions and queue them to the `vision_processor`.
- [X] T007 [US1] Update `src/desktop/src/background_task.rs` file watcher to queue `notify::EventKind::Create` events for image files to the `vision_processor`.

**Checkpoint**: At this point, User Story 1 should be fully functional and testable independently

---

## Phase 4: User Story 2 - Image Update Detection (Priority: P1)

**Goal**: When an existing image is updated on disk, the system should re-analyze it if its modified timestamp is newer than its corresponding markdown file.

**Independent Test**: Can be tested by modifying an existing image file and verifying that the vision analysis is re-triggered and the markdown file is updated.

### Implementation for User Story 2

- [X] T008 [US2] Update `notify::EventKind::Modify` handling in `src/desktop/src/background_task.rs` to detect image changes, check timestamps against corresponding `.md` files, and queue updates.

**Checkpoint**: At this point, User Stories 1 AND 2 should both work independently

---

## Phase 5: User Story 3 - Hidden Images (Priority: P2)

**Goal**: Images should remain hidden from the UI and LLM file tools so that the system operates strictly on the text descriptions.

**Independent Test**: Can be tested by attempting to list files or view the directory tree, ensuring image files are not visible.

### Implementation for User Story 3

- [X] T009 [P] [US3] Update file tree rendering in `src/desktop/src/ui/panels/tree.rs` to filter out image extensions (`.jpg`, `.png`, `.gif`, etc.).
- [X] T010 [P] [US3] Update `src/desktop/src/tools/` (e.g., `list_files` and `grep`) to ignore image files when scanning directories or searching file contents.

**Checkpoint**: All user stories should now be independently functional

---

## Phase N: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [X] T011 [P] Ensure error scenarios in `vision_processor.rs` log to `BackgroundProcessLog` properly.
- [X] T012 Run `quickstart.md` validation scenarios.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3+)**: All depend on Foundational phase completion
  - User stories can then proceed in parallel (if staffed)
  - Or sequentially in priority order (P1 → P2 → P3)
- **Polish (Final Phase)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2) - No dependencies on other stories
- **User Story 2 (P1)**: Can start after Foundational (Phase 2) - Integrates with US1 indexing mechanisms
- **User Story 3 (P2)**: Can start after Foundational (Phase 2) - No dependencies on other stories

### Within Each User Story

- Core implementation before integration
- Story complete before moving to next priority

### Parallel Opportunities

- All Setup tasks marked [P] can run in parallel
- All Foundational tasks marked [P] can run in parallel (within Phase 2)
- Once Foundational phase completes, all user stories can start in parallel (if team capacity allows)
- Different user stories can be worked on in parallel by different team members

---

## Parallel Example: User Story 3

```bash
# Launch UI and Tool hiding tasks for User Story 3 together:
Task: "Update file tree rendering in src/desktop/src/ui/panels/tree.rs to filter out image extensions."
Task: "Update src/desktop/src/tools/ to ignore image files when scanning directories."
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL - blocks all stories)
3. Complete Phase 3: User Story 1
4. **STOP and VALIDATE**: Test User Story 1 independently
5. Deploy/demo if ready

### Incremental Delivery

1. Complete Setup + Foundational → Foundation ready
2. Add User Story 1 → Test independently → Deploy/Demo (MVP!)
3. Add User Story 2 → Test independently → Deploy/Demo
4. Add User Story 3 → Test independently → Deploy/Demo
5. Each story adds value without breaking previous stories

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: User Story 1 & 2
   - Developer B: User Story 3
3. Stories complete and integrate independently
