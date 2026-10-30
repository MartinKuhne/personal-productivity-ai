# UI/UX Design Review: FastMD Agent (Personal Productivity AI)

> **Status:** current — re-reviewed against the **iced** UI (`src/app/ui_iced/`)
> **Date:** 2026-10-19
> **Scope:** `src/app/ui_iced/`, its framework-free base (`src/app/ui/`), and the
> agent event layer (`src/agent/`, `src/app/agent/`, `src/app/bus/events/`)
> **Supersedes:** the 2026-09-04 review of the egui build (same file, `1d5eb0d`)
> **Requirements touched:** UI-001..066, REQ-450, REQ-460..465, FMD-060..072, NFR-001..009
>
> Every claim below carries a `file:line` citation. Contrast figures are computed
> directly against `src/app/ui/theme.rs:79-100` (see Appendix A). Statements that
> are inference rather than citation are labelled `[INFERENCE]`.

### How to read this revision

The previous review described the **egui** build, which has since been deleted
(`TASK-029b`). Findings are marked:

| Marker | Meaning |
|---|---|
| **(persist)** | Still true in the iced build |
| **(fixed)** | Resolved by the iced port |
| **(new)** | Introduced or newly discovered in the iced build |
| **(corrected)** | The previous review's claim was wrong and is retracted |

---

### 0. General UI/UX Fundamentals

**Rating: Needs Improvement** *(was Adequate)*

**Evidence:**

- **Feedback on disabled controls is actively harmful (new).** Clicking an inert
  `Ok` (empty name field) does not no-op — the click falls through the button to
  the modal scrim and **dismisses the dialog**. This is enshrined as expected
  behaviour in `interaction_tests.rs:649-653`: *"the inert Ok click must fall
  through to the modal scrim, which dismisses"*. A user clicking a disabled
  button because "nothing happened" loses their typed input.
- **A hint that is false (persist).** `strings.rs:363` —
  `"Type command (Enter to submit, Shift+Enter for new line)"`. The code's own
  doc contradicts it: iced `text_input` "publishes `on_submit` on Enter with
  **no modifier check**" (`panels/bottom.rs:56-65`). Shift+Enter submits.
- **Error-prevention asymmetry (new).** Single-target delete **is** now confirmed
  (`update.rs:324-335` → `DialogKind::DeleteConfirm`) — the previous review's
  headline gap is partly fixed. But multi-select delete is **not**: it fans out N
  unconfirmed deletes, *"Deliberately un-confirmed — `DialogKind::DeleteConfirm`
  carries a single path"* (`update.rs:649-660`). The more destructive action is
  the less protected one.
- **No undo mechanism (persist).** Escape drops the editor buffer with no dirty
  check (`app.rs:401` `self.editor.take()`), even though `TabEntry::unsaved` is
  known and painted `●` (`panels/center.rs:665-670`). Close-tab / Close-Others /
  Close-All execute immediately (`command_executor.rs:37-57`).
- **Terminology drift (new).** "Background operations" (`strings.rs:277`) vs
  "Background Processes" (`strings.rs:674`); **two distinct `UserCommand`
  variants for one window** (`ToggleAgentDebugWindow` vs `ToggleDebugWindow`,
  `user_command.rs:72` vs `:152`); "Chat models" / "Model:" / "Available
  Models:" (`strings.rs:289, 434, 373`); "Close" that actually *clears* the
  session (`prompt_input.rs:103-111`); Clear / Forget / Restart as near-synonyms.
- **~6 distinct button-styling approaches (new).** Bare `button(text(..))`, one
  shared `confirm_cancel_buttons` helper (`overlay.rs:448-466`), per-button
  ad-hoc `.color()` overrides (`prompt_input.rs:153-210`), icon-only Phosphor
  buttons, and `mouse_area` faking a button in three places
  (`agent_debug.rs:1073-1085`, `left.rs:981-996`, `center.rs:703-710`) —
  pointer cursor, **no keyboard activation, no disabled state**.
- **Palette and string discipline is genuinely strong (fixed).** Zero raw colour
  literals outside `ui/theme.rs`; all user text in `strings.rs` ([GUD-003],
  AGENTS §4). This makes every contrast fix below a one-token change.

**Anti-patterns identified:**

- **Inert button dismisses the modal** — Critical
- **False `Shift+Enter` hint** — Major
- **Multi-delete unconfirmed while single-delete is confirmed** — Major
- **No undo for any file operation** — Critical
- **Terminology drift / two commands for one window** — Minor
- **`mouse_area` substituted for `button`, killing keyboard access** — Major

**Recommendations:**

1. Give the modal card its own blocking `mouse_area` above the scrim, or make
   disabled buttons hit-testable. Update `interaction_tests.rs:649-653`, which
   currently asserts the bug.
2. Correct `COMMAND_INPUT_HINT` to describe what actually happens.
3. Route multi-select delete through a plural confirm dialog.
4. Adopt one shared `button` helper; ban `mouse_area` where `button` applies.
5. Collapse the duplicate agent-debug `UserCommand` variants.

---

### 1. Transparency & Reasoning Visibility

**Rating: Needs Improvement** *(was Adequate)*

**Evidence:**

