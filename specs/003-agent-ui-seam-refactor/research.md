# Research: Agent Loop / UI Seam Refactor

**Feature**: 003-agent-ui-seam-refactor
**Date**: 2026-08-10
**Phase**: 0

Resolves all NEEDS CLARIFICATION from [plan.md](plan.md) Technical Context and
documents the key design decisions with rationale and alternatives considered.

Grounded in the actual codebase inventory (see plan.md Technical Context and the
exploration findings). All file paths/line numbers reference
`src/desktop/src/`.

---

## 1. Broadcast-drop semantics for a lagging UI subscriber (RESOLVED)

### The question

`plan.md` Technical Context flagged: the proposed `Bus<AgentEvent>` is backed by
`tokio::sync::broadcast` (capacity 8192, `bus/core.rs:17`). Unlike the current
`std::sync::mpsc::Sender<BackgroundEvent>` (which never drops — `send` only
fails if the receiver is gone), tokio broadcast **silently drops old messages
for lagging subscribers**. A `BusReader` that falls behind sees a
`RecvError::Lagged(n)` and the oldest `n` messages are lost. If the UI drains
once per frame (~16 ms at 60 fps) and the agent emits a burst of
`ContentDelta`/`ToolCallStarted`/`ToolResult` events faster than the UI can
process for several frames, agent events could be lost — a regression vs. the
current mpsc path which backpressures (blocks the agent thread on `send`).

### Decision

**Keep `Bus<AgentEvent>` (tokio broadcast, capacity 8192), and handle lag
explicitly on the UI side.** Do not fall back to mpsc.

Rationale:
1. **RUST-052 mandates `Bus<T>` broadcast** for background→UI fan-out. The
   current `tx_gui: mpsc::Sender<BackgroundEvent>` is itself a drift from this
   rule; the refactor fixes it. Reverting to mpsc would preserve the drift.
2. **Capacity 8192 is large.** A single agent turn emits on the order of 10-50
   events (a handful of `Thinking`/`Status`/`DebugEntry` + one `ContentDelta`
   per streaming chunk + a few `ToolCallStarted`/`ToolResult`). At 50
   events/turn, the buffer holds ~160 turns of backlog — far more than a UI
   frame's worth. Lag only occurs if the UI thread is blocked for seconds,
   which would already be a UI-freeze bug (a FR-001 violation) independent of
   the channel choice.
3. **The agent thread is already decoupled** — it runs on a background thread
   (`std::thread::spawn`, `manager.rs:352`). Backpressure on mpsc would block
   the *agent* thread, not the UI thread. Blocking the agent to wait for a slow
   UI is the wrong direction: it would stall LLM streaming and tool execution
   because the UI can't keep up rendering. Broadcast's drop-old behavior is
   preferable to agent-thread backpressure.
4. **The content is reconstructable.** `ContentDelta` events are additive
   deltas; the UI accumulates them into a transcript. A dropped `ContentDelta`
   would leave a gap in the rendered text. To make this detectable rather than
   silent, the UI's `BusReader<AgentEvent>::try_recv` MUST check for
   `Lagged` and, on lag, emit a visible marker (e.g. `[output truncated — UI
   fell behind the agent]`) and re-sync on the next `SessionStarted` or
   `Status` boundary. This turns a silent data-loss bug into a visible
   degradation that is far less harmful than a frozen UI or stalled agent.

### Alternatives considered

