---

description: "Task list for Agent Loop / UI Seam Refactor"
---

# Tasks: Agent Loop / UI Seam Refactor

**Input**: Design documents from `/specs/003-agent-ui-seam-refactor/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/agent-seam.md, quickstart.md

**Tests**: Included — the constitution mandates testability (Principle I) and the AGENTS.md development process requires tests to pass between steps. Tests follow the quickstart.md validation scenarios.

**Organization**: Tasks are grouped by user story (US1–US4) in priority order. Each task is a migration step from plan.md. **⚠️ CRITICAL**: The migration steps are strictly sequential (FR-017, SC-008) — each task MUST compile and pass the quality gate before the next begins. The `[P]` marker indicates parallelizable tasks (different files, no dependency on the immediately preceding task); most tasks are NOT parallel due to the strict migration order.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3, US4)
- Include exact file paths in descriptions

## Path Conventions

- **Single crate**: all paths relative to `src/desktop/src/` unless prefixed with `src/desktop/` (for `tests/`, `Cargo.toml`, etc.)
- Repo root: `C:\Users\mkuhn\src\ppai`
- Quality gate (run after EVERY task, per `src/desktop/AGENTS.md:78-86`):
  ```powershell
  cargo check --quiet
  cargo nextest run --status-level fail --show-progress none
  cargo clippy -- -D warnings
  cargo fmt --check
  cargo doc --no-deps --quiet
  ```

## Migration Step → User Story Mapping

| Migration step (plan.md) | Primary user story | Phase |
|--------------------------|--------------------|-------|
| 1. Introduce `agent/events.rs` | (shared infrastructure) | Setup |
| 2. Add `Bus<AgentEvent>` (dual-publish) | (shared infrastructure) | Foundational |
| 3. Move `response_formatter` to `ui/` + transcript accumulator | US1 (enables rendering from new channel) | US1 |
| 4. `Vec<ToolSideEffect>` from `execute_all` | US1 (file reindex path) | US1 |
| 5. Switch orchestrator drain to `Bus<AgentEvent>` | US1 (the flip) | US1 |
| 6. Split `AgentSessionManager` → `AgentPanelState` | US2 (agent has no UI state) | US2 |
| 7. Delete `AgentContext::tx_gui` + `current_response` | US2 (agent UI-free) | US2 |
| 8. Restructure `WebDelegateResponse` (structured trace) | US2 (no string in agent) | US2 |
| 9. Remove `BackgroundEvent::Agent` variant | US2 (clean boundary) | US2 |
| 10. Replace `session_counter` with `Uuid` | US4 (session identity) | US4 |

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Create the new event types as additive declarations. No behavior change — the existing `BackgroundEvent::Agent(AgentEvent)` path stays untouched.

- [x] T001 [P] Create `agent/events.rs` with `AgentEvent` (all variants carry `session_id: Uuid`), `AgentStatus`, `ToolSideEffect`, `AgentPrompt`, `DelegateToolCall` types per data-model.md §2-3,5,11 — `src/desktop/src/agent/events.rs` (migration step 1)
- [x] T002 [P] Add `Serialize` derive to `TokenUsageInfo` in `src/desktop/src/bus/events/messages.rs` (research.md §6)
- [x] T003 [P] Re-export `McpAuthEvent` from `src/desktop/src/bus/events/mod.rs` for consistency (research.md §9)
- [x] T004 [P] Update `doc/technical-context/ARCHITECTURE_C4.md` to document the planned agent↔UI seam boundary (RUST-042)

**Checkpoint**: New types compile; no runtime behavior changed. Quality gate passes.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Wire the new `Bus<AgentEvent>` channel alongside the existing `tx_gui` mpsc path. The agent dual-publishes; the UI subscribes but does NOT render from the new path yet (research.md §8 — strangler-fig migration).

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [x] T005 Add `Bus<AgentEvent>` field to `AgentSessionManager` in `src/desktop/src/agent/manager.rs`; expose `event_bus()` accessor for UI subscription; clone the `Bus` handle into the driver/agent context (migration step 2)
- [x] T006 Wire agent loop to dual-publish on `Bus<AgentEvent>` + old `tx_gui` in `src/desktop/src/agent/agent_impl.rs` — every `ctx.tx_gui.send(...)` site also publishes the equivalent structured `AgentEvent` on the bus (migration step 2)
- [x] T007 Add `BusReader<AgentEvent>` to `AppOrchestrator` in `src/desktop/src/app/orchestrator.rs`; subscribe during init; drain in `drain_background_channel` but do NOT route to render path yet (migration step 2)
- [x] T008 Write shadow-assertion test that `Bus<AgentEvent>` receives the same events (same order, same session_id) as the old `tx_gui` mpsc path in `src/desktop/src/agent/agent_impl_tests.rs` (migration step 2 verification, quickstart scenario 2 precursor)
- [x] T009 Implement `BusReader::Lagged(n)` handling in `src/desktop/src/app/orchestrator.rs` drain loop — on lag, emit visible truncation marker `[output truncated — UI fell behind the agent]` into transcript, log the lag count, re-sync on next `SessionStarted`/`Status` boundary (research.md §1, quickstart scenario 5)

**Checkpoint**: New channel runs in shadow; old path still authoritative. Quality gate passes. UI renders unchanged from old path.

---

## Phase 3: User Story 1 — No-Regression Rendering Through Decoupled Channel (Priority: P1) 🎯 MVP

**Goal**: The UI renders agent output identically to the pre-refactor baseline, but from the new `Bus<AgentEvent>` channel. File side-effects trigger reindexing through the typed path.

**Independent Test**: Submit a representative prompt (thinking + content + tool call) and assert the rendered transcript matches the pre-refactor baseline; confirm file creation triggers reindex (quickstart scenarios 3, 4).

**Depends on**: Phase 2 (dual-publish channel exists).

### Implementation for User Story 1

- [x] T010 [US1] Move `response_formatter.rs` to `src/desktop/src/ui/render/agent_render.rs` — relocate `format_tool_call_message`, `format_tool_result_message`, `format_delegate_tool_call_message`, `split_thinking_and_content`; update all import sites (migration step 3, RUST-055)
- [ ] T011 [US1] Create `AgentTranscript` view model in `src/desktop/src/ui/agent/transcript.rs` per data-model.md §10 — `blocks: Vec<TranscriptBlock>`, `content: String`, `thinking: String`; accumulates `ContentDelta`/`ToolCallStarted`/`ToolResult`/`Thinking` from `Bus<AgentEvent>` into ordered blocks (migration step 3)
- [ ] T012 [US1] Return `Vec<ToolSideEffect>` from `ToolExecutor::execute_all` in `src/desktop/src/agent/tool_executor.rs`; drop `tx_gui` parameter; delete `notify_file_creations` function (lines 160-215) (migration step 4, FR-006)
- [ ] T013 [US1] Publish `AgentEvent::ToolSideEffect` for each side effect from agent loop in `src/desktop/src/agent/agent_impl.rs` after `execute_all` returns (migration step 4)
- [ ] T014 [US1] Add `AgentEvent::ToolSideEffect(FileCreated { path, tags })` → `FsEvent::FileModified { path, tags }` reissue branch in `src/desktop/src/app/orchestrator.rs` drain loop — call `self.handle_fs_event(FsEvent::FileModified { path, tags })` (migration step 4, FR-007)
- [ ] T015 [US1] Switch orchestrator drain from `BackgroundEvent::Agent` to `Bus<AgentEvent>` reader in `src/desktop/src/app/orchestrator.rs` — route all agent events through the new bus; update `AgentState` mutations to use structured events; build `AgentTranscript` from `ContentDelta`/`ToolCallStarted`/`ToolResult` (migration step 5)
- [ ] T016 [US1] Remove dual-publish from `src/desktop/src/agent/agent_impl.rs` — publish on `Bus<AgentEvent>` only; old `tx_gui` sends deleted (migration step 5)
- [ ] T017 [US1] Write no-regression test: assert rendered transcript matches pre-refactor baseline for a prompt that triggers thinking + content + tool call in `src/desktop/tests/` or `src/desktop/src/ui/render/e2e_tests/` (quickstart scenario 3, SC-002)
- [ ] T018 [US1] Write file side-effect reissue test in `src/desktop/src/agent/tool_executor_tests.rs` (assert `execute_all` returns `Vec<ToolSideEffect>`, no `FsEvent` sent) and orchestrator drain test (assert `ToolSideEffect` → `FsEvent::FileModified` reissue) (quickstart scenario 4, SC-005)
- [ ] T019 [US1] Write broadcast lag handling test: flood > 8192 `ContentDelta` events, assert `BusReader` returns `Lagged(n)` and orchestrator emits truncation marker (quickstart scenario 5)

**Checkpoint**: User Story 1 fully functional — UI renders identically from `Bus<AgentEvent>`; file reindex works through typed path. This is the MVP.

---

## Phase 4: User Story 2 — Agent Layer Is Testable Without the UI (Priority: P2)

**Goal**: The `agent/` layer has zero UI imports, zero UI channel handles, zero UI widget state. It builds and its unit-test suite passes with no UI setup.

**Independent Test**: Grep `src/desktop/src/agent/` for `egui`/`tx_gui`/`FsEvent` → zero hits; run agent unit test with no UI crate (quickstart scenarios 1, 2).

**Depends on**: US1 (T015/T016 — UI must render from `Bus<AgentEvent>` before `tx_gui` can be deleted).

### Implementation for User Story 2

- [ ] T020 [US2] Split `AgentSessionManager` — extract `AgentPanelState` (show_results, show_debug_window, debug_search_text, debug_auto_scroll, command_input, scroll_to_id, active_session_id) to `src/desktop/src/ui/agent/panel_state.rs` per data-model.md §9; remove these fields from `AgentState` and `AgentSessionManager` (migration step 6, FR-013, SC-007)
- [ ] T021 [US2] Update `src/desktop/src/ui/panels/center.rs` to read `scroll_to_id` and `show_results` from `AgentPanelState` instead of `AgentState`/`AgentSessionManager`; update `apply_task_toggle` call site (line 213) to mutate `AgentTranscript.content` instead of `AgentState.response` (migration step 6, research.md §7)
- [ ] T022 [US2] Update `src/desktop/src/ui/agent_debug_window.rs` to read `show_debug_window`, `debug_search_text`, `debug_auto_scroll` from `AgentPanelState`; read `debug_entries` from `AgentState` (migration step 6)
- [ ] T023 [US2] Delete `tx_gui: Sender<BackgroundEvent>` and `current_response: String` fields from `AgentContext` in `src/desktop/src/agent/context.rs`; change `session_number: usize` → `session_id: Uuid` (migration step 7, FR-005/FR-011)
- [ ] T024 [US2] Remove `tx_gui` parameter from `execute_all` call site and all agent-loop helper functions (`emit_usage`, `handle_reasoning`, `handle_content`, `process_tool_results`, `emit_tool_results_debug`) in `src/desktop/src/agent/agent_impl.rs` — they publish on `Bus<AgentEvent>` only (migration step 7)
- [ ] T025 [US2] Restructure `WebDelegateResponse` in `src/desktop/src/agent/tools/dtos.rs` — replace `tool_call_trace: String` with `tool_calls: Vec<DelegateToolCall>`; add `DelegateToolCall { name, args, result }` struct with `Serialize, Debug, JsonSchema` derives (migration step 8, FR-014, data-model.md §11)
- [ ] T026 [US2] Update `src/desktop/src/agent/tools/web.rs` — replace `delegate_trace: String` accumulator (line 398) with `Vec<DelegateToolCall>` accumulator; remove `format_delegate_tool_call_message` import (line 6); build structured `DelegateToolCall` per sub-call (migration step 8)
- [ ] T027 [US2] Delete inline trace-append logic at `src/desktop/src/agent/agent_impl.rs:322-328` (the `if fn_name == "web_delegate" && ... trace.push_str(...)` block) — the trace is now structured data in the `ToolResult` payload (migration step 8, research.md §4, SC-006)
- [ ] T028 [US2] Remove `Agent(AgentEvent)` variant from `BackgroundEvent` enum and delete `From<AgentEvent> for BackgroundEvent` + `From<AgentDebugEntry> for BackgroundEvent` impls in `src/desktop/src/bus/events/typed.rs`; `BackgroundEvent` keeps `Fs`/`Process`/`McpAuth` variants (migration step 9)
- [ ] T029 [US2] Write agent isolation unit test in `src/desktop/src/agent/events_tests.rs` — create `Bus<AgentEvent>`, build `AgentContext` with `session_id: Uuid::new_v4()` + mock LLM, run `run_agent_inner`, drain `BusReader`, assert `SessionStarted → Status → [Thinking|ContentDelta|ToolCallStarted|ToolResult]* → Status(Done) → SessionFinished` with no UI crate/egui/`AppOrchestrator` (quickstart scenario 2, SC-001/SC-004)

**Checkpoint**: User Story 2 complete — `agent/` is UI-free. Verify: `rg -n "egui|tx_gui|Sender<BackgroundEvent>" src/desktop/src/agent/` → zero hits (quickstart scenario 1, INV-1/INV-2/INV-3).

---

## Phase 5: User Story 3 — UI Restyles Agent Output From Structured Data (Priority: P3)

**Goal**: The UI formats tool calls, tool results, thinking splits, and delegate traces from structured event data. A maintainer can change display formatting in the UI layer alone without touching `agent/`.

**Independent Test**: Change a tool-call display format in `ui/render/agent_render.rs` only; confirm rendered output changes with no agent-layer edit (quickstart scenario 8).

**Depends on**: US1 (T010/T011 — formatter moved + transcript view model exists) and US2 (T025/T026 — structured delegate trace exists).

### Implementation for User Story 3

- [ ] T030 [US3] Implement tool-call formatting from structured `AgentEvent::ToolCallStarted`/`ToolResult` in `src/desktop/src/ui/render/agent_render.rs` — format `name` + `args` (JSON) into markdown; replaces agent-side `format_tool_call_message`/`format_tool_result_message` (FR-012)
- [ ] T031 [US3] Implement `split_thinking_and_content` as a UI presentation-layer concern in `src/desktop/src/ui/render/agent_render.rs` — the agent emits raw `ContentDelta`; the UI splits on the `🤔` delimiter and renders thinking vs content separately (FR-012, AGENT-022 drift flagged)
- [ ] T032 [US3] Implement delegate trace formatting from `Vec<DelegateToolCall>` in `src/desktop/src/ui/render/agent_render.rs` — format each `DelegateToolCall { name, args, result }` into a `>>`-prefixed markdown group; replaces `format_delegate_tool_call_message` (FR-014, SC-006)
- [ ] T033 [US3] Write UI restyle test in `src/desktop/src/ui/render/e2e_tests/` — change tool-call display format in `agent_render.rs` only, assert rendered output changes, assert no `src/desktop/src/agent/` file modified (quickstart scenario 8, User Story 3 acceptance scenario 1)
- [ ] T034 [US3] Write structured delegate trace test — assert `WebDelegateResponse.tool_calls: Vec<DelegateToolCall>` renders from structured data; grep `src/desktop/src/agent/` for `tool_call_trace`/`format_delegate_tool_call_message` → zero hits (quickstart scenario 7, SC-006, INV-7)

**Checkpoint**: User Story 3 complete — presentation is UI-side. Verify: change a formatting rule in `ui/` only → output changes, no `agent/` edit.

---

## Phase 6: User Story 4 — Sessions Carry Identity and History Across Prompts (Priority: P3)

**Goal**: Each session carries a `Uuid` identity. Continuation prompts reuse history; new sessions reset it. The integer `session_counter` is gone.

**Independent Test**: Submit two prompts with the same `session_id` → second sees first's history; submit with a new `session_id` → history reset (quickstart scenario 6).

**Depends on**: US2 (T023 — `session_id: Uuid` on `AgentContext`).

### Implementation for User Story 4

- [ ] T035 [US4] Replace `session_counter: usize` on `AgentSessionManager` with `Uuid` session tracking in `src/desktop/src/agent/manager.rs` — driver keeps `HashMap<Uuid, SessionState>` for per-session history; `submit_prompt` sends `AgentPrompt` with `session_id` (migration step 10, FR-008/FR-009)
- [ ] T036 [US4] Remove `session: usize` field from `AgentDebugEntry` in `src/desktop/src/bus/events/debug.rs` — session identity lives on the enclosing `AgentEvent::DebugEntry { session_id }` variant; `turn: usize` stays (migration step 10, data-model.md §4)
- [ ] T037 [US4] Implement long-lived driver thread in `src/desktop/src/agent/manager.rs` — single thread owns `Receiver<AgentPrompt>`, blocks on `recv()`, processes prompts sequentially; eliminate the double-spawn at `agent_impl.rs:20-22` (research.md §3, migration step 10)
- [ ] T038 [US4] Write session continuity test in `src/desktop/src/agent/events_tests.rs` — two prompts same `session_id` → history carries over; new `session_id` → history reset (quickstart scenario 6, FR-009)

**Checkpoint**: User Story 4 complete — `Uuid` session identity works; `session_counter` and `AgentDebugEntry.session` gone.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Final cleanup, documentation, and full validation across all stories.

- [ ] T039 [P] Update `doc/technical-context/ARCHITECTURE_C4.md` to reflect the final agent↔UI seam (two channels, structured events, `AgentPanelState` split) — RUST-042
- [ ] T040 [P] Update `src/desktop/src/agent/SPEC.md` and `src/desktop/src/bus/events/` docs to reflect `AgentEvent` relocation and `BackgroundEvent` slimming — RUST-040 drift flag for AGENT-011/016/022
- [ ] T041 Run full quickstart.md validation — all 8 scenarios pass; run `rg -n "egui|tx_gui|Sender<BackgroundEvent>|FsEvent|tool_call_trace" src/desktop/src/agent/` → zero hits (INV-1 through INV-7); `cargo doc --no-deps --quiet` builds clean

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately. Tasks T001-T004 are `[P]` (parallel — different files, additive).
- **Foundational (Phase 2)**: Depends on Setup (T001 — `AgentEvent` type exists). BLOCKS all user stories. T005→T006→T007→T008→T009 sequential.
- **US1 (Phase 3)**: Depends on Foundational (Phase 2). T010→T011→T012→T013→T014→T015→T016 sequential; T017-T019 `[P]` (tests, different files) after T016.
- **US2 (Phase 4)**: Depends on US1 (T015/T016 — UI must render from `Bus<AgentEvent>` before `tx_gui` removed). T020→T021→T022 sequential (split manager); T023→T024 sequential (delete tx_gui); T025→T026→T027 sequential (restructure web delegate); T028 (remove variant); T029 test.
- **US3 (Phase 5)**: Depends on US1 (T010/T011) and US2 (T025/T026). T030-T032 `[P]` (different formatting functions, same file — but no inter-dependency); T033-T034 `[P]` (tests).
- **US4 (Phase 6)**: Depends on US2 (T023 — `session_id: Uuid` on `AgentContext`). T035→T036→T037 sequential; T038 test.
- **Polish (Phase 7)**: Depends on all user stories. T039-T040 `[P]`; T041 last.

### Migration Step Order (STRICT — FR-017, SC-008)

```
T001 (step 1) → T005-T009 (step 2) → T010-T011 (step 3) → T012-T014 (step 4)
→ T015-T016 (step 5) → T020-T022 (step 6) → T023-T024 (step 7)
→ T025-T027 (step 8) → T028 (step 9) → T035-T037 (step 10)
```

Each task MUST compile and pass the quality gate before the next. No skipping, no reordering. The `[P]` markers within a phase indicate tasks that touch different files and could theoretically overlap, but the strict migration order takes precedence.

### User Story Dependencies

- **US1 (P1)**: Depends on Foundational. No dependencies on other stories. **MVP**.
- **US2 (P2)**: Depends on US1 (cannot delete `tx_gui` until UI renders from bus). Independently testable (grep + isolation test).
- **US3 (P3)**: Depends on US1 (transcript view model) + US2 (structured delegate trace). Independently testable (restyle test).
- **US4 (P3)**: Depends on US2 (`session_id` field exists). Independently testable (continuity test).

### Parallel Opportunities

- **Phase 1**: T001-T004 all `[P]` (additive, different files).
- **Phase 3 (US1)**: T017-T019 `[P]` (tests after T016 completes).
- **Phase 5 (US3)**: T030-T032 `[P]` (formatting functions, no inter-dependency); T033-T034 `[P]` (tests).
- **Phase 7**: T039-T040 `[P]` (different doc files).

---

## Parallel Example: Phase 1 (Setup)

```bash
# Launch all Setup tasks together (different files, additive, no dependencies):
Task: "Create agent/events.rs with AgentEvent, AgentStatus, ToolSideEffect, AgentPrompt, DelegateToolCall"
Task: "Add Serialize derive to TokenUsageInfo in bus/events/messages.rs"
Task: "Re-export McpAuthEvent from bus/events/mod.rs"
Task: "Update ARCHITECTURE_C4.md with planned seam boundary"
```

## Parallel Example: Phase 5 (US3 — Formatting)

```bash
# After US1 (T010/T011) and US2 (T025/T026) complete:
Task: "Implement tool-call formatting from structured events in ui/render/agent_render.rs"
Task: "Implement split_thinking_and_content in UI in ui/render/agent_render.rs"
Task: "Implement delegate trace formatting from Vec<DelegateToolCall> in ui/render/agent_render.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001-T004)
2. Complete Phase 2: Foundational (T005-T009) — CRITICAL, blocks all stories
3. Complete Phase 3: User Story 1 (T010-T019) — the flip to `Bus<AgentEvent>`
4. **STOP and VALIDATE**: No-regression baseline test (T017); file reindex test (T018); lag test (T019)
5. At this point the app works identically to before, but from the new channel. Deploy/demo if ready.

