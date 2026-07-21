# Architecture Refactor - Actionable Todos

## Priority 1: Extract FastMdApp Components (God Object)

### Task 1.1: Extract AgentSessionManager
- **File**: `src/ui/app.rs` → `src/agent/manager.rs` (new)
- **Fields to extract**: `agent_running`, `agent_status`, `agent_thinking`, `agent_response`, `agent_scroll_to_id`, `agent_cancel_flag`, `agent_history`, `agent_token_usage`, `agent_total_usage`, `submit_prompt`
- **Methods to extract**: `start_agent_session`, handling of `BackgroundMessage::Agent*` messages
- **Create**: `AgentSessionManager::new()`, `handle_background_message(&mut self, msg: BackgroundMessage)`, `start_session(prompt: String, context: AgentContext)`, `cancel()`, `clear_history()`, `get_state() -> AgentState`
- **Acceptance**: `FastMdApp` no longer has any `agent_*` fields; all agent state lives in `AgentSessionManager`. Unit tests pass for manager.

### Task 1.2: Extract FileEventProcessor ✅ **COMPLETE**
- **File**: `src/ui/app.rs::process_file_events` → `src/file_processor.rs` (new)
- **Fields extracted**: `all_files`, `all_dirs`
- **Methods**: `process_events(&mut self) -> bool`, `is_workspace_file(path: &Path) -> bool`
- **Acceptance**: `FastMdApp::update_ui` calls `file_processor.process_events()`. Processor encapsulates all event handling.
- **Files modified**: `src/file_processor.rs` (new), `src/ui/app.rs`, `src/ui/panels/left.rs`, `src/ui/panels/top.rs`, `src/ui/modals.rs`, `src/file_events.rs`

### Task 1.3: Extract DialogManager
- **File**: `src/ui/app.rs` dialog fields + modal functions → `src/ui/dialog_manager.rs` (new)
- **Fields**: `move_dialog_open`, `file_to_move`, `selected_move_folder`, `create_dir_dialog_open`, `create_dir_parent`, `create_dir_name`, `rename_dialog_open`, `file_to_rename`, `rename_new_name`, `batch_dialog_open`, `batch_dialog_config`, `batch_handle`, `batch_cancel_flag`
- **Methods**: `show_move_modal(&mut self, ctx)`, `show_create_dir_modal(...)`, `show_rename_modal(...)`, `show_batch_modal(...)`, plus getters/setters for dialog state
- **Acceptance**: Modal show functions moved to `DialogManager`. `FastMdApp::update_ui` calls `dialogs.update(ctx)`.

### Task 1.4: Extract BackgroundLogManager (complete)
- **Already have**: `BackgroundProcessManager` in `src/background/manager.rs`
- **Work**: Move `background_manager` field and `show_background_logs_window` call entirely into manager or UI. `FastMdApp` should not own this.
- **Acceptance**: `FastMdApp` no longer has `background_manager` or `show_background_logs` fields.

### Task 1.5: Extract TabManager
- **File**: `src/ui/app.rs` tab-related fields → `src/ui/tab_manager.rs` (new)
- **Fields**: `loaded_path`, `current_yaml`, `current_markdown`, `tabs`, `toc`, `scroll_to_header_id`
- **Methods**: `open_file(&mut self, path: PathBuf)`, `close_tab(path: &Path)`, `close_others(active: &Path)`, `close_all()`, `reload_if_needed(&mut self) -> bool`, `update_content(&mut self, path: &Path)`, `get_active_content() -> Option<(&Yaml, &str)>`
- **Acceptance**: Tab rendering calls `tab_manager.get_active_content()`. File loading logic in `TabManager::update_content`.

