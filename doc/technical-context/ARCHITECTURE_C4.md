# FastMD C4 Architecture Diagram

## Context Diagram (Level 1)

```mermaid
C4Context
  title FastMD System Context

  Person(user, "User", "Views, edits and batch-processes Markdown libraries; drives an AI agent")

  System_Boundary(fb, "FastMD") {
    System(fastmd, "FastMD Desktop", "Rust + iced native Windows app (crate 'fastmd')")
  }

  System_Ext(llm, "OpenAI-compatible LLM", "OpenRouter / any compatible endpoint")
  System_Ext(jmap, "JMAP Server", "Email, calendar, contacts (Rustave Stork AG)")
  System_Ext(caldav, "CalDAV/CardDAV", "Calendar + address-book servers")
  System_Ext(searxng, "SearXNG", "Web search backend (self-hosted)")
  System_Ext(nominatim, "Nominatim / Open-Meteo", "Geocoding + weather")
  System_Ext(playwright, "Chromium via Playwright", "Browser automation sub-system")
  System_Ext(pdf, "PDF Converter CLI", "External command (pdf_converter_command)")
  System_Ext(fs, "Local Filesystem", "Content libraries on disk")

  Rel(user, fastmd, "Uses")
  Rel(fastmd, llm, "Chat completions / vision")
  Rel(fastmd, jmap, "JMAP (email/calendar/contacts)")
  Rel(fastmd, caldav, "CalDAV/CardDAV")
  Rel(fastmd, searxng, "web_search")
  Rel(fastmd, nominatim, "weather tool (geocode + forecast)")
  Rel(fastmd, playwright, "browser_* tools")
  Rel(fastmd, pdf, "PDF rendering worker")
  Rel(fastmd, fs, "Reads/writes content libraries")
```
## Container Diagram (Level 2)

The `fastmd` crate produces a single desktop application. The UI runs on the
**iced 0.14** runtime stack, all of it inside the one native binary:

| Layer | Crate (locked in `Cargo.lock`) | Role |
| --- | --- | --- |
| Windowing / OS event loop | `winit` 0.30 | Window creation, input events, DPI reports, close requests |
| GPU renderer | `wgpu` 27 | Hardware-accelerated paint (the `wgpu` cargo feature) |
| Software renderer | `tiny-skia` 0.11 | CPU paint path when no usable GPU backend is present |
| Text shaping | `cosmic-text` 0.15 | Font resolution, glyph shaping, text layout, hit-testing |
| Elm runtime + widgets | `iced` 0.14 / `iced_widget` 0.14.2 | `boot`/`update`/`view`/`subscription` loop, widget kit, theme/palette plumbing |

The application's own async work (buses, background workers, the agent driver)
runs on `tokio`; iced's futures executor is the default `thread-pool`, so the
two never share a runtime — they meet only at the `ui_iced::subscription`
bridge (see the Messaging diagram below).

```mermaid
C4Container
  title FastMD — Containers

  Container_Boundary(app_crate, "fastmd crate") {
    Container(ui_iced, "ui_iced", "Iced UI subsystem: boot/update/view/subscription, panels, overlays, renderers")
    Container(ui_free, "ui (framework-free)", "Domain view-models, string + palette tokens, tab/selection/tree/persisted state")
    Container(orch, "AppOrchestrator", "Wires config, workspace, tabs, tags, dialogs, agent session, command executor")
    Container(bus, "bus", "Bus<T> broadcast + typed mpsc channels + UserCommand intake")
    Container(bg, "background", "Indexer, file watcher, PDF/image workers, log ring buffer")
    Container(md, "markdown", "GFM parse -> RenderEvent/InlineElem model, Fair Table Width solver")
    Container(ws, "workspace", "Workspace file/directory projection, content libraries, Tags")
    Container(exp, "export", "Print + Typst PDF pipeline")
  }

  Container(agent_crate, "fastmd-agent", "Separate crate: agent turn loop, LLM client, tool registry + tool implementations")
  Container(iced_pro, "iced-pro", "Separate crate: UIF-* design tokens, contrast solver, focus model, menu engine, surface hosts. Consumed by fastmd as the design-truth source since INT-1")
  Container(iced_rt, "iced runtime", "winit + wgpu/tiny-skia + cosmic-text under the Elm loop")

  Rel(ui_iced, ui_free, "borrows view-models, strings, palette tokens")
  Rel(ui_iced, bus, "publishes UserCommand; reads buses through the subscription bridge")
  Rel(ui_iced, iced_rt, "builds Elements; the runtime paints and dispatches input")
  Rel(ui_free, iced_pro, "derives overlapping palette tokens from UIF-TOK-005")
  Rel(ui_iced, iced_pro, "maps iced theme roles, scrim alpha and splitter grab from tokens")
  Rel(ui_iced, orch, "reducer drives orchestrator state transitions")
  Rel(orch, ws, "file/tag/directory state")
  Rel(orch, agent_crate, "submits AgentPrompt, observes AgentEvent")
  Rel(orch, md, "parses the open document")
  Rel(orch, exp, "print / save-as-PDF jobs")
  Rel(bg, bus, "publishes FileChanged / DirectoryChanged / BackgroundEvent")
  Rel(ws, bus, "consumes separate file and directory event buses")
```

There are three crates in this workspace that matter to the picture above:
`fastmd` (the application, `src/app/`), `fastmd-agent` (the agent engine,
`src/agent/`) and `iced-pro` (`src/iced-pro/`). The first two are wired
together — `AppOrchestrator` submits prompts to and observes events from
`fastmd-agent`.

`iced-pro` is now consumed by the application, but only as the **design
token source**. Since integration increment INT-1 the `fastmd` crate
declares `iced-pro = { path = "src/iced-pro" }`, and:

- `src/app/ui/theme.rs` derives every overlapping semantic palette token
  from `iced_pro::token::color::Palette::dark()` (`UIF-TOK-005`) instead
  of a literal, and delegates the inverted editor set to
  `iced_pro::token::color::EditorPalette::inverted()` (decision **D-4**);
