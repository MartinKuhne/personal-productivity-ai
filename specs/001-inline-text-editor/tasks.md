# Tasks: Inline Text Editor

**Input**: Design documents from `/specs/001-inline-text-editor/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, quickstart.md

**Tests**: Tests are included as per the constitution's requirement for test-driven changes.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **Desktop app**: `src/desktop/src/`, `src/desktop/tests/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and basic structure

- [ ] T001 Review `quickstart.md` validation scenarios to align on end-to-end testing goals

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [x] T002 [P] Implement `DocumentContent` parsing and to_string logic in `src/desktop/src/document.rs`
- [x] T003 [P] Write unit tests for `DocumentContent` in `src/desktop/tests/document_test.rs`
- [x] T004 Create base `EditorState` structure with open/close methods in `src/desktop/src/editor.rs`

**Checkpoint**: Foundation ready - user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - Edit a file successfully (Priority: P1) 🎯 MVP

**Goal**: Edit the Markdown content of files without leaving the application or switching contexts to an external editor, saving time and keeping focus.

**Independent Test**: Can be fully tested by enabling the inline editor in config, clicking "Edit" on a valid Markdown file, typing changes, and saving. Validated by ensuring changes persist and front-matter is unharmed.

### Tests for User Story 1 ⚠️

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [x] T005 [P] [US1] Unit test for EditorState open/save transitions in `src/desktop/tests/editor_test.rs`

### Implementation for User Story 1

- [x] T006 Add `inline_editor_enabled` to configuration models in `src/desktop/src/app.rs`
- [x] T007 [US1] Implement basic modal/overlay UI with `egui::TextEdit` and Save button in `src/desktop/src/editor.rs`
- [x] T008 [US1] Implement line/column tracking in the status bar from `egui::TextEdit` cursor range in `src/desktop/src/editor.rs`
- [x] T009 [US1] Hook `EditorState::save` to write `DocumentContent` back to disk in `src/desktop/src/editor.rs`
- [x] T010 [US1] Wire the [Edit] context menu in `src/desktop/src/app.rs` to open the inline editor if enabled

**Checkpoint**: At this point, User Story 1 should be fully functional and testable independently

---

## Phase 4: User Story 2 - Discard unsaved changes (Priority: P2)

**Goal**: Users may start editing and decide they made a mistake or changed their minds, needing a safe way to abort without modifying the file.

**Independent Test**: Can be tested by typing text into the editor and clicking Cancel, ensuring the file remains completely unmodified on disk.

### Tests for User Story 2 ⚠️

- [x] T011 [P] [US2] Unit test for EditorState close/cancel behavior in `src/desktop/tests/editor_test.rs`

### Implementation for User Story 2

- [x] T012 [US2] Add [Cancel] button and wire `EditorState::close()` logic in `src/desktop/src/editor.rs`

**Checkpoint**: At this point, User Stories 1 AND 2 should both work independently

---

## Phase 5: User Story 3 - Validate Markdown (Priority: P2)

**Goal**: To prevent rendering breakages or corrupting notes, the system must ensure users cannot save syntactically invalid Markdown.

**Independent Test**: Can be tested by entering intentionally broken Markdown syntax and clicking Save, expecting an error instead of a successful write.

### Tests for User Story 3 ⚠️

- [x] T013 [P] [US3] Unit test for failed validation during save in `src/desktop/tests/editor_test.rs`

### Implementation for User Story 3

- [x] T014 [US3] Implement `pulldown-cmark` validation pass before saving in `src/desktop/src/editor.rs`
- [x] T015 [US3] Display parse error message in the UI if validation fails in `src/desktop/src/editor.rs`

**Checkpoint**: All user stories should now be independently functional

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [x] T016 Run `quickstart.md` validation scenarios end-to-end
- [x] T017 Code cleanup, fix all compilation/lint warnings
- [x] T018 Verify performance goals (parsing/loading overhead < 100ms) saves, no UI blocking on typical files) are met

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3+)**: All depend on Foundational phase completion
  - User stories can then proceed sequentially in priority order (P1 → P2 → P3)
- **Polish (Final Phase)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2) - No dependencies on other stories
- **User Story 2 (P2)**: Can start after Foundational (Phase 2) - Depends on US1's basic UI being present
- **User Story 3 (P3)**: Can start after Foundational (Phase 2) - Depends on US1's save logic

### Within Each User Story

- Tests MUST be written and FAIL before implementation
- Models before services
- Core implementation before UI integration
- Story complete before moving to next priority

### Parallel Opportunities

- Foundational parsing logic and tests (T002, T003) can be written in parallel to base UI structure (T004).
- Tests for any User Story can be written in parallel with app configuration changes.

---

## Parallel Example: User Story 1

```bash
# Developer A focuses on the UI and file saving state
Task: "[US1] Implement basic modal/overlay UI with egui::TextEdit and Save button in src/desktop/src/editor.rs"

# Developer B focuses on the application integration
Task: "[P] [US1] Add inline_editor_enabled to configuration models in src/desktop/src/app.rs"
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