### Task 1.6: Extract SelectionManager
- **File**: `src/ui/app.rs` selection fields → `src/ui/selection_manager.rs` (new)
- **Fields**: `selected_file`, `selected_files`, `selected_dir`, `expanded_dirs`
- **Methods**: `select_file(path: Option<PathBuf>)`, `toggle_file(path: PathBuf)`, `select_dir(path: Option<PathBuf>)`, `toggle_expanded(path: PathBuf)`, `clear()`, getters
- **Acceptance**: All selection state modifications go through manager. Panels read via getters.

### Task 1.7: Extract PanelLayout
- **File**: `src/ui/app.rs` layout fields → `src/ui/panel_layout.rs` (new)
- **Fields**: `left_panel_width`, `left_panel_dirty`, `left_panel_reset_count`
- **Methods**: `calculate_left_width(&mut self, files: &[PathBuf])`, `reset_count()`, `increment_reset_count()`, `needs_recalc() -> bool`
- **Acceptance**: Width calculation moved to layout manager. `show_left_panel` uses `layout.get_width()`.

### Task 1.8: Simplify FastMdApp
- **After all extractions**: `FastMdApp` should have ~20 fields: config, content_libraries, channels (tx, rx), and the 6-7 manager structs.
- **Refactor `update_ui`**: Should be ~50 lines orchestrating managers: `file_processor.process_events()`, `agent.handle_messages()`, `dialogs.update()`, `tabs.reload_if_needed()`, then show panels.
- **Acceptance**: `FastMdApp` is now a composition of focused managers. Each manager < 200 lines. All existing tests still pass.

---

## Priority 2: Simplify Agent Loop

### Task 2.1: Create AgentContext Struct
- **File**: `src/agent/mod.rs` (or new `src/agent/context.rs`)
- **Create**: `pub struct AgentContext { config: AppConfig, tx_gui: Sender<BackgroundMessage>, file_event_bus: Bus<FileEvent>, active_file: Option<PathBuf>, active_dir: Option<PathBuf>, selected_files: HashSet<PathBuf>, prompt: String, cancel_flag: Arc<AtomicBool>, history: Option<Vec<Value>>, current_response: String }`
- **Acceptance**: `run_agent(ctx: AgentContext)` uses context instead of 11 separate parameters.

### Task 2.2: Extract LLMClient
- **File**: `src/agent/llm_client.rs` (new)
- **Methods**: `new(config: &AppConfig) -> Self`, `chat_completion(&self, messages: &[Value], tools: &[Value]) -> Result<LLMResponse, AgentError>`, `stream_chat_completion(...)` if needed
- **Responsibilities**: Build ureq agent, set headers, send request, parse response, extract usage, handle HTTP errors (status, network, JSON), map Anthropic fields
- **Acceptance**: `run_agent` calls `llm_client.chat_completion(messages, tools).await?`. LLMClient has no knowledge of tools or UI.

### Task 2.3: Extract ToolExecutor
- **File**: `src/agent/tool_executor.rs` (new)
- **Methods**: `new(config: AppConfig, file_event_bus: Bus<FileEvent>) -> Self`, `execute_parallel(&self, calls: &[ToolCall]) -> HashMap<call_id, ToolResult>`, `execute_sequential(&self, calls: &[ToolCall]) -> HashMap<...>`, `execute_single(&self, call: ToolCall) -> ToolResult`
- **Responsibilities**: Call `execute_tool` registry, handle safe vs unsafe separation, spawn Tokio tasks for parallel, collect results
- **Acceptance**: `run_agent` becomes: `let results = executor.execute_parallel(safe_calls).await?; for call in unsafe_calls { executor.execute_single(call).await?; }`

### Task 2.4: Extract SystemPromptBuilder
- **File**: `src/agent/prompt_builder.rs` (new)
- **Methods**: `new(config: &AppConfig) -> Self`, `with_active_file(mut self, path: Option<PathBuf>) -> Self`, `with_active_dir(...)`, `with_selected_files(...)`, `build(self) -> String`
- **Acceptance**: `get_base_system_prompt` becomes one method in builder. Context injection uses builder pattern: `PromptBuilder::new(&config).with_active_file(active_file).build()`.

