# FastMD C4 Architecture Diagram

This document describes the **actual** structure of the `fastmd` Rust crate
(`src/desktop/`), mapped to the C4 model. It is kept in sync with
`src/desktop/SPEC.md` (EARS-formatted requirements) and the module tree under
`src/desktop/src/`. When the code changes, update this document alongside it.

Legend: REQ-xxx references are requirement IDs from `SPEC.md`.

---

## Context Diagram (Level 1)

```mermaid
C4Context
  title FastMD System Context

  Person(user, "User", "Views, edits and batch-processes Markdown libraries; drives an AI agent")

  System_Boundary(fb, "FastMD") {
    System(fastmd, "FastMD Desktop", "Rust + egui native Windows app (crate 'fastmd')")
  }

  System_Ext(llm, "OpenAI-compatible LLM", "OpenRouter / any compatible endpoint (REQ-601)")
  System_Ext(jmap, "JMAP Server", "Email, calendar, contacts (Rustave Stork AG)")
  System_Ext(caldav, "CalDAV/CardDAV", "Calendar + address-book servers")
  System_Ext(searxng, "SearXNG", "Web search backend (self-hosted)")
  System_Ext(nominatim, "Nominatim / Open-Meteo", "Geocoding + weather (REQ extras)")
  System_Ext(playwright, "Chromium via Playwright", "Browser automation sub-system (browser.rs)")
  System_Ext(pdf, "PDF Converter CLI", "External command (pdf_converter_command)")
  System_Ext(fs, "Local Filesystem", "Content libraries on disk")

  Rel(user, fastmd, "Uses")
  Rel(fastmd, llm, "Chat completions / vision (REQ-601, 470..478)")
  Rel(fastmd, jmap, "JMAP (email/calendar/contacts)")
  Rel(fastmd, cald, "CalDAV/CardDAV")
  Rel(fastmd, searxng, "web_search (REQ-665)")
  Rel(fastmd, nominatim, "weather tool (geocode + forecast)")
  Rel(fastmd, playwright, "browser_* tools (browser.rs)")
  Rel(fastmd, pdf, "PDF rendering worker (REQ-450..458)")
  Rel(fastmd, fs, "Reads/writes content libraries (REQ-700..708)")
```

---

## Container Diagram (Level 2)

The `fastmd` crate produces two binaries and one library. The library is the
shared core; the desktop binary wires it into an `eframe` runtime; the `deploy`
binary is currently an empty placeholder (`src/bin/deploy.rs`).

```mermaid
C4Container fastmd
  title FastMD Container Diagram

  Container_Boundary(app, "fastmd crate") {
    Component(bin_fastmd, "fastmd bin", "src/main.rs", "mimalloc + tracing + rustls init; eframe::run_native 'FastMD Viewer' 1000x700 (REQ-501)")
    Component(bin_deploy, "deploy bin", "src/bin/deploy.rs", "Placeholder, empty")
    Component(lib, "fastmd lib", "src/lib.rs", "Re-exports public API: run_agent, Task, AppConfig, FastMdApp, tools::execute_tool, ...")
  }

  Rel(bin_fastmd, lib, "links")
  Rel(bin_deploy, lib, "links (future)")
```

Top-level public modules exposed by `lib.rs`:
`agent`, `background`, `background_task`, `batch`, `browser`, `config`,
`directory_tracker`, `document`, `editor`, `error`, `file_events`,
`file_processor`, `messages`, `print`, `tag_manager`, `tools`, `ui`, `utils`.

---

## Component Diagram (Level 3) — UI Layer

`ui/` renders the desktop app. `FastMdApp` (`app.rs`, 1346 lines) is the root
`eframe::App` and owns all cross-cutting state. A 5-pane layout is enforced by
`PanelLayout` driving per-pane render functions in `ui/panels/*` (REQ-101).

