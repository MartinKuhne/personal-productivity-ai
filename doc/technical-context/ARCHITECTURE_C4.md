# FastMD C4 Architecture Diagram

## Context Diagram (Level 1)

```mermaid
C4Context
  title FastMD System Context

  Person(user, "User", "Views, edits and batch-processes Markdown libraries; drives an AI agent")

  System_Boundary(fb, "FastMD") {
    System(fastmd, "FastMD Desktop", "Rust + egui native Windows app (crate 'fastmd')")
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

The `fastmd` crate produces a single desktop application

## Component Diagram (Level 3) — UI Layer

`ui/` renders the desktop app. `FastMdApp` (`app.rs`, 1346 lines) is the root
`eframe::App` and owns all cross-cutting state. A 5-pane layout is enforced by
`PanelLayout` driving per-pane render functions in `ui/panels/*`.

```mermaid
C4Component
  title FastMD — UI Layer

  Component(app, "FastMdApp", "Root eframe::App; owns session, tasks, VFS, tags, dialogs, panels, tabs, selection, bus")
  Component(panels, "PanelLayout + panels", "5-pane layout")
  Component(render, "render", "build_toc, render_markdown (GFM), render_yaml_table")
  Component(tree, "tree", "Flattened directory tree rendering")
  Component(tabs, "TabManager", "Tabbed documents")
  Component(sel, "SelectionManager", "Multi-select")
  Component(dialog, "DialogManager", "Move/rename/create-dir")
  Component(modals, "modals", "Modal rendering (private)")
  Component(bglogs, "background_logs", "Background Processes tab")
  Component(osshell, "os_shell", "OS integration: open in editor, show in explorer, print")
  Component(tblw_pure, "table_width (pure core)", "Fair Table Width Algorithm: pure f32 column-width solver")
  Component(tblw_adapter, "table_width (egui adapter)", "egui bridging: measure_cached, ftwa_cached, re-exports pure core")

  Rel(app, panels, "delegates update()")
  Rel(app, render, "renders markdown")
  Rel(app, tree, "draws directory tree")
  Rel(app, tabs, "tab lifecycle")
  Rel(app, sel, "selection state")
  Rel(app, dialog, "modal actions")
  Rel(app, bglogs, "shows background logs")
  Rel(app, osshell, "OS integration")
  Rel(render, tblw_adapter, "measure_cached, ftwa_cached")
  Rel(tblw_adapter, tblw_pure, "re-exports ftwa, Breakpoint, ColumnWidths, DeficitStrategy, CellTokens, compute_column_breakpoints")
```

Supporting UI types: `TreeNode{name,path,is_dir,children:BTreeMap}`,
`ToCEntry{title,level,id}`, `PersistedUiState{left_panel_width,collapsed_dirs}`.

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
[`McpClientManager`](../../src/desktop/src/integrations/mcp/manager.rs). `ToolContext<'a>` is the single
parameter passed to every `Tool::execute`, carrying `&AppConfig` and
`&Bus<FileEvent>`.

```mermaid
C4Component
  title FastMD — Tool System

  Component(reg, "ToolManager", "catalog + per-group state + error tracking + parallel-safety; register_builtin, register_mcp_tool, execute, get_schema, safety_of, parallel_safe_tools, set_group_enabled, record_error, clear_error, refresh_state, refresh_mcp_tools")
  Component(tctx, "ToolContext", "{config, file_event_bus}; thin shim over app::vfs::resolve::resolve(vpath, allow_write, libraries) -> Option<(PathBuf,bool)>")
  Component(mcp, "McpToolAdapter", "wraps integrations::mcp::McpClientManager into a Tool impl; LLM-tool-loop glue in agent/tools/mcp/adapter.rs")
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

`background/` hosts long-running workers, coordinated by `background_task::Task`
which owns the `mpsc` channels and the `notify::RecommendedWatcher`. A
`Bus<FileEvent>` (see Event Bus below) feeds producers and consumers.

```mermaid
C4Component
  title FastMD — Background Workers

  Component(task, "Task", "Owns rx/tx (std::sync::mpsc), file_event_bus: Bus<FileEvent>, watcher: Option<notify::RecommendedWatcher>; subscribes to Bus<ConfigArrived> at construction and only spawns the indexing thread after the first ConfigArrived (or the CONFIG_ARRIVAL_TIMEOUT fallback); run_indexing wires all workers")
  Component(indexer, "Indexer", "Worker pool up to 4 threads")
  Component(watcher, "FileWatcher", "notify 6.0; recursive; auto-watch new dirs")
  Component(pdf, "PdfConverterWorker", "PdfConversionJob queue; pdf_converter_command")
  Component(vision, "ImageVisionWorker", "process_image: base64 data URL -> vision use_case model")
  Component(router, "BusRouter", "Routes FileEvents between producers and consumers")
  Component(bgmgr, "BackgroundProcessManager", "VecDeque ring buffer MAX_LOG_ENTRIES=10_000; filter/search/auto_scroll/show_background_logs; log persistence to logs/background-process.log; SharedProcessManager = Arc<Mutex<...>>")
  Component(bgmodels, "models", "BackgroundLogEntry, LogCategory{Indexer,Watcher,PDF Converter,Image Vision,LLM Tools}")

  Rel(task, indexer, "drives")
  Rel(task, watcher, "owns")
  Rel(task, pdf, "spawns")
  Rel(task, vision, "spawns")
  Rel(task, router, "wires")
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
  Component(fev, "FileEvent & FileEventProducer", "FileEventKind{Discovered, Updated, Removed, DirDiscovered, DirRemoved}; FileEventProducer convenience wrapper")
  Component(bev, "BackgroundEvent & Sub-Enums", "Typed UI event wrapper: FsEvent, ProcessEvent, McpAuthEvent (AgentEvent removed — agent events now on Bus<AgentEvent>)")
  Component(cfg_bus, "ConfigArrived & config_bus", "Startup config broadcast; CONFIG_ARRIVAL_TIMEOUT (100ms) fallback")
  Component(router, "BusRouter", "Subscribes to Bus<FileEvent>; routes .pdf to tx_pdf and images to tx_img MPSC queues")
  Component(task, "Task", "Owns rx/tx: mpsc::channel<BackgroundEvent> and file_event_bus: Bus<FileEvent>")
  Component(ui, "FastMdApp", "UI thread loop; drains Task.rx (BackgroundEvent) on every frame pass")

  Rel(fev, core, "uses Bus<FileEvent>")
  Rel(cfg_bus, core, "uses Bus<ConfigArrived>")
  Rel(router, core, "subscribes to Bus<FileEvent>")
  Rel(task, core, "owns Bus<FileEvent>")
  Rel(ui, task, "drains rx: Receiver<BackgroundEvent>")
  Rel(router, task, "forwards PDF/image paths to worker channels")
```

### Overview & Design Principles

1. **Explicit Thread Communication**: Shared mutable state across thread boundaries is strictly avoided. Subsystems communicate exclusively via event channels.
2. **MPMC Broadcast vs MPSC Channels**:
   - **Multi-Producer Multi-Consumer (`Bus<T>`)**: Used when multiple consumers must independently observe the same event (e.g., file changes observed by Indexer, DirectoryTracker, FileEventProcessor, and BusRouter).
   - **Multi-Producer Single-Consumer (`mpsc::channel`)**: Used when events target a single aggregator (e.g., UI frame loop draining `BackgroundEvent` or worker task queues).
3. **Non-Blocking UI Integration**: `BusReader` provides non-blocking `try_recv()` and `recv_timeout()` methods wrapped in a `Mutex`, allowing the single-threaded `egui` UI loop to poll events without stalling rendering.

### Message Types & Payloads

- **`FileEvent`** (`bus/events/file.rs`): Published whenever content libraries change on disk or in memory.
  - Variants (`FileEventKind`): `Discovered`, `Updated`, `Removed`, `DirDiscovered`, `DirRemoved`.
  - Content: `paths: Vec<PathBuf>`.
  - Helpers: `FileEventProducer` simplifies publishing single or batch file/dir operations, including rename semantics (`Removed` + `Discovered`).
- **`BackgroundEvent`** (`bus/events/typed.rs`): Top-level enum carrying domain-specific asynchronous updates to the UI loop:
  - **`FsEvent`**: `FileParsed { path, tags }`, `DirParsed { path }`, `FileModified { path, tags }`, `FileDeleted { path }`, `Finished`, `FinishedWithoutWatcher`.
  - **`ProcessEvent`**: `LogEntry(BackgroundLogEntry)`, `FileLoaded { path, content }`.
  - **`McpAuthEvent`**: `Completed { server_name, error }`.
- **`AgentEvent`** (`agent/events.rs`): Agent→UI `Bus<AgentEvent>` broadcast (tokio, capacity 8192). 11 session-tagged variants: `SessionStarted`, `SessionFinished{history}`, `Status(status: AgentStatus)`, `Thinking`, `ContentDelta`, `ToolCallStarted{id,name,args}`, `ToolResult{id,name,result}`, `ToolSideEffect`, `DebugEntry`, `TokenUsage`, `Failed`. Every variant carries `session_id: Uuid`.
- **`AgentPrompt`** (`agent/events.rs`): UI→agent mpsc input carrying `session_id`, `text`, `active_file/dir`, `selected_files`, `cancel_flag: Arc<AtomicBool>`. The long-lived driver thread blocks on `recv()`.
- **`ConfigArrived`** (`bus/events/config.rs`): Published once at startup carrying `config: AppConfig`. Enables lazy component initialization and prevents race conditions during startup.
- **`TokenUsageInfo`** (`bus/events/messages.rs`): Detailed LLM token consumption metrics (`prompt_tokens`, `completion_tokens`, `total_tokens`, `cached_tokens`, `reasoning_tokens`).

### Event Routing & Dispatch

1. **`BusRouter` (`bus/router/bus_router.rs`)**:
   - Subscribes to `Bus<FileEvent>`.
   - Filters for `Discovered` and `Updated` events.
   - Inspects file extensions:
     - `.pdf` paths -> forwarded to `tx_pdf: Sender<PathBuf>` (`PdfConverterWorker`).
     - Image paths (`.jpg`, `.jpeg`, `.png`, `.gif`, `.webp`, `.bmp`, `.tiff`, `.avif`) -> forwarded to `tx_img: Sender<PathBuf>` (`ImageVisionWorker`).
2. **UI Frame Drain (`ui/app/update.rs`)**:
   - `drain_background_channel()`: Drains `BackgroundEvent` (Fs/Process/McpAuth) from the old mpsc path; dispatches to `FileEventProcessor`, `TagManager`, `BackgroundProcessManager`, tools dialog.
   - `drain_agent_event_bus()`: Drains `BusReader<AgentEvent>` per frame; routes lifecycle events (`SessionStarted`/`SessionFinished`/`Status`/`Failed`/`TokenUsage`/`DebugEntry`) to `AgentSessionManager`; routes content events (`ContentDelta`/`Thinking`/`ToolCallStarted`/`ToolResult`) to `AgentTranscript` in `ui/agent/transcript.rs`; reissues `ToolSideEffect` as `FsEvent::FileModified`.
   - UI widget state lives in `AgentPanelState` (`ui/agent/panel_state.rs`): `show_results`, `show_debug_window`, `debug_search_text`, `debug_auto_scroll`, `command_input`, `scroll_to_id`, `active_session_id`.

### Messaging Actors & Recipients Summary

| Channel / Bus | Transport Primitive | Payload Type | Producers (Actors) | Consumers (Recipients) |
|---|---|---|---|---|
| `Bus<FileEvent>` | `tokio::sync::broadcast` (8192 cap) via `Bus<T>` | `FileEvent` | `FileWatcher`, `Indexer`, UI Dialogs (`FileEventProducer`), Tool Executors | `DirectoryTracker`, `FileEventProcessor`, `Indexer`, `BusRouter` |
| `BackgroundEvent` | `std::sync::mpsc::channel` | `BackgroundEvent` (`Fs`, `Process`, `McpAuth`) | Agent Thread, Indexer Pool, `PdfConverterWorker`, `ImageVisionWorker`, MCP Auth Flow | `FastMdApp` (UI thread loop) |
| `Bus<ConfigArrived>` | `tokio::sync::broadcast` (8192 cap) via `Bus<T>` | `ConfigArrived` | `main()` | `Task`, `AgentSessionManager`, `FastMdApp` |
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

  Component(cfg, "config", "AppConfig, LlmConfig{model,api_url,api_key,cost,use_case}, JmapClient, CalDavClient, CardDavClient, content_libraries: Vec<ContentLibrary>; load_config, get_config_path; Debug redacts secrets; config_bus / ConfigArrived (tokio broadcast) used to fan out the loaded config to Task, AgentSessionManager, and FastMdApp on startup. VFS types re-exported from app::vfs for backwards compat.")
  Component(ev, "file_events", "Bus<T> (tokio::sync::broadcast, BUS_CAPACITY=8192); FileEvent; FileEventKind{Discovered,Updated,Removed,DirDiscovered,DirRemoved}; FileEventProducer; BusReader. Multi-producer/multi-consumer")
  Component(fp, "file_processor", "FileEventProcessor{reader, all_files, all_files_set, all_dirs, all_dirs_set, indexing_finished, indexing_finished_handled}")
  Component(dt, "directory_tracker", "Single source of truth for known dirs; consumes DirDiscovered/DirRemoved + file Discovered")
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
  Rel(ev, fp, "feeds")
  Rel(ev, dt, "feeds")
  Rel(fp, tm, "file list -> tags")
  Rel(msg, ev, "background messages")
```

## Test Surface

- **Integration tests** (`src/desktop/tests/`): background manager, discovery,
  document, editor, log persistence, PDF converter, pulldown config, table
  layout.
- **Inline `#[cfg(test)]` modules:** `agent/agent_impl_tests.rs`,
  `tools/jmap/tests.rs`, `bus/core.rs`, `bus/events/file.rs`, `bus/events/typed.rs`, `bus/config.rs`, `bus/router/bus_router.rs`.
- **UI tests:** `egui_kittest` 0.35 (eframe + snapshot) harnesses following the
  egui `__run_test_ctx` / `State::test_ctx` pattern. Dev-deps also include
  `accesskit 0.24`, `proptest`, `filetime`, `tempfile`, `tokio`.