### Task 2.5: Extract ResponseFormatter
- **File**: `src/agent/response_formatter.rs` (new)
- **Methods**: `split_thinking_and_content(text: &str) -> (String, String)`, `format_tool_call_message(call: &ToolCall) -> String`, `format_tool_result_message(call: &ToolCall, result: &ToolResult) -> String`
- **Acceptance**: All formatting logic (lines 600-850 in agent.rs) moved here. `run_agent` uses formatter to build UI strings.

### Task 2.6: Refactor run_agent
- **Goal**: Reduce from 863 lines to ~150 lines of orchestration
- **Pattern**:
```rust
pub fn run_agent(ctx: AgentContext) {
    let builder = SystemPromptBuilder::new(&ctx.config)
        .with_active_file(ctx.active_file)
        .with_active_dir(ctx.active_dir)
        .with_selected_files(&ctx.selected_files);
    let system_prompt = builder.build();
    
    let llm = LLMClient::new(&ctx.config);
    let executor = ToolExecutor::new(ctx.config, ctx.file_event_bus);
    let formatter = ResponseFormatter;
    
    // message history setup...
    
    loop {
        let resp = llm.chat_completion(messages, tools).await?;
        // handle response, tool calls, etc.
    }
}
```
- **Acceptance**: All helper functions inlined in `run_agent` are now in extracted modules. No function in `agent.rs` exceeds 50 lines.

---

## Priority 3: Clean Up Tool Registry & Virtual FS

### Task 3.1: Remove root_path parameter
- **File**: `src/tools/registry.rs::execute_tool`
- **Change**: Remove the `root_path: &Path` parameter from signature and all call sites (agent.rs, background_task.rs, batch/coordinator.rs).
- **Acceptance**: All calls compile with 4-parameter signature: `execute_tool(config, name, args, bus)`.

### Task 3.2: Create ToolContext struct
- **File**: `src/tools/context.rs` (new)
- **Define**: `pub struct ToolContext<'a> { pub config: &'a AppConfig, pub file_event_bus: &'a Bus<FileEvent> }`
- **Methods**: `fn resolve_virtual_path(&self, vpath: &str, allow_write: bool) -> Result<PathBuf, String>` (handles library lookup, readonly check, traversal protection), `fn publish_file_event(&self, kind: FileEventKind, path: &Path)`
- **Acceptance**: `execute_tool` receives `ToolContext` instead of separate params. Tools use `ctx.resolve_virtual_path()`.

### Task 3.3: Push path resolution into ToolContext
- **Refactor** all tool implementations in `registry.rs` to use `ctx.resolve_virtual_path()` instead of inline `resolve_and_check_path` closure.
- **Example**:
```rust
// Before:
let (path, readonly) = resolve_path(&input.path)?.ok_or_else(|| ...)?;
if readonly { return Err(...) }
tool_create_file(&path.to_string_lossy(), ...)

// After:
let physical_path = ctx.resolve_virtual_path(&input.path, true)?;
tool_create_file(&physical_path.to_string_lossy(), ...)
```
- **Acceptance**: Tools no longer call `resolve_path` directly; ToolContext handles all virtual FS concerns.

### Task 3.4: Extract Tool trait and Registry
- **File**: `src/tools/mod.rs` refactor
- **Define**: `pub trait Tool: Send + Sync { fn name(&self) -> &'static str; fn description(&self) -> &'static str; fn input_type(&self) -> TypeId; fn is_enabled(&self, config: &AppConfig, prompt: &str) -> bool; fn execute(&self, ctx: ToolContext, args: &str) -> Result<String, String>; }`
- **Implement**: Each tool as a struct (e.g., `GrepTool`, `ReadFileTool`) implementing `Tool`.
- **Create**: `ToolRegistry` struct with `register(tool: Box<dyn Tool>)`, `execute(name, ctx, args)`, `get_schema(config, prompt) -> Vec<Value>`.
- **Acceptance**: `define_tools!` macro replaced with registry initialization: `registry.register(GrepTool); registry.register(ReadFileTool); ...`. CSV tools are just registered tools.