```mermaid
C4Component fastmd
  title FastMD — UI Layer

  Component(app, "FastMdApp", "ui/app.rs", "eframe::App root; owns AgentSessionManager, BackgroundProcessManager, Task, DirectoryTracker, FileEventProcessor, TagManager, DialogManager, PanelLayout, SelectionManager, TabManager, bus")
  Component(panels, "PanelLayout + panels", "ui/panel_layout.rs, ui/panels/{top,bottom,left,right,center}.rs", "5-pane layout REQ-101")
  Component(render, "render", "ui/render.rs", "build_toc, render_markdown (GFM pulldown-cmark REQ-201/216), render_yaml_table (REQ-212)")
  Component(tree, "tree", "ui/tree.rs", "flatten_tree, draw_tree_node, FlatRow, TREE_ROW_HEIGHT, TreeNodeContext, render_flat_row")
  Component(tabs, "TabManager", "ui/tab_manager.rs", "Tabbed docs REQ-190..198, 619")
  Component(sel, "SelectionManager", "ui/selection_manager.rs", "Multi-select REQ-180..183")
  Component(dialog, "DialogManager", "ui/dialog_manager.rs", "move/rename/create-dir REQ-155..157")
  Component(modals, "modals", "ui/modals.rs", "Modal rendering (private module)")
  Component(bglogs, "background_logs", "ui/background_logs.rs", "Background Processes tab REQ-460..465")
  Component(osshell, "os_shell", "ui/os_shell.rs", "open_in_system_editor, show_in_file_explorer, ShellExecute 'print' REQ-159")
  Component(tblw, "table_width", "ui/table_width/mod.rs", "Table column width algorithm")

  Rel(app, panels, "delegates update()")
  Rel(app, render, "renders markdown")
  Rel(app, tree, "draws directory tree")
  Rel(app, tabs, "tab lifecycle")
  Rel(app, sel, "selection state")
  Rel(app, dialog, "modal actions")
  Rel(app, bglogs, "shows background logs")
  Rel(app, osshell, "OS integration")
```

Supporting UI types: `TreeNode{name,path,is_dir,children:BTreeMap}`,
`ToCEntry{title,level,id}`, `PersistedUiState{left_panel_width,collapsed_dirs}`.

---

## Component Diagram (Level 3) — Agent Core

`agent/` implements the LLM tool-loop. `run_agent` spawns a dedicated thread,
builds messages from `SystemPromptBuilder`, queries an OpenAI-compatible
endpoint, and loops turns (`Continue` / `Done` / `Failed`) honouring a cancel
flag. REQ-601..640, 613..619.

```mermaid
C4Component fastmd
  title FastMD — Agent Core

  Component(mgr, "AgentSessionManager", "agent/manager.rs (278)", "AgentState{running,status,thinking,response,scroll_to_id,history,token_usage,total_usage}; start_session; handles BackgroundMessage::Agent*; cancel via AtomicBool")
  Component(impl, "run_agent / run_agent_inner", "agent/agent_impl.rs (212)", "Resolves LLM client, builds messages, get_tools_schema, ToolExecutor::new, turn loop")
  Component(ctx, "AgentContext", "agent/context.rs", "config, prompt, history, active_file, active_dir, selected_files, cancel_flag, channels")
  Component(llm, "LLMClient", "agent/llm_client.rs", "parse_usage_block; OpenAI-compatible HTTP")
  Component(pb, "SystemPromptBuilder", "agent/prompt_builder.rs", "with_active_file/dir/selected_files; USER.md injection per library (REQ-614)")
  Component(rf, "ResponseFormatter", "agent/response_formatter.rs", "split_thinking_and_content (🤔...🤔 REQ-616), format_tool_call_message, format_tool_result_message")
  Component(te, "ToolExecutor", "agent/tool_executor.rs", "safe tools parallel / unsafe tools sequential (REQ-609)")

  Rel(mgr, impl, "start_session -> run_agent")
  Rel(impl, ctx, "passes context")
  Rel(impl, llm, "chat completions")
  Rel(impl, pb, "system prompt")
  Rel(impl, te, "dispatch tools")
  Rel(impl, rf, "format responses")
  Rel(mgr, rf, "surface to UI")
```

---

## Component Diagram (Level 3) — Tool System

`tools/` defines the `Tool` trait and a `ToolRegistry` holding
`HashMap<&'static str, Box<dyn Tool>>`. `ToolContext<'a>` is the single
parameter passed to every `Tool::execute`, carrying `&AppConfig` and
`&Bus<FileEvent>` (REQ-609).

