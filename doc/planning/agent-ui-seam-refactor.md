# Agent Loop / UI Seam Refactor

Status: proposal
Date: 2026-08-10

## Context

The agent LLM/tool-call loop (`src/desktop/src/agent/agent_impl.rs`) and the
UI rendering layer are too coupled. Today the agent thread holds a direct
handle to the UI's mpsc channel (`AgentContext::tx_gui:
Sender<BackgroundEvent>`), formats user-facing markdown strings inside the
loop (`response_formatter.rs` calls inside `process_turn`), ships a running
`full_response: String` buffer as `AgentEvent::Response`, and seeds the next
session from a UI-edited copy of that buffer (`AgentContext::current_response`
cloned from `AgentState.response`). The `AgentSessionManager` is a god object
spanning agent lifecycle, agent domain state, and pure UI widget state
(`show_debug_window`, `debug_search_text`, `debug_auto_scroll`,
`debug_json_rows`, `command_input`, `show_results`, `scroll_to_id`).
`AgentState::scroll_to_id` even references `egui::Id` in its docstring
(`agent/manager.rs:30`) — a UI-leak into the agent tree.

The tool executor reaches the UI through a second back-channel:
`ToolExecutor::execute_all` takes `tx_gui` and sends `FsEvent::FileModified`
directly (`tool_executor.rs:202-208`) to trigger a UI-side reindex when
`create_note` succeeds. This bypasses `AgentEvent` entirely.

The result is that the agent layer cannot be tested, reused, or restyled
without the UI, and the UI cannot restyle agent output without re-parsing
markdown.

Related requirements (drift, not changed by this proposal):
- [AGENT-008] async execution without stalling the UI — satisfied today by
  `std::thread::spawn`; this proposal keeps that property.
- [AGENT-010] display thinking + render markdown response in real-time —
  satisfied today by `AgentEvent::Thinking` / `AgentEvent::Response`; this
  proposal keeps the real-time property but changes the payload shape.
- [AGENT-011] / [AGENT-016] print tool call invocations with formatted JSON —
  today done agent-side by `format_tool_call_message`; this proposal moves
  formatting to the UI side while keeping the structured data on the channel.

## Decision

Introduce a clean seam between the agent loop and the UI with two channels and
a structured side-effect return type. Remove all UI concerns from `agent/`.

### 1. `AgentPrompt` mpsc channel — UI → agent input

Replace the current `orchestrator.submit_prompt: Option<String>` →
`start_agent_session(prompt)` → `manager.start_session(...)` spawn-per-prompt
flow with a long-lived mpsc channel.

```rust
// src/desktop/src/agent/events.rs (new)
use uuid::Uuid;

pub struct AgentPrompt {
    /// UUID identifying the session this prompt belongs to. The UI mints
    /// a fresh `Uuid::new_v4()` to start a new session and reuses the
    /// same UUID for every prompt that continues that session. This
    /// lets the agent driver keep multiple concurrent sessions
    /// disambiguated and carry over the right conversation history
    /// ([AGENT-021]) without an integer counter.
    pub session_id: Uuid,
    pub text: String,
    pub active_file: Option<PathBuf>,
    pub active_dir: Option<PathBuf>,
    pub selected_files: HashSet<PathBuf>,
}
```

- The UI owns the `Sender<AgentPrompt>`; the agent owns the
  `Receiver<AgentPrompt>`.
- The agent loop becomes a long-lived driver thread that pulls prompts from
  the receiver. When a prompt arrives with a `session_id` the driver has not
  seen before, it starts a new session (emits a `SessionBoundary` debug
  entry keyed on that UUID, resets history per [AGENT-021]). When the
  `session_id` matches an existing in-progress session, it continues that
  session — reusing its conversation history and appending to its
  transcript. The integer `session_counter` on the current manager is
  replaced by the UUID; this is forward-compatible with multiple
  concurrent sessions.
- Shared, long-lived resources (`config`, `tool_manager`, `browser_session`,
  `pdf_backing`, `file_event_bus`, `uuid_gen`) are owned by the agent driver
  and reused across prompts — they are no longer cloned into a per-session
  `AgentContext` from the orchestrator.
- `AgentContext` is reduced to per-session data: `prompt`, `active_file`,
  `active_dir`, `selected_files`, `cancel_flag`, `session_id: Uuid`,
  `history`. The `tx_gui` and `current_response` fields are removed.

### 2. `AgentEvent` broadcast channel — agent → UI output

Replace the `Sender<BackgroundEvent>` handle in the agent with a
`Bus<AgentEvent>` broadcast (the existing `bus/core.rs::Bus<T>` backed by
`tokio::sync::broadcast`, capacity 8192). The agent publishes structured
events; the UI subscribes.

```rust
// src/desktop/src/agent/events.rs (new)
use uuid::Uuid;

