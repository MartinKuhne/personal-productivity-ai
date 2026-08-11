# Feature Specification: Agent Loop / UI Seam Refactor

**Feature Branch**: `003-agent-ui-seam-refactor`

**Created**: 2026-08-10

**Status**: Draft

**Input**: User description: "Agent Loop / UI Seam Refactor — introduce a clean seam between the agent LLM/tool-call loop and the UI rendering layer. Remove all UI concerns from the agent layer via a prompt input channel, a structured event output channel, and a typed tool side-effect return. Split the god-object session manager into agent lifecycle state and pure UI panel state."

## User Scenarios & Testing *(mandatory)*

<!--
  User stories are ordered as user journeys by priority. Each is independently
  testable and delivers a standalone slice of value. The refactor is delivered
  incrementally (small, manageable iterations per the constitution's Modularity
  principle); these stories map onto that incremental delivery.
-->

### User Story 1 - Agent Output Renders Unchanged Through a Decoupled Channel (Priority: P1)

An end-user submits a prompt to the agent. While the agent works, the user
sees real-time streaming of the agent's thinking, assistant content, tool
calls (with their arguments), and tool results — rendered exactly as they are
today. The agent continues to run asynchronously without stalling the UI, and
cancelling a turn still stops it cleanly. After the refactor, the only
difference the user perceives is none: identical output, identical timing.

**Why this priority**: This is the no-regression guarantee. The refactor must
not change observable agent behavior for end-users; every other story builds
on top of this contract being preserved.

**Independent Test**: Can be fully tested by submitting a representative
prompt (one that triggers thinking, content, and at least one tool call) and
asserting the rendered transcript matches the pre-refactor baseline byte-for-
byte, while confirming the UI never freezes during the turn.

**Acceptance Scenarios**:

1. **Given** a running app with an active agent session, **When** the user
   submits a prompt that invokes a tool, **Then** thinking, content, tool
   call, and tool result all appear in real-time in the same order and format
   as before the refactor.
2. **Given** an in-progress agent turn, **When** the user cancels, **Then**
   the turn stops promptly, the session ends cleanly, and the UI returns to
   an idle state.
3. **Given** a prompt that produces a large multi-paragraph response,
   **When** the agent streams it, **Then** the UI renders incrementally
   without freezing and the final rendered text matches the full response.
4. **Given** an agent session that creates a note via a tool, **When** the
   tool succeeds, **Then** the file/tag/tree tabs reindex exactly as they did
   before the refactor.

---

### User Story 2 - Agent Layer Is Testable Without the UI (Priority: P2)

A maintainer writes unit tests for the agent loop by submitting a prompt and
asserting on structured events that the agent publishes — with no UI
framework, no UI state, and no UI channel handles involved. The agent layer
imports nothing from the UI, holds no widget state, and formats no
user-facing display strings. A test harness feeds prompts and reads events;
that is the entire surface.

**Why this priority**: Testability is the core constitutional driver
(Principle I) and the primary motivation for the refactor. Once the seam
exists, every subsequent change to the agent becomes cheaper and safer.

**Independent Test**: Can be fully tested by compiling and running the agent
layer's unit test suite in isolation (no UI crate on the dependency path) and
confirming a new prompt→events test passes with no UI setup.

**Acceptance Scenarios**:

1. **Given** the agent layer compiled in isolation, **When** a test submits a
   prompt and collects published events, **Then** the events include
   thinking, content deltas, tool call, and tool result in order — with no UI
   dependency required to build or run the test.
2. **Given** the agent source tree, **When** scanned for UI imports/references,
   **Then** zero UI modules, UI state types, or UI channel handles are
   referenced.
3. **Given** a tool-execution path, **When** inspected, **Then** it references
   no UI channel or UI type; it communicates side effects as returned data.

---

### User Story 3 - UI Restyles Agent Output From Structured Data (Priority: P3)

A maintainer changes how tool calls, tool results, or thinking sections are
displayed — for example, collapsing tool arguments, reformatting JSON, or
splitting thinking from content differently. They do this by reading the
structured event data the UI already accumulates, not by re-parsing formatted
markdown strings produced by the agent. The presentation layer (how thinking
is split, how tool calls are formatted) lives entirely on the UI side.

**Why this priority**: This unlocks iterative UI improvements without
touching the agent, reinforcing the seam and making the app easier to evolve.

**Independent Test**: Can be fully tested by changing a display formatting
rule in the UI layer alone and confirming the rendered output changes without
any agent-layer edit or recompile.

**Acceptance Scenarios**:

