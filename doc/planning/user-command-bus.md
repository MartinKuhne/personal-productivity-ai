# User Command Bus — Audit & Implementation Plan

> **Status:** planning / pre-implementation
> **Scope:** Introduce a `Bus<UserCommand>` broadcast channel and a unified
> `UserCommand` enum so every user input (toolbar, command line, tabs, TOC,
> file tree, modals, keyboard shortcuts) flows through one typed intake, with
> execution centralised in an orchestrator-side executor. This decouples UI
> panels from `AppOrchestrator` state mutation, retires the
> `submit_prompt: Option<String>` deferred-action slot, and preserves the
> existing Tier-4 click-capture testability of the `apply_*` helpers.
> **Author:** opencode
> **Date:** 2026-08-25

---

## 1. Motivation

Today every user input mutates `FastMdApp` / `AppOrchestrator` state inline, or
via an `apply_*` helper that reaches into orchestrator fields. There is no
single typed intake for "the user asked for X". The only deferred path is
`orchestrator.submit_prompt: Option<String>` — a one-shot slot polled by
`handle_deferred_actions`, not a channel.

Consequences:

- **Tight coupling.** UI panels borrow `&mut` slices of the orchestrator (or
  snapshot/flush via `TreeNodeContext`) just to express intent. Render code
  mixes "what the user did" with "what should happen".
- **Unobservable intent.** There is no place to log, throttle, replay, or
  record *commands* as first-class events. Side-effectful mutations are
  scattered across `apply_*` helpers and `write_back` flushes.
- **Test friction.** The `apply_*` helpers were deliberately extracted so they
  can be unit-tested without the egui harness (Tier-4 click-capture tests via
  `on_click(&'static str)` callbacks). But because they mutate orchestrator
  state directly, each test still has to stand up that state. A
  `… → UserCommand` function needs no orchestrator at all.
- **Snapshot/flush hazard.** `TreeNodeContext::from_app_state(...)` snapshots
  ten orchestrator fields, mutates the snapshot, then `write_back(...)` flushes
  four of them. The two-phase model is a classic source of lost updates when
  the snapshot and the orchestrator drift.

The codebase already has the pattern this refactor wants: the `Bus<T>`
broadcast system (`src/app/bus/core.rs`) is used for `Bus<FileEvent>`,
`Bus<SeamAgentEvent>`, and `Bus<ConfigArrived>`, and RUST-052 mandates that
background work reach the UI through event-driven fan-out on these buses. This
plan extends the same pattern to *user intent*.

---

## 2. Existing infrastructure (verified from source)

### 2.1 `Bus<T>` broadcast system — `src/app/bus/core.rs`

Tokio `broadcast::channel` wrapper, capacity 8192.
`Bus<T: Clone + Send + 'static>` with:

- `subscribe()` → `BusReader<T>`
- `subscribe_async()`
- `publish(event)`
- `subscriber_count()`

`BusReader` exposes `try_recv()` and `try_recv_exposing_lag()` returning
`BroadcastRecvError::{Empty, Lagged(n), Closed}`.

Already used for `Bus<FileEvent>`, `Bus<SeamAgentEvent>` (aliased
`AgentEvent`), `Bus<ConfigArrived>`. Per RUST-052, the UI subscribes as a
`BusReader` and drains each frame.

### 2.2 `AppOrchestrator` — `src/app/orchestrator.rs:28`

Owns all app state: `content_libraries`, `rx/tx: BackgroundEventSender`,
`file_event_bus: Bus<FileEvent>`, `file_event_reader`, `file_processor`,
`pdf_backing_tracker`, `tags`, `directory_tracker`, `selection: FileSelection`,
`tabs: Tabs`, `_watcher`, `agent: AgentSession`, `dialogs: Dialogs`,
`submit_prompt: Option<String>` (deferred-action slot — the seed of the channel
idea), `text_buffer`, `inline_editor_enabled`, `background_manager`,
`config: AppConfig`, `config_reader`, `pending_file_load`,
`tool_context: Arc<ArcSwap<AgentToolContext>>`, `agent_event_bus:
Bus<SeamAgentEvent>`, `agent_event_reader`, `agent_event_lagged: bool`,
`agent_transcript: AgentTranscript`, `agent_panel_state: AgentPanelState`.

Frame-drain methods called from `update_ui`: `drain_config_bus`,
`drain_background_channel`, `drain_agent_event_bus`. Also `process_file_events`,
`handle_fs_event(FsEvent)`, `handle_process_event(ProcessEvent)`,
`handle_mcp_auth_event(McpAuthEvent)`, `handle_file_selection`,
`start_agent_session(prompt: String)`, `task_take_finished_watcher`,
`close_tabs_for_removed_files`, `is_workspace_file`.

### 2.3 Frame driver — `src/app/ui/app/update.rs:41`

