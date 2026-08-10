# Data Model: Agent Loop / UI Seam Refactor

**Feature**: 003-agent-ui-seam-refactor
**Date**: 2026-08-10
**Phase**: 1

Entity definitions, fields, relationships, validation rules, and state
transitions for the refactor. Derived from [spec.md](spec.md) Key Entities and
grounded in the current codebase (see [research.md](research.md) for decisions).

All types are Rust types in the `fastmd` crate (`src/desktop/`). Paths are
relative to `src/desktop/src/`.

---

## 1. `AgentPrompt` (NEW — `agent/events.rs`)

Input message: UI → agent on `std::sync::mpsc::Sender<AgentPrompt>`.

```rust
pub struct AgentPrompt {
    pub session_id: Uuid,
    pub text: String,
    pub active_file: Option<PathBuf>,
    pub active_dir: Option<PathBuf>,
    pub selected_files: HashSet<PathBuf>,
}
```

**Fields**:
- `session_id: Uuid` — identity of the session this prompt belongs to. UI mints
  `Uuid::new_v4()` for a new session; reuses for continuation prompts (FR-008).
- `text: String` — the user's prompt text. MUST be non-empty after trim
  (validation: agent rejects empty/whitespace-only prompts without starting a
  turn — spec Edge Case).
- `active_file`, `active_dir`, `selected_files` — UI selection context passed
  through to tools (replaces the same fields on today's `AgentContext`,
  `context.rs:22-24`).

**Relationships**: consumed by the agent driver thread's `Receiver<AgentPrompt>`.
Produces `AgentEvent::SessionStarted` (new session_id) or continues an existing
session (known session_id).

**Validation rules**:
- `text.trim().is_empty()` → reject (no turn started; emit `AgentEvent::Failed`
  or a `Status` indicating rejection).
- `session_id` MUST be a valid `Uuid` (invariant — `Uuid::new_v4()` guarantees
  this).

**State transitions**: N/A (immutable input message).

---

## 2. `AgentEvent` (NEW — `agent/events.rs`)

Output message: agent → UI on `Bus<AgentEvent>` (tokio broadcast, capacity
8192). Replaces the current `AgentEvent` enum in `bus/events/typed.rs:22-34`.

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

**Fields**:
- Every variant carries `session_id: Uuid` — the UI routes events to the correct
  session transcript (FR-003, FR-009). Forward-compatible with N concurrent
  sessions (today: 1 active).
- `ContentDelta.text` — an incremental chunk of assistant content. The UI
  accumulates these into its transcript view model (FR-010). Replaces the
  current `AgentEvent::Response(full_response.clone())` running buffer
  (`agent_impl.rs:215, 297, 330`).
- `ToolCallStarted.id` / `ToolResult.id` — correlation id (the tool call's
  `call_id` from the LLM response). One `ToolCallStarted` ↔ one `ToolResult`
  per tool call.
- `ToolResult.result` — the tool's result as JSON. For `web_delegate`, this
  contains `tool_calls: Vec<DelegateToolCall>` (structured, no string trace —
  research.md §4/5).
- `DebugEntry.entry` — `AgentDebugEntry` (see §4, modified).
- `Failed.error` — user-facing error string (e.g. LLM API error, invalid
  schema).

**Derives**: `Debug, Clone, Serialize`. `Serialize` added (research.md §6) for
snapshot/debug persistence. `Uuid` is serializable (`uuid` serde feature,
`Cargo.toml:83`).

**Relationships**: produced by the agent driver thread; consumed by UI
subscribers (`BusReader<AgentEvent>`).

**Validation rules**:
- `session_id` MUST match a session that emitted `SessionStarted` (UI-side
  invariant; unknown session_id → ignore or log).
- `ToolCallStarted` and `ToolResult` ids MUST correlate (UI-side invariant for
  transcript pairing).

**State transitions**: N/A (immutable event).

---

## 3. `AgentStatus` (NEW — `agent/events.rs`)