```mermaid
C4Component fastmd
  title FastMD — Tool System

  Component(reg, "ToolRegistry", "tools/registry.rs (1810)", "register_all, execute, get_tools_schema; paginate_in_range helper")
  Component(tctx, "ToolContext", "tools/context.rs (268)", "{config, file_event_bus}; resolve_virtual_path(vpath, allow_write) -> Option<(PathBuf,bool)>")
  Component(fs, "filesystem tools", "tools/filesystem.rs", "grep, read_file, read_file_lines, create_file, insert_lines, delete_lines, replace_text, list_files")
  Component(yaml, "yaml_header", "tools/yaml_header.rs", "read_yaml_header, write_yaml_header")
  Component(web, "web", "tools/web.rs", "web_fetch (pagination/headers/5-min cache REQ-660..665), web_search (SearXNG), web_delegate sub-agent")
  Component(csv, "csv_db", "tools/csv_db/", "{mod, operations, query (evalexpr REQ-651/652), schema}; add_rows, delete_rows, create_csv, list_csv, query; gated by prompt keywords (REQ-650)")
  Component(jmap, "jmap", "tools/jmap/", "{client, calendar, contacts, email, tests}; search/get/add/update/delete calendar/email/contact")
  Component(caldav, "caldav", "tools/caldav.rs", "CalDAV calendar tools")
  Component(carddav, "carddav", "tools/carddav.rs", "CardDAV contact tools")
  Component(weather, "weather", "tools/weather.rs", "Nominatim geocode + Open-Meteo forecast (not in SPEC tool table)")
  Component(dtos, "dtos", "tools/dtos.rs", "Shared tool data-transfer objects")

  Rel(reg, tctx, "passes to each Tool::execute")
  Rel(reg, fs, "registers")
  Rel(reg, yaml, "registers")
  Rel(reg, web, "registers")
  Rel(reg, csv, "registers (conditional)")
  Rel(reg, jmap, "registers")
  Rel(reg, caldav, "registers")
  Rel(reg, carddav, "registers")
  Rel(reg, weather, "registers (extra)")
  Rel(tctx, fs, "resolve_virtual_path for read/write")
```

Tool inventory (matches `Tools.md` + conditional tools):
- **Core Workspace (12):** `grep`, `read_tags`, `list_files_by_tag`,
  `list_files`, `read_file`, `read_file_lines`, `create_file`,
  `insert_lines`, `delete_lines`, `replace_text`, `read_yaml_header`,
  `write_yaml_header`.
- **Web Integration (3):** `web_fetch`, `web_search`, `web_delegate`.
- **JMAP Productivity (11):** `search_calendar`, `get_calendar`,
  `get_calendar_item`, `add_calendar_item`, `update_calendar_item`,
  `delete_calendar_item`, `search_email`, `get_email_by_id`, `send_email`,
  `search_contact`, `get_contact`, `add_contact`.
- **Conditional / extra (6):** `add_rows`, `delete_rows`, `create_csv`,
  `list_csv`, `query` (CSV DB, prompt-keyword gated, REQ-650..653),
  `weather` (not in SPEC tool table).

> `tools/Spotify.md` is a **proposal** for 25+ `spotify_*` tools via OAuth2
> PKCE; not implemented.

---

## Component Diagram (Level 3) — Background Workers

`background/` hosts long-running workers, coordinated by `background_task::Task`
which owns the `mpsc` channels and the `notify::RecommendedWatcher`. A
`Bus<FileEvent>` (see Event Bus below) feeds producers and consumers.

```mermaid
C4Component fastmd
  title FastMD — Background Workers

  Component(task, "Task", "background_task.rs (518)", "Owns rx/tx (std::sync::mpsc), file_event_bus: Bus<FileEvent>, watcher: Option<notify::RecommendedWatcher>; spawns indexing thread; run_indexing wires all workers")
  Component(indexer, "Indexer", "background/indexer.rs", "Worker pool up to 4 threads (REQ-301/302)")
  Component(watcher, "FileWatcher", "background/watcher.rs", "notify 6.0; recursive; auto-watch new dirs (REQ-401/407)")
  Component(pdf, "PdfConverterWorker", "background/pdf_converter.rs", "PdfConversionJob queue; pdf_converter_command (REQ-450..458)")
  Component(vision, "ImageVisionWorker", "background/vision_processor.rs", "process_image: base64 data URL -> vision use_case model (REQ-470..478)")
  Component(router, "BusRouter", "background/bus_router.rs", "Routes FileEvents between producers and consumers")
  Component(bgmgr, "BackgroundProcessManager", "background/manager.rs (239)", "VecDeque ring buffer MAX_LOG_ENTRIES=10_000; filter/search/auto_scroll/show_background_logs; log persistence to logs/background-process.log (REQ-464); SharedProcessManager = Arc<Mutex<...>>")
  Component(bgmodels, "models", "background/models.rs", "BackgroundLogEntry, LogCategory{Indexer,Watcher,PDF Converter,Image Vision,LLM Tools}")

  Rel(task, indexer, "drives")
  Rel(task, watcher, "owns")
  Rel(task, pdf, "spawns")
  Rel(task, vision, "spawns")
  Rel(task, router, "wires")
  Rel(bgmgr, bgmodels, "stores entries")
```

---

## Component Diagram (Level 3) — Supporting Modules

Cross-cutting modules that the UI, Agent, Tools and Background all depend on.