`FastMdApp::update_ui(ui)` order:

1. `apply_persisted_font_scale`
2. `drain_config_bus`
3. `drain_background_channel`
4. `process_file_events_and_repaint`
5. `drain_agent_event_bus`
6. consume `ALT+A` key
7. `handle_file_selection`
8. `show_editor_overlay`
9. `show_modals`
10. `render_panels`
11. `handle_deferred_actions`
12. `update_persisted_ui_state`
13. drain egui `PlatformOutput::commands` (OpenUrl)

`handle_deferred_actions` (`update.rs:151`) takes
`orchestrator.submit_prompt` → `start_agent_session`, and polls
`dialogs.batch_handle` for completion.

Panel render order (`src/app/ui/app/render.rs`): `show_editor_overlay` →
`show_modals` → `render_panels` (top → bottom → right → left → center).

### 2.4 Existing command-shaped enums (precedent)

- **`CommandIntent`** — `src/app/ui/panels/bottom.rs:12`:
  `ShowModels`, `ShowDeprecatedModelMessage`, `RunAgent(String)`, `Empty`.
  Parsed by `parse_command_intent(&str)` (matches `/models`, `/model `, else
  `RunAgent`).
- **`TabAction`** — `src/app/ui/panels/center.rs:14`: `Close(usize)`,
  `CloseOthers(usize)`, `CloseAll`. Pure
  `apply_tab_action(&mut tabs, &mut selected_file, action)`.
- **`NameEntryAction`** — `src/app/ui/modals.rs:15`: `Submit`, `Cancel`
  (shared by create-dir/create-document dialogs).
- **`BatchDialogResult`** — `src/app/agent/batch/types.rs:257`:
  `Process(BatchConfig)`, `Cancel`.

Each is narrowly scoped to one panel/modal; none is unified.

---

## 3. User-command surface audit

Eight input entry points (A–H). Every one currently mutates orchestrator state
inline or via a snapshot/flush.

### A. Top toolbar + hamburger menu — `src/app/ui/panels/top.rs`

**Toolbar buttons** (direct `on_click`):
- `apply_batch_button_click(app)` → `dialogs.batch_dialog_open = true`
- `apply_tools_button_click(app)` → `dialogs.tools_dialog_open = true`
- Tag filter ComboBox → mutates `tags.selected_tag: Option<String>`,
  recomputes `selected_file`, sets `tree_dirty`
- Spinner/indexing status (display-only)

**Hamburger menu** (`☰` button, `HAMBURGER_MENU_ID_SALT`, three submenus per
`src/app/ui/strings.rs`):
- *Chat models* (`MENU_CHAT_MODELS`) — `apply_chat_model_selection(app,
  String)` → sets `config.selected_chat_model` + `agent.set_agent_config`.
  Fires `CHAT_MODEL_SELECTION_EVENT` for Tier-4 capture.
- *Table wrap algorithm* (`MENU_TABLE_WRAP_ALGORITHM`) —
  `apply_table_width_strategy_change(app, DeficitStrategy, &mut persist)` →
  sets `config.table_width_strategy` + persists.
- *Windows* (`MENU_WINDOWS`) submenu:
  - *Background operations* (`MENU_BACKGROUND_OPERATIONS`) →
    `apply_background_logs_toggle(app, bool)` → toggles
    `background_manager.show_background_logs`. Fires
    `BACKGROUND_OPERATIONS_EVENT`.
  - *Agent debug* (`MENU_AGENT_DEBUG`) →
    `apply_agent_debug_toggle(app, bool)` → toggles
    `agent_panel_state.show_debug_window`. Fires `AGENT_DEBUG_EVENT`.

Toolbar has Tier-4 `on_click(&'static str)` event-name capture for tests;
`show_top_panel_capture` / `show_top_panel_capture_with_persist`.

### B. Bottom panel / command input — `src/app/ui/panels/bottom.rs`

- Enter key / Send click → `apply_send_click(app)`: trims
  `agent_panel_state.command_input`, clears it, `parse_command_intent` →
  direct state mutation (`set_status`, `set_response`, `show_results=true`) or
  `start_agent_session(prompt)`. `Empty` → no-op.
- `command_input` text buffer (multiline; Enter submits, Shift+Enter newline).

### C. Center panel / tabs / agent session — `src/app/ui/panels/center.rs`

- Tab close × → `apply_tab_close_click(app, i)` → `TabAction::Close(i)`
- 'Close Other Tabs' → `apply_tab_close_others_click(app, i)` →
  `TabAction::CloseOthers(i)`
- 'Close All Tabs' → `apply_tab_close_all_click(app)` → `TabAction::CloseAll`
- Agent session 'Close' button → `clear_agent_session_state(app)`: hides
  results, clears history/response/thinking, `agent_transcript.reset()`, and
  `agent.cancel()` if running.