- **Tool name and arguments are surfaced inline (persist, good).**
  `"> **Executing tool \`{}\`**\n"` with pretty-printed JSON
  (`ui/agent/agent_render.rs:22, 39`).
- **Tool output is a lie-by-omission (persist, worse than previously recorded).**
  `format_tool_result_message` (`agent_render.rs:76-219`) hand-maps ~9 tools to
  canned one-liners. Everything else hits the fallback at `:211-217`:
  `"> **Result (\`{}\`):** Tool '{}' completed successfully."` — the actual
  payload never reaches the user.
- **`create_note` hides its own arguments (new).** Only `path` is rendered;
  content, tags and overwrite flags are invisible (`agent_render.rs:21-29`).
- **No elapsed time exists to display (new).** `AgentEvent::ToolCallStarted` and
  `ToolResult` carry no timestamp or duration (`bus/events/agent.rs:43-56`).
- **No planning visibility at all (persist).** `AgentEvent` has exactly 11
  variants — `SessionStarted, SessionFinished, Status, Thinking, ContentDelta,
  ToolCallStarted, ToolResult, ToolSideEffect, DebugEntry, TokenUsage, Failed`
  (`bus/events/agent.rs:21-76`). There is no `PlanProposed`, `StepStarted` or
  `TodoUpdated`. The loop is `loop { process_turn() }` (`agent_impl.rs:65-85`)
  with no declared step count — the system cannot say how long it will take
  because it does not know.
- **Provenance is shown to the model and hidden from the human (new, and
  backwards).** `agent_impl.rs:317-335` wraps tool results in a `datamark`
  envelope carrying `provenance=tool:{name}` **for the LLM**; the user-facing
  formatter never reads it. `web_search` renders *"> {} search results
  returned"* with **no URLs, titles or snippets** (`agent_render.rs:197-200`).
- **Thinking disclosure is a real win (persist).** The UI-048 collapsible
  "Thinking Process" block (`agent/transcript.rs:202-240`).
- **`<span>` artefacts reach the renderer (new).** `format_delegate_trace` emits
  `<span>…**Executing tool**…</span>` (`agent_render.rs:46-67`). iced's markdown
  `Item` set has no raw/HTML variant (the module's own API table,
  `agent/transcript.rs:19-27`), so these render as **visible literal tags**.

**Anti-patterns identified:**

- **Generic "completed successfully" fallback hides the payload** — Major
- **Zero source attribution; provenance routed to the LLM instead of the user** — Major
- **No plan stage before execution** — Major
- **`create_note` argument suppression** — Major
- **Literal `<span>` tags in rendered output** — Minor

**Recommendations:**

1. Replace the generic fallback with a truncated raw-result preview plus a
   "view full output" action routing to the debug window.
2. Add `duration_ms` to `ToolResult`; render it on the tool row.
3. Surface `datamark` provenance as a clickable source line beside each result;
   show URLs/titles for `web_search`.
4. Add a `PlanProposed` / `StepStarted` event pair and a Review-Plan stage.
5. Strip or convert the `<span>` wrapper in `format_delegate_trace`.

---

### 2. User Control & Intervention

**Rating: Missing** *(was Needs Improvement)*

**Evidence:**

- **The approval seam exists and is deliberately empty (persist).**
  ```rust
  impl ToolCallPolicy for DefaultToolCallPolicy {
      fn check_write_allowed(&self, _path: &Path) -> Result<(), String> { Ok(()) }
  }
  ```
  (`agent/tools/policy.rs:4-16`), wired unconditionally at `agent/context.rs:269`,
  `agent/session.rs:381`, `agent/tool_executor.rs:130`. The only rejecting
  implementation in the repo is a test fixture (`RejectingPolicy`).
- **Execution is un-gated at the call site (persist).**
  `ctx.observer.on_tool_call_started(...)` then `executor.execute_all(tc)`
  (`agent_impl.rs:208-217`). The user sees "Executing tool X" *as it happens* —
  the file is already written.
- **Stop is not cancellation (new, and more serious than previously recorded).**
  `cancel()` sets the flag and immediately reports `"Aborted by user."`
  (`agent/session.rs:233-240`), but the flag is polled **only at the top of the
  turn loop** (`agent_impl.rs:65-68`). Nothing inside `process_turn` checks it —
  a turn already inside `llm.chat_completion(...)` (`:137`) or `execute_all(tc)`
  (`:212`) runs to completion. Meanwhile `cancel_enabled(running)` → false, so
  the Stop button **vanishes while the thread keeps burning API calls and writing
  files** (`prompt_input.rs:179-188`).
- **No step-level control (persist).** No skip, no edit-and-continue, no
  "review step 4". Control is binary: run or Stop.
- **No resume control (persist).** `cancel()` never resets the flag; a new prompt
  mints a fresh one (`session.rs:261-263`). Continuation works only implicitly
  and only if the aborted turn reached `SessionFinished`.
- **No progressive delegation or trust levels (persist).**

**Anti-patterns identified:**

- **No approval gate before autonomous file writes/deletes** — Critical
- **Stop reports success synchronously while work continues** — Critical
- **Control is binary (run-all / abort-all)** — Critical
- **No resume after interruption** — Major

**Recommendations:**

1. Implement an `AskFirst` `ToolCallPolicy` and a blocking approval card that
   holds `execute_all` until answered.