- `src/app/ui_iced/theme.rs` maps the framework palette onto the iced
  theme roles and takes the modal scrim from
  `iced_pro::surface::zorder::scrim_for`;
- `src/app/ui_iced/layout.rs` binds the pane-grid resize leeway to
  `iced_pro::token::size::SPLITTER_GRAB` (`UIF-TOK-002`).

What is **not** yet consumed: the framework's widgets, menu engine, modal
hosts, focus model and geometry solvers. The app still renders with stock
iced widgets, and the six FastMD document/log content tokens stay
app-owned literals under decision **D-2**. Porting the widget layer onto
`iced-pro` is the follow-on work decision **D-8** describes. See the
`iced-pro` component diagram further down for what that crate contains and
what it deliberately does not own.

## Optional Vector Search

When the `vector-search` feature is enabled, `app/background/vector_search.rs` owns a SahomeDB collection and `app/background/embeddings.rs` builds an OpenAI-compatible embedding client (via `async-openai`) from the user-configured `embeddings` model (AGENT-005). `Task` creates the shared service before starting the initial scan, subscribes it to `Bus<FileChanged>` before scan events are published, and injects the service into `AgentSession` through the existing `Extensions` container. Markdown chunks are hashed, embedded, and stored in the local collection; the read-only `vector_search` built-in tool performs searches through that service. The service does not replay file events emitted before its subscription and reports progress through `BackgroundEvent::Process(LogEntry(...))` every 100 processed Markdown files.


### UI Layer (Level 3)

`ui_iced/` renders the desktop app. The UI is one iced Elm loop — `boot`
(builds the state, returns the batched boot `Task` chain), `update` (the
reducer), `view` (pure widget construction) and `subscription` (the inbound
bus bridge) — wired together by `ui_iced::app::launch`. The loop's state is
`AppState` (`ui_iced/app.rs`), a **thin wrapper** around the framework-free
`FastMdApp` domain object plus the iced-specific runtime bits: the pane-grid
state, the live bus receivers, the injected UI-state storage, the OS scale
baseline and the last known viewport size.

The window is a **three-band** layout: the top toolbar band, the middle band,
and the bottom command band. The middle band is an
`iced::widget::pane_grid` split `left | (center | right)` — file tree,
document/agent surface, and the gated table-of-contents pane. Pane-grid resize
drags reduce 1:1 onto the pure `PanelLayout` widths, clamped by the UI-016
min/max constants. Every floating surface (context menu, inline editor, small
dialogs, log viewer, agent debug window, tools dialog, About, attributions,
batch dialog) is mounted by a single **overlay host** above the bands in a
fixed z-order, with a scrim layer that swallows input below the top card.

The view holds no state. Each panel is a pure function over a
view-model snapshot; all snapshots live in `AppState::models` and are rebuilt
by `AppState::rebuild_view_models` at the end of every `update` call, because
several iced widgets (notably `pick_list`) borrow their option vectors out of
the snapshot and cannot own them inside `view`. All colours flow from
`ui/theme.rs`, the single source of truth for the dark palette (`Palette`,
whose token type is `iced::Color`) and the inverted inline-editor token set
(`EditorColors`); `ui_iced/theme.rs` maps those semantic tokens onto the iced
palette roles and hands the resulting `iced::Theme::custom` to the launch
builder. Repeated iced idioms — the cursor-anchored, viewport-clamped context
menu, the blocking modal card, the confirm/cancel button pair, the
follow-the-stream scroll rule — live in `ui_iced/overlay.rs` and
`ui_iced/logs.rs`, which the panels, modals, tree and renderers call instead
of hand-rolling the widget tree.

```mermaid
C4Component
  title FastMD — UI Layer (ui_iced)

  Component(app, "app", "AppState (thin wrapper over FastMdApp) + ViewModels + OverlayState; boot/boot_with_persisted (build_state performs the synchronous DEC-009 ConfigArrived publish), window_settings, launch() builder chain")
  Component(update, "update", "Top-level reducer: window close/save, pane resize, Message::Command publish-and-execute, Inbound bus reductions, finish_frame tail + view-model rebuild")
  Component(view, "view", "Three-band layout with the real panels mounted, under the overlay host (z-ordered floating windows)")
  Component(sub, "subscription", "Inbound Subscription<Message> lanes: ConfigArrived, FileChanged, DirectoryChanged, AgentEvent, BackgroundEvent (pure bus_event_to_message mapper, Lagged(n) surfaced) + the winit window lane (close_requests, resize_events)")
  Component(msg, "message", "Top-level Message enum: one variant per logical interaction channel (Window, Layout, Command, Tree, Tab, Editor, Agent, Search, Logs, Theme, Render, Dialog, About, Batch, Tools, AgentDebug, Inbound, Boot)")
  Component(layout, "layout", "Three-band pane-grid geometry: Configuration tree, MiddleBandPanes id discovery, resize -> PanelLayout clamping")
  Component(panels, "panels", "Pure pane view functions: top toolbar, left tree/search, center tabs/document, bottom command band, right ToC rules")
  Component(render, "render", "Markdown viewing path: RenderEvent -> RenderBlock descriptors -> iced widgets (heading, inline, code, table, yaml)")
  Component(overlay, "overlay", "Context-menu + blocking-modal primitives: anchor clamping, scrim blocker, confirm/cancel pair")
  Component(editor, "editor", "Inline Markdown editor modal: text_editor host, DEC-007 undo/redo history stack, pre-save GFM validation, front-matter-preserving save")
  Component(agentui, "agent", "Agent panel: streaming transcript through iced's built-in markdown widget, thinking-delimiter gate, prompt row with send/queue/cancel + chat-model pick_list")
  Component(windows, "logs / agent_debug / dialogs / about / attributions / batch_dialog / tools_dialog / search", "The floating-window view-models and pure rules behind each overlay card")
  Component(persist, "persisted", "PersistedUiState save/restore through the injected UI-state storage handler")
  Component(tablew, "table_width (iced adapter)", "Available width -> FTWA solver inputs -> pinned iced column widths; heuristic text measurer")
  Component(theme, "theme", "Palette token -> iced palette-role mapping; Theme::custom('FastMD Dark'); scrim colour (UI-002/GUD-003)")
  Component(fonts, "fonts", "iced::font::load bootstrap for the embedded icon font + platform symbol fallback")
  Component(osshell, "os_shell", "Framework-free platform wrappers: open-in-system-editor, reveal-in-file-explorer, open-URL")
  Component(logo, "logo", "The single place the shared logo mark element is constructed")
  Component(helpers, "test_helpers (cfg(test))", "Framework-free fixtures: test_app / test_app_state over in-memory storage")

  Rel(app, update, "boot -> update")
  Rel(app, view, "boot -> view")
  Rel(app, sub, ".subscription(receivers)")
  Rel(update, msg, "reduces every channel")
  Rel(update, sub, "Inbound messages originate here")
  Rel(view, panels, "mounts the bands")
  Rel(view, overlay, "stacks the floating layers")
  Rel(view, render, "renders the open document")
  Rel(panels, msg, "emits Message::Command / panel channels")
  Rel(panels, overlay, "context menus + modal cards")
  Rel(panels, layout, "pane geometry")
  Rel(render, tablew, "solved column widths")
  Rel(theme, app, ".theme(fastmd_dark_theme)")
  Rel(fonts, app, "font boot task in boot()")
  Rel(update, osshell, "platform side effects")
  Rel(panels, logo, "logo_mark")
```