- Task checkbox toggles in rendered markdown →
  `ui::render::apply_task_toggle(&mut transcript.content, idx, checked)`
- `render_agent_session` (`center.rs:149`) is the agent-results view.

### D. Right panel / TOC — `src/app/ui/panels/right.rs`

- TOC row click → `apply_toc_row_click(app, entry_id)` →
  `tabs.scroll_to_header_id = Some(entry_id.to_string())`

### E. Left panel / file tree — `src/app/ui/tree/`

- `TreeNodeContext` (`src/app/ui/tree/context.rs`) is built via
  `from_app_state(...)` (snapshot of selection/tabs/dialogs/layout/
  submit_prompt/content_libraries/bg_tx/file_event_bus/inline_editor_enabled/
  modifiers/open_editor/pdf_backing_tracker), mutated by handlers, then
  `write_back(&selection, &mut tabs, &mut dialogs, &mut submit_prompt)` flushes
  to orchestrator. **This snapshot/flush is the main decoupling blocker.**
- `apply_file_row_click(ctx, row)` (`handlers.rs:51`): shift/ctrl/cmd → toggle
  multi-select; else single-select + add tab. Always refreshes `selected_dir`
  to file's parent.
- `apply_directory_row_click(ctx, row)` (`handlers.rs:107`): toggle expansion +
  set `selected_dir`.
- `build_merge_prompt(...)` (`handlers.rs:129`): multi-select merge.
- `show_file_context_menu` (`render.rs:137`): Show in Explorer
  (`crate::ui::show_in_file_explorer`), Copy Path (`ui.copy_text`), Save as PDF
  (spawns `pdf-export` thread, gated `#[cfg(feature="pdf-export")]`), Rename,
  Move, Delete (`crate::utils::recycle_bin::delete` +
  `file_event_producer.publish_removed`), Note skills (reads
  `System/Skills/Note/*.md`, sets `selected_dir`+`selected_file`+
  `submit_prompt`).
- `show_dir_context_menu` (`render.rs:17`): Show in Explorer, Copy Path,
  Rename, Move, Create Directory, New Document, Delete, Folder skills (sets
  `selected_dir`+`selected_file`+`submit_prompt`).
- 'Open in editor' (`ctx.open_editor`) — inline editor open.

### F. Modals — `src/app/ui/modals.rs` + `render.rs::show_modals`

- Move dialog:
  `show_move_modal_dialog(&mut dialogs, &content_libraries, &file_processor, &file_event_bus, ctx)`
- Create directory dialog:
  `show_create_dir_dialog(&mut dialogs, &mut file_processor, &mut _watcher, &file_event_bus, ctx)`
- Rename dialog:
  `show_rename_dialog(RenameDialogCtx{dialogs, file_event_bus, loaded_path, selected_file, selected_dir, tabs, file_processor, app_tags, expanded_dirs, ctx})`
- Create document dialog:
  `show_create_document_dialog(&mut dialogs, &file_event_bus, ctx)`
- `show_background_logs_window(self, ctx)`
- `show_agent_debug_window(self, ctx)`
- Tools dialog: `show_tools_dialog(ctx, self)` (`src/app/ui/tools_dialog.rs`)
  renders a table of tool groups (`ToolGroupState`). Per-row controls that
  today mutate state inline:
  - **Enable checkbox** (`render_row` col 1) — on change, clones
    `AppConfig`, flips `config.tool_groups.<family>` (internal) or
    `config.mcp_servers[name].enabled` (MCP), calls `save_config`, writes back
    via `*app.config_mut() = new_config`. This is the "enable/disable tool"
    the user called out.
  - **Restart link** (`TOOLS_RESTART`, shown when `group.last_error` is
    `Some`) — `ui.small_button` → `tool_context.rcu(...)` calling
    `registry.clear_error(&id)`.
  - **Authenticate button** (`TOOLS_AUTH_BUTTON`, shown for MCP servers with
    `needs_auth` and no `Authorization` header) — on click, calls
    `dialogs.set_oauth_in_progress(name)` and `spawn_auth_flow(...)` (a
    background thread that runs `mgr.authenticate(&server_name)` and sends
    `McpAuthEvent::Completed` over the background channel). While in progress,
    a disabled `TOOLS_AUTH_RUNNING` label is shown.
  - **Forget button** (`TOOLS_FORGET`) — clears `needs_auth` via
    `mcp_manager.mark_needs_auth(name, false)`.
  - Display-only columns: group name (with error tooltip), kind label, tool
    names, prompt char count.
- Batch dialog: `show_batch_modal(self, ctx, &mut dialog_config)` →
  `BatchDialogResult::Process(BatchConfig)` spawns
  `BatchCoordinator::new(config, app_config, tx.clone(), file_event_bus.clone(),
  prompt_text, Arc<SystemClock>).execute()` storing `dialogs.batch_handle` +
  `dialogs.batch_cancel_flag`; `Cancel` closes dialog.