1. **Given** a rendered agent transcript, **When** the maintainer changes the
   tool-call display format in the UI layer only, **Then** the rendered tool
   call changes with no change to the agent layer.
2. **Given** an assistant message containing a thinking delimiter, **When**
   the UI renders it, **Then** thinking and content are split and displayed
   separately using a UI-side splitting rule (not an agent-side split).
3. **Given** a web-delegate tool result, **When** rendered, **Then** its
   trace is displayed from structured per-call data, not by parsing a
   pre-formatted trace string.

---

### User Story 4 - Sessions Carry Identity and History Across Prompts (Priority: P3)

A user working through a multi-step task submits several follow-up prompts in
the same conversation. Each continuation prompt reuses the same session
identity, so the agent carries over the correct conversation history and
appends to the transcript. When the user starts a fresh session, history is
reset. This works today and must keep working; the identity scheme simply
becomes robust and forward-compatible (no integer counter).

**Why this priority**: Session continuity is an existing requirement that
must not regress; it is lower priority than the no-regression and
testability stories but is part of the seam contract.

**Independent Test**: Can be fully tested by submitting two continuation
prompts with the same session identity and asserting the second sees the
first's history, then submitting a prompt with a new identity and asserting
history is empty.

**Acceptance Scenarios**:

1. **Given** a completed first prompt in a session, **When** the user submits
   a second prompt with the same session identity, **Then** the agent
   continues the conversation using the prior history.
2. **Given** a prior session, **When** the user starts a new session (new
   identity), **Then** conversation history is reset for the new session.
3. **Given** agent events arriving, **When** the UI receives them, **Then**
   each event is routed to the correct session by its session identity.

---

### Edge Cases

- What happens when a prompt arrives for a session that was just cancelled?
  The agent treats it as a new session (fresh history) or rejects it
  gracefully with a clear status.
- What happens when the agent publishes events faster than the UI drains them
  for several frames? The broadcast channel must not drop agent output under
  normal single-session load; a lagging subscriber is detected and recovered.
- What happens when a tool fails mid-execution? The failure is reported as a
  structured event; the session continues or ends per existing behavior, and
  no file reindex is triggered for the failed side effect.
- What happens when a prompt is empty or whitespace-only? The agent rejects
  it without starting a turn.
- What happens when two prompts are queued for the same session in quick
  succession? They are processed in order within that session's history.
- What happens to the transcript view model when the user toggles a task
  checkbox mid-stream? The toggle state is owned by the UI view model and
  survives incremental deltas without corrupting the in-flight response.
- What happens when the web-delegate produces a trace with zero tool calls?
  The structured trace is empty and renders nothing, with no string to strip.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The agent MUST process submitted prompts asynchronously so the
  UI never freezes during a turn.
- **FR-002**: The agent MUST stream its progress to the UI in real-time,
  including thinking text, assistant content, tool calls (with name and
  arguments), tool results, and token usage.
- **FR-003**: The agent MUST publish its progress as structured events on a
  single agent→UI output channel, where every event is tagged with the
  session identity it belongs to.
- **FR-004**: The UI MUST render agent output from the structured events it
  accumulates, not by re-parsing formatted display strings produced by the
  agent.
- **FR-005**: The agent layer MUST NOT import or reference any UI code, UI
  state types, or UI channel handles. It MUST be buildable and unit-testable
  in isolation.
- **FR-006**: The tool-execution layer MUST NOT reference any UI channel or
  UI type. It MUST return side effects as structured data to the agent.
- **FR-007**: When a tool creates or modifies a file, the side effect MUST
  travel to the UI as a structured event on the agent output channel, and the
  UI MUST re-issue it through the existing file-event handling path to
  trigger reindexing of affected files, tags, and trees.
- **FR-008**: Each agent session MUST carry a unique identity. The UI mints a
  fresh identity to start a new session and reuses the same identity for
  continuation prompts.
- **FR-009**: Continuing a session (same identity) MUST reuse and append to
  that session's conversation history. Starting a new session (new identity)
  MUST reset history.
- **FR-010**: The UI MUST own the transcript view model, accumulating content
  and tool deltas from agent events into a displayable transcript, including
  any per-item interaction state (e.g., task toggle state).
- **FR-011**: The agent MUST NOT accumulate and resend a running full-response
  buffer each turn. It MUST emit each content/tool chunk once as a delta,
  reducing output channel traffic from quadratic to linear growth.
- **FR-012**: Splitting thinking from content (on the thinking delimiter) and
  formatting tool calls/results for display MUST be UI-side concerns. The
  agent MUST emit raw assistant content and structured tool data.