Supporting UI types (framework-free, in `ui/`): `TreeNode{name,path,is_dir,children:BTreeMap}`,
`ToCEntry{title,level,id}`, `PersistedUiState{schema_version,left/right_panel_width,window_x/y/width/height,font_size_scale,expanded_dirs}`.

## Component Diagram (Level 3) — Agent Core

`agent/` implements the LLM tool-loop. A single long-lived driver thread
(spawned by `AgentSessionManager`) owns a `Receiver<AgentPrompt>` and blocks
on `recv()`. On each prompt, it builds a per-session `AgentContext` and runs
`run_agent` directly inline (no double-spawn). The agent publishes
`AgentEvent`s on a `Bus<AgentEvent>` (tokio broadcast); the UI subscribes
and drains per frame.

> **Agent↔UI Seam (feature 003, complete)**: `agent/events.rs` defines the
> structured seam types — `AgentPrompt` (UI→agent mpsc input, carries
> `session_id: Uuid` + `cancel_flag: Arc<AtomicBool>`), `AgentEvent`
> (agent→UI `Bus<AgentEvent>` broadcast output, 11 session-tagged variants),
> `AgentStatus` (typed status), `ToolSideEffect` (file-creation side effect),
> `DelegateToolCall` (structured web-delegate trace). The old
> `BackgroundEvent::Agent(LegacyAgentEvent)` variant and the
> `tx_gui: Sender<BackgroundEvent>` path have been removed. UI formatting
> lives in `ui/render/agent_render.rs`; the agent has zero `ui::render` imports.

```mermaid
C4Component
  title FastMD — Agent Core

  Component(mgr, "AgentSessionManager", "AgentState{running,status,thinking,response,history,token_usage,total_usage,debug_entries}; owns prompt_tx (UI→agent mpsc) + event_bus (agent→UI Bus); driver_handle: JoinHandle; current_session_id: Uuid"")
  Component(impl, "run_agent / run_agent_inner", "Resolves LLM client, builds messages, get_tools_schema, ToolExecutor::new, turn loop; called inline by the driver (no double-spawn)")
  Component(ctx, "AgentContext", "config, prompt, history, active_file, active_dir, selected_files, cancel_flag, session_id: Uuid, agent_event_bus, file_event_bus; no tx_gui, no current_response, no session_number")
  Component(ev, "events", "AgentPrompt (UI→agent mpsc, carries cancel_flag), AgentEvent (agent→UI Bus, 11 session-tagged variants), AgentStatus, ToolSideEffect, DelegateToolCall — the agent↔UI seam types")
  Component(llm, "LLMClient", "parse_usage_block; OpenAI-compatible HTTP")
  Component(pb, "SystemPromptBuilder", "with_active_file/dir/selected_files; USER.md injection per library")
  Component(te, "ToolExecutor", "returns (results, Vec<ToolSideEffect>); safe tools parallel / unsafe tools sequential")

  Rel(mgr, impl, "submit_prompt -> run_agent")
  Rel(impl, ctx, "builds per-session context")
  Rel(impl, ev, "publishes AgentEvent on Bus<AgentEvent>")
  Rel(mgr, ev, "owns event_bus: Bus<AgentEvent>")
  Rel(impl, llm, "chat completions")
  Rel(impl, pb, "system prompt")
  Rel(impl, te, "dispatch tools")
```

## Component Diagram (Level 3) — Tool System

`tools/` defines the `Tool` trait and a `ToolManager` (formerly
`ToolRegistry`) that owns the catalog of built-in and MCP-discovered
tools, the per-group enable/parallel-safe/error state, and the
[`McpClients`](../../src/agent/lib/mcp/mod.rs). `ToolContext<'a>` is the single
parameter passed to every `Tool::execute`, carrying `&AppConfig` and
`&Bus<FileChanged>`.

