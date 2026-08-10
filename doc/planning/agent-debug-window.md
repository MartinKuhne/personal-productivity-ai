# Agent Debug Window

Status: proposal
Date: 2026-08-09

## Context

The agent loop sends prompts to an LLM, receives responses, and executes tool
calls — all on a background thread. The UI only shows the agent's visible output
(status, thinking, response, token usage). There is no way to inspect raw API
traffic.

Goal: a scrollable debug log window showing one line per turn, click-to-expand
for full content, with a disclosure arrow. Entries accumulate across sessions
with a session-boundary indicator.

## Decision

Add `AgentDebugEntry`, emit from the agent loop via the existing `mpsc` channel,
accumulate in `AgentState`, render in an `egui::Window` toggled by `ALT+A`.

---

### 1. Data model

New file: `src/desktop/src/bus/events/debug.rs`

```rust
/// Direction of a debug entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugEntryKind {
    /// Net-new messages sent to the LLM this turn (delta vs previous outgoing).
    Outgoing,
    /// Full JSON response received from the LLM.
    Incoming,
    /// Tool results returned after execution.
    ToolResults,
}

/// A session-boundary marker — a non-interactive row indicating the start of a
/// new agent session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugEntryRow {
    /// A normal debug entry (collapsible, with content).
    Entry,
    /// A session-divider row: "── Session 2 ──".
    SessionBoundary,
}

/// A single debug log entry capturing the raw content of one part of one turn.
#[derive(Debug, Clone)]
pub struct AgentDebugEntry {
    /// Monotonic turn number within the session (1-based).
    pub turn: usize,
    /// Monotonic session number (1-based), incremented on each new prompt.
    pub session: usize,
    /// When the entry was created.
    pub timestamp: chrono::DateTime<chrono::Local>,
    /// Kind of the entry.
    pub kind: DebugEntryKind,
    /// One-line summary shown in the collapsed row.
    pub summary: String,
    /// Full JSON content shown when expanded. `None` for SessionBoundary.
    pub content: Option<serde_json::Value>,
    /// Whether this row is a session boundary.
    pub row_type: DebugEntryRow,
}
```

- **`turn`** is 1-based within the session.
- **`session`** is a monotonically incrementing counter across all sessions.
- **`summary`** examples: `Turn 1 — Outgoing (+2 messages, 4 tools)`, `Turn 2 — Incoming (assistant: 1 tool call)`, `Turn 2 — Tool results (read_file → 3.2 KB)`.
- **`content`** for Outgoing entries is the *delta* — only the messages added since the previous turn's outgoing, not the full accumulated array. For Incoming it is the full response JSON. For ToolResults it is an array of `{call_id, name, args, result}`.
- **Session-boundary rows** are inserted when a new prompt starts. They display as a non-interactive divider like `── Session 3 ──` with a different background color.

### 2. Delta computation for Outgoing entries

The messages array grows each turn:
- Turn 1: `[system..., user_prompt]`
- Turn 2: `[system..., user_prompt, assistant_1, tool_result_1, tool_result_2]`
- Turn 3: `[system..., user_prompt, assistant_1, tool_result_1, tool_result_2, assistant_2, ...]`

The outgoing entry for turn N stores only the messages at indices `[prev_len..]`
— the net-new messages added during the previous turn. Turn 1's outgoing
is the full initial array (no prior turn).

Computing the delta requires tracking the messages length at each turn's
outgoing point. In `run_agent_inner()`, maintain `prev_messages_len: usize`:

```rust
// At outgoing emission point:
let delta: Vec<_> = messages[prev_messages_len..].to_vec();
// ... emit entry with delta ...
prev_messages_len = messages.len();
```

The `tools_json` and model info are still included in the content alongside
the delta messages, since they are the full payload sent.

### 3. Event plumbing

Add to `AgentEvent` in `bus/events/typed.rs`:

```rust
pub enum AgentEvent {
    // ... existing variants ...
    DebugEntry(AgentDebugEntry),
}
```

Add `From<AgentDebugEntry> for BackgroundEvent`:

```rust
impl From<AgentDebugEntry> for BackgroundEvent {
    fn from(entry: AgentDebugEntry) -> Self {
        Self::Agent(AgentEvent::DebugEntry(entry))
    }
}
```

