# Tasks: User Command Bus

**Input**: Design documents from `/specs/001-user-command-bus/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and basic structure

- [x] T001 Create `UserCommand` enum with initial variants in `src/app/bus/events/user_command.rs`
- [x] T002 Expose `user_command` module in `src/app/bus/events/mod.rs`
- [x] T003 Create `UserCommandProducer` struct in `src/app/bus/events/user_command.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [x] T004 Add `user_command_bus` and `user_command_reader` fields to `AppOrchestrator` in `src/app/orchestrator.rs`
- [x] T005 Implement `drain_user_command_bus` method on `AppOrchestrator` in `src/app/orchestrator.rs`
- [x] T006 Wire `drain_user_command_bus` into `update_ui` in `src/app/ui/app/update.rs` (after `drain_agent_event_bus`)
- [x] T007 Create `CommandExecutor` module in `src/app/command_executor.rs` and implement `apply_user_command` method

**Checkpoint**: Foundation ready - user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - Centralized User Intent Routing (Priority: P1) 🎯 MVP

**Goal**: All user inputs flow through a single typed event bus (demonstrated via the bottom panel as MVP).

**Independent Test**: Can be fully tested by verifying that any UI interaction in the bottom panel produces an event on the `Bus<UserCommand>`.

### Implementation for User Story 1

- [x] T008 [P] [US1] Migrate bottom panel `apply_send_click` to return `UserCommand` in `src/app/ui/panels/bottom.rs`
- [x] T009 [P] [US1] Update `apply_user_command` in `src/app/command_executor.rs` to handle bottom panel variants
- [x] T010 [US1] Update `src/app/ui/panels/bottom.rs` tests to assert on published `UserCommand`

**Checkpoint**: At this point, User Story 1 should be fully functional and testable independently

---

## Phase 4: User Story 2 - Isolated UI State Rendering (Priority: P1)

**Goal**: UI components only publish intents rather than mutate application state.

**Independent Test**: Can be tested by verifying that UI panels no longer require mutable borrows of the application orchestrator to handle events.

### Implementation for User Story 2

- [x] T011 [US2] Extend `UserCommand` enum in `src/app/bus/events/user_command.rs` for Top Toolbar, Center Panel, Modals, and File Tree
- [x] T012 [P] [US2] Migrate Top Toolbar and Hamburger Menu `apply_*` helpers in `src/app/ui/panels/top.rs` to publish commands
- [x] T013 [P] [US2] Migrate Center Panel `apply_tab_close_*` helpers in `src/app/ui/panels/center.rs` to publish commands
- [x] T014 [P] [US2] Migrate Right Panel `apply_toc_row_click` in `src/app/ui/panels/right.rs` to publish commands
- [x] T015 [P] [US2] Migrate Modals (`show_*_dialog`) in `src/app/ui/modals.rs` to publish commands on confirm/cancel
- [x] T016 [P] [US2] Migrate Tools Dialog (`show_tools_dialog`) in `src/app/ui/tools_dialog.rs` to publish commands
- [x] T017 [P] [US2] Migrate File Tree `TreeNodeContext` handlers in `src/app/ui/tree/handlers.rs` and context menus in `src/app/ui/tree/render.rs`
- [x] T018 [P] [US2] Migrate Global Keyboard Shortcuts in `src/app/ui/app/update.rs` to publish commands
- [x] T019 [US2] Implement corresponding executor arms in `src/app/command_executor.rs` for all new variants

**Checkpoint**: At this point, User Stories 1 AND 2 should both work independently

---

## Phase 5: User Story 3 - Independent UI Click Testing (Priority: P2)

**Goal**: The UI interaction tests remain independent of the full application state.

**Independent Test**: Can be tested by running Tier-4 click-capture tests and asserting on the returned `UserCommand`.

### Implementation for User Story 3