2. Propagate the cancel flag into `process_turn`, the LLM call and per-tool
   execution. Failing that, change the status to *"Stopping — finishing current
   step…"* and keep the button live until the thread actually exits.
3. Add "Pause after this step" distinct from hard stop.
4. Add an explicit Resume that preserves session state.

---

### 3. Trust Calibration

**Rating: Missing** *(was Needs Improvement)*

**Evidence:**

- **Zero confidence signalling (persist).** Searched
  `confidence|citation|uncertain|hallucin` across `src/app` — no UI hits.
- **Progressive disclosure exists but is buried (persist, with a dead shortcut).**
  The Agent Debug window exposes full request/response/tool JSON, `EXTERNAL_DATA`
  provenance envelopes, per-row Copy JSON and collapsed-by-default disclosure
  (`agent_debug.rs:933-990, 1037-1052`). It sits two clicks deep in
  hamburger → "Windows" → "Agent debug" with no hint that it holds the
  unredacted I/O the transcript hides. Its `ALT+A` shortcut is **dead**:
  `UserCommand::ToggleAgentDebugWindowShortcut` (`user_command.rs:138-140`) has a
  handler (`command_executor.rs:81`) and **no emitter** anywhere in `ui_iced`.
- **A label that promises a retry it does not perform (new).**
  `TOOLS_RESTART = "Restart"` (`strings.rs:135-136`) is wired to
  `clear_error_command` → `UserCommand::ClearToolGroupError`
  (`tools_dialog.rs:468-473`). It clears the error record. It does not re-run
  discovery or re-authenticate.
- **No honest capability boundary (persist).** An empty transcript renders
  literally nothing (§7).
- **Full autonomy from the first query (persist).** No graduated trust.

**Anti-patterns identified:**

- **No confidence indicators** — Major
- **"Restart" that only clears an error record** — Major
- **Deep-buried transparency surface with a dead shortcut** — Major
- **Full autonomy demanded at launch, not earned** — Major

**Recommendations:**

1. Add binary High/Low confidence markers — binary outperforms numeric
   percentages in NN/G usability testing.
2. Wire `ALT+A`; promote the debug window's contents in its label or add a
   "what's in here" hint.
3. Rename `TOOLS_RESTART` → "Clear error", or make it actually re-run discovery.
4. Add a graduated autonomy ladder keyed to observed reliability.

---

### 4. Information Architecture

**Rating: Adequate** *(unchanged)*

**Evidence:**

- **Real surface separation (fixed).** The conversation panel, the **Background
  Processes** window and the **Agent Debug** window are distinct surfaces on
  distinct z-layers: `OverlayLayer::{ContextMenu, Editor, Dialog, Logs,
  AgentDebug, Tools, About, Attributions, Batch}`, bottom-most first
  (`app.rs:409-428`). The previous review's "center panel does dual duty" is
  superseded.
- **Autonomous work is still conflated into the chat thread (persist, and the
  cause is now explicit).** Tool-call blockquotes are inline assistant markdown.
  `panel_state.rs:15-27` documents that the port deliberately renders the flat
  `content` buffer rather than the structured `blocks`, because the in-place
  task-toggle rewrite and the bus-lag marker only mutate `content`.
  `TranscriptBlock::ToolCall { id, name, args, result }`
  (`ui/agent/transcript.rs:22-27`) is **never rendered** — a complete typed
  tool-call model sitting as dead code.
- **No workflow timeline (persist).** Nothing shows done / running / blocked /
  next, and nothing survives interruption.
- **Chat-only for long-horizon tasks (persist).**

**Anti-patterns identified:**

- **Conversation thread doubles as activity log** — Major
- **No persistent workflow timeline** — Major
- **Typed `TranscriptBlock::ToolCall` model unused** — Major

**Recommendations:**

1. Render `TranscriptBlock::ToolCall` as a distinct, collapsible, colour-coded
   card instead of markdown prose — the model already exists.
2. Add a persistent run timeline in the activity surface.
3. Resolve the `content`-vs-`blocks` tension so the structured model can be the
   render source (the in-place mutations are the blocker).

---

### 5. Status Communication

**Rating: Needs Improvement** *(was Strong — the previous rating is retracted)*

**Evidence — the complete status vocabulary (9 values, all generic):**

| Literal | Source |
|---|---|
| `""` | default (`session.rs` state default) |
| `"Initializing agent..."` | `AGENT_LAUNCHING_STATUS`, `session.rs:258` |
| `"Agent launch failed."` | `session.rs:266` |
| `"Aborted by user."` | `session.rs:239` — bare literal, violates RUST-021 |
| `"Waiting for LLM completions..."` | `agent/events.rs:133` |
| `"Executing tools..."` | `agent/events.rs:134` |
| `"Done"` | `events.rs:135` (duplicated as a literal at `command_executor.rs:19`) |
| `"Error: {raw}"` | `orchestrator.rs:402` |
| `"Error"` | `command_executor.rs:26` |

- **"Executing tools..." while the count and names are known at that exact
  instant (persist, and trivially fixable).** `let tool_count =
  tools_json.as_array()...` (`agent_impl.rs:114`) and the loop over every named
  call (`:190-210`) both *precede* `on_status(ExecutingTools)`. The debug entry
  even parameterises it — `"Turn {} — Outgoing (+{} messages, {} tools)"`
  (`:120-125`). The information exists; the user-facing line discards it.