### G. Global keyboard shortcuts — `src/app/ui/app/update.rs:56`

- `ALT+A` → toggles `agent_panel_state.show_debug_window`
- Bottom-panel Enter handled inside `show_bottom_panel`
- egui `OutputCommand::OpenUrl` (markdown link clicks) drained at end of frame
  → `os_shell::dispatch_platform_commands(&commands, os_shell::open_url)`

### H. Deferred actions — `update.rs::handle_deferred_actions`

- `orchestrator.submit_prompt.take()` → `start_agent_session(prompt)`
  (populated by tree skill-prompt paths)
- `dialogs.batch_handle.take()` → if `thread.is_finished()` join + clear
  `batch_cancel_flag`, else put back

---

## 4. Supporting structs (current shape)

### `Dialogs` — `src/app/ui/dialogs.rs:24`

Owns: move dialog (`move_dialog_open`, `file_to_move`,
`selected_move_folder`), create-dir (`create_dir_dialog_open`,
`create_dir_parent`, `create_dir_name`), create-document
(`create_document_dialog_open`, `create_document_parent`,
`create_document_name`), rename (`rename_dialog_open`, `file_to_rename`,
`rename_new_name`), batch (`batch_dialog_open`,
`batch_dialog_config: BatchDialogConfig`, `batch_handle: Option<BatchHandle>`,
`batch_cancel_flag: Option<Arc<AtomicBool>>`), tools (`tools_dialog_open`),
`oauth_status: HashMap<String, OAuthFlowStatus>`. Helpers:
`set_oauth_in_progress`, `set_oauth_idle`, `is_oauth_in_progress`.

### `AgentSession` — `src/agent/session.rs`

`cancel()` sets cancel flag + `running=false` + status. `submit_prompt
(AgentPrompt)` mints session_id, sets running, sends on `prompt_tx` to
long-lived driver thread. `queue_prompt`/`take_next_queued_prompt` for
continuation. `clear_history`. `apply_token_usage`.
`set_agent_config`/`replace_agent_config`. Driver thread processes prompts
sequentially.

---

## 5. Proposed design

### 5.1 `UserCommand` enum

New file `src/app/bus/events/user_command.rs`, re-exported through the
`bus::events` module (where `FileEvent`, `SeamAgentEvent`, `ConfigArrived`
already live).

```rust
/// One unit of user intent captured at the UI boundary and dispatched
/// centrally by the orchestrator.
///
/// Variants are grouped by surface. Every variant carries *all* data the
/// executor needs to apply the command — no back-references to UI state.
pub enum UserCommand {
    // ── Command input / agent (surface B, C) ───────────────────────────
    RunAgent(String),
    ShowModels,
    ShowDeprecatedModelMessage,
    CancelAgent,
    // ── Tabs (surface C) ───────────────────────────────────────────────
    CloseTab(usize),
    CloseOtherTabs(usize),
    CloseAllTabs,
    // ── TOC (surface D) ────────────────────────────────────────────────
    ScrollToHeader(String),
    // ── Toolbar buttons + hamburger menu submenus (surface A) ──────────
    //   `OpenBatchDialog` / `OpenToolsDialog` are toolbar buttons.
    //   `SelectChatModel`, `ChangeTableWidthStrategy`,
    //   `ToggleBackgroundLogs`, `ToggleAgentDebugWindow` are hamburger
    //   menu items (Chat models / Table wrap algorithm / Windows
    //   submenus). `SelectTagFilter` is the toolbar tag ComboBox.
    OpenBatchDialog,
    OpenToolsDialog,
    ToggleBackgroundLogs(bool),
    ToggleAgentDebugWindow(bool),
    SelectChatModel(String),
    ChangeTableWidthStrategy(DeficitStrategy),
    SelectTagFilter(Option<String>),
    // ── Tools dialog — per-group controls (surface F) ──────────────────
    //   Enable/disable a tool group (internal family or MCP server).
    //   Carries the group id + new enabled flag; the executor mutates
    //   `AppConfig.tool_groups` / `mcp_servers[*].enabled` and persists
    //   via `save_config`.
    SetToolGroupEnabled { id: ToolGroupId, enabled: bool },
    //   "Restart" link on a group row with a recorded error — clears
    //   `ToolGroupState::last_error` via `ToolRegistry::clear_error`.
    ClearToolGroupError(ToolGroupId),
    //   "Authenticate" button on an MCP server row that needs auth —
    //   spawns the OAuth flow thread. The executor calls
    //   `dialogs.set_oauth_in_progress(name)` and spawns the flow.
    StartMcpAuth(String),
    //   "Forget" button on an MCP server row — clears the `needs_auth`
    //   flag via `mcp_manager.mark_needs_auth(name, false)`.
    ForgetMcpAuth(String),
    // ── File tree — selection (surface E) ──────────────────────────────
    SelectFile { path: PathBuf, multi: bool },
    SelectDirectory { path: PathBuf, toggle_expand: bool },
    OpenInEditor(PathBuf),
    // ── File tree — context menu (surface E) ───────────────────────────
    ShowInExplorer(PathBuf),
    CopyPath(PathBuf),
    SaveAsPdf(PathBuf),
    Rename(PathBuf),
    Move(PathBuf),
    Delete(PathBuf),
    CreateDirectory { parent: PathBuf },
    CreateDocument { parent: PathBuf },
    RunSkillPrompt { content: String, target_dir: Option<PathBuf>, target_file: Option<PathBuf> },
    MergePrompt(Vec<PathBuf>),
    // ── Dialog confirmations (surface F) ────────────────────────────────
    ConfirmMove { file: PathBuf, destination: PathBuf },
    ConfirmCreateDirectory { parent: PathBuf, name: String },
    ConfirmCreateDocument { parent: PathBuf, name: String },
    ConfirmRename { path: PathBuf, new_name: String },
    StartBatch(BatchConfig),
    CancelBatch,
    // ── Keyboard shortcuts (surface G) ─────────────────────────────────
    ToggleAgentDebugWindowShortcut,
}
```

