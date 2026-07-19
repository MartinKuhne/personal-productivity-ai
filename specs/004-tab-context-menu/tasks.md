# Tasks: Tab Context Menu

**Input**: Design documents from `specs/004-tab-context-menu/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, quickstart.md

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2)
- Include exact file paths in descriptions

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and basic structure
*No additional setup or dependencies required for this feature. Existing framework will be used.*

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

- [x] T001 Implement `TabContextAction` enum to represent the context menu actions in `src/desktop/src/ui/panels/center.rs`.

**Checkpoint**: Foundation ready - user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - Tab Management (Priority: P1) 🎯 MVP

**Goal**: Essential for basic workspace hygiene and usability, allowing users to quickly close irrelevant files and focus.

**Independent Test**: Right-click tabs and verify "Close", "Close Others", and "Close All" function correctly and prompt for unsaved changes if necessary.

### Implementation for User Story 1

- [x] T002 [US1] Add right-click interaction over tabs to display the context menu in `src/desktop/src/ui/panels/center.rs`.
- [x] T003 [US1] Implement "Close" action in the tab context menu, integrating with the existing tab close flow in `src/desktop/src/ui/panels/center.rs`.
- [x] T004 [US1] Implement "Close Others" action, ensuring all other tabs correctly attempt to close and prompt for unsaved changes in `src/desktop/src/ui/panels/center.rs`.
- [x] T005 [US1] Implement "Close All" action, attempting to close all tabs and prompting for unsaved changes in `src/desktop/src/ui/panels/center.rs`.

**Checkpoint**: At this point, User Story 1 should be fully functional and testable independently

---

## Phase 4: User Story 2 - File Operations from Tabs and Tree (Priority: P2)

**Goal**: Provide consistent quick access to common file operations from multiple UI locations.

**Independent Test**: Right-click tabs and directory tree files, verifying the file operations trigger system-level actions and the format task.

### Implementation for User Story 2

- [x] T006 [P] [US2] Extract reusable file operation actions (Show in Explorer, Open in Editor) from `src/desktop/src/ui/tree.rs` into a shared module if necessary to avoid code duplication, or prepare to reuse logic directly in `src/desktop/src/ui/panels/center.rs`.
- [x] T007 [US2] Implement "Copy Path" action in the tab context menu utilizing `ui.output_mut` in `src/desktop/src/ui/panels/center.rs`.
- [x] T008 [US2] Implement "Show in File Explorer" action in the tab context menu, mirroring the logic from `tree.rs`, in `src/desktop/src/ui/panels/center.rs`.
- [x] T009 [US2] Implement "Open in Editor" action in the tab context menu, mirroring the logic from `tree.rs`, in `src/desktop/src/ui/panels/center.rs`.
- [x] T010 [US2] Implement "Format Markdown" action in the tab context menu in `src/desktop/src/ui/panels/center.rs`.

**Checkpoint**: At this point, User Stories 1 AND 2 should both work independently

---

## Phase N: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [x] T011 Code cleanup and ensuring consistent naming for context menu items between the file tree and tab bar in `src/desktop/src/ui/panels/center.rs` and `src/desktop/src/ui/tree.rs`.
- [x] T012 Run quickstart.md validation manually to ensure everything works flawlessly.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3+)**: All depend on Foundational phase completion
  - User stories can then proceed in parallel (if staffed)
  - Or sequentially in priority order (P1 → P2)
- **Polish (Final Phase)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2) - No dependencies on other stories
- **User Story 2 (P2)**: Can start after Foundational (Phase 2) - Should be independently testable

### Within Each User Story

- Core implementation before integration
- Story complete before moving to next priority

### Parallel Opportunities

- Foundational tasks marked [P] can run in parallel (within Phase 2)
- Once Foundational phase completes, all user stories can start in parallel (if team capacity allows)
- Models within a story marked [P] can run in parallel
- Different user stories can be worked on in parallel by different team members

---

## Parallel Example: User Story 2

```bash
# Extract logic while implementing UI components concurrently
Task: "Extract reusable file operation actions... in src/desktop/src/ui/tree.rs"
Task: "Implement 'Copy Path' action... in src/desktop/src/ui/panels/center.rs"
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
4. Each story adds value without breaking previous stories

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: User Story 1
   - Developer B: User Story 2
3. Stories complete and integrate independently

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- Verify tests fail before implementing
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- Avoid: vague tasks, same file conflicts, cross-story dependencies that break independence