- **Nothing streams (new, contradicts the previous review).** The previous review
  stated tool inputs/outputs "ARE shown as they happen (streaming)". They are
  not. `llm.chat_completion(...)` is a blocking, non-streaming call
  (`agent_impl.rs:137`) and `handle_content` emits **one** `ContentDelta`
  carrying the entire assistant message (`:286-294`). The user sees
  *"Agent is thinking..."* for the whole turn, then a wall of text. iced's
  incremental `Content::{parse, push_str, items}` buffer is listed in this
  module's own API table (`agent/transcript.rs:23`) and never called.
- **No spinner, no progress bar, no elapsed timer, no step counter (new).**
  No `progress_bar` anywhere in `ui_iced`; the spinner is explicitly excluded
  (`batch_dialog.rs:55`). Wall-clock timestamps exist per debug row
  (`agent_debug.rs:198`) but deltas are never derived.
- **Dropped-output notice goes into the wrong buffer (new).** `handle_bus_lag`
  pushes `LAG_TRUNCATION_MARKER` into **`.thinking`** (`update.rs:1323-1326`) —
  the collapsed Thinking block — while the orchestrator's own path writes it into
  `.content` (`orchestrator.rs:434-448`). Both are live. Path B also omits
  newlines, so repeated lags glue into
  `[output truncated…][output truncated…]`. The dropped `count` never reaches
  the user (`subscription.rs:497-503`), against the spirit of `[FMD-072]`.
- **`filtered_out()` is computed and never rendered (new).** Both
  `AgentDebugViewModel::filtered_out()` (`agent_debug.rs:867-871`) and
  `BackgroundLogsViewModel::filtered_out()` (`logs.rs:559-561`) carry
  `[FMD-067]` "report the loss" doc comments and are **called only in tests**.
- **Token and cost usage is never displayed (new).** `apply_token_usage` maintains
  `token_usage` and `total_usage` (`session.rs:188-217`); no view reads them.
- **Agent failure is visually indistinguishable from health (new).**
  `"Status: Error: {raw}"` renders in `palette.text_strong` with styling
  identical to a healthy status (`agent/mod.rs:128-134`).

**Anti-patterns identified:**

- **No actual streaming behind a static "Agent is thinking…"** — Critical
- **Truncation marker lands in the collapsed thinking buffer, not the transcript** — Major
- **Silent event loss: `filtered_out()` and drop counts never shown** — Major
- **Error status indistinguishable from normal status** — Major
- **Generic status strings while specifics are computed and discarded** — Major (near-zero-effort fix)
- **No elapsed/progress indicator of any kind** — Major

**Recommendations:**

1. Parameterise the status line from data already in hand:
   *"Executing 3 tools: read_note, web_search, create_note"*.
2. Add an elapsed timer during LLM calls.
3. Fix the lag marker to land in `content`, with newlines and the dropped count.
4. Render `filtered_out()` in both viewers.
5. Surface token/cost in the debug window header.
6. Render agent failures in `palette.error` with an icon.

---

### 6. Error Recovery

**Rating: Missing** *(was Adequate — the previous "excellent error categorization" rating is retracted)*

**Evidence:**

- **Three-part error messages: 1 of ~9 (persist).** Only `PDF_BACKED_ERROR`
  (`strings.rs:743-744`) delivers what / why / next — and its "next" names an
  internal API (`write_yaml_header`), not a UI action. Everything else is a bare
  statement: `"Name must not be empty."` (`:555`), `"Name contains invalid
  characters."` (`:559` — does not say *which*), `"Select a destination
  folder."` (`:563`), `"No dialog is open."` (`:567` — internal-state leak),
  `"Error: could not access background manager"` (`:707`).
- **The worst failure in the app is completely silent (new).** The editor save
  path (`update.rs:839-847`) logs the `fs::write` error and `return`s **before**
  `close_editor` (line 854). The modal stays open, the buffer is unchanged, and
  the validation line (`editor.rs:596-603`) is driven only by
  `MarkdownParseError`, never by I/O failure. **The user clicks Save, nothing
  happens, and receives no signal why.**
- **No retry affordance exists anywhere (persist, and stronger than recorded).**
  Searched `retry|try again|attempt|reconnect|recover` in `ui_iced` — two hits,
  both prose in doc comments (`os_shell.rs:31, 49`). The only real retry message
  in the repo is in the Discord integration (`bot.rs:174`).
- **Errors reach the view as pre-formatted strings, so the view cannot route
  recovery (new).** `AgentEvent::Failed { error: String }` is untyped
  (`bus/events/agent.rs:75`). `ToolErrorKind` — a real taxonomy of
  `Discovery / Authentication / Execution / ConfigInvalid`
  (`agent/tools/registry/errors.rs:7-31`) — reaches the user as Rust `Debug`
  output via `format!("{:?}", error.kind)` (`tools_dialog.rs:380`), i.e. the
  literal string `"ConfigInvalid"`, with no `strings.rs` mapping, unlike every
  other user-facing label in the app.
- **Rich error types collapsed before the view (new).** `RecycleBinError { Io,
  Aborted, Com, Trash }` (`utils/recycle_bin.rs:157-181`) is collapsed to a
  single `tracing::error!` (`command_executor.rs:266-271`); the user-aborted vs
  I/O-failure distinction is lost.