Reuses the existing `mpsc` channel. No new transport.

### 4. Agent-side instrumentation — `agent/agent_impl.rs`

Add accessors to `LLMClient`: `pub fn model_name(&self) -> &str` and
`pub fn max_tokens(&self) -> u32`.

In `run_agent_inner()`, add local state:

```rust
let mut turn_number: usize = 0;
let mut prev_messages_len: usize = 0;
let session_number: usize = /* from AgentContext or manager */;
```

In `process_turn()`, at the top, increment `turn_number += 1`.

**4a. Session-boundary emission** — in `AgentSessionManager::start_session()`,
increment a `session_counter: usize` field and pass it through `AgentContext`.
In `run_agent_inner()`, emit a `SessionBoundary` entry before the loop starts:

```rust
tx.send(AgentDebugEntry {
    turn: 0,
    session: session_number,
    timestamp: chrono::Local::now(),
    kind: DebugEntryKind::Outgoing, // irrelevant for boundary
    summary: format!("Session {}", session_number),
    content: None,
    row_type: DebugEntryRow::SessionBoundary,
}.into());
```

**4b. Outgoing — before `llm.chat_completion()` (line 90)**

```rust
let delta = messages[prev_messages_len..].to_vec();
let payload = serde_json::json!({
    "model": llm.model_name(),
    "max_tokens": llm.max_tokens(),
    "tools": tools_json,
    "new_messages": delta,
});
prev_messages_len = messages.len();

tx.send(AgentDebugEntry {
    turn: turn_number,
    session: session_number,
    timestamp: chrono::Local::now(),
    kind: DebugEntryKind::Outgoing,
    summary: format!("Turn {} — Outgoing (+{} messages, {} tools)",
        turn_number, delta.len(), tool_count),
    content: Some(payload),
    row_type: DebugEntryRow::Entry,
}.into());
```

**4c. Incoming — after `llm.chat_completion()` succeeds (line 91)**

Emit the full `resp_val` as content. Parse tool call count for summary.

**4d. Tool results — after `executor.execute_all()` (line 134)**

Emit an array of `{call_id, name, args, result}`.

### 5. Manager-side accumulation — `agent/manager.rs`

Add to `AgentState`:

```rust
pub debug_entries: Vec<AgentDebugEntry>,
```

Add to `AgentSessionManager`:

```rust
pub show_debug_window: bool,
session_counter: usize,
```

In `handle_agent_event()`, add:

```rust
AgentEvent::DebugEntry(entry) => {
    self.state.debug_entries.push(entry);
    None
}
```

In `start_session()`:
- Increment `self.session_counter += 1`
- Pass `session_number: self.session_counter` through `AgentContext`
- **Do not** clear `debug_entries` — they accumulate across sessions

`AgentContext` gets a new field: `pub session_number: usize`.

### 6. UI window — `ui/agent_debug_window.rs`

**Window**: `egui::Window` with title `"Agent Debug"`, resizable, collapsible,
default size `[600, 400]`. Toggled by `app.orchestrator.agent.show_debug_window`.

**Toolbar row**: search text field, auto-scroll checkbox, `[Clear]` button.

**Row rendering**: `ScrollArea::both().show_rows()` with two row types:

**Session-boundary row** (non-interactive):
```
────────── Session 3 ──────────
```
Rendered with subdued styling (gray, centered label with horizontal rules).

**Entry row, collapsed**:
```
▶ 14:32:05.123  [Outgoing]  Turn 2 (+3 messages, 4 tools)
```
The `▶` / `▼` is a disclosure arrow (unexpanded / expanded). Clicking the row toggles expansion.

**Entry row, expanded**:
```
▼ 14:32:05.123  [Outgoing]  Turn 2 (+3 messages, 4 tools)
│
│  {                                       ← monospace JSON
│    "model": "gpt-4",
│    "tools": [...],
│    "new_messages": [...]
│  }
│
│  [Copy JSON]
```

- The JSON block renders in a `ScrollArea::vertical().max_height(400.0)` so very large payloads don't blow out the window.
- The disclosure arrow uses egui's `RichText` with `▶` / `▼` unicode characters.
- Click target is the entire horizontal row (timestamp + kind + summary).