```rust
#[derive(Debug, Clone, Serialize)]
pub enum AgentStatus {
    AwaitingLlm,
    ExecutingTools,
    Done,
}
```

**Fields**: variants only (no data). Replaces today's `AgentEvent::Status(String)`
(`typed.rs:25`) with typed states.

**State transitions** (per session):
```
(new session) → AwaitingLlm → ExecutingTools → AwaitingLlm → ... → Done → SessionFinished
                                  ↘ Failed → SessionFinished
```
- `AwaitingLlm` ↔ `ExecutingTools`: cycles per turn (LLM call → tool execution
  → next LLM call).
- `Done`: the turn loop ended normally (LLM returned a final response).
- `Failed`: an error terminated the session.
- `SessionFinished` (the `AgentEvent` variant, not a status): emitted after
  `Done` or `Failed`, or on cancel (FR-015).

---

## 4. `AgentDebugEntry` (MODIFIED — `bus/events/debug.rs`)

Current (`debug.rs:27-43`):
```rust
pub struct AgentDebugEntry {
    pub turn: usize,
    pub session: usize,          // ← REMOVED
    pub timestamp: DateTime<Local>,
    pub kind: DebugEntryKind,
    pub summary: String,
    pub content: Option<serde_json::Value>,
    pub row_type: DebugEntryRow,
}
```

After:
```rust
pub struct AgentDebugEntry {
    pub turn: usize,
    // session field removed — session identity is on the enclosing
    // AgentEvent::DebugEntry { session_id } variant.
    pub timestamp: DateTime<Local>,
    pub kind: DebugEntryKind,
    pub summary: String,
    pub content: Option<serde_json::Value>,
    pub row_type: DebugEntryRow,
}
```

**Change**: `session: usize` field removed. The session is identified by the
`session_id: Uuid` on the enclosing `AgentEvent::DebugEntry` variant. `turn:
usize` stays (per-session turn counter, 1-based; 0 for session boundaries).

**Relationships**: carried inside `AgentEvent::DebugEntry`. `DebugEntryKind`
(`Outgoing`, `Incoming`, `ToolResults`) and `DebugEntryRow` (`Entry`,
`SessionBoundary`) unchanged.

**Migration note**: today `From<AgentDebugEntry> for BackgroundEvent`
(`typed.rs:151`) wraps as `Agent(AgentEvent::DebugEntry(entry))`. After the
refactor, `AgentDebugEntry` flows only inside `AgentEvent::DebugEntry` on
`Bus<AgentEvent>`. The `From` impl is deleted (step 9).

---

## 5. `ToolSideEffect` (NEW — `agent/events.rs`)

```rust
#[derive(Debug, Clone, Serialize)]
pub enum ToolSideEffect {
    FileCreated { path: PathBuf, tags: Vec<String> },
}
```

**Fields**:
- `FileCreated.path` — absolute path of the created file (today: the resolved
  `lib.root_folder.join(rest)` from `tool_executor.rs:202`).
- `FileCreated.tags` — tags extracted from the note's front matter (today: the
  `tags` vec sent in `FsEvent::FileModified` at `tool_executor.rs:202-208`).

**Relationships**: produced by `ToolExecutor::execute_all` (returned as
`Vec<ToolSideEffect>`); republished by the agent as `AgentEvent::ToolSideEffect`;
re-issued by the UI as `FsEvent::FileModified { path, tags }` (FR-007, SC-005).

**Validation rules**:
- `path` MUST be absolute (invariant — `tool_executor.rs` resolves to absolute
  today).
- Only emitted for **successful** file creations (spec Edge Case: failed tool →
  no side effect).