- [x] T020 [P] [US3] Convert `show_top_panel_capture` tests in `src/app/ui/panels/top_tests.rs` to assert on published commands
- [x] T021 [P] [US3] Convert tab-close tests in `src/app/ui/panels/center_tests.rs` to assert on published commands
- [x] T022 [P] [US3] Convert modal tests in `src/app/ui/modals_tests.rs` to assert on published commands
- [x] T023 [P] [US3] Convert tree handler tests in `src/app/ui/tree/handlers_tests.rs` to assert on published commands

**Checkpoint**: All user stories should now be independently functional

---

## Phase N: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [x] T024 Remove `submit_prompt` field from `AppOrchestrator` in `src/app/orchestrator.rs`
- [x] T025 Remove `submit_prompt` deferred-action branch in `handle_deferred_actions` within `src/app/ui/app/update.rs`
- [x] T026 [P] Remove obsolete types (`CommandIntent`, `TabAction`, `NameEntryAction`, `BatchDialogResult`) across UI modules
- [x] T027 [P] Clean up unused `write_back` logic in `TreeNodeContext` in `src/app/ui/tree/context.rs`
- [x] T028 [P] Add `///` docs to all `UserCommand` variants and new public items to satisfy RUST-011
- [x] T029 Run quickstart.md validation

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
- **User Story 2 (P1)**: Can start after User Story 1 (US1 establishes the pattern and execution boundary)
- **User Story 3 (P2)**: Integrates testing improvements for US2, should run after or in parallel with US2 components.

### Parallel Opportunities

- Foundational tasks marked [P] can run in parallel (e.g. within Phase 4, panel migrations)
- All test refactors in US3 can run in parallel

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL - blocks all stories)
3. Complete Phase 3: User Story 1 (Bottom Panel command extraction)
4. **STOP and VALIDATE**: Test User Story 1 independently
5. Verify the architecture proves out without regressions

### Incremental Delivery

1. Complete Setup + Foundational → Foundation ready
2. Add User Story 1 → Test independently → Validate MVP
3. Add User Story 2 (remaining panels) → Test independently 
4. Add User Story 3 (test refactoring) → Asserts the UI logic cleanly
5. Each story adds value without breaking previous stories

## Phase 6: Convergence

- [ ] T030 Refactor `src/app/ui/panels/bottom.rs` to publish commands for agent cancellation and prompting instead of directly mutating `app.agent_mut()` per US2 (partial)
- [ ] T031 Remove redundant and impure `ctx.write_back()` call in `src/app/ui/panels/left.rs` and the corresponding logic in `TreeNodeContext` per US2 (partial)
- [ ] T032 Rewrite and restore commented-out Tier-4 tests in `src/app/ui/tree/render_tests.rs` and `src/app/ui/panels/center_tests.rs` to assert on published commands per US3 (partial)

## Phase 7: Convergence

- [ ] T033 Migrate `show_tools_dialog` row handlers in `src/app/ui/tools_dialog.rs` to publish `UserCommand::SetToolGroupEnabled` / `ClearToolGroupError` / `StartMcpAuth` / `ForgetMcpAuth` instead of directly mutating `AppConfig` and `ToolContext` per FR-001, FR-003, T016 (partial)
- [ ] T034 Route file-tree single- and multi-select `Delete` / `Move` actions through `Bus<UserCommand>` in `src/app/ui/tree/render.rs` instead of calling `recycle_bin::delete` and `FileEventProducer::publish_removed` inline per FR-001, FR-003, T017 (partial)
- [ ] T035 Implement `UserCommand::CopyPath` handling in `src/app/command_executor.rs` (currently `tracing::warn` stub) or delegate clipboard via UI plumbing so spec SC-001 routing is complete per FR-004 (partial)
- [ ] T036 Remove or justify retention of obsolete `CommandIntent` enum in `src/app/ui/panels/bottom.rs` and satisfy `CommandExecutor::_ => {}` wildcard by handling all variants explicitly per T026, RUST-011 (partial)