Design rules:

- **Payload-complete.** Each variant carries every field the executor needs;
  no `&mut` borrow of orchestrator state at the call site.
- **Clone + Send + 'static.** Required by `Bus<T>`. `BatchConfig`,
  `DeficitStrategy`, and `PathBuf` already satisfy this.
- **No `Empty`.** The "do nothing" case is represented by *not publishing*.
  (Keeps the bus free of no-op traffic and makes lag diagnostics meaningful.)

### 5.2 Plumbing on the orchestrator

1. Add `user_command_bus: Bus<UserCommand>` and
   `user_command_reader: BusReader<UserCommand>` to `AppOrchestrator`,
   alongside the existing three buses. Subscribe the reader in the
   orchestrator constructor.
2. Add `drain_user_command_bus(&mut self)` that loops `try_recv()` until
   `Empty`/`Closed` and dispatches each command to a new
   `command_executor.rs` module. Call it from `update_ui` immediately after
   `drain_agent_event_bus` (step 5 → 6 in the order above), so commands
   produced during this frame's panel render are applied before
   `handle_deferred_actions` and before the next frame's render.
3. Expose a `UserCommandProducer` — a cheap cloneable handle wrapping a
   `Bus<UserCommand>` (mirroring the existing `FileEventProducer`), with a
   `publish(UserCommand)` method. Thread it into `TreeNodeContext::from_app_state`,
   panel render functions, and modal helpers so they can publish without
   borrowing the orchestrator.

### 5.3 `command_executor` module — `src/app/command_executor.rs`

Pure-ish dispatch: takes `&mut self`-shaped state slices (or `&mut
AppOrchestrator` for the first cut) and matches on `UserCommand`. Each arm is
the body that today lives in an `apply_*` helper or a `write_back` flush.
Keeping the executor in one module makes the blast radius of any command
trivially greppable and gives the executor its own unit tests independent of
the egui harness.

```rust
impl AppOrchestrator {
    /// Apply one user command. Called from `drain_user_command_bus`.
    ///
    /// Owns *all* side effects of user intent. UI panels publish; this is
    /// the single place that mutates orchestrator state in response.
    fn apply_user_command(&mut self, cmd: UserCommand) {
        match cmd {
            UserCommand::RunAgent(prompt) => self.start_agent_session(prompt),
            UserCommand::ShowModels => self.agent_panel_state.set_status(/* … */),
            // … one arm per variant …
        }
    }
}
```

### 5.4 Testability preservation

The existing `apply_*` helpers become **pure `… → UserCommand`** functions.
Tier-4 click-capture tests assert on the returned command — no orchestrator
state needed. The executor gets its own tests on the orchestrator side
covering each command's state mutation. This *strictly improves* testability:
today's helpers both decide and mutate; after the split, decision and effect
are each tested in isolation.

---

## 6. Staged implementation plan

One surface per task. Each task ends with the full quality gate green:

- `cargo check --quiet` — no errors or warnings
- `cargo nextest run --status-level fail --show-progress none` — all tests pass
- `cargo clippy -- -D warnings` — no lint warnings
- `cargo fmt --check` — code is properly formatted
- `cargo doc --no-deps --quiet` — documentation builds without warnings

> **Note on the quality-gate working directory.** `AGENTS.md` says to run the
> gate from `src/desktop/`, but that directory does not exist in this repo.
> The crate root is the repository root (`C:\Users\mkuhn\src\ppai\Cargo.toml`,
> `[lib] path = "src/app/lib.rs"`). Run all quality-gate commands from the
> repo root. The `src/desktop/` reference in `AGENTS.md` is stale.

