---
description: "Task list for CSV Database Tools feature implementation"
---

# Tasks: csv-database-tools

**Input**: Design documents from `specs/005-csv-database-tools/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/tool-schemas.md

**Tests**: Test tasks are included per `AGENTS.md` guidelines for unit and functional test coverage.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- exact file paths in descriptions

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and basic structure

- [ ] T001 Create project structure `src/desktop/src/tools/csv_db/` with `mod.rs`, `operations.rs`, `query.rs`, and `schema.rs`
- [ ] T002 Add `csv` and `evalexpr` to dependencies in `src/desktop/Cargo.toml`
- [ ] T003 Hook `csv_db` module into the tool registry in `src/desktop/src/tools/mod.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

- [X] T004 Implement `CsvDatabase`, `QueryRequest`, and `QueryResponse` models in `src/desktop/src/tools/csv_db/schema.rs`
- [X] T005 Implement storage directory resolution (using `%APPDATA%\fastmd\db\` fallback) in `src/desktop/src/tools/csv_db/operations.rs`

**Checkpoint**: Foundation ready - user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - Context-Aware Tool Presentation (Priority: P1) 🎯 MVP

**Goal**: Selectively expose CSV tools based on keyword matching in the user query.

**Independent Test**: Can be tested by simulating user prompts with and without the keywords and asserting the presence of the tools in the LLM's context.

### Tests for User Story 1

- [X] T006 [P] [US1] Add unit tests for keyword triggering logic in `src/desktop/src/tools/csv_db/mod.rs`

### Implementation for User Story 1

- [X] T007 [US1] Define tool interfaces for `add_rows`, `delete_rows`, `create_csv`, `list_csv`, `query` and register trigger keywords in `src/desktop/src/tools/csv_db/mod.rs`

**Checkpoint**: At this point, tools will appear when keywords are mentioned, but their logic is stubbed.

---

## Phase 4: User Story 2 - Dynamic Querying & File Ops (Priority: P1)

**Goal**: Implement `create_csv`, `list_csv`, `add_rows`, `delete_rows`, and `query` core operations (with best-effort type inference).

**Independent Test**: Invoke `query` tool with a mock CSV and a valid expression and verify correct rows are returned.

### Tests for User Story 2

- [X] T008 [P] [US2] Add unit tests for create, list, and add_rows in `src/desktop/src/tools/csv_db/operations.rs`
- [X] T009 [P] [US2] Add unit tests for query type inference and evaluation in `src/desktop/src/tools/csv_db/query.rs`

### Implementation for User Story 2

- [X] T010 [P] [US2] Implement `create_csv` and `list_csv` file operations in `src/desktop/src/tools/csv_db/operations.rs`
- [X] T011 [P] [US2] Implement `add_rows` with strict schema validation in `src/desktop/src/tools/csv_db/operations.rs`
- [X] T012 [P] [US2] Implement `delete_rows` using `evalexpr` for predicates in `src/desktop/src/tools/csv_db/query.rs`
- [X] T013 [P] [US2] Implement `query` evaluating `evalexpr` predicates with best-effort number inference in `src/desktop/src/tools/csv_db/query.rs`

**Checkpoint**: Tools are fully functional for filtering, appending, and listing CSVs.

---

## Phase 5: User Story 3 - Data Aggregation (Priority: P2)

**Goal**: Compute sum and average of numeric columns.

**Independent Test**: Run a query with `sum` or `average` aggregation on a known dataset and check accuracy.

### Tests for User Story 3

- [X] T014 [P] [US3] Add unit tests for aggregation functions in `src/desktop/src/tools/csv_db/query.rs`

### Implementation for User Story 3

- [X] T015 [US3] Extend `query` operation to support `sum` and `average` aggregations in `src/desktop/src/tools/csv_db/query.rs`

**Checkpoint**: Aggregation functionality works within the query tool.

---

## Phase 6: User Story 4 - Configurable Storage (Priority: P3)

**Goal**: Allow override of the default `%APPDATA%\fastmd\db\` storage location.

**Independent Test**: Create a CSV database after configuring a custom path, and verify it stores there.

### Implementation for User Story 4

- [X] T016 [US4] Add storage configuration override parameter/setting to the desktop configuration in `src/desktop/src/config.rs` (or equivalent file)
- [X] T017 [US4] Update storage directory resolution to respect the override in `src/desktop/src/tools/csv_db/operations.rs`

**Checkpoint**: All user stories complete. Storage is configurable.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [X] T018 Run the validation scenarios from `quickstart.md`
- [X] T019 Ensure `cargo check` and `cargo test` run perfectly clean with no warnings

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3+)**: All depend on Foundational phase completion
  - US1 and US2 are P1 and should be prioritized.
  - US3 depends on US2.
  - US4 can run independently anytime after Phase 2.
- **Polish (Final Phase)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2)
- **User Story 2 (P1)**: Can start after Foundational (Phase 2)
- **User Story 3 (P2)**: Depends on US2 (requires core query logic).
- **User Story 4 (P3)**: Can start after Foundational (Phase 2)

### Parallel Opportunities

- Tests (T008, T009) can run in parallel with implementation if adopting true TDD.
- T010, T011, T012, T013 are marked [P] as they target different functions, though they sit in similar files.
- US1 and US2 can be implemented in parallel.

---

## Parallel Example: User Story 2

```bash
# Launch implementation for different operations together:
Task: "Implement `create_csv` and `list_csv` file operations in `src/desktop/src/tools/csv_db/operations.rs`"
Task: "Implement `delete_rows` using `evalexpr` for predicates in `src/desktop/src/tools/csv_db/query.rs`"
```

---

## Implementation Strategy

### MVP First (User Story 1 & 2)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL)
3. Complete Phase 3 & 4: US1 and US2
4. **STOP and VALIDATE**: Test CSV tools end-to-end via `quickstart.md`.

### Incremental Delivery

1. Foundation ready.
2. Deliver US1 & US2 -> MVP is working.
3. Add US3 -> Aggregations.
4. Add US4 -> Configurable storage.