```mermaid
C4Component
  title FastMD — Tool System

  Component(reg, "ToolManager", "catalog + per-group state + error tracking + parallel-safety; register_builtin, register_mcp_tool, execute, get_schema, safety_of, parallel_safe_tools, set_group_enabled, record_error, clear_error, refresh_state, refresh_mcp_tools")
  Component(tctx, "ToolContext", "{config, file_event_bus}; thin shim over app::vfs::resolve::resolve(vpath, allow_write, libraries) -> Option<(PathBuf,bool)>")
  Component(mcp, "McpToolAdapter", "wraps agent::lib::mcp::McpClientManager into a Tool impl; LLM-tool-loop glue in agent/tools/mcp/adapter.rs")
  Component(fs, "filesystem tools", "grep, read_file, read_lines, create_file, insert_lines, replace_text, list_files")
  Component(yaml, "yaml_header", "read_yaml_header, write_yaml_header")
  Component(web, "web", "web_fetch (pagination/headers/5-min cache), web_search (SearXNG), web_delegate sub-agent")
  Component(csv, "csv_db", "{mod, operations, query (evalexpr), schema}; add_rows, delete_rows, create_csv, list_csv, query; gated by prompt keywords")
  Component(jmap, "jmap", "{client, calendar, contacts, email, tests}; search/get/add/update/delete calendar/email/contact")
  Component(caldav, "caldav", "CalDAV calendar tools")
  Component(carddav, "carddav", "CardDAV contact tools")
  Component(weather, "weather", "Nominatim geocode + Open-Meteo forecast")
  Component(dtos, "dtos", "Shared tool data-transfer objects")

  Rel(reg, tctx, "passes to each Tool::execute")
  Rel(reg, fs, "registers")
  Rel(reg, yaml, "registers")
  Rel(reg, web, "registers")
  Rel(reg, csv, "registers (conditional)")
  Rel(reg, jmap, "registers")
  Rel(reg, caldav, "registers")
  Rel(reg, carddav, "registers")
  Rel(reg, weather, "registers (extra)")
  Rel(reg, mcp, "registers each MCP-discovered tool")
  Rel(tctx, fs, "app::vfs::resolve::resolve for read/write")
```

```mermaid
C4Component
  title FastMD — Virtual File System (app/vfs/)

  Component(vp, "virtual_path", "VirtualPath, VirtualPathError{EmptyPath,TraversalDetected,InvalidFormat,LibraryNotFound,LibraryNotWritable}; parse/resolve/is_writable; rejects '..' traversal")
  Component(lib, "library", "ContentLibraryExt trait (display_label_for, contains_path, resolve, is_writable, root_path); library_display_label free function")
  Component(res, "resolve", "Pure resolve(vpath, allow_write, libraries) -> Result<Option<(PathBuf,bool)>, String>; resolve_writable helper for mutating tools")
  Component(tctx, "ToolContext (shim)", "resolve_virtual_path / resolve_writable forward to app::vfs::resolve::resolve")
  Component(cl, "ContentLibrary (data type)", "struct ContentLibrary{root_folder, name, kind, readonly, priority} — data shape owned by config/, behaviour owned by app::vfs")

  Rel(tctx, res, "calls resolve(allow_write, libraries) with self.config.content_libraries")
  Rel(res, vp, "VirtualPath::parse, resolve, is_writable")
  Rel(res, lib, "ContentLibraryExt::{is_writable, root_path, resolve} on matched library")
  Rel(lib, cl, "impl ContentLibraryExt for ContentLibrary")
```

Tool inventory (matches `Tools.md` + conditional tools):
- **Core Workspace (11):** `grep`, `read_tags`, `list_files_by_tag`,
  `list_files`, `read_file`, `read_lines`, `create_file`,
  `insert_lines`, `replace_text`, `read_yaml_header`,
  `write_yaml_header`.
- **Web Integration (3):** `web_fetch`, `web_search`, `web_delegate`.
- **JMAP Productivity (11):** `search_calendar`, `get_calendar`,
  `get_calendar_item`, `add_calendar_item`, `update_calendar_item`,
  `delete_calendar_item`, `search_email`, `get_email_by_id`, `send_email`,
  `search_contact`, `get_contact`, `add_contact`.
- **Conditional / extra (6):** `add_rows`, `delete_rows`, `create_csv`,
  `list_csv`, `query` (CSV DB, prompt-keyword gated),
  `weather` (not in SPEC tool table).

> `tools/Spotify.md` is a **proposal** for 25+ `spotify_*` tools via OAuth2
> PKCE; not implemented.

## Component Diagram (Level 3) — Background Workers

`background/` hosts the crawler and long-running workers, coordinated by `background::Task`,
which owns the worker queues, event buses, and the `BackgroundEvent` broadcaster.

```mermaid
C4Component
  title FastMD — Background Workers

  Component(task, "Task", "Owns BackgroundEvent broadcaster, file/directory buses, worker queues, and watcher slot; starts indexing after ConfigArrived or timeout")
  Component(indexer, "Indexer", "Background crawler; publishes file and directory events")
  Component(filebus, "Bus<FileChanged>", "File additions, changes, and deletions")
  Component(dirbus, "Bus<DirectoryChanged>", "Directory additions and deletions")
  Component(bev, "BackgroundEvent", "Background results delivered to the UI through the async bridge")
  Component(tags, "TagManager", "Subscribes to FileChanged; four workers attempt YAML front-matter extraction")
  Component(watcher, "FileWatcher", "notify 6.0; recursive; auto-watch new dirs")
  Component(pdf, "PdfConverterWorker", "PdfConversionJob queue; pdf_converter_command")
  Component(vision, "ImageVisionWorker", "process_image: base64 data URL -> vision use_case model")
  Component(router, "BusRouter", "PDF manager subscribes before scan; routes eligible PDFs; image routing is feature-gated")
  Component(bgmgr, "BackgroundProcessManager", "VecDeque ring buffer MAX_LOG_ENTRIES=10_000; filter/search/auto_scroll/show_background_logs; log persistence to logs/background-process.log; SharedProcessManager = Arc<Mutex<...>>")
  Component(bgmodels, "models", "BackgroundLogEntry, LogCategory{Indexer,Watcher,PDF Converter,Image Vision,LLM Tools}")

  Rel(task, indexer, "drives")
  Rel(task, tags, "spawns")
  Rel(task, filebus, "owns")
  Rel(task, dirbus, "owns")
  Rel(indexer, filebus, "publishes file discoveries")
  Rel(indexer, dirbus, "publishes directory discoveries")
  Rel(tags, filebus, "subscribes")
  Rel(tags, bev, "publishes tag results")
  Rel(task, watcher, "owns")
  Rel(watcher, filebus, "publishes file changes")
  Rel(watcher, dirbus, "publishes directory changes")
  Rel(task, pdf, "spawns")
  Rel(task, vision, "spawns")
  Rel(task, router, "wires")
  Rel(router, filebus, "subscribes")
  Rel(router, pdf, "queues eligible PDFs")
  Rel(bgmgr, bgmodels, "stores entries")
```