### Stage 0 — Scaffolding (no behaviour change)

**Goal:** land the bus, the enum, the producer handle, and the drain hook
without changing any existing call site.

- Add `UserCommand` enum in `src/app/bus/events/user_command.rs` with a
  minimal initial variant set (just enough to exercise the path: `RunAgent`,
  `ShowModels`, `CloseAllTabs`). Re-export from `bus::events`.
- Add `UserCommandProducer` (clones `Bus<UserCommand>`, exposes `publish`).
- Add `user_command_bus` + `user_command_reader` to `AppOrchestrator`.
- Add `drain_user_command_bus(&mut self)` that calls `apply_user_command` for
  each received command. `apply_user_command` has a match with the initial
  variants only; unknown variants (`_ => {}`) for now so later stages can
  extend without re-touching the drain.
- Wire `drain_user_command_bus` into `update_ui` after `drain_agent_event_bus`.
- Unit test: publish a `CloseAllTabs` command, assert the drain clears tabs.

**Exit criteria:** bus exists and drains, zero call sites changed, gate green.

### Stage 1 — Bottom panel (proof of concept)

**Goal:** migrate `apply_send_click` to publish `UserCommand` instead of
mutating state inline. This is the smallest surface and the one with the
cleanest existing precedent (`CommandIntent` already maps 1:1).

- Extend `UserCommand` with `ShowDeprecatedModelMessage`, `CancelAgent`.
- `apply_send_click` returns `Option<UserCommand>` (pure: parse + build the
  command). The egui click handler publishes it.
- Move the dispatch body into `apply_user_command`:
  - `RunAgent(String)` → `start_agent_session`
  - `ShowModels` → `set_status` / `set_response` / `show_results=true`
  - `ShowDeprecatedModelMessage` → same family
  - `CancelAgent` → `agent.cancel()` + clear results
- Convert existing `bottom.rs` Tier-4 tests to assert on the returned
  `UserCommand`. Add executor tests for each arm.
- Retire `CommandIntent` (its variants are now `UserCommand` variants; the
  `parse_command_intent` body moves into the `… → UserCommand` helper).

**Exit criteria:** bottom panel no longer touches orchestrator state; gate
green; `CommandIntent` removed.

### Stage 2 — Top toolbar + hamburger menu

**Goal:** migrate the two toolbar buttons, the three hamburger submenus
(Chat models, Table wrap algorithm, Windows), and the tag filter.

- Extend `UserCommand` with `OpenBatchDialog`, `OpenToolsDialog`,
  `ToggleBackgroundLogs(bool)`, `ToggleAgentDebugWindow(bool)`,
  `SelectChatModel(String)`, `ChangeTableWidthStrategy(DeficitStrategy)`,
  `SelectTagFilter(Option<String>)`.
- Each `apply_*` helper becomes `… → UserCommand`; the egui `on_click` /
  `on_value_change` / menu-item handler publishes.
- Executor arms: open dialogs, toggle flags, set model + `set_agent_config`,
  set + persist config, set tag + recompute `selected_file` + set
  `tree_dirty`.
- Convert `show_top_panel_capture` / `show_top_panel_capture_with_persist`
  Tier-4 tests to assert on published commands. Add executor tests.

**Exit criteria:** top toolbar and hamburger menu publish only; gate green.

### Stage 3 — Center panel (tabs + agent session)

**Goal:** migrate tab actions and the agent-session Close button.

- Extend `UserCommand` with `CloseTab(usize)`, `CloseOtherTabs(usize)`.
- `apply_tab_close_*` helpers become `… → UserCommand`.
- Executor arms reuse the existing pure `apply_tab_action` body (move it into
  the executor).
- `clear_agent_session_state` → `UserCommand::CancelAgent` (already added in
  Stage 1); the Close button publishes it.
- Retire `TabAction` (its variants are now `UserCommand` variants; the pure
  `apply_tab_action` body moves into the executor).
- Tier-4 tab-close tests assert on published commands. Executor tests cover
  each `TabAction` arm + the agent-cancel arm.

**Exit criteria:** center panel publishes only; `TabAction` removed; gate
green.

### Stage 4 — Right panel (TOC)

**Goal:** migrate `apply_toc_row_click`.

- Extend `UserCommand` with `ScrollToHeader(String)`.
- `apply_toc_row_click` → `… → UserCommand`; handler publishes.
- Executor arm sets `tabs.scroll_to_header_id`.
- Tier-4 test asserts on the command.

**Exit criteria:** right panel publishes only; gate green.

### Stage 5 — Modals

**Goal:** translate `NameEntryAction` and `BatchDialogResult` at the dialog
boundary into `UserCommand`.

- Extend `UserCommand` with `ConfirmMove`, `ConfirmCreateDirectory`,
  `ConfirmCreateDocument`, `ConfirmRename`, `StartBatch(BatchConfig)`,
  `CancelBatch`.