### Task 3.5: Integrate CSV tools into main registry
- **Remove**: `crate::tools::csv_db::get_csv_tools(config, prompt)` special case.
- **Register**: `CsvListTool`, `CsvQueryTool`, `CsvAddRowsTool`, `CsvDeleteRowsTool`, `CsvCreateTool` as regular `Tool` implementations.
- **Acceptance**: All tools come from same registry. No special case in `get_tools_schema`.

---

## Priority 4: Decompose Background Processing

### Task 4.1: Extract Indexer
- **File**: `src/background_task.rs` → split
- **Create**: `src/background/indexer.rs` with `Indexer` struct
- **Fields**: `config: AppConfig`, `tx_gui: Sender<BackgroundMessage>`, `bus: Bus<FileEvent>`, `work_queue: (tx, rx)`
- **Methods**: `new(...) -> Self`, `spawn_workers(num: usize)`, `scan_libraries(&self)`, `run()`
- **Acceptance**: Initial scanning and worker pool encapsulated. `Task::new` calls `Indexer::new(config, tx, bus).spawn()`.

### Task 4.2: Extract PdfConverterWorker
- **File**: `src/background/pdf_converter.rs` already exists
- **Work**: Move thread-spawning logic from `background_task::run_indexing` into `PdfConverterWorker::spawn(rx_pdf, tx_gui, bus, cmd_template)`.
- **Define**: `struct PdfConverterWorker { rx: Receiver<PathBuf>, tx: Sender<BackgroundMessage>, bus: Bus<FileEvent>, cmd: Option<Vec<String>> }` with `run(self)` method.
- **Acceptance**: PDF worker is its own module. `background_task` only wires channels.

### Task 4.3: Extract ImageVisionWorker
- **File**: `src/background/vision_processor.rs` already exists
- **Work**: Similar to PDF worker. Create `ImageVisionWorker::spawn(rx_img, tx_gui, config, bus)`.
- **Acceptance**: Image worker standalone.

### Task 4.4: Extract FileWatcher
- **File**: `src/background/watcher.rs` (new)
- **Struct**: `FileWatcher { watcher: notify::RecommendedWatcher, config: AppConfig, tx: Sender<BackgroundMessage>, bus: Bus<FileEvent> }`
- **Methods**: `new(...) -> Result<Self, ...>`, `start(self) -> Result<..., ...>` (spawns thread with closure), `handle_event(event: notify::Event)` (logic from `run_indexing` lines 234-348)
- **Acceptance**: `Task::new` does: `FileWatcher::new(config, tx, bus)?.start()`.

### Task 4.5: Extract FileEventBusRouter
- **File**: `src/background/bus_router.rs` (new)
- **Purpose**: Subscribe to bus and forward PDF/image events to respective workers.
- **Struct**: `BusRouter { bus: Bus<FileEvent>, tx_pdf: Sender<PathBuf>, tx_img: Sender<PathBuf> }`
- **Methods**: `spawn(self)` - spawns two subscriber threads (one for PDF, one for images)
- **Acceptance**: `Task::new` calls `BusRouter::new(bus, tx_pdf, tx_img).spawn()`.

### Task 4.6: Simplify Task::new
- **After extractions**: `Task::new` becomes ~30 lines wiring components together:
```rust
let (tx, rx) = channel();
let bus = Bus::new();

Indexer::new(config.clone(), tx, bus.clone()).spawn_workers(4).scan_libraries();
PdfConverterWorker::new(config.pdf_converter_command, tx.clone(), bus.clone()).spawn();
ImageVisionWorker::new(config, tx.clone(), bus.clone()).spawn();
FileWatcher::new(config, tx.clone(), bus.clone())?.start();
BusRouter::new(bus.clone(), tx_pdf, tx_img).spawn();

Self { rx, tx, file_event_bus: bus, _watcher: Some(watcher) }
```
- **Acceptance**: `background_task.rs` is <200 lines and only coordinates.