- **FR-013**: Pure UI view state — debug-window toggles, debug search text,
  auto-scroll behavior, debug row data, command input, results visibility,
  and scroll targets — MUST live on the UI side, not in the agent session
  manager.
- **FR-014**: The web-delegate tool trace MUST be carried as structured
  per-call data in the tool result, not as a pre-formatted string. No
  string-stripping workaround may be required before the result is sent to the
  model.
- **FR-015**: Cancelling an in-progress turn MUST stop the turn promptly and
  end the session cleanly, emitting a session-finished marker.
- **FR-016**: The refactor MUST preserve all existing [AGENT-xxx] requirement
  behaviors (async execution, real-time thinking + markdown, formatted tool
  call display, session history continuity) — only the layer and payload shape
  change.
- **FR-017**: The refactor MUST be delivered in small, independently testable
  iterations; each iteration MUST compile and pass tests before the next
  begins (per the Modularity principle).

### Key Entities *(include if feature involves data)*

- **AgentSession**: A conversation identified by a unique session identity.
  Owns its conversation history and lifecycle (started, running, finished).
  Multiple sessions are conceptually possible; today only one is active at a
  time, but the types must not assume a single session.
- **AgentPrompt**: A single user submission belonging to a session. Carries
  the session identity, the prompt text, and the active file/directory and
  selected-files context.
- **AgentEvent**: A structured, session-tagged message published by the agent
  to the UI. Variants include session lifecycle markers, status, thinking,
  content deltas, tool-call-started, tool-result, tool side effect, debug
  entry, token usage, and failure.
- **ToolSideEffect**: A typed description of an effect a tool had on the
  system (e.g., a file was created at a path with given tags). Produced by
  the tool executor as data and republished by the agent as an event.
- **AgentTranscript (UI-owned)**: The UI's view model accumulating agent
  events into a displayable, interactable transcript (content blocks, tool
  calls, results, and per-item interaction state such as task toggles).
- **AgentPanelState (UI-owned)**: Pure UI view state for the agent panel —
  window toggles, search text, scroll targets, command input. Decoupled from
  agent lifecycle state.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The agent layer builds and its full unit-test suite passes with
  zero UI-module imports or UI-channel references on the agent dependency
  path.
- **SC-002**: End-users observe no regression in real-time agent output —
  thinking, content, tool calls, and results render identically to the
  pre-refactor baseline for a representative prompt set.
- **SC-003**: Agent→UI output channel traffic grows linearly (O(n)) with
  output length, not quadratically — measured as total bytes published for a
  multi-turn conversation growing proportionally to the final transcript size.
- **SC-004**: A maintainer can write a passing agent-loop unit test by
  submitting one prompt and asserting on structured events, with no UI setup
  required, in under 5 minutes.
- **SC-005**: File creation by a tool triggers UI reindexing through the
  typed side-effect path in 100% of successful tool executions — no
  back-channel path exists.
- **SC-006**: The web-delegate trace reaches the model as structured data
  with no string-stripping workaround present in the codebase.
- **SC-007**: The session manager no longer holds any UI widget state — a
  scan finds zero display flags, scroll targets, or command-input fields in
  the agent layer.
- **SC-008**: Every iteration of the migration compiles cleanly and passes
  its test suite before the next iteration starts (zero red intermediate
  states on the integration branch).

## Assumptions

- The existing asynchronous execution model (background thread, UI never
  blocks) is preserved; only the channel shape and ownership change.
- The existing UI-side file-event plumbing is reused; the refactor only
  changes who originates the file-modified signal (UI re-issues it from a
  structured agent event instead of the tool layer sending it directly).
- A broadcast-channel primitive suitable for agent events already exists in
  the codebase and is reused rather than introduced.
- Today only one agent session is active at a time. The types and channel are
  made forward-compatible with multiple concurrent sessions, but implementing
  concurrent multi-session UI is out of scope for this refactor.
- No [AGENT-xxx] requirement values change; this refactor changes
  implementation structure and the layer at which certain requirements are
  satisfied, not the requirements themselves.
- Migration proceeds incrementally per the documented step-by-step plan;
  dual-publishing (old + new channels) is permitted as a transitional measure
  within a single iteration but is removed by the end of the refactor.
- Existing snapshot/end-to-end render tests that assert on legacy response
  payloads will be updated to assert on the structured transcript as part of
  the migration; this is expected migration cost, not new scope.
- The desktop app remains a single-user, local-first application; no
  multi-user or networked session concerns apply.