- Dialogs still own their open/name/parent fields (those are dialog-local
  state, not user intent). On `Submit`, they publish the corresponding
  `Confirm*` command carrying the captured fields; on `Cancel`, they publish
  `Cancel*` (or just close locally — see Open question 2).
- Executor arms perform the actual file operations (move/create/rename/delete)
  via `file_processor` + `file_event_bus`, and spawn the `BatchCoordinator` for
  `StartBatch`.
- Retire `NameEntryAction` and `BatchDialogResult` once all callers publish
  `UserCommand` instead.
- Modal tests assert on published commands; executor tests cover each
  file-operation arm.

**Exit criteria:** modals publish on submit/cancel; `NameEntryAction` and
`BatchDialogResult` removed; gate green.

### Stage 5b — Tools dialog (enable/disable tool + OAuth controls)

**Goal:** migrate the four per-row controls in `show_tools_dialog`
(`src/app/ui/tools_dialog.rs`) so tool-group toggles, restart, authenticate,
and forget all flow through `UserCommand`.

- `UserCommand` already gained `SetToolGroupEnabled { id, enabled }`,
  `ClearToolGroupError(ToolGroupId)`, `StartMcpAuth(String)`,
  `ForgetMcpAuth(String)` in the enum (§5.1).
- `render_row` becomes pure-ish: the checkbox `on_change`, the Restart
  `small_button.clicked()`, the Authenticate `button.clicked()`, and the
  Forget `small_button.clicked()` each publish the corresponding command
  instead of cloning `AppConfig` / calling `save_config` / `rcu` / spawning
  threads inline.