- **No partial-success concept (persist).** `BatchRunStatus` models
  `has_handle / finished / cancel_requested` only (`batch_dialog.rs:130-176`) —
  no per-file success/failure counts, despite batch work being inherently
  partial-failure-prone.
- **Log-only failures with no user surface (new):** file read for "Run as prompt"
  (`update.rs:695-703`), skill read (`:726-734`), print hand-off
  (`:638-648`), delete / create / rename
  (`command_executor.rs:265-387`), UI-state persist (`update.rs:535-541`), and
  `os_shell.rs:33-54` — `let _ = opener::open(...)`, which fails without even a
  log.
- **The previous review's "6+ distinct failure modes" is a producer-side
  observation that does not survive the boundary.** Those LLM-client messages are
  flattened into `Failed { error: String }` and rendered as one undifferentiated
  status line.

**Anti-patterns identified:**

- **Silent data loss on failed save** — Critical
- **No retry affordance of any kind** — Critical
- **Untyped errors at the view boundary prevent routed recovery** — Major
- **Developer `Debug` identifiers (`ConfigInvalid`) shown to users** — Major
- **No partial-failure model for batch operations** — Major
- **Many failures log-only, including one that swallows the error entirely** — Major

**Recommendations:**

1. Introduce a typed `UiError { kind, message, recovery }` at the view boundary.
2. Route the editor `fs::write` error into the existing validation slot and keep
   the modal open with a three-part message.
3. Enforce the what / why / what-next shape for every user-facing error.
4. Add kind-aware Retry (re-discover, re-authenticate, re-write) rather than a
   single generic button.
5. Map `ToolErrorKind` to human strings in `strings.rs`.
6. Add per-item success/failure counts to batch results.

---

### 7. Conversational Design

**Rating: Missing** *(was Adequate)*

**Evidence:**

- **First run is a blank void (new, and a direct rule violation).**
  `transcript_body` with no turns, no thinking and not running produces
  `Column::with_children(vec![])`. The module confirms it: *"an empty transcript
  renders an empty column"* (`agent/transcript.rs:284-286`). The only test is
  `empty_body_builds` (`agent/transcript_tests.rs:176-180`), which asserts
  nothing but non-panic. This violates the project's own rule
  (`src/app/AGENTS.md` §6: *"render an explicit placeholder element rather than
  an empty container"*).
- **`is_empty()` exists and is correct but no view consumes it (new).**
  `AgentPanelViewModel::is_empty` (`panel_state.rs:363-367`) is never called
  from any render function.
- **No welcome message, capability description, suggested prompts or docs link
  (persist).** Searched `onboard|welcome|getting started|suggested|example
  prompt` — absent.
- **No copy, no save, no share — despite four strings already written for it
  (persist, and closer than recorded).** `COPY_MARKDOWN_BUTTON = "Copy markdown"`,
  `COPIED_BUTTON = "Copied!"`, `SAVE_AS_FILE_BUTTON = "Save as file..."`,
  `SAVED_BUTTON = "Saved!"` (`strings.rs:448-457`) are all dead — no consumer in
  `ui_iced`.
- **Scroll behaviour is now genuinely good (fixed).** The previous review's
  "forced auto-scroll" complaint is resolved: follow-the-stream is a single
  shared rule, `should_follow = auto_scroll && viewport.is_at_bottom()`
  (`logs.rs:422`), reused across three windows. The view only *reports*
  geometry via `on_scroll` (`agent/transcript.rs:288-294`) and the reducer
  decides — so a user who has scrolled up is not yanked to the bottom.
- **Context awareness is present (persist).** Active model, queue depth and
  indexing state are all shown.
- **Voice input: absent (persist).**

**Anti-patterns identified:**

- **Empty first-run state renders nothing** — Major (violates AGENTS §6)
- **No capability introduction or suggested prompts** — Major
- **No copy/save/share despite existing strings** — Major
- **`is_empty()` computed but unused** — Minor

**Recommendations:**

1. Render an explicit empty state: what the agent can do in *this* workspace,
   3–4 clickable example prompts wired to `AgentMessage::Submit`, and a docs link.
2. Wire the four dead copy/save strings.
3. Offer contextual follow-up suggestions as buttons after each turn.
4. Add voice-to-text as an accessibility option.

---

### 8. Generative UI

**Rating: Missing (Not Applicable)** *(unchanged)*

No agent-generated UI components. The UI is a fixed widget tree; the agent emits
markdown text only. There is no generative-UI injection surface.

**Caveat (new):** the `<span>` passthrough in §1 shows agent-authored HTML
fragments do reach the renderer's text path. Harmless today (rendered as literal
characters), but worth a sanitisation pass before any richer markdown surface is
added.

---

### 9. Human-AI Balance

**Rating: Missing** *(was Adequate)*

**Evidence:**

- **No adjustable autonomy (persist).** One mode: full autonomy, no way to
  reduce it (§2).
- **No co-pilot / suggest-then-accept pattern (persist).** The agent acts; the
  user watches.