## Component Diagram (Level 3) — Messaging Architecture (`bus/`)

The `bus/` subsystem provides explicit, type-safe event transportation across thread
boundaries in `fastmd`. All cross-thread updates (file modifications, background indexing
progress, agent turn streaming, worker log entries, MCP authentication) flow through either
multi-producer/multi-consumer broadcast buses (`Bus<T>`) or domain-specific typed `mpsc` channels.

```mermaid
C4Component
  title FastMD — Messaging & Event Bus Subsystem

  Component(core, "Bus<T> / BusReader<T>", "Thread-safe MPMC broadcast channel backed by tokio::sync::broadcast (capacity 8192)")
  Component(fev, "FileChanged", "FileChanged{kind Added|Deleted|Changed, files[{path, file_name}]} on Bus<FileChanged> ([REQ-330..334]); FileEventProducer publishes file-only changes")
  Component(dev, "DirectoryChanged", "DirectoryChanged{path, kind Added|Deleted} on Bus<DirectoryChanged> ([REQ-320..322]); DirectoryEventProducer publishes directory-only changes")
  Component(bev, "BackgroundEvent & Sub-Enums", "Typed UI event wrapper: FsEvent, ProcessEvent, McpAuthEvent (AgentEvent removed — agent events now on Bus<AgentEvent>)")
  Component(cfg_bus, "ConfigArrived & config_bus", "Startup config broadcast; published synchronously by ui_iced::app::build_state (DEC-009); subscribers fall back to AppConfig::default() after CONFIG_ARRIVAL_TIMEOUT (10 s, defense-in-depth)")
  Component(router, "BusRouter", "Subscribes to Bus<FileChanged>; routes eligible .pdf and feature-gated image paths to worker queues")
  Component(tags, "TagManager", "Subscribes to Bus<FileChanged>; a four-thread pool attempts YAML extraction for Added/Changed paths")
  Component(task, "Task", "Owns BackgroundEvent broadcast sender, file_event_bus: Bus<FileChanged>, and dir_event_bus: Bus<DirectoryChanged>")
  Component(ui, "FastMdApp", "UI thread loop; receives Task BackgroundEvents through the iced subscription")

  Rel(fev, core, "uses Bus<FileChanged>")
  Rel(dev, core, "uses Bus<DirectoryChanged>")
  Rel(cfg_bus, core, "uses Bus<ConfigArrived>")
  Rel(router, core, "subscribes to Bus<FileChanged>")
  Rel(tags, core, "subscribes to Bus<FileChanged>")
  Rel(tags, bev, "sends FsEvent tag results")
  Rel(task, tags, "spawns tag workers")
  Rel(task, core, "owns both file and directory buses")
  Rel(ui, task, "receives BackgroundEvent through the async bridge")
  Rel(router, task, "forwards PDF/image paths to worker channels")
```

### Overview & Design Principles

1. **Explicit Thread Communication**: Shared mutable state across thread boundaries is strictly avoided. Subsystems communicate exclusively via event channels.
2. **MPMC Broadcast vs MPSC Channels**:
   - **Multi-Producer Multi-Consumer (`Bus<T>`)**: Used when multiple consumers must independently observe the same event (for example, `FileChanged` is observed by the workspace projection, PDF-backing tracker, `BusRouter`, and vector search; `DirectoryChanged` is observed by the workspace projection).
   - **Multi-Producer Single-Consumer (`mpsc::channel`)**: Used when events target a single recipient (e.g., PDF/image worker queues or `AgentPrompt`). `BackgroundEvent` instead uses its own Tokio broadcast channel before the iced bridge forwards it to the UI.
3. **Non-Blocking UI Integration**: `BusReader` provides non-blocking `try_recv()` and `recv_timeout()` methods wrapped in a `Mutex`. In the iced build the UI never polls them directly: `ui_iced::subscription` turns every bus into an `iced::Subscription<Message>`, so background notifications arrive as messages on the Elm loop and the update/view path never blocks on I/O ([FMD-065]).

### Message Types & Payloads

- **`FileChanged`** (`bus/events/file.rs`, `Bus<FileChanged>`, [REQ-330..334]): one event-level `Kind` (`FileChangedKind::{Added, Deleted, Changed}`) describes the whole `[Files]` list ([REQ-331]); every entry (`FileEntry`) carries a fully qualified `path` ([REQ-333]) and its materialized `file_name` ([REQ-334]). A rename publishes `[Deleted]` then `[Added]` ([REQ-404]). `FileEventProducer` only publishes this file payload.
- **`DirectoryChanged`** (`bus/events/directory.rs`, `Bus<DirectoryChanged>`, [REQ-320..322]): one directory per event — fully qualified `path` ([REQ-321]) and `kind` (`DirectoryChangedKind::{Added, Deleted}`, [REQ-322]). A directory rename publishes `[Deleted]` then `[Added]` ([REQ-404]). `DirectoryEventProducer` only publishes this directory payload.
- **Producers:** `Indexer::scan_libraries` publishes file additions and directory additions as it enumerates entries ([REQ-320]/[REQ-330]); `FileWatcher` publishes file changes and directory lifecycle changes on their distinct buses ([REQ-402]); filesystem commands and agent tools publish file changes through the file producer.
- **`BackgroundEvent`** (`bus/events/typed.rs`): Top-level enum carrying domain-specific asynchronous updates to the UI loop:
  - **`FsEvent`**: `FileParsed { path, tags }`, `DirParsed { path }`, `FileModified { path, tags }`, `FileDeleted { path }`, `Finished`, `FinishedWithoutWatcher`. `DirParsed` has no current crawler producer; directory lifecycle is carried by `Bus<DirectoryChanged>`, while the legacy typed variant and handler remain for compatibility/tests.
  - **`ProcessEvent`**: `LogEntry(BackgroundLogEntry)`, `FileLoaded { path, content }`.
  - **`McpAuthEvent`**: `Completed { server_name, error }`.