pub enum AgentEvent {
    /// Session lifecycle marker. `Started` is emitted when the driver
    /// encounters a new `session_id` on an incoming `AgentPrompt`;
    /// `Finished` when the session's agent loop ends (no more pending
    /// prompts for that session, or the session is cancelled).
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

/// `AgentDebugEntry` (in `bus/events/debug.rs`) drops its `session: usize`
/// field — the session is now identified by the `session_id: Uuid` carried
/// on the enclosing `AgentEvent::DebugEntry` variant. The `turn: usize`
/// field stays (per-session turn counter).
pub enum AgentStatus {
    AwaitingLlm,
    ExecutingTools,
    Done,
}

pub enum ToolSideEffect {
    FileCreated { path: PathBuf, tags: Vec<String> },
}
```

- `AgentEvent` is defined in `agent/events.rs` — agent-owned, no UI imports.
- Every variant carries the `session_id: Uuid` so the UI can route events
  to the right session transcript. This is forward-compatible with
  multiple concurrent sessions; today the UI only models one, but the
  channel and state types no longer assume it.
- The agent loop publishes on `Bus<AgentEvent>` (clone the `Bus` handle into
  the driver thread). No `tx_gui` field on `AgentContext`.
- The UI subscribes via `bus.subscribe()` → `BusReader<AgentEvent>` and drains
  once per frame in `drain_background_channel` (orchestrator side).
- `full_response: String` accumulator is removed. The agent emits
  `ContentDelta`, `ToolCallStarted`, `ToolResult` as structured deltas; the UI
  accumulates them into its own transcript view model. This removes the O(n²)
  running-buffer channel traffic and the shared-mutable-buffer contract that
  today lets the UI write back via `apply_task_toggle`.
- `split_thinking_and_content` (the `🤔` delimiter split, [AGENT-022]) moves
  out of the agent loop and into the UI presentation layer. The agent emits
  raw assistant message content; the UI decides how to split and display
  thinking vs. content.

### 3. `Vec<ToolSideEffect>` from `execute_all` — no `tx_gui` in tools

`ToolExecutor::execute_all` drops the `tx_gui: &Sender<BackgroundEvent>`
parameter and returns side effects as data:

```rust
pub fn execute_all(
    &self,
    tool_calls: &[serde_json::Value],
) -> (Vec<(String, String, String, String)>, Vec<ToolSideEffect>) {
    // ...
    let side_effects = collect_side_effects(&results);
    (results, side_effects)
}
```

- `notify_file_creations` (the `FsEvent::FileModified` send at
  `tool_executor.rs:202-208`) is removed from the tool executor.
- The agent loop receives `Vec<ToolSideEffect>` from `execute_all` and
  publishes each as `AgentEvent::ToolSideEffect(...)` on the broadcast bus.
- The tool layer no longer imports or references any UI channel.

### 4. Dedicated receiver re-issues file events

On the UI/orchestrator side, a dedicated `BusReader<AgentEvent>` (or a branch
in the existing drain loop) listens for `AgentEvent::ToolSideEffect` and
re-issues the file event through the existing file-event plumbing:

```rust
// In AppOrchestrator drain loop (app/orchestrator.rs)
match agent_event {
    AgentEvent::ToolSideEffect(ToolSideEffect::FileCreated { path, tags }) => {
        // Re-issue as FsEvent to drive tag/tree/tab reindex,
        // same as the old notify_file_creations did.
        self.handle_fs_event(FsEvent::FileModified { path, tags });
    }
    // ... other variants → AgentState / transcript updates
}
```

This keeps a single agent→UI channel (`Bus<AgentEvent>`) and makes the
file-event reissue an explicit UI-side concern, not a tool-layer back-channel.

### 5. UI concerns removed from `agent/`

After the refactor, `agent/` contains no UI state:

- **Move `response_formatter.rs`** (`format_tool_call_message`,
  `format_tool_result_message`, `format_delegate_tool_call_message`,
  `split_thinking_and_content`) to `ui/render/agent_render.rs` or a new
  `ui/agent/` module. The agent emits structured `AgentEvent::ToolCallStarted`
  / `ToolResult` / `ContentDelta`; the UI formats them into markdown for
  `render_markdown`.
- **Move `web_delegate`'s `tool_call_trace: String` field** to structured
  data: `WebDelegateResponse` carries `Vec<DelegateToolCall { name, args,
  result }>`. The `format_delegate_tool_call_message` call in
  `tools/web.rs:477` is removed; the UI formats the structured trace. This
  deletes the `strip_web_delegate_trace` workaround in `agent_impl.rs` —
  the trace is never a string field in the tool result, so there is nothing
  to strip before sending to the LLM.
- **Split `AgentSessionManager`**:
  - `AgentSessionManager` (in `agent/`) — lifecycle only: owns the
    `Receiver<AgentPrompt>`, the `Bus<AgentEvent>`, shared resources
    (`config`, `tool_manager`, `browser_session`, `pdf_backing`), the
    driver thread, and `AgentState` (now structured: `running`,
    `status: AgentStatus`, `thinking: Option<String>`, `history`, `usage`,
    `debug_entries`). No `scroll_to_id`, no `show_*` flags, no
    `command_input`.
  - `AgentPanelState` (in `ui/` or `app/`) — pure UI view state:
    `show_results`, `show_debug_window`, `debug_search_text`,
    `debug_auto_scroll`, `debug_json_rows`, `command_input`,
    `scroll_to_id`. Owned by `FastMdApp` or the orchestrator, not by the
    agent manager.
- **Delete `AgentContext::tx_gui` and `AgentContext::current_response`**.
  The agent thread never holds a UI channel handle; it publishes on
  `Bus<AgentEvent>`. The UI transcript buffer is UI-owned; the agent never
  reads it back. If the agent needs prior context, it uses `history`
  (already a field).
- **Remove `scroll_to_id` from `AgentState`** and its `egui::Id` docstring
  reference (`agent/manager.rs:30`). It moves to `AgentPanelState`.

### Resulting boundary

```
agent/                              ui/ + app/
─────────────────────────────────────────────────────────
AgentPrompt (mpsc Receiver)         Sender<AgentPrompt> on UI
  ← UI submits prompt                (owned by orchestrator)
  Prompt carries session_id: Uuid   UI mints Uuid::new_v4() per session
AgentEvent (Bus<AgentEvent> pub)    BusReader<AgentEvent> on UI
  → structured events                (drained each frame)
  Every variant carries session_id  UI routes by session_id (today: 1
                                     session; forward-compatible with N)
                                     dedicated receiver branch re-issues
                                     ToolSideEffect → FsEvent
ToolExecutor::execute_all           response_formatter moved here
  returns Vec<ToolSideEffect>          (formats ToolCallStarted/Result)
  no tx_gui param                   split_thinking_and_content moved here
                                     AgentPanelState lives here
AgentSessionManager                 (show_results, debug window state,
  lifecycle + AgentState              scroll_to_id, command_input)
  (structured, no markdown)         UI tracks active_session_id: Uuid
  sessions keyed by Uuid            and next_session_id to mint on "New"
```

## Consequences

### Positive

- `agent/` is UI-free: no `egui` references, no UI channel handles, no UI
  widget state, no markdown formatting. It can be tested in isolation with
  a `Bus<AgentEvent>` subscriber and an `AgentPrompt` sender.
- The UI owns the transcript buffer and can restyle, filter, or interact with
  tool calls without re-parsing markdown.
- Channel traffic drops from O(n²) (running buffer resent each turn) to O(n)
  (deltas). The agent emits each content/tool chunk once.
- `ToolSideEffect` makes the `create_note` → reindex path explicit and
  type-safe; the tool layer no longer reaches into the UI channel.
- `web_delegate`'s trace is structured; the `strip_web_delegate_trace`
  workaround is no longer needed because the trace is never a string field
  in the LLM-bound payload.
- `AgentSessionManager` stops being a god object; UI dialog state lives on
  the UI side.

### Negative / migration cost

- Every `AgentEvent` consumer (orchestrator drain loop, debug window) must
  switch from string payloads to structured variants. The UI must accumulate
  a transcript view model from deltas — a new responsibility.
- `apply_task_toggle` (center.rs:213-218) currently mutates
  `AgentState::response` in place. After the refactor, the toggle state lives
  in the UI transcript view model, which is cleaner but requires moving the
  task-toggle wiring.
- `AgentEvent::SessionFinished` no longer carries `history` re-seed
  semantics through the UI buffer (`current_response`); history is managed
  agent-side keyed on `session_id`. The UI signals "new session / reset
  history" (AGENT-021) by minting a fresh `Uuid::new_v4()` on the next
  `AgentPrompt.session_id` — no separate command channel needed.
- Snapshot / e2e render tests that assert on `AgentEvent::Response(String)`
  content must be updated to assert on the structured transcript.

## Migration plan

Each step is independently testable. Do not combine steps.

1. **Introduce `agent/events.rs`** with `AgentEvent`, `AgentStatus`,
   `ToolSideEffect`, `AgentPrompt`. Additive; no behavior change. The
   existing `BackgroundEvent::Agent(AgentEvent)` path stays during
   migration.
2. **Add `Bus<AgentEvent>` to `AgentSessionManager`** alongside the existing
   `tx_gui`. The agent loop publishes on both channels (the new bus and the
   old mpsc) during the transition. UI drains the old path unchanged.
3. **Move `response_formatter.rs` to `ui/`**. Introduce a UI-side
   transcript accumulator that builds markdown from structured
   `AgentEvent::ToolCallStarted` / `ToolResult` / `ContentDelta`. Wire the
   new accumulator into `center.rs` behind the old `state.response` path.
4. **Return `Vec<ToolSideEffect>` from `execute_all`**. Drop `tx_gui` from
   the signature. The agent loop publishes `AgentEvent::ToolSideEffect` on
   the new bus. Add the dedicated receiver branch in the orchestrator that
   re-issues `FsEvent::FileModified`.
5. **Switch the orchestrator drain** from the old `BackgroundEvent::Agent`
   path to the new `Bus<AgentEvent>` reader. Remove the dual-publish from
   step 2.
6. **Split `AgentSessionManager`** — pull `AgentPanelState` out to `ui/`.
   Move `scroll_to_id`, `show_results`, debug-window fields,
   `command_input`.
7. **Delete `AgentContext::tx_gui` and `current_response`**. The agent
   thread publishes on `Bus<AgentEvent>` only.
8. **Restructure `WebDelegateResponse`** — replace `tool_call_trace: String`
   with `Vec<DelegateToolCall>`. Delete `strip_web_delegate_trace`. Move
   the delegate-trace formatting to the UI.
9. **Remove the old `BackgroundEvent::Agent(AgentEvent)` enum** and the
   `From` impls. `AgentEvent` now lives in `agent/events.rs` and flows only
   on `Bus<AgentEvent>`.
10. **Replace `session_counter` with `Uuid` session identity** — the UI
    mints `Uuid::new_v4()` when starting a new session and reuses it for
    continuation prompts. `AgentDebugEntry.session: usize` is removed; the
    `session_id: Uuid` on `AgentEvent::DebugEntry` identifies the session.
    `AgentContext.session_number: usize` becomes `session_id: Uuid`. The
    driver keeps a `HashMap<Uuid, SessionState>` so it can resume the right
    conversation history; today only one session is active at a time, but
    the channel and state types are forward-compatible with N.

## File impact summary

| File | Change |
|---|---|
| `agent/events.rs` (new) | `AgentEvent` (all variants carry `session_id: Uuid`), `AgentStatus`, `ToolSideEffect`, `AgentPrompt` (carries `session_id: Uuid`) |
| `agent/agent_impl.rs` | Remove `tx_gui` sends; publish `AgentEvent` on `Bus`; emit deltas instead of `full_response`; tag every event with the prompt's `session_id`; delete `strip_web_delegate_trace` |
| `agent/context.rs` | Remove `tx_gui`, `current_response`; reduce to per-session data; `session_number: usize` → `session_id: Uuid` |
| `agent/manager.rs` | Own `Receiver<AgentPrompt>` + `Bus<AgentEvent>`; track sessions by `Uuid` (replace `session_counter`); remove UI fields; remove `scroll_to_id` from `AgentState` |
| `bus/events/debug.rs` | `AgentDebugEntry.session: usize` field removed — session identity lives on the enclosing `AgentEvent::DebugEntry { session_id }` |
| `agent/tool_executor.rs` | `execute_all` returns `(results, Vec<ToolSideEffect>)`; remove `tx_gui` param; delete `notify_file_creations` |
| `agent/tools/dtos.rs` | `WebDelegateResponse.tool_call_trace: String` → `Vec<DelegateToolCall>` |
| `agent/tools/web.rs` | Remove `format_delegate_tool_call_message` import; return structured trace |
| `agent/response_formatter.rs` → `ui/render/agent_render.rs` | Move module; UI owns markdown formatting |
| `bus/events/typed.rs` | Remove `AgentEvent` (moved to `agent/events.rs`); `BackgroundEvent` keeps `FsEvent` / `ProcessEvent` / `McpAuthEvent` |
| `app/orchestrator.rs` | Own `Sender<AgentPrompt>` (mint `Uuid::new_v4()` for new sessions) + `BusReader<AgentEvent>`; route events by `session_id`; dedicated `ToolSideEffect` → `FsEvent` reissue branch |
| `ui/panels/center.rs` | Read structured transcript from `AgentPanelState` keyed on `session_id`; `apply_task_toggle` mutates UI view model |
| `ui/agent_debug_window.rs` | Read `AgentState.debug_entries` (tagged with `session_id`) from the new `AgentSessionManager`; read `AgentPanelState` for window controls |
| `ui/app/mod.rs`, `ui/app/update.rs` | Own `AgentPanelState`; submit prompts via `Sender<AgentPrompt>` |

## Spec traceability

This proposal does not change any [AGENT-xxx] requirement. It changes
implementation structure only. Drift to flag (per RUST-040):
- [AGENT-011] / [AGENT-016] — tool call display formatting moves from agent
  to UI; the requirement is satisfied at a different layer.
- [AGENT-022] — `split_thinking_and_content` moves from agent to UI; the
  requirement is satisfied at a different layer.
- `AgentState::scroll_to_id` referencing `egui::Id` (manager.rs:30) is
  existing drift against the spirit of RUST-058; this refactor removes it.