---

## Priority 5: Improve Configuration & Library Abstraction

### Task 5.1: Create VirtualPath struct
- **File**: `src/config/virtual_path.rs` (new)
- **Define**: `pub struct VirtualPath { library: String, sub_path: PathBuf }`
- **Methods**: `parse(vpath: &str) -> Result<Self, Error>`, `resolve(&self, libraries: &[ContentLibrary]) -> Result<PathBuf, Error>`, `is_writable(&self, libraries: &[ContentLibrary]) -> Result<bool, Error>`, `to_string(&self) -> String` (for round-trip)
- **Acceptance**: Virtual path parsing/handling centralized. Tests cover parsing "Lib/sub/file.md", "Lib/../escaping" (rejected).

### Task 5.2: Add methods to ContentLibrary
- **File**: `src/config.rs`
- **Add**: 
  - `impl ContentLibrary { pub fn contains_path(&self, path: &Path) -> bool { ... }`
  - `pub fn resolve(&self, sub: &Path) -> PathBuf { Path::new(&self.root_folder).join(sub) }`
  - `pub fn is_writable(&self) -> bool { !self.readonly }`
- **Acceptance**: No external code manually does `lib.root_folder.join(...)`; uses `lib.resolve()`.

### Task 5.3: Migrate path resolution to ToolContext
- **In `ToolContext::resolve_virtual_path`**: Use `VirtualPath::parse` → find library → check `is_writable()` → `library.resolve(sub_path)`.
- **Update all tools**: Replace any manual path handling with `ctx.resolve_virtual_path()`.
- **Acceptance**: Tools no longer know about library-first component in virtual paths. They just receive `PathBuf` after resolution.

---

## Priority 6: Refactor Batch Processing

### Task 6.1: Extract JobDiscoverer trait
- **File**: `src/batch/discoverer.rs` (new)
- **Define**: `trait JobDiscoverer { fn discover(&self) -> Result<Vec<PathBuf>, Error>; }`
- **Implement**: `FileMatcherDiscoverer { directory: PathBuf, pattern: String }`, `DirectoryDiscoverer { directory: PathBuf }`
- **Acceptance**: `BatchCoordinator` takes a `Box<dyn JobDiscoverer>` instead of containing matching logic.

### Task 6.2: Extract BatchJobExecutor
- **File**: `src/batch/executor.rs` (new)
- **Define**: `struct BatchJobExecutor { app_config: AppConfig, file_event_bus: Bus<FileEvent>, prompt: String }`
- **Methods**: `execute(&self, job: &BatchJob) -> Result<BatchJobStatus, Error>` (or returns `(status, error)` as before)
- **Acceptance**: `BatchCoordinator::run` loops: `for job in jobs { let status = executor.execute(&job)?; }`

### Task 6.3: Extract BatchLogger (or use tracing)
- **Option A**: Create `BatchLogger` with methods `session_start(total)`, `job_start(id, path)`, `job_end(id, path)`, `job_error(id, path, err)`, `session_end(completed, failed, total)`, `session_cancelled(completed, total)`.
- **Option B (preferred)**: Replace manual logging with `tracing` macros: `info!(job_id = ?, path = ?, "Starting batch job")`. Delete `log_*` methods.
- **Acceptance**: `BatchCoordinator` has zero logging boilerplate.

### Task 6.4: Simplify BatchCoordinator::run
- **After extractions**:
```rust
let targets = self.discoverer.discover()?;
let jobs = targets.into_iter().enumerate().map(|(i, p)| BatchJob::new(i, p, self.prompt.clone())).collect();
self.logger.session_start(jobs.len());
let result = self.executor.execute_concurrent(jobs, self.config.concurrency)?;
self.logger.session_end(&result);
```
- **Acceptance**: `run` is <50 lines. All complexity in helper structs.