- **`AgentEvent`** (`agent/events.rs`): Agent→UI `Bus<AgentEvent>` broadcast (tokio, capacity 8192). 11 session-tagged variants: `SessionStarted`, `SessionFinished{history}`, `Status(status: AgentStatus)`, `Thinking`, `ContentDelta`, `ToolCallStarted{id,name,args}`, `ToolResult{id,name,result}`, `ToolSideEffect`, `DebugEntry`, `TokenUsage`, `Failed`. Every variant carries `session_id: Uuid`.
- **`AgentPrompt`** (`agent/events.rs`): UI→agent mpsc input carrying `session_id`, `text`, `active_file/dir`, `selected_files`, `cancel_flag: Arc<AtomicBool>`. The long-lived driver thread blocks on `recv()`.
- **`ConfigArrived`** (`bus/events/config.rs`): Published once at startup carrying `config: AppConfig`. Enables lazy component initialization and prevents race conditions during startup. In the iced build the publish is synchronous inside `ui_iced::app::build_state` (after `BusReceivers::new`), so every subscriber is already live when the event is sent; a 10 s `CONFIG_ARRIVAL_TIMEOUT` fallback covers a delayed or missed publish.
- **`TokenUsageInfo`** (`bus/events/messages.rs`): Detailed LLM token consumption metrics (`prompt_tokens`, `completion_tokens`, `total_tokens`, `cached_tokens`, `reasoning_tokens`).

### Event Routing & Dispatch

1. **`BusRouter` (`bus/router/bus_router.rs`)**:
   - Subscribes to `Bus<FileChanged>` only; directory lifecycle events do not enter the conversion router.
   - Filters for `[FileChanged][Added]` and `[FileChanged][Changed]` events.
   - The production PDF route subscribes before the initial crawler publishes; it filters `.pdf` Added/Changed entries, checks the corresponding `.md` timestamp, and only queues conversion when a converter is configured.
   - With `image-library`, the post-scan image route filters image extensions and forwards paths to `ImageVisionWorker`.
2. **`TagManager` (`background/tag_manager.rs`)**:
   - Subscribes to `Bus<FileChanged>` before the initial scan, ignores deletions, and queues every Added/Changed path for four background YAML-front-matter readers ([REQ-422]).
   - Workers return `FsEvent::FileParsed` / `FsEvent::FileModified` via `BackgroundEvent`; the UI reducer applies tag state, while `AppOrchestrator::is_workspace_file` keeps hidden formats such as PDFs out of the visible workspace ([REQ-451], [FMD-069]).
3. **UI Inbound Bridge (`ui_iced/subscription.rs` + `ui_iced/update.rs`)**:
    - The bus→`Subscription` bridge owns one stream per bus (`Bus<ConfigArrived>`, `Bus<FileChanged>`, `Bus<DirectoryChanged>`, `Bus<AgentEvent>`) and one for `Task.rx_futures` (`futures::channel::mpsc<BackgroundEvent>`). A dedicated thread forwards `BackgroundEvent` values from the task's Tokio broadcast receiver into that futures channel for iced's executor. Each stream maps its payload to `Message::Inbound(InboundMessage::…)` through the pure `bus_event_to_message` mapper. They are merged with `Subscription::batch` alongside the winit **window lane** (`iced::window::close_requests` / `resize_events`, mapped by the pure `close_request_to_message` / `resize_event_to_message` helpers onto `Message::Window`). With `exit_on_close_request: false` the close lane is the only path to the DEC-006 save-and-close reduction and the resize lane keeps `AppState::viewport` in step with real resizes.
   - A `Lagged(n)` error on any bus is not a disconnect: the stream logs the count and forwards `InboundMessage::EventsDropped { source, count }` so the reducer can surface the loss and re-synchronise on the next valid event ([FMD-067], [FMD-072]).
   - `reduce_inbound` in `ui_iced/update.rs` applies `FileChanged` and `DirectoryChanged` messages to the orchestrator's `Workspace` projection, then runs `process_file_events` to drain any legacy/catch-up readers; it also calls `drain_agent_event_bus` for `Agent`, `handle_fs_event` / `handle_process_event` / `handle_mcp_auth_event` for `Background`, and the config projection for `ConfigArrived`. The subscription bridge is the live inbound path in the iced build ([FMD-065]).
   - UI widget state lives in `AgentPanelState` (`ui/agent/panel_state.rs`) plus the reducer-held `OverlayState` / `ViewModels` (`ui_iced/app.rs`): `show_results`, `show_debug_window`, `debug_search_text`, `debug_auto_scroll`, `command_input`, `scroll_to_id`, `active_session_id`.

### Messaging Actors & Recipients Summary

| Channel / Bus | Transport Primitive | Payload Type | Producers (Actors) | Consumers (Recipients) |
|---|---|---|---|---|
| `Bus<FileChanged>` | `tokio::sync::broadcast` (8192 cap) via `Bus<T>` | `FileChanged` | `FileWatcher`, `Indexer`, filesystem commands, agent/tool file operations | `Workspace` / `AppOrchestrator`, PDF-backing tracker, `TagManager`, `BusRouter`, vector search |
| `Bus<DirectoryChanged>` | `tokio::sync::broadcast` (8192 cap) via `Bus<T>` | `DirectoryChanged` | `FileWatcher`, `Indexer`, directory commands | `Workspace` / `AppOrchestrator` |
| `BackgroundEvent` | `tokio::sync::broadcast` (1024 cap) | `BackgroundEvent` (`Fs`, `Process`, `McpAuth`) | `TagManager` workers, Indexer, `PdfConverterWorker`, `ImageVisionWorker`, MCP Auth Flow | `FastMdApp` (UI thread loop via async bridge) |
| `Bus<ConfigArrived>` | `tokio::sync::broadcast` (8192 cap) via `Bus<T>` | `ConfigArrived` | `ui_iced::app::build_state` (synchronous, DEC-009) | background `Task` thread, MCP config subscriber, subscription bridge → UI reducer |
| `Bus<AgentEvent>` | `tokio::sync::broadcast` (8192 cap) via `Bus<T>` | `AgentEvent` (11 variants) | Agent driver thread (`run_agent`) | `FastMdApp` (UI frame drain → `AgentTranscript` + `AgentSessionManager`) |
| `AgentPrompt` | `std::sync::mpsc::channel` | `AgentPrompt { session_id, text, cancel_flag, ... }` | `AgentSessionManager::submit_prompt` (UI → agent) | Agent driver thread (`recv()` loop) |
| PDF Worker Queue | `std::sync::mpsc::channel` | `PathBuf` | `BusRouter` (for `.pdf` files) | `PdfConverterWorker` |
| Image Vision Queue | `std::sync::mpsc::channel` | `PathBuf` | `BusRouter` (for image files) | `ImageVisionWorker` |
| Agent Cancel | `std::sync::atomic::AtomicBool` | `bool` | `AgentSessionManager` (UI Stop button) | `run_agent_inner` turn loop |