**Expanded state**: stored in a `HashSet<(usize, DebugEntryKind)>` (keyed by
 `(session, turn, kind)` to survive filtering/search) on the local window state.
Actually simpler: store the index into the *filtered* displayed list, and
recompute when the filter changes (clear expanded set on search change).

**Search**: text search on `summary` (case-insensitive). Session boundaries are
always shown (not filtered).

### 7. ALT+A keyboard shortcut

In the render cycle (`update.rs` or `render.rs`), consume ALT+A before the
window renders:

```rust
if ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::A)) {
    app.orchestrator.agent.show_debug_window =
        !app.orchestrator.agent.show_debug_window;
}
```

This must be consumed *before* the panels and windows render, so that the
window appears on the same frame the key is pressed. The natural place is
at the top of `FastMdApp::update_ui()` or at the top of `show_modals()`.

**No other global shortcuts exist today**, so there's no conflict or
routing infrastructure to extend. The `consume_key` call will prevent the
`A` key from reaching any focused text field, which is acceptable for
a debug toggle.

### 8. Integration touchpoints

| File | Change |
|------|--------|
| `bus/events/mod.rs` | Add `pub mod debug;`, re-export types |
| `bus/events/typed.rs` | Add `DebugEntry(AgentDebugEntry)` to `AgentEvent`; add `From` impl |
| `bus/events/debug.rs` | **New** — `AgentDebugEntry`, `DebugEntryKind`, `DebugEntryRow` |
| `agent/llm_client.rs` | Add `pub fn model_name(&self) -> &str` and `pub fn max_tokens(&self) -> u32` |
| `agent/context.rs` | Add `pub session_number: usize` field |
| `agent/agent_impl.rs` | Turn counter, prev_messages_len, 4 emission points (boundary + 3 per-turn) |
| `agent/manager.rs` | Add `debug_entries` to `AgentState`; add `show_debug_window`, `session_counter`; handle `DebugEntry`; pass `session_number` to context |
| `ui/agent_debug_window.rs` | **New** — `show_agent_debug_window(app, ctx)` |
| `ui/mod.rs` | Add `pub mod agent_debug_window;` |
| `ui/app/update.rs` | Consume ALT+A shortcut at top of `update_ui()` |
| `ui/app/render.rs` | Call `show_agent_debug_window()` in `show_modals()` |
| `ui/strings.rs` | Window title, search label, clear button, auto-scroll, kind labels |
| `app/orchestrator.rs` | No changes — state lives on `agent: AgentSessionManager` |

### 9. Implementation order

1. **Data model** — `bus/events/debug.rs`
2. **Event plumbing** — `bus/events/typed.rs`, `bus/events/mod.rs`
3. **LLMClient accessors** — `agent/llm_client.rs`
4. **AgentContext** — add `session_number` field
5. **Manager accumulation** — `agent/manager.rs`: state fields, handler, `session_counter`
6. **Agent instrumentation** — `agent/agent_impl.rs`: turn counter, delta, 4 emission points
7. **UI strings** — `ui/strings.rs`
8. **UI window** — `ui/agent_debug_window.rs`
9. **Integration** — `update.rs` (ALT+A), `render.rs` (show window), `mod.rs`
10. **Tests** — sidecar tests for `agent_impl.rs`, UI tests for debug window

### 10. Consequences

- **Positive**: Full transparency into agent-LLM communication with minimal noise
  (outgoing shows deltas, not re-sent history).
- **Positive**: Session boundaries make it easy to find where one prompt ended
  and the next began.
- **Positive**: Follows established patterns — same channel, same window pattern,
  no new architectural concepts.
- **Positive**: ALT+A is discoverable, does not conflict with any existing shortcut.
- **Neutral**: Delta computation requires tracking `prev_messages_len` in the
  agent loop, a minor state addition.
- **Neutral**: Entries accumulate forever (never cleared). A typical session
  generates ~3-10 entries per turn × ~5-10 turns = ~15-100 entries. Even across
  many sessions this stays small. Memory is not a concern. A `[Clear]` button
  is provided for manual cleanup.
- **Neutral**: Window state (open/closed) resets on restart — not persisted.
