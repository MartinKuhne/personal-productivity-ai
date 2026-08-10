# Quickstart: Agent Loop / UI Seam Refactor

**Feature**: 003-agent-ui-seam-refactor
**Date**: 2026-08-10
**Phase**: 1

Runnable validation scenarios that prove the refactor works end-to-end. Each
scenario maps to spec acceptance criteria and can be run after the relevant
migration step(s) are complete.

**Prerequisites**:
- Rust toolchain (edition 2024)
- Working dir: `src/desktop/` (all commands run from here unless noted)
- Repo root: `C:\Users\mkuhn\src\ppai`
- An LLM API key configured (for live agent scenarios) OR the scenarios marked
  "mock" use the existing test fixtures in `agent_impl_tests.rs` /
  `agent_impl_tests.rs` sidecars.

**Quality gate** (run after every migration step — RUST quality gate,
`src/desktop/AGENTS.md:78-86`):

```powershell
cargo check --quiet
cargo nextest run --status-level fail --show-progress none
cargo clippy -- -D warnings
cargo fmt --check
cargo doc --no-deps --quiet
```

All five MUST pass cleanly before proceeding to the next step (FR-017, SC-008).

---

## Scenario 1: Agent layer compiles UI-free (INV-1, INV-2, SC-001)

**Validates**: FR-005 — the agent layer has no UI imports, no UI channel handles,
and builds in isolation.

**When to run**: after step 7 (delete `AgentContext::tx_gui`) and step 9 (remove
`BackgroundEvent::Agent`).

### Steps

1. From repo root, grep the agent tree for any UI-crate or UI-channel references:

   ```powershell
   # Should return zero hits for each:
   rg -n "egui|eframe" src/desktop/src/agent/
   rg -n "tx_gui|Sender<BackgroundEvent>" src/desktop/src/agent/
   rg -n "FsEvent" src/desktop/src/agent/tool_executor.rs
   ```

2. Build the crate (agent module is part of the single crate; a clean `cargo
   check` confirms the agent module compiles with no UI-crate references on its
   import path):

   ```powershell
   cargo check --quiet
   ```

### Expected outcome

- All three grep commands return **zero hits**.
- `cargo check` succeeds with no errors or warnings.

### What this proves

The agent layer is UI-free (User Story 2, acceptance scenario 2). No `egui`
imports, no `tx_gui` field, no `FsEvent` sends from the tool executor.

---

## Scenario 2: Agent-loop unit test with no UI setup (SC-004)

**Validates**: FR-005 — a maintainer can write a passing agent-loop unit test by
submitting one prompt and asserting on structured events, with no UI setup.

**When to run**: after step 5 (UI drains from `Bus<AgentEvent>`) and step 7
(delete `tx_gui`).

### Steps

1. Write a test (in `agent/events_tests.rs` or `agent_impl_tests.rs` sidecar)
   that:
   - Creates a `Bus<AgentEvent>` and subscribes a `BusReader`.
   - Builds an `AgentContext` with `session_id: Uuid::new_v4()`, a mock LLM
     client (reuse existing mock fixtures from `agent_impl_tests.rs`), and the
     `Bus<AgentEvent>` handle (no `tx_gui`, no UI).
   - Calls `run_agent_inner(ctx)` (or the driver entry).
   - Drains the `BusReader` and asserts the event sequence:
     `SessionStarted → Status(AwaitingLlm) → [Thinking|ContentDelta|ToolCallStarted|ToolResult]* → Status(Done) → SessionFinished`.

2. Run the test:

   ```powershell
   cargo nextest run agent::events_tests --status-level fail
   # or:
   cargo test --lib agent::events_tests::
   ```

### Expected outcome

- The test passes with **no UI crate, no egui context, no `AppOrchestrator`
  setup** — only `Bus<AgentEvent>` + `AgentContext` + mock LLM.
- The asserted event sequence matches the contract in
  [contracts/agent-seam.md](contracts/agent-seam.md) "Event ordering guarantees".

### What this proves

A maintainer can test the agent loop in isolation in under 5 minutes (SC-004,
User Story 2 acceptance scenario 1).

---

## Scenario 3: No-regression render baseline (SC-002)