### Incremental Delivery

1. Setup + Foundational → new channel runs in shadow (dual-publish)
2. US1 → UI renders from `Bus<AgentEvent>`; old `tx_gui` still exists but unused for rendering → **MVP**
3. US2 → `agent/` is UI-free; `tx_gui` deleted; `BackgroundEvent::Agent` removed → testable in isolation
4. US3 → presentation is UI-side; delegate trace structured → restyle without touching agent
5. US4 → `Uuid` session identity; `session_counter` gone → forward-compatible with N sessions
6. Polish → docs updated, full validation, all invariants verified

### Single-Developer Sequential Strategy

This refactor is strictly sequential (FR-017). A single developer executes:
T001 → T002 → ... → T041 in order, running the quality gate after each task.

### Quality Gate (after EVERY task)

```powershell
cd src/desktop
cargo check --quiet
cargo nextest run --status-level fail --show-progress none
cargo clippy -- -D warnings
cargo fmt --check
cargo doc --no-deps --quiet
```

All five MUST pass cleanly before proceeding to the next task (RUST quality gate, SC-008).

---

## Notes

- This is a **refactor**, not a greenfield feature. Tasks are migration steps, not new-feature builds.
- The migration is **strictly sequential** (FR-017, SC-008) — each step compiles and passes tests before the next. `[P]` markers are advisory; the migration order takes precedence.
- The 10 migration steps map to plan.md's migration plan; the 4 user stories map to spec.md's user scenarios.
- Key discrepancies from the proposal are resolved in research.md (notably: `strip_web_delegate_trace` does not exist — the inline trace-append at `agent_impl.rs:322-328` is deleted instead; `tx_gui` is `std::sync::mpsc::Sender` not tokio broadcast — this refactor changes the channel type and fixes an existing RUST-052 drift).
- `apply_task_toggle` stays defined in `markdown/document.rs:63` (correctly placed); only its target buffer changes from `AgentState.response` to `AgentTranscript.content` (research.md §7).
- After each task, commit with a message matching repo style. Do NOT combine steps into one commit.