- **Keep `std::sync::mpsc::Sender<AgentEvent>`** (no broadcast). Rejected: violates
  RUST-052, and mpsc is single-consumer — the debug window, transcript
  accumulator, and file-event reissue branch would all have to share one drain
  point (which is what `drain_background_channel` does today, but it means every
  consumer is coupled to the orchestrator's single `rx`). Broadcast lets each
  consumer subscribe independently, which is the architectural point of the
  refactor.
- **Use `tokio::sync::mpsc` (async mpsc, unbounded).** Rejected: same
  single-consumer limitation, and introduces async into the UI drain path
  (currently sync `try_recv` in `drain_background_channel`,
  `orchestrator.rs:277`).
- **Increase capacity to e.g. 65536.** Rejected as primary mitigation: it
  masks the problem rather than solving it. 8192 is already generous; if the UI
  falls behind by 8192 events, something is fundamentally wrong (UI freeze).
  The explicit lag-handling is the real fix. (If profiling later shows 8192 is
  genuinely tight under heavy tool-call fanout, bumping the constant in
  `bus/core.rs:17` is a one-line change — but that's a tuning decision, not a
  design decision.)

### Implication for tasks

- The orchestrator drain loop (`drain_background_channel`,
  `orchestrator.rs:276-296`) MUST handle `BusReader::try_recv` returning a
  lagged error, not just `Ok`/`Empty`. Today's mpsc `try_recv` never returns
  lagged.
- A failing test MUST be written that floods a `Bus<AgentEvent>` faster than a
  dummy reader drains, and asserts the reader sees a lag marker rather than
  silently losing content (FR coverage for the edge case in spec.md "Edge
  Cases").

---

## 2. `AgentEvent` type ownership location (RESOLVED)

### Decision

`AgentEvent`, `AgentStatus`, `ToolSideEffect`, and `AgentPrompt` are defined in
**`src/desktop/src/agent/events.rs`** (new file, agent-owned). `Bus<AgentEvent>`
(the transport) stays generic in `bus/core.rs`. `BackgroundEvent`
(`bus/events/typed.rs`) loses its `Agent(AgentEvent)` variant.

### Rationale

- RUST-050 (bounded subsystems): the agent subsystem owns its own event
  payload types. Today `AgentEvent` lives in `bus/events/typed.rs:22-34` alongside
  `FsEvent`/`ProcessEvent`/`McpAuthEvent` — a bus-module god-enum. Moving it to
  `agent/events.rs` puts the type where the producer lives.
- `Bus<T>` is generic (`bus/core.rs:24-27`); it doesn't need the type to live in
  `bus/`. The UI subscribes via `bus.subscribe::<AgentEvent>()` →
  `BusReader<AgentEvent>`, importing the type from `agent::events`.
- This does **not** create a circular dependency: `agent/` already imports
  `bus::core::Bus` (for `file_event_bus: Bus<FileEvent>` on `AgentContext`,
  `context.rs:21`). `ui/` already imports both `agent` and `bus`. No new
  dependency edges.

### Alternatives considered

- **Keep `AgentEvent` in `bus/events/typed.rs`, just add `session_id` to each
  variant.** Rejected: leaves the agent's domain events in the bus module,
  perpetuating the god-enum and the `BackgroundEvent::Agent(AgentEvent)`
  wrapping. The refactor's point is that the agent owns its events.
- **Put `AgentEvent` in a new shared `events/` crate/module.** Rejected:
  over-engineering for a single-crate app. `agent/events.rs` is sufficient.

---

## 3. Thread model: long-lived driver thread (RESOLVED)

### Current state (discrepancy noted)

`AgentSessionManager::start_session` (`manager.rs:352`) spawns a thread that
calls `run_agent(ctx)`, and `run_agent` (`agent_impl.rs:20-22`) **immediately
spawns another thread** via `std::thread::spawn(move || run_agent_inner(ctx))`.
So today the agent runs on a double-spawned thread — the outer thread
(`start_session`'s spawn) does nothing but spawn the inner thread and exit.

### Decision

Replace the spawn-per-prompt flow with a **single long-lived driver thread** that
owns `Receiver<AgentPrompt>` and processes prompts sequentially. When a prompt
arrives, the driver builds a per-session `AgentContext` and runs
`run_agent_inner(ctx)` inline (on the driver thread itself). No double-spawn.

### Rationale

- The proposal specifies "a long-lived driver thread that pulls prompts from the
  receiver."
- Today only one session is active at a time (spec Assumption: "today only one
  agent session is active at a time"). Sequential processing on one driver
  thread preserves this. The `pending_prompts: Vec<String>` queue
  (`AgentState`, `manager.rs:38`) is replaced by the mpsc channel's own buffer.
- Eliminating the double-spawn is a cleanup that falls out of the refactor —
  there's no reason for two threads when one suffices.
- The driver thread is spawned once (at app startup, in `AgentSessionManager::new`
  or a new `start_driver` method) and lives for the app's lifetime. It blocks on
  `Receiver<AgentPrompt>::recv()`.

### Alternatives considered

- **Keep spawn-per-prompt** but feed via `AgentPrompt` channel. Rejected: the
  spawn-per-prompt model is what creates the need to clone shared resources
  (`config`, `tool_manager`, `browser_session`, `pdf_backing`, `uuid_gen`) into
  each `AgentContext` from the orchestrator. A long-lived driver owns them once.
- **Thread pool for concurrent sessions.** Rejected: out of scope (spec
  Assumption: concurrent multi-session UI is out of scope). The types are
  forward-compatible with N sessions (session_id-tagged events), but the driver
  processes one at a time.

### Implication for tasks

- `AgentSessionManager` gains a `start_driver(&mut self)` (or the driver is
  spawned in `new`) that takes ownership of the `Receiver<AgentPrompt>` and a
  cloned `Bus<AgentEvent>` handle.
- `start_session` (the old spawn-per-prompt entry, `manager.rs:313-355`) is
  replaced by `submit_prompt(prompt: AgentPrompt)` which sends on the
  `Sender<AgentPrompt>`.
- `run_agent` (`agent_impl.rs:20-22`) loses its inner `std::thread::spawn`;
  `run_agent_inner` is called directly by the driver.

---

## 4. `strip_web_delegate_trace` does not exist (DISCREPANCY RESOLVED)

### Finding

The proposal references a `strip_web_delegate_trace` workaround in
`agent_impl.rs` that would be deleted by the refactor. **No such function
exists.** The equivalent behavior is **inline** in `process_tool_results` at
`agent_impl.rs:322-328`:

```rust
if fn_name == "web_delegate"
    && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&result)
    && let Some(trace) = parsed.get("tool_call_trace").and_then(|t| t.as_str())
    && !trace.is_empty()
{
    full_response.push_str(trace);
}
```

It **appends** the trace verbatim to `full_response` (it does not strip
anything). The trace string is produced in `tools/web.rs:477-479` via
`format_delegate_tool_call_message` and stored as `WebDelegateResponse.tool_call_trace: String`
(`tools/dtos.rs:489-492`).

### Decision

- The inline trace-append logic at `agent_impl.rs:322-328` is **deleted** as
  part of step 8 (restructure `WebDelegateResponse`).
- `WebDelegateResponse.tool_call_trace: String` is replaced by
  `tool_calls: Vec<DelegateToolCall>` where `DelegateToolCall { name, args,
  result }` carries structured per-call data.
- `tools/web.rs` stops calling `format_delegate_tool_call_message` (the
  `delegate_trace: String` accumulator at `web.rs:398` is replaced by a
  `Vec<DelegateToolCall>` accumulator).
- The agent emits each delegate sub-call as a `ToolCallStarted`/`ToolResult`
  event (or the UI formats the structured `Vec<DelegateToolCall>` from the
  `ToolResult` payload — see decision §5).
- The LLM-bound tool result payload contains `tool_calls: Vec<DelegateToolCall>`
  (structured, serializable) — no pre-formatted string to strip or inject.

### Rationale

The proposal's intent (no string-formatting in the agent, no string-stripping
before the LLM) is achieved. The exact mechanism differs only because the
"workaround" was inline rather than a named helper.

---

## 5. Delegate sub-call events vs. structured `ToolResult` payload (RESOLVED)

### The question

When `web_delegate` runs N sub-agent tool calls internally, should the agent
loop emit N separate `AgentEvent::ToolCallStarted`/`ToolResult` events for the
sub-calls, or should it emit one `ToolResult` for `web_delegate` whose payload
contains the structured `Vec<DelegateToolCall>` trace?

### Decision

**Emit one `AgentEvent::ToolResult` for `web_delegate` whose `result:
serde_json::Value` contains the `tool_calls: Vec<DelegateToolCall>` array.** Do
not emit separate sub-call events on the bus.

### Rationale

- The sub-calls are an internal implementation detail of `web_delegate`. The
  LLM sees one tool result (`web_delegate`'s output). The agent loop calls
  `execute_all` once per turn; each top-level tool call gets one
  `ToolCallStarted` + one `ToolResult`. Mirroring that on the bus keeps the
  invariant "one `ToolCallStarted` ↔ one `ToolResult` per `execute_all` entry."
- The UI formats the structured trace from the `ToolResult` payload (User Story
  3, acceptance scenario 3). This is the "UI restyles from structured data"
  property — the UI reads `result.tool_calls` and formats each sub-call.
- Emitting sub-call events would require the agent loop to reach inside
  `web_delegate`'s execution, re-coupling the loop to a specific tool's
  internals.

### Alternatives considered

- **Emit N sub-call events.** Rejected (above). Would also complicate the
  transcript view model (sub-call events would need a parent-call id to group
  them).

---

## 6. `TokenUsageInfo` Serialize (RESOLVED)

### Finding

`TokenUsageInfo` (`bus/events/messages.rs:28-46`) derives `Debug, Clone,
Default, serde::Deserialize` but **not `Serialize`**. It's carried by-value in
`AgentEvent::TokenUsage(TokenUsageInfo)` (`typed.rs:30`).

### Decision

**Add `Serialize` to `TokenUsageInfo`'s derive list** when moving `AgentEvent`
to `agent/events.rs`. Also add `Serialize` to `AgentEvent` and `AgentStatus`
derives.

### Rationale

- `AgentDebugEntry` already derives `Serialize` (`debug.rs:27`) — debug entries
  are serialized for the debug window's JSON view. If `AgentEvent::DebugEntry`
  or `AgentEvent::TokenUsage` ever needs to be serialized (snapshot tests,
  debug-record persistence, logging), the payload must be serializable.
- The `serde` feature is already on `uuid` (`Cargo.toml:83`), so `Uuid` in
  every `AgentEvent` variant is serializable.
- Trivial change (one derive), prevents a future blocker.

---

## 7. `AgentState::scroll_to_id` and `apply_task_toggle` (RESOLVED)

### Finding

- `AgentState::scroll_to_id: Option<String>` (`manager.rs:31`, docstring
  `manager.rs:28-30` references `egui::Id`). It is mutated by the UI
  (`ui/panels/center.rs:205`, passed as `&mut agent.state_mut().scroll_to_id` to
  `render_markdown`). The agent never reads it back — pure UI state parked on
  the agent struct.
- `apply_task_toggle` is **defined in `markdown/document.rs:63`**
  (`pub fn apply_task_toggle(markdown: &mut String, task_index: usize, checked:
  bool)`) and re-exported from `ui/render/mod.rs:33`. The call site is
  `ui/panels/center.rs:211-218`, which mutates `agent.state_mut().response`.

### Decision

- `scroll_to_id` moves to `AgentPanelState` (UI-owned). `AgentState` loses the
  field. The `egui::Id` docstring reference in `manager.rs:30` is deleted — no
  egui mention in `agent/`.
- `apply_task_toggle` stays defined in `markdown/document.rs` (it's a markdown
  operation, correctly placed). Its call site in `center.rs` changes target:
  instead of `&mut agent.state_mut().response`, it mutates the UI transcript
  view model's content buffer. The function signature is unchanged (it takes
  `&mut String`); only the `&mut String` source changes.

### Rationale

- `scroll_to_id` is UI scroll state; it belongs in `AgentPanelState` (FR-013,
  SC-007).
- `apply_task_toggle` is a markdown mutation, not agent or UI state — leaving it
  in `markdown/` is correct per RUST-050. The refactor only changes which buffer
  it mutates (the UI view model's buffer, not the agent's `response` buffer).

---

## 8. Dual-publish migration safety (RESOLVED)

### The question

Migration step 2 (plan) says "add `Bus<AgentEvent>` alongside `tx_gui`; publish
on both channels during transition." If the UI drains both, it would process
every event twice (once via `BackgroundEvent::Agent`, once via
`Bus<AgentEvent>`), causing duplicated rendering.

### Decision

**During dual-publish (steps 2-4), the UI drains ONLY the old
`BackgroundEvent::Agent` path.** The new `Bus<AgentEvent>` is subscribed but its
reader is not yet wired into the transcript render path — it is only asserted-on
in tests (events arrive in the expected order/shape). Step 5 flips the drain:
the UI switches to the `Bus<AgentEvent>` reader and stops handling
`BackgroundEvent::Agent`. Step 9 deletes the old `BackgroundEvent::Agent`
variant.

### Rationale

- This is the standard "strangler fig" migration: new path runs in shadow
  (asserted but not rendered), old path stays authoritative, then the flip.
- At no point does the UI render from both channels simultaneously.
- Each step remains independently testable (FR-017): step 2's test asserts the
  new bus receives the same events the old mpsc does; step 5's test asserts the
  UI renders identically from the new bus alone.

---

## 9. `McpAuthEvent` re-export (MINOR, RESOLVED)

### Finding

`McpAuthEvent` is defined in `bus/events/typed.rs:77-85` but **not re-exported**
from `bus/events/mod.rs` (only reachable as
`crate::bus::events::typed::McpAuthEvent`). The orchestrator imports it
(`app/orchestrator.rs:9`).

### Decision

When `BackgroundEvent` loses its `Agent` variant (step 9), add `McpAuthEvent` to
the `bus/events/mod.rs` re-export block for consistency. This is a one-line
cleanup, not a behavioral change.

---

## Summary of resolved NEEDS CLARIFICATION

| # | Question | Decision |
|---|----------|----------|
| 1 | Broadcast drop for lagging UI | Keep `Bus<AgentEvent>`; handle `Lagged` explicitly with visible marker + re-sync |
| 2 | `AgentEvent` ownership location | `agent/events.rs` (agent-owned); `BackgroundEvent` loses `Agent` variant |
| 3 | Thread model | Single long-lived driver thread; no double-spawn |
| 4 | `strip_web_delegate_trace` absent | Delete inline trace-append (`agent_impl.rs:322-328`); restructure `WebDelegateResponse` |
| 5 | Delegate sub-call events | One `ToolResult` with structured `Vec<DelegateToolCall>` payload; no sub-call events |
| 6 | `TokenUsageInfo` Serialize | Add `Serialize` derive |
| 7 | `scroll_to_id` / `apply_task_toggle` | `scroll_to_id` → `AgentPanelState`; `apply_task_toggle` stays in `markdown/`, target buffer changes |
| 8 | Dual-publish safety | UI drains old path only during steps 2-4; flip at step 5 |
| 9 | `McpAuthEvent` re-export | Add to `bus/events/mod.rs` re-export when touching the file |

All NEEDS CLARIFICATION resolved. No outstanding unknowns for Phase 1.