**Validates**: FR-016 — end-users observe no regression in real-time agent output.
Thinking, content, tool calls, and results render identically to the
pre-refactor baseline.

**When to run**: after step 5 (UI renders from `Bus<AgentEvent>`) and before
step 9 (old path removed). This is the critical no-regression gate.

### Prerequisites

- A representative prompt set captured pre-refactor: at least one prompt that
  triggers thinking + content + a tool call (e.g. `create_note`) + a tool
  result. Save the rendered transcript (markdown text) as a baseline file.
- Existing snapshot/e2e render tests in `src/desktop/tests/` and
  `ui/render/e2e_tests/` (these assert on rendered output — they will need
  updating to assert on the structured transcript per spec Assumptions).

### Steps

1. Run the existing snapshot/e2e render tests (updated to assert on the
   structured transcript):

   ```powershell
   cargo nextest run --filter-expr 'test(snapshot)' --status-level fail
   cargo nextest run --filter-expr 'test(e2e)' --status-level fail
   ```

2. For a live check (requires LLM API key), run the app with
   `EGUI_INSPECTION=1` (RUST-030), submit the representative prompt, and
   visually confirm the rendered transcript matches the pre-refactor baseline:

   ```powershell
   $env:EGUI_INSPECTION=1; cargo run
   ```

3. Submit a prompt that creates a note via `create_note`; confirm the
   file/tag/tree tabs reindex (FR-007, User Story 1 acceptance scenario 4).

### Expected outcome

- All snapshot/e2e tests pass (after updating assertions to the structured
  transcript).
- Live: thinking, content, tool call (with args), and tool result appear in
  real-time in the same order and format as before the refactor (User Story 1,
  acceptance scenario 1).
- File creation triggers reindexing (acceptance scenario 4).

### What this proves

No observable regression for end-users (SC-002, User Story 1).

---

## Scenario 4: File side-effect reissues FsEvent (SC-005, FR-007)

**Validates**: file creation by a tool triggers UI reindexing through the typed
side-effect path — no back-channel `FsEvent` send from the tool layer.

**When to run**: after step 4 (`execute_all` returns `Vec<ToolSideEffect>`) and
step 5 (UI reissues from `AgentEvent::ToolSideEffect`).

### Steps

1. Write a unit test in `tool_executor_tests.rs` (sidecar, RUST-001) that:
   - Calls `execute_all` with a `create_note` tool call (mocked file write).
   - Asserts the returned `Vec<ToolSideEffect>` contains
     `ToolSideEffect::FileCreated { path, tags }` with the correct path/tags.
   - Asserts `execute_all` did **not** send any `FsEvent` (no `tx_gui` parameter
     exists — INV-3).

2. Write an orchestrator-drain test that:
   - Publishes `AgentEvent::ToolSideEffect(FileCreated { path, tags })` on the
     `Bus<AgentEvent>`.
   - Asserts `drain_background_channel` calls `handle_fs_event(FsEvent::FileModified
     { path, tags })` (verify via a spy/counter on `handle_fs_event` or the
     resulting `TagManager`/`DirectoryTracker` state).

3. Run:

   ```powershell
   cargo nextest run tool_executor --status-level fail
   cargo nextest run orchestrator --status-level fail
   ```

### Expected outcome

- `execute_all` returns the side effect as data; no `FsEvent` is sent from the
  tool layer.
- The orchestrator reissues `FsEvent::FileModified` from the
  `ToolSideEffect` event, driving the same reindex path as before.

### What this proves

The `create_note` → reindex path is explicit and type-safe (SC-005); the tool
layer no longer reaches into the UI channel (FR-006).

---

## Scenario 5: Broadcast lag is handled, not silent (Edge Case)

**Validates**: the spec Edge Case "agent publishes events faster than the UI
drains them" — a lagging subscriber sees a visible marker, not silent data loss.

**When to run**: after step 5 (UI drains from `Bus<AgentEvent>`).

### Steps

1. Write a test that:
   - Creates a `Bus<AgentEvent>` (capacity 8192).
   - Publishes > 8192 `ContentDelta` events without any subscriber draining.
   - Subscribes a `BusReader` **after** publishing.
   - Drains and asserts the reader sees a `Lagged(n)` result (not silent
     `Ok`s with missing events).
   - Asserts that the orchestrator's lag-handling branch emits a visible
     truncation marker into the transcript (per research.md §1).