- **Contextual help is deliberately absent — behind a factually incorrect
  blocker (new).** `panels/left.rs:86-89`: *"iced 0.14 has no tooltip primitive
  under DEC-005… so the search/clear buttons' the legacy UI tooltips… are not
  rendered."* **This is wrong.** `tools_dialog.rs:54-56` uses
  `iced::widget::tooltip::Tooltip` successfully — and `about.rs:64` makes the
  same false claim. Five tooltip strings exist and are never shown:
  `SEARCH_TRIGGER_TOOLTIP`, `SEARCH_CLEAR_TOOLTIP`, `CODE_COPY_BUTTON_TOOLTIP`,
  `CLOSE_TAB_TOOLTIP`, `ABOUT_COPY_COMMIT_TOOLTIP`
  (`strings.rs:60, 177, 180, 214, 491`).
- **Familiar real-world vocabulary is mostly sound (persist).**
  Delete / Cancel / Rename / Move / Close, tab bar, file tree.

**Anti-patterns identified:**

- **No autonomy levels (read-only / ask-on-write / full)** — Major
- **Help suppressed by a blocker that does not exist** — Major
- **No suggest-mode / co-pilot loop** — Major

**Recommendations:**

1. Use the tooltip widget that already ships and is already in use, and recover
   the five orphaned strings. Correct `about.rs:64` and `left.rs:86-89`.
2. Add an autonomy selector: Read-only / Suggest / Ask before writing / Full.
3. Add confirmation prompts for write operations even in full-autonomy mode.

---

### 10. Accessibility & Inclusion

**Rating: Missing** *(was Needs Improvement)*

**Contrast — computed against `ui/theme.rs:79-100` (full working in Appendix A):**

| Foreground | Background | Ratio | AA normal (4.5:1) |
|---|---|---|---|
| `error` on `accent` | red on indigo | **1.05:1** | **FAIL — effectively invisible** |
| `text_faint` on `accent` | | 1.33:1 | FAIL |
| `text_muted` on `accent` | | 1.71:1 | FAIL |
| `text` on `accent` | every selected tree row | **2.95:1** | FAIL (even large text) |
| `log_timestamp` on `surface` | | 3.16:1 | FAIL |
| `error` on `surface_variant` | tools-dialog errors @ 12px | 4.38:1 | FAIL |
| `accent` on `surface` | markdown links @ 14px | 4.45:1 | FAIL |
| `text_strong` on `accent` | active tab @ 13px | 4.47:1 | FAIL |
| `text` / `text_strong` / `text_muted` / `warning` on `surface` | | 13.2 / 19.9 / 7.6 / 6.9 | PASS |

- **The 1.05:1 case is a real, reachable combination (new).** `tree_row` paints
  the label `palette.error` for PDF-backed files (`left.rs:965` →
  `row_text_color`, `:1004-1010`, REQ-450) and paints the row background
  `palette.accent` when selected (`left.rs:977`). Selecting a PDF-backed file
  yields red-on-indigo.
- **No focus management whatsoever (new).** Zero production hits for
  `focus_next|focus_previous|request_focus|focused`. No focus moved into a modal
  on open, **no focus trap**, no restoration on close. The scrim is a
  `mouse_area` (`overlay.rs:487-499`) — it blocks the **mouse** only.
  `[INFERENCE]` Tab can traverse to widgets behind a blocking modal.
- **Icon-only controls with no accessible equivalent (new — reverses the
  previous "buttons are text-labeled, which is good").** Search
  (`left.rs:887-894`), clear ✕ (`:896-905`), tab close ✕
  (`center.rs:680-685`), code-block copy (`render/code.rs:131-136`), and the
  hamburger itself — `HAMBURGER_MENU_BUTTON = ICON_LIST`, a private-use-area
  codepoint U+E2F0 (`strings.rs:234`).
- **No accessibility tree (new).** Searched `aria|role|semantic|a11y` — the only
  hit is aspirational prose in `search.rs:278`. `[INFERENCE]` iced 0.14 provides
  no accessibility-tree mapping, so no label, role or state can reach a screen
  reader.
- **Colour-only meaning (persist).** Selection state carries no checkmark, border
  or weight change — only the `accent` fill. PDF-backed tree rows are marked by
  red text alone.
- **Sub-24px hit targets (new).** Tab close ✕ ≈13–17px (13pt glyph, no padding,
  inside a 4px-padded tab), tree rows ≈18–22px, ToC rows ≈20px, hamburger and
  pick-lists ≈20px (`PICK_LIST_PADDING` 2.0 vertical, `top.rs:295-300`).
- **11px text (new).** `SECONDARY_TEXT_SIZE` (`left.rs:151`),
  `CODE_LANGUAGE_SIZE` (`render/code.rs:38`).
- **Font scaling is orphaned (new).** `font_size_scale` is persisted
  (`ui/persisted.rs:55`), sanitised (`update.rs:965-976`) and applied
  (`app.rs:188-192`, DEC-008) — but `ThemeMessage::ScaleChanged` is emitted
  **only from tests**. No control exists.
- **Good redundant cueing where it was applied (persist).** Stop (label always
  present), tool errors (`⚠` + text body), `✓ parallel` chip, queued badge,
  indexing status.

**Anti-patterns identified:**