---

## Priority 7: Reduce Coupling Between UI and State

### Task 7.1: Ensure all state access via getters/setters
- **Review**: `src/ui/panels/*.rs` to ensure no direct field access like `app.selected_file` or `app.tabs`.
- **Fix**: Use `app.selection().selected_file()`, `app.tabs().get_active()`, etc.
- **Acceptance**: Panel code only calls methods on `FastMdApp`, never reads fields directly.

### Task 7.2: Move panel-local state into panel modules
- **Identify**: State used only within a single panel (e.g., `left_panel_reset_count` in left panel). Move into panel module as local state in `show_left_panel` function or `LeftPanelState` struct.
- **Acceptance**: `FastMdApp` fields reduced by at least 3-5 fields.

---

## Priority 8: Error Handling Improvements

### Task 8.1: Define error enums
- **File**: `src/error.rs` (new)
- **Define**: `enum AgentError { ApiKeyMissing, HttpRequestFailed { status: u16, body: String }, NetworkError(String), InvalidJsonResponse(String), ToolPanic(String), ToolError { name: String, message: String }, ConfigError(String), IoError(std::io::Error) }` (derive `std::error::Error`, `Display`)
- **Acceptance**: All `run_agent` error paths return `Result<(), AgentError>`.

### Task 8.2: Replace panics with error propagation
- **In `run_agent`**: `.unwrap()` → `?` or explicit error handling.
- **Examples**: `tokio::runtime::Builder...build().unwrap()` → `.build().map_err(|e| AgentError::IoError(e))`; `rx.recv().unwrap()` in tests okay, but production should break loop on disconnect.
- **Acceptance**: No `unwrap()` or `expect()` in production agent code (tests can keep).

### Task 8.3: Add retry logic for transient network errors
- **In `LLMClient`**: Wrap HTTP call with `retry_with_backoff` (3 retries, exponential backoff). Retry on timeout, connection errors, 5xx, 429.
- **Acceptance**: Transient failures automatically retried; only persistent errors bubble up.

### Task 8.4: Map errors to user-friendly messages in UI
- **In `AgentSessionManager`**: Convert `AgentError` to display string for status bar: `match error { AgentError::ApiKeyMissing => "API key not configured. Please set in settings.", _ => format!("Error: {}", e) }`
- **Acceptance**: User sees actionable error messages, not stack traces.

---

## Execution Order

1. **Start with Priority 1** (God Object extraction) - this unlocks other refactors by making codebase more modular.
2. **Then Priority 3** (Tool registry cleanup) - simplifies tool code before agent refactor.
3. **Then Priority 2** (Agent extraction) - easier once tools are clean.
4. **Then Priority 4** (Background workers) - independent, can run in parallel.
5. **Then Priority 5** (Virtual FS) - small but important abstraction.
6. **Then Priority 6** (Batch) - depends on agent refactor.
7. **Then Priority 7** (UI coupling) - can do alongside others.
8. **Finally Priority 8** (Error handling) - polish across all modules.

---

## Testing Strategy

- **Run existing tests after each task**: `cargo test --workspace`
- **Add tests for extracted components**: Each new struct should have unit tests in same file or `mod tests`.
- **Integration tests**: Existing `tests/` should continue passing; no functionality change.
- **Bench**: No performance regression expected (cleaner code may improve).

---

## Success Criteria

- All existing tests pass.
- No file exceeds 500 lines (except auto-generated or data files).
- No method has > 4 parameters (PSD-002).
- `FastMdApp` has ≤ 20 fields and `update_ui` ≤ 100 lines.
- All public interfaces hide implementation details (no virtual path strings exposed outside VFS layer).
- LLM agent loop is testable in isolation (can inject mock LLM client).
- Error handling uses typed errors, no panics in production code.
