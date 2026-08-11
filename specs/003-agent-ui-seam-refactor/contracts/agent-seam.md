# Contract: Agent Seam (AgentPrompt + AgentEvent)

**Feature**: 003-agent-ui-seam-refactor
**Date**: 2026-08-10
**Phase**: 1

The contract between the agent layer and the UI layer of `fastmd`. This is an
**internal module-boundary contract** (not an external API) — it defines the two
channels and the structured payloads that cross the seam.

After the refactor, `agent/` imports nothing from `ui/`, and `ui/` reaches the
agent only through:
1. `Sender<AgentPrompt>` — submit prompts (UI → agent)
2. `BusReader<AgentEvent>` — receive structured events (agent → UI)
3. `AgentSessionManager` public accessors — read agent domain state
   (`AgentState.debug_entries`, `AgentState.running`) for the debug window

No `Sender<BackgroundEvent>` (`tx_gui`) handle crosses into `agent/`. No
`FsEvent` is sent from the tool layer. The agent layer is buildable and
unit-testable with no UI dependency (FR-005, SC-001).

---

## Input contract: `AgentPrompt`

**Transport**: `std::sync::mpsc::Sender<AgentPrompt>` (owned by the UI/orchestrator;
the agent driver owns the `Receiver<AgentPrompt>`).

**Type**: `agent::events::AgentPrompt` (see [data-model.md](../data-model.md) §1)

```rust
pub struct AgentPrompt {
    pub session_id: Uuid,
    pub text: String,
    pub active_file: Option<PathBuf>,
    pub active_dir: Option<PathBuf>,
    pub selected_files: HashSet<PathBuf>,
}
```

### Sender obligations (UI side)

- **Mint `session_id`**: `Uuid::new_v4()` for a new session; reuse the same
  `Uuid` for continuation prompts in the same session (FR-008).
- **Non-empty `text`**: the sender SHOULD NOT send empty/whitespace-only prompts.
  The agent rejects them (spec Edge Case), but the UI should guard at the input
  layer.
- **Thread safety**: `Sender<AgentPrompt>` is `Send`; the UI may call
  `submit_prompt` from the UI thread (it is — `AppOrchestrator::start_agent_session`
  → `prompt_tx.send(...)`).
- **Backpressure**: `mpsc::Sender::send` blocks if the channel buffer is full.
  The buffer should be large enough (or unbounded) that a queued prompt is never
  lost under normal single-session use. Today's `pending_prompts: Vec<String>`
  queue is replaced by this channel's buffer.

### Receiver obligations (agent driver side)

- **Block on `recv()`**: the driver thread blocks on
  `Receiver<AgentPrompt>::recv()` until a prompt arrives.
- **New session_id**: emit `AgentEvent::SessionStarted { session_id }`; reset
  history for that session (FR-009); create a `SessionState` entry in the
  driver's `HashMap<Uuid, SessionState>`.
- **Known session_id**: continue that session — reuse its `history`, append to
  its transcript (FR-009).
- **Sequential processing**: process one prompt to completion before the next
  (today: one active session; the types are forward-compatible with N).
- **Cancel**: if `cancel_flag` is set, stop the in-progress turn, emit
  `AgentEvent::SessionFinished { session_id }` (FR-015).

### Validation

| Rule | Enforced by | On violation |
|------|-------------|--------------|
| `text.trim()` non-empty | Agent driver | Reject; emit `AgentEvent::Failed { session_id, error: "Empty prompt" }` or `Status(Done)` without a turn |
| `session_id` is a valid `Uuid` | Type system (`Uuid`) | Cannot violate (invariant) |
| `active_file` is absolute (if `Some`) | UI selection layer | Agent does not re-validate; tools resolve paths |

---

## Output contract: `AgentEvent`

**Transport**: `Bus<AgentEvent>` (tokio broadcast, capacity 8192,
`bus::core.rs:17`). The agent owns the `Bus<AgentEvent>` handle (cloned into the
driver thread); the UI subscribes via `bus.subscribe()` → `BusReader<AgentEvent>`
and drains once per frame in `AppOrchestrator::drain_background_channel`.

**Type**: `agent::events::AgentEvent` (see [data-model.md](../data-model.md) §2)

```rust
#[derive(Debug, Clone, Serialize)]
pub enum AgentEvent {
    SessionStarted { session_id: Uuid },
    SessionFinished { session_id: Uuid },
    Status { session_id: Uuid, status: AgentStatus },
    Thinking { session_id: Uuid, text: String },
    ContentDelta { session_id: Uuid, text: String },
    ToolCallStarted { session_id: Uuid, id: String, name: String, args: serde_json::Value },
    ToolResult { session_id: Uuid, id: String, name: String, result: serde_json::Value },
    ToolSideEffect { session_id: Uuid, effect: ToolSideEffect },
    DebugEntry { session_id: Uuid, entry: AgentDebugEntry },
    TokenUsage { session_id: Uuid, usage: TokenUsageInfo },
    Failed { session_id: Uuid, error: String },
}
```

### Publisher obligations (agent side)

- **Tag every event with `session_id`**: the UI routes events to the correct
  session transcript by `session_id` (FR-003). No event is untagged.
- **Session lifecycle ordering**: `SessionStarted` MUST be the first event for a
  given `session_id`; `SessionFinished` MUST be the last (after `Done`/`Failed`
  or cancel). No events for that `session_id` after `SessionFinished`.
- **ContentDelta**: emit incremental content chunks (not a running buffer). The
  agent MUST NOT accumulate and resend `full_response` (FR-011, SC-003). Each
  chunk is emitted once.