- **1.05:1 contrast on selected PDF-backed rows** — Critical
- **2.95:1 on every selected tree row** — Critical
- **No focus trap / no focus management on modals** — Major
- **Icon-only controls with unrendered tooltip strings** — Major
- **Colour-only selection and PDF-backed markers** — Major
- **Sub-24px hit targets, incl. a 13px tab-close** — Major
- **11px body text** — Minor
- **Font-scale feature orphaned with no control** — Minor

**Recommendations:**

1. Fix the selected-row colour matrix: force `text_strong` on the `accent` fill,
   and give PDF-backed rows a non-colour marker (suffix or icon) instead of
   red-only.
2. Add a focus-trap + focus-into-modal helper to `overlay::modal`.
3. Replace icon-only buttons with icon+label, or at minimum render the five
   existing tooltip strings.
4. Raise the 11px minimum to 12px; grow the tab-close hit target to ≥24px.
5. Ship the orphaned font-scale control.

---

## Executive Summary

The iced port is architecturally disciplined — strict view-model snapshots, a
single colour-token source, pure view functions and a clean command channel make
this a genuinely maintainable UI codebase, and details like the shared
follow-the-stream rule show real UX care. But the **AI-facing dimensions are
largely unbuilt**: there is no approval gate before the agent writes or deletes
files (`DefaultToolCallPolicy` returns `Ok(())` unconditionally), nothing
actually streams (one blocking `chat_completion` per turn behind a static
"Agent is thinking…"), and Stop reports `"Aborted by user."` synchronously while
the turn runs to completion, burning API cost and writing files after the button
has disappeared. Error handling is the weakest layer: the most damaging failure —
a failed editor save — is completely silent, errors arrive at the view as
pre-formatted strings that make routed recovery impossible, and there is no retry
anywhere. Accessibility is unaddressed, with a reachable **1.05:1** contrast
combination and no focus management at all. Notably, several blockers cited in
code comments for missing help and tooltip features are factually wrong — the
widget exists and is already used elsewhere in the same tree.

Two ratings from the previous review are explicitly retracted: **Status
Communication (Strong → Needs Improvement)**, because the claimed streaming does
not exist; and **Error Recovery (Adequate → Missing)**, because the praised
failure-mode categorisation does not survive the boundary into the view.

---

## Top 3 Strengths

1. **View-model purity and one-way data flow.** Every panel renders from an
   immutable snapshot and emits only `Message::Command(UserCommand)`; the
   reducer owns all effects. `[FMD-065/068/069]` are genuinely honoured, which
   is what makes the whole UI unit-testable without a live runtime.
2. **Single-source colour and string discipline.** Zero raw colour literals
   outside `ui/theme.rs`; all user text in `strings.rs`. A brand or locale change
   touches one file — and every contrast fix below is a one-token change.
3. **Respectful scroll plus real progressive disclosure.** One `should_follow`
   rule shared across three windows means users are never yanked to the bottom
   mid-read, and the Agent Debug window offers genuinely deep drill-down (raw
   request/response, tool I/O, provenance envelopes, per-row Copy JSON,
   collapsed by default).

---

## Top 3 Critical Issues

1. **No human-in-the-loop before destructive agent actions, and Stop doesn't
   stop.** `DefaultToolCallPolicy::check_write_allowed` is a no-op wired at three
   call sites; `execute_all` runs un-gated; the cancel flag is polled only at
   loop top while the UI immediately shows `"Aborted by user."` and hides the
   Stop button. Users cannot prevent, and cannot reliably halt, autonomous file
   mutation.
2. **Silent data loss on save, with no recovery path.** A failed `std::fs::write`
   logs and `return`s before closing the editor (`update.rs:839-847`); the
   validation slot only renders markdown errors, so the user sees nothing.
   Combined with **zero retry affordances** and untyped
   `Failed { error: String }`, there is no way to diagnose or recover.
3. **Accessibility failure at the most common interaction.** Selecting a
   PDF-backed tree row yields **1.05:1** contrast (red on indigo,
   `left.rs:965/977/1004-1010`); every selected row is 2.95:1. Add no focus
   trap on modals, icon-only controls whose tooltip strings exist but are
   deliberately unrendered behind a false blocker, and colour-only selection
   state.

---

## Priority Action Plan