- Executor arms:
  - `SetToolGroupEnabled` → clone config, flip
    `config.tool_groups.<family>` / `mcp_servers[name].enabled`, `save_config`,
    write back `*config_mut() = new_config`. (This is the "change which tools
    are enabled" path the user called out.)
  - `ClearToolGroupError` → `tool_context.rcu(...)` calling
    `registry.clear_error(&id)`.
  - `StartMcpAuth` → `dialogs.set_oauth_in_progress(name)` + spawn the OAuth
    flow thread (the existing `spawn_auth_flow` body moves here; it still
    sends `McpAuthEvent::Completed` over the background channel).
  - `ForgetMcpAuth` → `mcp_manager.mark_needs_auth(name, false)`.
- The display-only columns (name, kind, tool list, char count, error tooltip)
  stay in `render_row` — they read state, they don't mutate it.
- `ToolGroupId` and `InternalToolGroup` must satisfy `Clone + Send + 'static`
  for the bus. `ToolGroupId` already derives `Clone, Debug, PartialEq, Eq,
  PartialOrd, Ord, Hash`; add `Send + 'static` bounds (they're already Send
  by construction — enum of `Copy` internal + `String`). Verify the bus
  bound compiles in Stage 0; if not, box/arc the `ToolGroupId` here.
- Tools-dialog tests assert on published commands; executor tests cover each
  arm (config mutation + persist for toggle, error-clear for restart, thread
  spawn + `McpAuthEvent` for auth, flag-clear for forget).

**Exit criteria:** tools dialog publishes only; no inline `save_config` /
`rcu` / thread spawn in `render_row`; gate green.

### Stage 6 — File tree (largest)

**Goal:** migrate `TreeNodeContext` handlers and context menus to publish
`UserCommand`, then retire the snapshot/flush pattern.

- Extend `UserCommand` with `SelectFile`, `SelectDirectory`, `OpenInEditor`,
  `ShowInExplorer`, `CopyPath`, `SaveAsPdf`, `Rename`, `Move`, `Delete`,
  `CreateDirectory`, `CreateDocument`, `RunSkillPrompt`, `MergePrompt`.
- `TreeNodeContext` gains a `UserCommandProducer`. Handlers (`apply_file_row_click`,
  `apply_directory_row_click`, `build_merge_prompt`, context-menu items) publish
  commands instead of mutating the snapshot.
- `write_back` is removed once no handler mutates the snapshot for those four
  fields. `from_app_state` keeps the read-only fields it still needs for
  *rendering* (selection, tabs, dialogs, layout, content_libraries, modifiers),
  but no longer snapshots `submit_prompt`.
- Executor arms:
  - `SelectFile` / `SelectDirectory` → update `selection`, refresh
    `selected_dir`, add tab if single-select.
  - `ShowInExplorer` → `crate::ui::show_in_file_explorer`.
  - `CopyPath` → `ui.copy_text`.
  - `SaveAsPdf` → spawn `pdf-export` thread (gated feature).
  - `Delete` → `crate::utils::recycle_bin::delete` + `file_event_producer.publish_removed`.
  - `RunSkillPrompt` → set `selected_dir`/`selected_file` + `start_agent_session`.
  - `MergePrompt` → build merge prompt + `start_agent_session`.
- Tree handler tests assert on published commands; executor tests cover the
  state-mutation arms. Context-menu actions that shell out (`ShowInExplorer`,
  `SaveAsPdf`, `Delete`) stay integration-tested.

**Exit criteria:** file tree publishes only; `write_back` removed; gate green.

### Stage 7 — Global keyboard shortcuts

**Goal:** migrate `ALT+A` and any future shortcuts to publish `UserCommand`.

- Extend `UserCommand` with `ToggleAgentDebugWindowShortcut`.
- `update_ui`'s `ALT+A` branch publishes instead of mutating
  `agent_panel_state.show_debug_window` directly.
- (The egui `OpenUrl` drain stays as-is — it's a platform-output side effect,
  not user intent in the command sense. If desired later, it can become
  `UserCommand::OpenUrl(String)`, but that's out of scope here.)

**Exit criteria:** `ALT+A` publishes; gate green.

### Stage 8 — Retire `submit_prompt` slot

**Goal:** remove the deferred-action slot now that skill-prompt paths publish
`RunSkillPrompt` (Stage 6).

- Remove `orchestrator.submit_prompt: Option<String>`.
- Remove the `submit_prompt` branch of `handle_deferred_actions`.
- The `dialogs.batch_handle` polling branch of `handle_deferred_actions` stays
  (it's not user intent — it's completion polling for a background thread).
- Update `TreeNodeContext::from_app_state` to drop `submit_prompt`.
- Update any remaining tests referencing `submit_prompt`.

**Exit criteria:** `submit_prompt` field and its deferred-action branch gone;
gate green; all skill-prompt paths flow through the bus.

### Stage 9 — Cleanup & documentation

- Update `doc/technical-context/ARCHITECTURE_C4.md` to reflect the new
  `Bus<UserCommand>` and the `command_executor` module (RUST-042: must update
  when module boundaries or contracts change).
- Add `//!` module doc to `user_command.rs` and `command_executor.rs`
  (RUST-010). Add `///` docs to every `pub` item (RUST-011).
- Cross-link `REQ-xxx` from `SPEC.md` in the new modules' doc comments where a
  user-facing behaviour maps to a requirement (RUST-040). **Do not edit
  `SPEC.md`** (RUST-043).
- Final full quality-gate run from the repo root.

**Exit criteria:** docs updated; gate green; plan closed.

---

## 7. Risks & mitigations

| Risk | Mitigation |
|------|------------|
| `UserCommand` grows large and unwieldy | Group variants by surface with comment headers (as above). If it exceeds ~40 variants, split into sub-enums per surface and flatten at the bus boundary. Revisit at Stage 6. |
| `BatchConfig` / `DeficitStrategy` clone cost | Both are small value-types; `BatchConfig` is already cloned into `BatchCoordinator::new`. No action unless a profile shows cost. |
| Frame ordering: a command published during `render_panels` is drained next frame, not this one | Acceptable — one frame of latency is invisible. If a command must take effect *before* the next render (e.g. opening a modal that suppresses panel input), drain the bus again after `render_panels` in `update_ui`. Decide per-stage as needed. |
| Lagged reader (capacity 8192) | A user can't out-type 8192 commands per frame. If a pathological case arises, `try_recv_exposing_lag` reports it; log + continue (mirrors the existing `agent_event_lagged` handling). |
| `TreeNodeContext` snapshot/flush removal destabilises tree render | Stage 6 is the largest surface; do it last, after the executor is battle-tested by Stages 1–5. Keep `write_back` until every handler publishes, then remove in one final edit. |
| Stale `src/desktop/` path in `AGENTS.md` | Already noted in §6. Do not "fix" `AGENTS.md` unless explicitly instructed. |

---

## 8. Open questions

1. **`REQ-xxx` traceability.** RUST-040 requires every user-facing behaviour to
   map to a requirement in `SPEC.md`. Should the new modules cite existing
   `REQ-xxx` IDs, or is there a SPEC.md section you'd like me to read first
   for the command-handling requirement? (I will not edit `SPEC.md` —
   RUST-043.)
2. **Cancel semantics for dialogs.** When a user cancels a modal, should that
   publish a `Cancel*` `UserCommand` (observable, replayable) or just close
   the dialog locally (dialog-local state, not intent)? Default in the plan
   above is: cancel is local; only submit publishes. Confirm or override.
3. **`OpenUrl` scope.** Should markdown-link clicks (egui `OpenUrl` commands)
   become `UserCommand::OpenUrl(String)`, or stay as platform-output side
   effects drained at frame end? Default in the plan: out of scope. Confirm.
4. **First deliverable scope.** Should I (a) stop here with the plan, or
   (b) proceed to Stage 0 (scaffolding) as the first implementation task?