- **ToolCallStarted ↔ ToolResult pairing**: for each tool call, emit
  `ToolCallStarted` (with `id`, `name`, `args`) before the corresponding
  `ToolResult` (with matching `id`). One `ToolCallStarted` ↔ one `ToolResult`.
- **ToolSideEffect**: emit once per successful side-effecting tool execution
  (e.g. `create_note` that creates a file). Not emitted on failure (spec Edge
  Case). The agent receives `Vec<ToolSideEffect>` from `execute_all` and
  publishes each.
- **No `FsEvent` sends**: the agent and tool layer MUST NOT send `FsEvent`
  directly (FR-006, SC-005). File-event reissue is a UI-side concern.
- **Publishing is non-blocking**: `Bus::publish` does not block (tokio
  broadcast). If no subscriber is listening, the event is dropped (acceptable —
  the UI subscribes at startup). If a subscriber lags, old events are dropped
  for it (see Lag handling below).

### Subscriber obligations (UI side)

- **Subscribe at startup**: `AppOrchestrator` creates a `BusReader<AgentEvent>`
  during init and drains it every frame in `drain_background_channel`
  (`ui/app/update.rs:52`).
- **Route by `session_id`**: maintain `active_session_id` on `AgentPanelState`;
  accumulate events for that session into `AgentTranscript` (FR-010). Events for
  other sessions are ignored (today: only one active session).
- **Reissue `ToolSideEffect` as `FsEvent`**: on `AgentEvent::ToolSideEffect(FileCreated
  { path, tags })`, call `self.handle_fs_event(FsEvent::FileModified { path, tags })`
  (FR-007). This replaces the old `notify_file_creations` direct send
  (`tool_executor.rs:202-208`).
- **Lag handling**: `BusReader::try_recv` may return `Lagged(n)` (research.md
  §1). On lag, emit a visible marker in the transcript
  (`[output truncated — UI fell behind the agent]`), log the lag count, and
  re-sync on the next `SessionStarted` or `Status` boundary. Do NOT silently
  continue (a dropped `ContentDelta` would corrupt the rendered text).

### Event ordering guarantees

Per session (invariant):

```
SessionStarted
  [Status(AwaitingLlm)]
  [Thinking]*
  [ContentDelta | ToolCallStarted ToolResult | DebugEntry | TokenUsage]*
  [Status(ExecutingTools)] [ToolCallStarted ToolResult]* [Status(AwaitingLlm)] ...
  [Status(Done) | Failed]
SessionFinished
```

- `Thinking`, `ContentDelta`, `ToolCallStarted`, `ToolResult`, `DebugEntry`,
  `TokenUsage` appear in emission order (the order the agent loop produces
  them).
- `Status` events bracket the phases (`AwaitingLlm` at turn start,
  `ExecutingTools` before tool calls, `Done` at normal end).
- `ToolSideEffect` follows the `ToolResult` that produced it.
- `SessionFinished` is always the last event for a `session_id`.

### `ToolResult.result` payload shape

For most tools: the tool's JSON return value (as today, but carried as a
`serde_json::Value` on the structured event rather than a string in
`full_response`).

For `web_delegate` specifically (FR-014, research.md §5):

```json
{
  "result": "summary text",
  "tool_calls": [
    { "name": "browser_navigate", "args": { "url": "..." }, "result": { "..." } },
    { "name": "browser_click", "args": { "selector": "..." }, "result": { "..." } }
  ]
}
```

The UI formats this structured array (not a pre-formatted `tool_call_trace`
string). No `strip_web_delegate_trace` / inline trace-append logic in the agent
(SC-006).

---

## Quality-gate invariants (verifiable)

| ID | Invariant | How verified |
|----|-----------|--------------|
| INV-1 | `agent/` has zero `egui` imports/references | Grep `src/desktop/src/agent/` for `egui` → zero hits (SC-001) |
| INV-2 | `agent/` has zero `Sender<BackgroundEvent>` / `tx_gui` references | Grep `src/desktop/src/agent/` for `tx_gui`/`BackgroundEvent` → zero hits |
| INV-3 | `agent/tool_executor.rs` has no `FsEvent` / `tx_gui` | Grep → zero hits (SC-005) |
| INV-4 | `BackgroundEvent` has no `Agent` variant | Inspect `bus/events/typed.rs` — `Agent` variant removed (step 9) |
| INV-5 | `AgentState` has no `scroll_to_id` / `response` / `pending_prompts` | Inspect `manager.rs` `AgentState` struct (SC-007) |
| INV-6 | Every `AgentEvent` variant carries `session_id` | Inspect `agent/events.rs` enum (FR-003) |
| INV-7 | `WebDelegateResponse` has `tool_calls: Vec<DelegateToolCall>`, no `tool_call_trace: String` | Inspect `tools/dtos.rs` (SC-006) |
| INV-8 | Agent loop unit test passes with no UI crate on dep path | `cargo test -p fastmd --lib agent::` with UI feature off (SC-001, SC-004) |

---

## Migration contract (transitional)

During migration steps 2-4 (dual-publish, research.md §8):

- The agent publishes on **both** `Bus<AgentEvent>` (new) and
  `Sender<BackgroundEvent>` (old `tx_gui`).
- The UI drains **only** the old `BackgroundEvent::Agent` path for rendering.
- The new `Bus<AgentEvent>` is subscribed and asserted-on in tests (events arrive
  in expected order/shape) but NOT rendered.
- Step 5 flips the drain: UI renders from `Bus<AgentEvent>`; old
  `BackgroundEvent::Agent` path is drained but ignored (or removed).
- Step 7 deletes `AgentContext::tx_gui`; step 9 deletes
  `BackgroundEvent::Agent`.

At no point does the UI render from both channels simultaneously (no duplicate
rendering).