| # | Action | Impact | Effort | Dimension |
|---|--------|--------|--------|-----------|
| 1 | Fix selected-row contrast: force `text_strong` on `accent` fill; give PDF-backed rows a non-colour marker instead of red-only | Critical | **XS** (~3 lines in `left.rs` + 1 token) | Accessibility |
| 2 | Fix silent editor-save failure: route the `fs::write` error into the existing validation slot, keep the modal open with a three-part message | Critical | **S** (`update.rs:839-847` + `editor.rs` slot) | Error Recovery |
| 3 | Fix the inert-button→scrim fall-through so a disabled `Ok` no longer dismisses the dialog; update `interaction_tests.rs:649-653` which currently asserts the bug | Critical | **S** (card-level blocker in `overlay::modal_at`) | General UI/UX |
| 4 | Make Stop honest: propagate the cancel flag into `process_turn` / LLM call / tools, or change status to *"Stopping — finishing current step…"* and keep the button live until the thread exits | Critical | **M** | User Control |
| 5 | Add a real approval gate: an `AskFirst` `ToolCallPolicy` impl plus a blocking approval card before `execute_all` | Critical | **M–L** | User Control |
| 6 | Parameterise the status line from data already computed — *"Executing 3 tools: read_note, web_search, create_note"* — plus an elapsed timer | High | **XS** (`events.rs:128-138`, `agent_impl.rs:114/190-210`) | Status |
| 7 | Fix the bus-lag marker: write to `content` not `thinking`, with newlines and the dropped count; render `filtered_out()` in both viewers | High | **S** (`update.rs:1313-1326`) | Status / Error Recovery |
| 8 | Replace the generic tool-result fallback with a truncated raw preview + "view full output"; show `create_note`'s hidden args; add `duration_ms` to `ToolResult` | High | **M** | Transparency |
| 9 | Ship an explicit empty-transcript state: capability summary + clickable example prompts; wire the four dead copy/save strings | High | **S** | Conversational Design |
| 10 | Introduce a typed `UiError { kind, message, recovery }` at the view boundary; stop `format!("{:?}", kind)` leaking `ConfigInvalid` to users; add kind-aware Retry | High | **M** | Error Recovery |
| 11 | Add a focus-trap + focus-into-modal helper to `overlay::modal`; stop Escape discarding a dirty editor without a dirty check | High | **M** | Accessibility / General UI/UX |
| 12 | Use the tooltip widget that already exists (`tools_dialog.rs:54`) to recover the 5 orphaned tooltip strings; correct `about.rs:64` and `left.rs:86-89` | Medium | **S** | Human-AI Balance / Accessibility |
| 13 | Route multi-select delete through a plural confirm dialog (parity with single delete) | Medium | **S** | General UI/UX |
| 14 | Surface token/cost usage (already accumulated at `session.rs:188-217`) in the debug header; wire the dead `ALT+A`; ship the orphaned font-scale control | Medium | **S** | Trust / Accessibility |
| 15 | Fix the false `Shift+Enter` hint; add a shortcut cheatsheet | Medium | **XS** | General UI/UX |
| 16 | Surface `datamark` provenance as clickable source lines beside tool results; show URLs/titles for `web_search` | Medium | **M** | Transparency / Trust |
| 17 | Render `TranscriptBlock::ToolCall` as a distinct collapsible card (the typed model is currently dead code); add a run timeline | Medium | **M–L** | Information Architecture |
| 18 | Terminology consolidation + one shared `button` helper; ban `mouse_area`-as-button; raise 11px → 12px; grow the tab-close target to ≥24px | Low | **M** | General UI/UX / Accessibility |

**Quick wins worth doing today:** #1, #2, #3, #6, #15 — all XS/S, all
high-visibility, and the palette/string discipline means each is a handful of
lines.

---

## Appendix A — Contrast arithmetic

sRGB relative luminance `L = 0.2126R + 0.7152G + 0.0722B`, each channel
linearised as `c/12.92` if `c ≤ 0.03928` else `((c + 0.055)/1.055)^2.4`.
Ratio `= (L_lighter + 0.05) / (L_darker + 0.05)`.

Base surface `surface = RGB(9, 9, 11)` → `L = 0.0028`.

Worked examples:

- `text_muted (160,160,160)` → `L = 0.3515`;
  `(0.3515 + 0.05) / (0.0028 + 0.05) = 0.4015 / 0.0528` → **7.61:1**
- `accent (99,102,241)` → `L = 0.1851`;
  `0.2351 / 0.0528` → **4.45:1**
- `error (244,15,2)` → `L = 0.1958`;
  `0.2458 / 0.0528` → **4.66:1**
- `error` vs `accent`: `(0.1958 + 0.05) / (0.1851 + 0.05) = 0.2458 / 0.2351` → **1.05:1**

Token values are taken verbatim from `Palette::fastmd_dark()`,
`src/app/ui/theme.rs:79-100`.

---

## Appendix B — Resolved or corrected since the 2026-09-04 review

| Previous finding | Status in the iced build |
|---|---|
| "No confirmation for delete" | **Partly fixed** — single delete is confirmed (`update.rs:324-335`); multi-select is still unconfirmed (`:649-660`) |
| "Forced auto-scroll during streaming" | **Fixed** — `should_follow = auto_scroll && viewport.is_at_bottom()` (`logs.rs:422`) |
| "Buttons are text-labeled, not icon-only, which is good" | **No longer true** — five icon-only controls incl. a PUA-codepoint hamburger (§10) |
| "Tool inputs and outputs ARE shown as they happen (streaming)" | **Retracted** — one blocking `chat_completion` per turn, one `ContentDelta` carrying the whole message (§5) |
| "Excellent error categorization" | **Retracted** — categorisation exists producer-side but is flattened to `Failed { error: String }` before the view (§6) |
| "Status Communication: Strong" | **Retracted** — 9 generic literals, no elapsed/progress indicator, truncation notice misfiled (§5) |
| "Tab overflow breaks the layout" | **Not re-verified** in this pass — tab overflow handling was not re-examined in the iced build |
| "No undo mechanism" | **Persists** — Critical (§0) |
| "No approval gates" | **Persists, worse** — the policy seam exists and is a verified no-op (§2) |
| "No confidence signalling" | **Persists** (§3) |
| "No adjustable autonomy" | **Persists** (§9) |
| "No save/share for conversations" | **Persists** — and four strings for it already exist unused (§7) |