**Extensibility**: `enum` allows future variants (`FileDeleted`, `FileModified`,
`TagsChanged`) without changing the `execute_all` signature. Today only
`FileCreated` is needed (matching `notify_file_creations`'s current behavior).

---

## 6. `AgentContext` (MODIFIED — `agent/context.rs`)

Current (`context.rs:18-47`): 16 fields including `tx_gui`, `current_response`,
`session_number`, `config`, `browser_session`, `pdf_backing`, `tool_manager`,
`uuid_gen`, `file_event_bus`.

After:
```rust
pub struct AgentContext {
    // Per-session data only:
    pub session_id: Uuid,            // was session_number: usize
    pub prompt: String,
    pub active_file: Option<PathBuf>,
    pub active_dir: Option<PathBuf>,
    pub selected_files: HashSet<PathBuf>,
    pub cancel_flag: Arc<AtomicBool>,
    pub history: Option<Vec<Value>>,
    pub model_name: Option<String>,
    // REMOVED: tx_gui, current_response, config, browser_session, pdf_backing,
    //          tool_manager, uuid_gen, file_event_bus
}
```

**Changes**:
- `tx_gui: Sender<BackgroundEvent>` — **REMOVED** (FR-005). The agent publishes
  on `Bus<AgentEvent>` (owned by the driver, not per-session).
- `current_response: String` — **REMOVED** (FR-011). No running buffer; the
  agent emits `ContentDelta`s.
- `session_number: usize` → `session_id: Uuid` (FR-008).
- Shared long-lived resources (`config`, `browser_session`, `pdf_backing`,
  `tool_manager`, `uuid_gen`, `file_event_bus`) — **moved to the driver thread**
  (owned by `AgentSessionManager` or the driver closure). The per-session
  `AgentContext` no longer clones them; the driver passes references or a shared
  `Arc` handle into `run_agent_inner`.

**Relationships**: built per-session by the driver from an `AgentPrompt`; passed
to `run_agent_inner`.

---

## 7. `AgentSessionManager` (MODIFIED — `agent/manager.rs`)

Current (`manager.rs:54-84`): god object — lifecycle + domain state + UI widget
state (`command_input`, `show_results`, `show_debug_window`, `debug_search_text`,
`debug_auto_scroll`, `session_counter`).

After:
```rust
pub struct AgentSessionManager {
    // Lifecycle + transport:
    prompt_tx: Option<Sender<AgentPrompt>>,       // UI submits via this
    event_bus: Bus<AgentEvent>,                    // agent publishes on this
    driver_handle: Option<JoinHandle<()>>,         // long-lived driver thread

    // Shared long-lived resources (moved from AgentContext):
    config: AppConfig,
    browser_session: Arc<BrowserSession>,
    pdf_backing: Arc<PdfBackingTracker>,
    tool_manager: Arc<RwLock<ToolManager>>,
    file_event_bus: Bus<FileEvent>,                // for tool file-event publishing
    uuid_gen: Arc<dyn UuidGenerator>,

    // Agent domain state (structured, no UI state):
    state: AgentState,                             // see §8
    cancel_flag: Option<Arc<AtomicBool>>,
    config_reader: Option<BusReader<ConfigArrived>>,
    config_arrived: bool,

    // REMOVED: command_input, show_results, show_debug_window,
    //          debug_search_text, debug_auto_scroll, session_counter
}
```

**Changes**:
- UI widget state fields → `AgentPanelState` (UI-owned, §9).
- `session_counter: usize` → replaced by `Uuid` session identity (UI mints;
  driver tracks `HashMap<Uuid, SessionState>` for history — research.md §3).
- New: `prompt_tx`, `event_bus`, `driver_handle` for the long-lived driver.
- `start_session` (old spawn-per-prompt, `manager.rs:313-355`) → replaced by
  `submit_prompt(prompt: AgentPrompt)` (sends on `prompt_tx`).

**Relationships**: owns the driver thread; provides `event_bus` for UI
subscription; provides `state` for the debug window to read `debug_entries`.

---

## 8. `AgentState` (MODIFIED — `agent/manager.rs`)

Current (`manager.rs:22-41`):
```rust
pub struct AgentState {
    pub running: bool,
    pub status: String,
    pub thinking: String,
    pub response: String,
    pub scroll_to_id: Option<String>,     // ← REMOVED (UI state)
    pub history: Option<Vec<Value>>,
    pub token_usage: Option<TokenUsageInfo>,
    pub total_usage: TokenUsageInfo,
    pub pending_prompts: Vec<String>,     // ← REMOVED (replaced by mpsc buffer)
    pub debug_entries: Vec<AgentDebugEntry>,
}
```

After:
```rust
pub struct AgentState {
    pub running: bool,
    pub status: AgentStatus,              // was String (typed)
    pub thinking: Option<String>,         // was String (Option: no thinking until first emit)
    pub history: Option<Vec<Value>>,
    pub usage: TokenUsageInfo,            // consolidated (was token_usage + total_usage)
    pub debug_entries: Vec<AgentDebugEntry>,
    // REMOVED: response (UI-owned transcript view model),
    //          scroll_to_id (→ AgentPanelState),
    //          pending_prompts (→ mpsc channel buffer)
}
```

**Changes**:
- `response: String` — **REMOVED**. The UI owns the transcript view model
  (FR-010). The agent emits `ContentDelta`s; it does not hold the rendered
  buffer. This breaks the `current_response = state.response.clone()` seed
  (`manager.rs:343`) — history is now managed agent-side keyed on `session_id`
  (FR-009).
- `scroll_to_id` — **REMOVED** (→ `AgentPanelState`, research.md §7).
- `pending_prompts: Vec<String>` — **REMOVED** (replaced by the
  `Receiver<AgentPrompt>` buffer; the driver processes prompts in arrival order).
- `status: String` → `status: AgentStatus` (typed).
- `thinking: String` → `thinking: Option<String>` (no thinking until the agent
  emits one; empty string was a sentinel).
- `token_usage` + `total_usage` → consolidated `usage: TokenUsageInfo` (running
  total; per-turn usage is emitted via `AgentEvent::TokenUsage`).

---

## 9. `AgentPanelState` (NEW — `ui/` or `app/`)

Pure UI view state, owned by `FastMdApp` or `AppOrchestrator` (not by
`AgentSessionManager`).

```rust
pub struct AgentPanelState {
    // Window/panel toggles:
    pub show_results: bool,
    pub show_debug_window: bool,
    pub debug_auto_scroll: bool,
    // Debug window controls:
    pub debug_search_text: String,
    // Command input:
    pub command_input: String,
    // Scroll target (was AgentState::scroll_to_id):
    pub scroll_to_id: Option<String>,
    // Active session identity (UI tracks which session to render):
    pub active_session_id: Option<Uuid>,
}
```

**Fields**: all UI-only. `scroll_to_id` moves here from `AgentState`
(research.md §7). `active_session_id` is new — the UI mints a `Uuid::new_v4()`
on "New session" and reuses it for continuation prompts (FR-008).

**Relationships**: owned by the UI layer; read/written by `ui/panels/center.rs`,
`ui/agent_debug_window.rs`, `ui/app/update.rs`. The `AgentSessionManager` does
not reference it (SC-007).

**Validation rules**: `active_session_id` MUST be `Some` when a session is
active and the UI is rendering its transcript.

---

## 10. `AgentTranscript` (NEW — `ui/agent/` or `ui/render/`)

UI-owned view model accumulating `AgentEvent` deltas into a displayable,
interactable transcript (FR-010).

```rust
pub struct AgentTranscript {
    pub session_id: Uuid,
    pub blocks: Vec<TranscriptBlock>,
    pub thinking: String,           // accumulated Thinking events
    pub content: String,            // accumulated ContentDelta events
}

pub enum TranscriptBlock {
    Content { text: String },
    ToolCall { id: String, name: String, args: serde_json::Value, result: Option<serde_json::Value> },
    Thinking { text: String },
}
```

**Fields**:
- `blocks: Vec<TranscriptBlock>` — ordered transcript entries. `Content` blocks
  accumulate `ContentDelta`s; `ToolCall` blocks pair `ToolCallStarted` with a
  later `ToolResult` (matched by `id`); `Thinking` blocks hold thinking text
  (split from content by the UI per FR-012).
- `content: String` — the accumulated content buffer (target for
  `apply_task_toggle` — research.md §7). Replaces `AgentState::response` as the
  buffer the UI mutates on task-toggle.
- `thinking: String` — accumulated thinking (UI decides how to split/render per
  FR-012; `split_thinking_and_content` moves here).

**Relationships**: owned by the UI (one per active session). Updated by the
orchestrator drain loop from `BusReader<AgentEvent>`. Read by
`ui/panels/center.rs` for rendering via `render_markdown`.

**State transitions**:
```
(new session) → empty transcript
  ContentDelta → append to current Content block (or start new one)
  Thinking → append to thinking buffer
  ToolCallStarted → push new ToolCall block (result: None)
  ToolResult → find ToolCall block by id, set result
  SessionFinished → freeze transcript (no further updates)
```

---

## 11. `WebDelegateResponse` (MODIFIED — `agent/tools/dtos.rs`)

Current (`dtos.rs:485-493`):
```rust
pub struct WebDelegateResponse {
    pub result: String,
    #[serde(default)]
    pub tool_call_trace: String,        // ← REPLACED
}
```

After:
```rust
pub struct WebDelegateResponse {
    pub result: String,
    #[serde(default)]
    pub tool_calls: Vec<DelegateToolCall>,
}

pub struct DelegateToolCall {
    pub name: String,
    pub args: serde_json::Value,
    pub result: serde_json::Value,
}
```

**Changes**:
- `tool_call_trace: String` → `tool_calls: Vec<DelegateToolCall>` (FR-014,
  research.md §4/5). The trace is structured per-call data, not a pre-formatted
  string.
- `format_delegate_tool_call_message` (`response_formatter.rs:43`) is no longer
  called from `tools/web.rs:477` — the UI formats the structured `tool_calls`
  array from the `ToolResult` payload.

**Derives**: `DelegateToolCall` derives `Serialize, Debug, JsonSchema` (matching
`WebDelegateResponse`'s existing derives). The LLM-bound tool result payload
contains the structured array — no string to strip or inject (SC-006).

**Relationships**: produced by `tools/web.rs`; serialized into the
`ToolResult.result` JSON payload; formatted by the UI.

---

## Entity relationship summary

```
AgentPrompt ──mpsc──→ AgentSessionManager (driver)
                          │
                          ├── owns AgentContext (per-session)
                          │       └── run_agent_inner
                          │             ├── publishes AgentEvent ──Bus<AgentEvent>──→ UI
                          │             │       ├── SessionStarted/Finished
                          │             │       ├── Status(AgentStatus)
                          │             │       ├── Thinking
                          │             │       ├── ContentDelta
                          │             │       ├── ToolCallStarted / ToolResult
                          │             │       │       └── WebDelegateResponse.tool_calls: Vec<DelegateToolCall>
                          │             │       ├── ToolSideEffect ──→ UI re-issues FsEvent::FileModified
                          │             │       ├── DebugEntry(AgentDebugEntry)  [no session field]
                          │             │       ├── TokenUsage(TokenUsageInfo)   [+ Serialize]
                          │             │       └── Failed
                          │             └── ToolExecutor::execute_all → (results, Vec<ToolSideEffect>)
                          │
                          └── AgentState (structured, no UI state)
                                  └── debug_entries: Vec<AgentDebugEntry>

UI layer:
  AgentPanelState (scroll_to_id, show_*, command_input, active_session_id)
  AgentTranscript (blocks, content, thinking) ← accumulates AgentEvent deltas
       └── apply_task_toggle mutates transcript.content (not AgentState.response)
```