## Component Diagram (Level 3) — Supporting Modules

Cross-cutting modules that the UI, Agent, Tools and Background all depend on.

```mermaid
C4Component
  title FastMD — Supporting Modules

  Component(cfg, "config", "AppConfig, LlmConfig{model,api_url,api_key,cost,use_case}, JmapClient, CalDavClient, CardDavClient, content_libraries: Vec<ContentLibrary>; load_config, get_config_path; Debug redacts secrets; config_bus / ConfigArrived (tokio broadcast) published synchronously by ui_iced::app::build_state to fan the loaded config out to the background Task thread and the MCP config subscriber on startup. VFS types re-exported from app::vfs for backwards compat.")
  Component(ev, "file_events", "Bus<FileChanged> (tokio::sync::broadcast, BUS_CAPACITY=8192); FileChanged and FileEventProducer; file-only payload")
  Component(dev, "directory_events", "Bus<DirectoryChanged> (tokio::sync::broadcast, BUS_CAPACITY=8192); DirectoryChanged and DirectoryEventProducer; directory-only payload")
  Component(ws, "watcher/workspace", "Workspace{file_reader, dir_reader, all_files, all_files_set, all_dirs, all_dirs_set, indexing_finished, indexing_finished_handled}; unified file/directory projection")
  Component(tm, "tag_manager", "TagManager{file_tags:BTreeMap<PathBuf,Vec<String>>, all_tags:BTreeSet<String>, prompt_paths:BTreeSet<PathBuf>, selected_tag}")
  Component(msg, "messages", "TokenUsageInfo{prompt_tokens,completion_tokens,total_tokens,cached_tokens,reasoning_tokens}")
  Component(doc, "document", "DocumentContent{front_matter: Option<String>, body: String}; parse via utils::markdown::parse_front_matter")
  Component(ed, "editor", "Inline editor; EditorColors inverted white-on-black; validation via pulldown-cmark; undo/redo >=100; clipboard; cursor nav; save combines body + original front-matter")
  Component(print, "print", "PrintJob{markdown_path, markdown_content, title}; markdown->HTML via pulldown-cmark; execute_print_blocking")
  Component(browser, "browser", "Playwright async: browser_navigate, browser_get_page_state, click, type, screenshot")
  Component(batch, "batch", "{coordinator, discoverer, executor, file_matcher, prompts, types}; batch prompt processing; concurrency 1-8; File vs Directory modes; dialog UI in ui/batch_dialog.rs")
  Component(utils, "utils", "markdown (parse_front_matter), path, tags (extract_tags_from_file)")
  Component(err, "error", "AgentError")

  Rel(cfg, ev, "ContentLibrary data flows into file events")
  Rel(ev, ws, "feeds file projection")
  Rel(dev, ws, "feeds directory projection")
  Rel(ws, tm, "workspace file changes inform tags")
  Rel(msg, ev, "background messages")
```

## Component Diagram (Level 3) — `iced-pro` crate (`src/iced-pro/`)

`iced-pro` implements the widget-library specification
`doc/planning/ui-framework.md`, whose requirement IDs are `UIF-*`.
Package `iced-pro`, library name `iced_pro`, iced 0.14 with the same
renderer features `fastmd` uses plus `advanced` — the only way a custom
widget can reach `iced_core`'s widget-author surface. The library root is
`mod.rs`, not `src/lib.rs`, so the gallery binary is declared explicitly
in `Cargo.toml`.

**Consumption status (INT-1).** `fastmd` depends on this crate and reads its
**tokens** — `token::color`, `token::size` and `surface::zorder::scrim_for`
— from `src/app/ui/theme.rs`, `src/app/ui_iced/theme.rs` and
`src/app/ui_iced/layout.rs`. It does **not** yet instantiate any
framework widget, menu engine, modal host or solver; those remain
described-but-not-consumed by the shipped application. No `fastmd` type
appears anywhere in `iced-pro`'s API, so the dependency stays one-way.
What the crate's own ledger says is and is not satisfied lives in
`src/iced-pro/spec_coverage.rs`.

```mermaid
C4Component
  title iced-pro — Subsystems (tokens consumed by fastmd; widgets not yet)

  Component(token, "token/", "Closed design-token set: space, size, shape, typography, color, contrast (UIF-TOK-001..006)")
  Component(st, "state.rs", "Interaction state machine: exclusive BaseState plus an orthogonal focus ring, resolved purely")
  Component(fm, "focus.rs", "Keyboard focus policy, iced-free: ordered groups, disabled members, nested traps, traversal")
  Component(br, "bridge.rs", "One-way focus bridge: model -> iced focus router; never reads focus back")
  Component(geo, "geometry/", "Pure boundary solvers: anchor placement/flips, movable title-bar clamps")
  Component(at, "widget/", "Atoms: button, text input (entry/search/trigger), adaptive ring, Focusable wrapper")
  Component(zo, "surface/zorder.rs", "Four z-bands as data: backdrop, pointer rule, key disposition, escape rank; registry routing")
  Component(mu, "surface/menu/", "Unified fixed + context menu engine: model, row-matrix view, cascade controller")
  Component(ho, "surface/ hosts", "modal.rs, input_dialog.rs, movable.rs — frame hosts, not widgets")
  Component(ga, "gallery.rs + bin/gallery.rs", "UIF-VER-004 harness: 91 cells, token-mapping gate, contrast gate")
  Component(sc, "spec_coverage.rs", "UIF-* coverage ledger + the audit primitive")
  Component(th, "test_helpers (cfg(test))", "Headless render/operate harness with focus read-back")

  Rel(at, token, "paints only from tokens")
  Rel(at, st, "resolves state")
  Rel(at, fm, "registers focus members")
  Rel(br, fm, "projects the model onto the router")
  Rel(ho, geo, "placement and clamping")
  Rel(ho, zo, "registers a band; answers Escape")
  Rel(mu, geo, "card placement")
  Rel(mu, at, "row widget")
  Rel(ga, at, "draws the matrix")
  Rel(ga, ho, "draws the matrix")
```