```mermaid
C4Component fastmd
  title FastMD — Supporting Modules

  Component(cfg, "config", "config.rs (798) + config/virtual_path.rs (302)", "AppConfig, LlmConfig{model,api_url,api_key,cost,use_case}, JmapClient, CalDavClient, CardDavClient, content_libraries; load_config, get_config_path; Debug redacts secrets. VirtualPath + VirtualPathError{EmptyPath,TraversalDetected,InvalidFormat,LibraryNotFound,LibraryNotWritable}; rejects '..' traversal (REQ-700..708)")
  Component(ev, "file_events", "file_events.rs (554)", "Bus<T> (tokio::sync::broadcast, BUS_CAPACITY=8192); FileEvent; FileEventKind{Discovered,Updated,Removed,DirDiscovered,DirRemoved}; FileEventProducer; BusReader. Multi-producer/multi-consumer")
  Component(fp, "file_processor", "file_processor.rs (186)", "FileEventProcessor{reader, all_files, all_files_set, all_dirs, all_dirs_set, indexing_finished, indexing_finished_handled}")
  Component(dt, "directory_tracker", "directory_tracker.rs (267)", "Single source of truth for known dirs; consumes DirDiscovered/DirRemoved + file Discovered")
  Component(tm, "tag_manager", "tag_manager.rs (220)", "TagManager{file_tags:BTreeMap<PathBuf,Vec<String>>, all_tags:BTreeSet<String>, prompt_paths:BTreeSet<PathBuf>, selected_tag}")
  Component(msg, "messages", "messages.rs (69)", "BackgroundMessage enum; TokenUsageInfo{prompt_tokens,completion_tokens,total_tokens,cached_tokens,reasoning_tokens}")
  Component(doc, "document", "document.rs (179)", "DocumentContent{front_matter: Option<String>, body: String}; parse via utils::markdown::parse_front_matter")
  Component(ed, "editor", "editor.rs (512)", "Inline editor; EditorColors inverted white-on-black (REQ-261); validation via pulldown-cmark (REQ-258); undo/redo >=100 (REQ-257); clipboard (REQ-255); cursor nav (REQ-256); save combines body + original front-matter (REQ-259)")
  Component(print, "print", "print.rs (206)", "PrintJob{markdown_path, markdown_content, title}; markdown->HTML via pulldown-cmark; execute_print_blocking")
  Component(browser, "browser", "browser.rs (116)", "Playwright async: browser_navigate, browser_get_page_state, click, type, screenshot")
  Component(batch, "batch", "batch/", "{coordinator, dialog, discoverer, executor, file_matcher, prompts, types}; batch prompt processing REQ-800..813; concurrency 1-8; File vs Directory modes")
  Component(utils, "utils", "utils/", "markdown (parse_front_matter), path, tags (extract_tags_from_file)")
  Component(err, "error", "error.rs", "AgentError")

  Rel(cfg, ev, "VirtualPath resolves against content_libraries")
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
  `tools/jmap/tests.rs`.
- **UI tests:** `egui_kittest` 0.35 (eframe + snapshot) harnesses following the
  egui `__run_test_ctx` / `State::test_ctx` pattern mandated by `AGENTS.md`
  §10. Dev-deps also include `accesskit 0.24`, `proptest`, `filetime`,
  `tempfile`, `tokio`.

---

## Key Runtime Flows

1. **Startup** (`main.rs`): mimalloc global allocator → panic hook →
   `tracing_subscriber::fmt::init()` → install rustls ring provider →
   `load_config()` → `SystemPromptBuilder::new(&config).build(&config)` →
   `eframe::run_native` with 1000x700 viewport titled "⚡ FastMD Viewer".
2. **Indexing** (`background_task::Task::run_indexing`): spawns indexer thread
   that wires `Indexer`, `FileWatcher`, `PdfConverterWorker`,
   `ImageVisionWorker`, `BusRouter`; emits `FileEvent`s on the `Bus`.
3. **UI update** (`FastMdApp::update`): `FileEventProcessor` drains the bus,
   `DirectoryTracker` reconciles known dirs, `TagManager` updates tags,
   `PanelLayout` draws 5 panes.
4. **Agent session** (`AgentSessionManager::start_session`): spawns
   `run_agent(AgentContext)`; turn loop runs safe tools in parallel and unsafe
   tools sequentially (`ToolExecutor`, REQ-609); responses flow back via
   `BackgroundMessage::Agent*` and are formatted by `ResponseFormatter`
   (🤔...🤔 thinking delimiters, REQ-616).
5. **Batch processing** (`batch/`): discoverer selects files/dirs, executor
   runs prompts with concurrency 1-8 (REQ-800..813).