2. Run:

   ```powershell
   cargo nextest run --filter-expr 'test(lag)' --status-level fail
   ```

### Expected outcome

- The `BusReader` returns `Lagged(n)` with `n > 0`.
- The orchestrator does not silently continue; it emits a visible marker and
  re-syncs on the next `SessionStarted`/`Status` boundary.

### What this proves

The broadcast-drop concern (research.md §1, plan.md Technical Context NEEDS
CLARIFICATION) is resolved — lag is detected and surfaced, not silently lost.

---

## Scenario 6: Session identity continuity (FR-008, FR-009)

**Validates**: continuing a session reuses history; a new session resets it.

**When to run**: after step 10 (Uuid session identity).

### Steps

1. Write a test that:
   - Submits two `AgentPrompt`s with the **same** `session_id`.
   - Asserts the agent driver reuses the first session's `history` for the
     second prompt (e.g. the second turn's LLM call includes the first turn's
     messages).
   - Submits a third `AgentPrompt` with a **new** `session_id`.
   - Asserts history is empty/reset for the new session.

2. Run:

   ```powershell
   cargo nextest run --filter-expr 'test(session_continuity)' --status-level fail
   ```

### Expected outcome

- Same `session_id` → history carries over (User Story 4, acceptance scenario 1).
- New `session_id` → history reset (acceptance scenario 2).

### What this proves

Session continuity works with `Uuid` identity (FR-008, FR-009); the integer
`session_counter` is gone.

---

## Scenario 7: Web-delegate trace is structured (SC-006, FR-014)

**Validates**: the web-delegate trace reaches the model as structured data; no
string-stripping workaround.

**When to run**: after step 8 (restructure `WebDelegateResponse`).

### Steps

1. Grep for any trace-string handling in the agent:

   ```powershell
   rg -n "tool_call_trace|strip_web_delegate|format_delegate_tool_call_message" src/desktop/src/agent/
   ```

2. Inspect `tools/dtos.rs` — confirm `WebDelegateResponse` has
   `tool_calls: Vec<DelegateToolCall>`, no `tool_call_trace: String` (INV-7).

3. Write a test that runs a `web_delegate` tool call (mocked sub-calls) and
   asserts the `ToolResult.result` JSON contains `tool_calls: [...]` (structured
   array), and that the agent loop does **not** append any trace string to
   content.

4. Run:

   ```powershell
   cargo nextest run web_delegate --status-level fail
   ```

### Expected outcome

- Grep returns zero hits for `tool_call_trace` / `strip_web_delegate` /
   `format_delegate_tool_call_message` in `agent/`.
- `WebDelegateResponse` has the structured `tool_calls` field.
- The LLM-bound result payload is structured (SC-006).

### What this proves

The delegate trace is structured per-call data; no string-injection or
stripping (User Story 3, acceptance scenario 3).

---

## Scenario 8: UI restyles tool calls without touching agent (FR-012, SC-007)

**Validates**: a maintainer changes tool-call display format in the UI layer
alone; the rendered output changes with no agent-layer edit.

**When to run**: after step 3 (move `response_formatter` to `ui/`) and step 6
(split `AgentPanelState`).

### Steps

1. Change a display formatting rule in the UI layer only (e.g. in
   `ui/render/agent_render.rs` or the transcript view model — change how
   `ToolCallStarted` args are rendered).
2. Rebuild and run:

   ```powershell
   cargo check --quiet
   cargo run
   ```

3. Submit a prompt that triggers a tool call; confirm the rendered tool-call
   format changed.

### Expected outcome

- The agent layer was not edited or recompiled (no `agent/` file touched).
- The rendered tool-call format reflects the UI-only change (User Story 3,
  acceptance scenario 1).

### What this proves

Presentation is a UI-side concern (FR-012); the seam is real.

---

## References

- [spec.md](spec.md) — acceptance scenarios, success criteria
- [plan.md](plan.md) — technical context, migration steps
- [research.md](research.md) — design decisions (broadcast-drop, thread model, etc.)
- [data-model.md](data-model.md) — entity definitions
- [contracts/agent-seam.md](contracts/agent-seam.md) — AgentPrompt + AgentEvent contract