### The contract it exposes

* **Tokens, not colours or numbers.** Every visual property a widget paints
  is read from `token::*`; the set is closed and nothing outside `token/`
  declares a visual literal.
* **Pure resolution before paint.** Style resolvers, geometry solvers and
  state machines are functions over values, so every spec matrix and every
  boundary rule is checkable without a window, a renderer or a GPU.
* **A focus model the host can drive.** `focus::FocusState` decides which
  id holds focus; `bridge::tab_operation` / `focus_operation` push that
  answer into iced. A host owns traversal by registering groups and
  pushing traps.
* **A layered-window model as data.** `ZBand::backdrop`, `pointer_rule`,
  `key_disposition` and `escape_rank` are queries, so a host never
  re-derives the spec's §5 table by hand.
* **Injected persistence.** `movable::GeometryStorage` (with `Noop` and
  `Memory` implementations) is the only persistence seam; the crate never
  touches a storage location itself (RUST-024).

### What it deliberately does not own (decision D-2)

Framework layer only. These stay with `fastmd` and are recorded as
`NotOwned` in the ledger rather than quietly skipped: the top navbar's slot
contents and label conventions, the directory tree and its filtering, the
pane splitters and pane width policy, the tab strip and tab contents, and
the business logic of the app-specific dialogs (Batch, Tools &
Integrations, About, Attributions). Window geometry, UI scale and their
cross-session persistence are likewise host-owned.

### Three decisions a future integrator has to know

1. **Escape rank is not band rank (ledger gap G-13).** §4.1 unwinds menus
   before dialogs while §5 elevates Modal (band 3) above Menu (band 2).
   Both are correct: they answer *"what is the user unwinding"* and *"what
   is painted in front"*. `escape_rank` is `Menu(3) > Modal(2) >
   UtilityOverlay(1) > Workspace(0)`, independent of band rank, so a menu
   opened from inside a dialog sits under that dialog's scrim in z but
   closes on the first `Escape`. **The topmost-painted surface is not the
   first to receive keys** — a host must not assume otherwise.
2. **The focus bridge runs one way: model → router, never the reverse.**
   The router holds a flat positional list and is structurally incapable
   of groups, nested traps or ordinal stability across a rebuild, and any
   widget that registered itself would become a source of truth about
   focus that could move a host against its own trap. Focus read-back
   exists only in the `#[cfg(test)]` harness so production code cannot
   reach it by accident.
3. **Stock iced widgets are not keyboard-reachable in this framework
   (decision D-7, from gap G-10).** Across `iced_widget` 0.14 only
   `text_input` and `text_editor` implement `operation::Focusable`, there
   is no automatic `Tab` traversal and no widget-level focus event. Every
   control that must be keyboard-reachable here is a custom widget built on
   `widget::focusable::Focusable`. Dropping a stock `button` into one of
   these surfaces silently breaks `UIF-PRIN-005`.

### Coverage ledger

`spec_coverage.rs` classifies all 46 numbered `UIF-*` ids: **25
Implemented, 14 Partial, 7 NotOwned**, each with the modules that satisfy
it and, where it is not fully satisfied, what exactly is missing. The
sidecar test reads `doc/planning/ui-framework.md` at test time and
asserts the two sets agree in both directions, so adding an id to the
spec without classifying it fails the build. The shape of the remaining
gap: the contrast half of `UIF-VER-001` is automated but nothing checks
for raw literals outside `token/` at source level; the snapshot gate
covers only `Palette::dark()` and the standard control height; the type
scale carries no font family and no 12px SemiBold / 13px Bold rung; and
there is **no tooltip surface anywhere in the crate** — `UIF-NAV-003` is
Partial because D-2 excludes the navbar's contents, not a generic
hover-delayed floating card, so that one is unimplemented framework work
rather than a scope exclusion.

## Test Surface

- **Integration tests** (`tests/`): the CommonMark spec conformance suite
  (`commonmark_spec_test.rs`) and the iced visual-test harness
  (`iced_visual_harness.rs`).
- **Inline `#[cfg(test)]` sidecar modules:** `agent/agent_impl_tests.rs`,
  `tools/jmap/tests.rs`, `bus/core.rs`, `bus/events/file.rs`, `bus/events/typed.rs`, `bus/config.rs`, `bus/router/bus_router.rs`.
- **UI tests — four tiers** (see `doc/distill/iced.md`):
  1. **Pure model** — the per-module `src/app/ui_iced/*_tests.rs` sidecars
     (39 files): view-model snapshots, message mappers, geometry solvers,
     reducers. No widget tree, no runtime.
  2. **Headless interaction** — `src/app/ui_iced/interaction_tests.rs`,
     driving the real `view` + real `update` through `iced_test::Simulator`
     and asserting the emitted `Message`s.
  3. **Screenshot baselines** — `tests/iced_visual/baselines/` (sha256 of
     the rendered frame), recorded and compared via the harness.
  4. **Recorded session replay** — `tests/iced_visual/ice/*.ice` replayed by
     `iced_test::run`.
  Tiers 3 and 4 are harness-complete but their artifacts await a human
  recording pass. Dev-deps also include `iced_test 0.14`, `proptest`,
  `filetime`, `tempfile`, `tokio` and `wiremock`.
