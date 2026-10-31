# Messaging — Enums, Payloads and Dispatch

> **Scope:** every `enum`/`struct` that crosses a thread or module boundary via `Bus<T>` (broadcast) or `mpsc::channel`. Single-threaded UI callbacks are out of scope.
> Source of truth is `src/app/bus/events/*`, `src/agent/events.rs`, `src/app/command_executor.rs`. Diagrams in `ARCHITECTURE_C4.md` § Messaging Architecture.

## Transport primitives

| Primitive | File | Capacity | Semantics |
|---|---|---|---|
| `Bus<T: Clone+Send+'static>` / `BusReader<T>` | `src/app/bus/core.rs:52` | `tokio::broadcast` 8192 | MPMC, `publish` -> all `subscribe` readers, `try_recv`/`try_recv_exposing_lag` non-blocking poll, `Lagged(n)` on overflow |
| `BackgroundEventSender` (`mpsc::Sender<BackgroundEvent>` + optional wake callback) | `src/app/bus/events/typed.rs:28` | unbounded | MPSC; the iced layer consumes it through the `ui_iced::subscription` bridge (a forwarding thread turns `recv` into a `Stream`), so the callback slot stays framework-free and unused in the iced build |
| `Sender<AgentPrompt>` / `Receiver<AgentPrompt>` | `src/agent/session.rs:66` | unbounded | MPSC UI->driver, driver `recv()` blocks |

All `Bus<T>` drains are fire-and-forget; no caller `await`s a reply. Completion is observed via a follow-on event (e.g. `AgentEvent::SessionFinished`, `FsEvent::Finished`). Only `BatchHandle::join` `src/app/agent/batch/coordinator.rs` and `AgentSession::drop` `src/agent/session.rs:483` `handle.join()` block.

---

## The iced inbound bridge (`ui_iced/subscription.rs`)

Every background→UI notification reaches the interface through one place: an
`iced::Subscription<Message>` built from the live `BusReceivers` handle and
handed to `iced::application(...).subscription(...)` at launch. The
producers are unchanged — this is the single point where the iced layer
touches a receiver ([FMD-065]).

| Producer | Event | `Message` |
|---|---|---|
| `Bus<ConfigArrived>` | `ConfigArrived { config }` | `Message::Inbound(InboundMessage::ConfigArrived(Arc<AppConfig>))` |
| `Bus<FileChanged>` | `FileChanged { kind, files }` (three `FileChangedKind`s; bursts may be batched) | `Message::Inbound(InboundMessage::FileChanged(event))` / `FileChangedBatch` |
| `Bus<DirectoryChanged>` | `DirectoryChanged { path, kind }` (bursts may be batched) | `Message::Inbound(InboundMessage::DirectoryChanged(event))` / `DirectoryChangedBatch` |
| `Bus<AgentEvent>` | all 11 `AgentEvent` variants | `Message::Inbound(InboundMessage::Agent(Arc<AgentEvent>))` |
| `std::sync::mpsc<BackgroundEvent>` | `Fs` / `Process` / `McpAuth` | `Message::Inbound(InboundMessage::Background(Arc<BackgroundEvent>))` |
| any bus, on `Lagged(n)` | dropped-event report | `Message::Inbound(InboundMessage::EventsDropped { source, count })` |

Shape notes:

- `Subscription::run` takes a bare `fn() -> S` and cannot carry captured
  state, so the bridge uses `Subscription::run_with(data, builder)`: the
  receiver handle is the hashed `data` (hashed by its `Arc` address so clones
  keep the identity stable) and the builder is a non-capturing `fn` returning
  a boxed stream.
- The bus and background-event streams are merged with `Subscription::batch`.
- `bus_event_to_message` is the pure half; the stream builders are the only
  impure half.
- A lag is not a disconnect: the stream logs the dropped count and keeps
  running, and the reducer surfaces the loss so the UI can re-synchronise on
  the next valid event ([FMD-067], [FMD-072]).

Outbound user intent does **not** ride this path. It goes
`panel -> Message::Command(UserCommand) -> reducer publishes -> 
AppOrchestrator::drain_user_command_bus -> command_executor`, which keeps
every intent self-contained and replayable ([FMD-068]).

---

## 1. `UserCommand` — UI intent (UI -> orchestrator)

Unified typed intake. Every variant carries all data the executor needs (no `&mut` borrow at publish). Published via `UserCommandProducer::publish` `src/app/bus/events/user_command.rs:178`, transported on `Bus<UserCommand>` (`user_command_bus`/`user_command_reader` `src/app/orchestrator.rs:73`), drained `drain_user_command_bus` `src/app/orchestrator.rs:84` -> `apply_user_command` `src/app/command_executor.rs:15` on UI thread next frame. No return value. No await.

| Variant | Arguments | Return | Await? | Processed where |
|---|---|---|---|---|
| `RunAgent(String)` | `prompt: String` | `()` | No | `apply_user_command:17` -> `start_agent_session` `src/app/orchestrator.rs:228` (builds `AgentPrompt`, `prompt_tx.send`) |
| `QueueAgentPrompt(String)` | `prompt` | `()` | No | `agent.queue_prompt` `src/agent/session.rs:154` |
| `ShowModels` | — | `()` | No | `agent.set_status/set_response`, `panel_state.show_results=true` |
| `ShowDeprecatedModelMessage` | — | `()` | No | same |
| `CancelAgent` | — | `()` | No | `agent.cancel` `src/agent/session.rs:227` (sets `AtomicBool`) |
| `ClearAgentSession` | — | `()` | No | `agent.clear_history` + transcript reset |
| `ApplyTaskToggles { toggles }` | `Vec<(usize,bool)>` | `()` | No | `transcript` content toggle |
| `ClearCommandInput` | — | `()` | No | `panel_state.command_input.clear` |
| `CloseTab(usize)` | `idx` | `()` | No | `tabs.tabs.remove(idx)` + selection fixup |
| `CloseOtherTabs(usize)` | `idx` | `()` | No | keep one tab |
| `CloseAllTabs` | — | `()` | No | `tabs.clear`, `selected_file=None` |
| `ScrollToHeader(String)` | `entry_id` | `()` | No | `tabs.scroll_to_header_id=Some` |
| `OpenBatchDialog` | — | `()` | No | `dialogs.batch_dialog_open=true` |
| `OpenToolsDialog` | — | `()` | No | `dialogs.tools_dialog_open=true` |
| `ToggleBackgroundLogs(bool)` | `show` | `()` | No | `background_manager.show_background_logs` |
| `ToggleAgentDebugWindow(bool)` | `show` | `()` | No | `panel_state.show_debug_window` |
| `ToggleAgentDebugWindowShortcut` | — | `()` | No | flip `show_debug_window` |
| `ToggleDebugWindow(bool)` | `show` | `()` | No | debug window visibility |
| `ClearDebugEntries` | — | `()` | No | `agent.state.debug_entries.clear` |
| `SetDebugJsonRows(usize)` | `rows` | `()` | No | debug panel config |
| `SetDebugSearchText(String)` | `text` | `()` | No | debug filter |
| `SetDebugAutoScroll(bool)` | `auto` | `()` | No | debug auto-scroll |
| `SelectChatModel(String)` | `model_name` | `()` | No | `config.selected_chat_model` + `agent.set_agent_config` |
| `ChangeTableWidthStrategy(DeficitStrategy)` | `strategy` | `()` | No | `config.table_width_strategy` + `config_storage.save_config` `src/app/command_executor.rs:95` |
| `SelectTagFilter(Option<String>)` | `tag` | `()` | No | `tags.selected_tag` |
| `SetToolGroupEnabled { id, enabled }` | `id: ToolGroupId`, `enabled: bool` | `()` | No | `config.tool_groups`/`mcp_servers` + `save_config` + `tool_context.rcu` `src/app/command_executor.rs:467` |
| `ClearToolGroupError(ToolGroupId)` | `id` | `()` | No | `registry.clear_error` |
| `StartMcpAuth(String)` | `server_name` | `()` | No | `dialogs.set_oauth_in_progress` + `thread::spawn` `mgr.authenticate` -> `McpAuthEvent::Completed` |
| `ForgetMcpAuth(String)` | `server_name` | `()` | No | `mcp_manager.mark_needs_auth(false)` |
| `SelectFile { path, multi }` | `path: PathBuf`, `multi: bool` | `()` | No | `selection.selected_files/selected_file`, `selected_dir`, `tabs.open_tab` |
| `SelectDirectory { path, toggle_expand }` | `path`, `toggle_expand` | `()` | No | `expanded_dirs`, `selected_dir`, `tree_dirty` |
| `OpenInEditor(PathBuf)` | `path` | `()` | No | `fs::read_to_string` + `text_buffer.open` or `open_in_system_editor` |
| `ShowInExplorer(PathBuf)` | `path` | `()` | No | `crate::ui::show_in_file_explorer` |
| `CopyPath(PathBuf)` | `path` | `()` | No | `arboard::Clipboard::set_text` |
| `SaveAsPdf(PathBuf)` | `path` | `()` | No | `rfd::FileDialog::save_file` + `execute_save_as_pdf_blocking` (UI thread, cf. fix proposal) |
| `Rename(PathBuf)` | `path` | `()` | No | `dialogs.file_to_rename` + open rename dialog |
| `Move(PathBuf)` | `path` | `()` | No | `dialogs.file_to_move` |
| `Delete(PathBuf)` | `path` | `()` | No | `recycle_bin::delete` + directory or file deletion event, routed by entry type |
| `CreateDirectory { parent }` | `parent: PathBuf` | `()` | No | open create-dir dialog |
| `CreateDocument { parent }` | `parent: PathBuf` | `()` | No | open create-document dialog |
| `RunSkillPrompt { content, target_dir, target_file }` | `content`, `Option<PathBuf>` x2 | `()` | No | set selection + `start_agent_session(content)` |
| `MergePrompt(Vec<PathBuf>)` | `files` | `()` | No | `build_merge_prompt` + `start_agent_session` |
| `ConfirmMove { file, destination }` | `PathBuf` x2 | `()` | No | `fs::rename` + file- or directory-bus rename events + workspace/tags/expanded_dirs update |
| `ConfirmCreateDirectory { parent, name }` | `parent`, `name: String` | `()` | No | `fs::create_dir_all` + `DirectoryEventProducer` + `watcher.watch` |
| `ConfirmCreateDocument { parent, name }` | `parent`, `name` | `()` | No | `write_new_document` + `publish_discovered` |
| `ConfirmRename { path, new_name }` | `path`, `new_name` | `()` | No | `fs::rename` + file- or directory-bus rename events + workspace/tags/tabs fixup |
| `StartBatch { config, prompt_text }` | `config: BatchConfig`, `prompt_text: String` | `()` | No | `BatchCoordinator::new` + `coordinator.execute()` `src/app/agent/batch/coordinator.rs:46` (`thread::spawn`) -> `batch_handle`; `prompt_text` is a snapshot of the selected prompt body at commit time |
| `CancelBatch` | — | `()` | No | close batch dialog, clear prompts |

---

## 2. File and directory events — filesystem truth (multi-producer, multi-consumer)

The event contracts intentionally use distinct buses: file changes use `Bus<FileChanged>` (`src/app/bus/events/file.rs`) and directory lifecycle changes use `Bus<DirectoryChanged>` (`src/app/bus/events/directory.rs`). Neither event payload is a union of the other.

| Message | Args | Return | Await? | Producers | Processed where |
|---|---|---|---|---|---|
| `FileChanged { kind, files }` | `FileChangedKind::{Added, Deleted, Changed}`, `Vec<FileEntry>` | `()` | No | `FileWatcher`, `Indexer`, filesystem commands, agent/tool file operations via `FileEventProducer` | `AppOrchestrator`/`Workspace`, `PdfBackingTracker`, `BusRouter` (PDF/image routing), vector search |
| `DirectoryChanged { path, kind }` | one `PathBuf`, `DirectoryChangedKind::{Added, Deleted}` | `()` | No | `FileWatcher`, `Indexer`, directory commands via `DirectoryEventProducer` | `AppOrchestrator`/`Workspace` (deletion cascades to descendants) |

`FileChanged` carries a kind-homogeneous file list; a file rename emits `Deleted(old)` followed by `Added(new)`. `DirectoryChanged` carries one directory path per event; a directory rename likewise emits `Deleted(old)` then `Added(new)`. The `Workspace` projection owns the unified file and directory indexes while the two event transports remain separate.

---

## 3. `BackgroundEvent` — typed mpsc to UI

Wrapper `src/app/bus/events/typed.rs:119`. In the iced build the channel is not polled per frame: `ui_iced::subscription` bridges it onto an `iced::Subscription<Message>` (a dedicated forwarding thread because iced's futures executor is the thread-pool, not tokio), each event arrives as `Message::Inbound(InboundMessage::Background(..))`, and `reduce_inbound` (`src/app/ui_iced/update.rs`) dispatches to `handle_fs_event`/`handle_process_event`/`handle_mcp_auth_event`. The optional wake callback on `BackgroundEventSender` (`src/app/bus/events/typed.rs:50`) is a framework-free hook the iced path does not install — the subscription stream is what wakes the loop.

| Variant | Inner enum | Args | Return | Await? | Produced by | Processed where |
|---|---|---|---|---|---|---|
| `Fs(FsEvent)` | `FsEvent` `src/app/bus/events/typed.rs:67` | see below | `()` | No | Indexer, watcher, `AppFileObserver` | `handle_fs_event` `src/app/orchestrator.rs:455` |
| `Process(ProcessEvent)` | `ProcessEvent` `src/app/bus/events/typed.rs:94` | see below | `()` | No | PdfConverter, ImageVision, file loader | `handle_process_event` |
| `McpAuth(McpAuthEvent)` | `McpAuthEvent` `src/app/bus/events/typed.rs:106` | see below | `()` | No | `StartMcpAuth` thread `src/app/command_executor.rs:492` | `handle_mcp_auth_event` |

### 3a. `FsEvent`

| Variant | Args | Return | Await? | Processed where |
|---|---|---|---|---|
| `FileParsed { path, tags }` | `PathBuf`, `Vec<String>` | `()` | No | `tags.add_tags`, `Workspace.add_file`, `tree_dirty` |
| `DirParsed { path }` | `PathBuf` | `()` | No | `Workspace.add_dir`, `tree_dirty` |
| `FileModified { path, tags }` | `PathBuf`, `Vec<String>` | `()` | No | `tags.add_tags`+`rebuild`, `Workspace.add_file`, clear `loaded_path`, `tree_dirty`; also emitted from `drain_agent_event_bus` for `ToolSideEffect` |
| `FileDeleted { path }` | `PathBuf` | `()` | No | `Workspace.remove_file`, `tags.remove/rebuild`, `close_tabs_for_removed_files` |
| `Finished` | — | `()` | No | `task_take_finished_watcher` -> `_watcher`, `indexing_finished=true`, `tags.rebuild` |
| `FinishedWithoutWatcher` | — | `()` | No | `indexing_finished=true` |

### 3b. `ProcessEvent`

| Variant | Args | Return | Await? | Processed where |
|---|---|---|---|---|
| `LogEntry(BackgroundLogEntry)` | `BackgroundLogEntry { timestamp, category: LogCategory, message }` `src/app/bus/events/messages.rs:64` | `()` | No | `BackgroundProcessManager` ring buffer `src/app/background/task.rs` |
| `FileLoaded { path, content }` | `PathBuf`, `Result<String,String>` | `()` | No | editor `TextBuffer` load |

### 3c. `McpAuthEvent`

| Variant | Args | Return | Await? | Processed where |
|---|---|---|---|---|
| `Completed { server_name, error }` | `String`, `Option<String>` | `()` | No | `dialogs.set_oauth_idle`, `tool_context` auth state, toast |

`From` impls `FsEvent/ProcessEvent/McpAuthEvent/BackgroundLogEntry -> BackgroundEvent` `src/app/bus/events/typed.rs:148`.

---

## 4. `AgentEvent` — agent -> UI broadcast

`src/app/bus/events/agent.rs:21`, `Bus<AgentEvent>` `agent_event_bus` `src/app/ui/app/init.rs:143`, 11 session-tagged variants (every variant carries `session_id: Uuid` `src/app/bus/events/agent.rs:80`). Produced by driver `run_agent` `src/agent/agent_impl.rs:20` via `BusAgentEventObserver` `src/app/bus/events/agent.rs:100` (wraps `AgentEventObserver` trait `src/agent/events.rs:178`) + `CompositeAgentEventObserver` `src/app/bus/events/agent.rs:228` (bus + `ConversationLoggerObserver`). Consumed `drain_agent_event_bus` `src/app/orchestrator.rs:360` `try_recv_exposing_lag` -> `AgentTranscript`/`AgentSession` state. Capacity 8192, `Lagged(n)` truncates.

| Variant | Args | Return | Await? | Produced by | Processed where |
|---|---|---|---|---|---|
| `SessionStarted { session_id }` | `Uuid` | `()` | No | `observer.on_session_started` `src/agent/agent_impl.rs:61` | `agent_transcript = new(session_id)` if mismatch; clears `agent_event_lagged` |
| `SessionFinished { session_id, history }` | `Uuid`, `Vec<Value>` | `()` | No | `on_session_finished` `src/agent/agent_impl.rs:90` | `agent.set_running(false)`, `set_history`, `take_next_queued_prompt` -> `start_agent_session` |
| `Status { session_id, status }` | `AgentStatus` | `()` | No | `on_status` `AwaitingLlm/ExecutingTools/Done` `src/agent/agent_impl.rs:112` | `agent.set_status(status.display_string())` |
| `Thinking { session_id, text }` | `String` | `()` | No | `handle_reasoning` `src/agent/agent_impl.rs:185` | `agent_transcript.apply_event` (thinking block) |
| `ContentDelta { session_id, text }` | `String` | `()` | No | `handle_content` `src/agent/agent_impl.rs:186` | `transcript.apply_event` (content block) |
| `ToolCallStarted { session_id, id, name, args }` | `String` x2, `Value` | `()` | No | `process_turn` loop `src/agent/agent_impl.rs:208` | `transcript.apply_event` (tool call row) |
| `ToolResult { session_id, id, name, result }` | `String` x2, `Value` | `()` | No | `process_tool_results` `src/agent/agent_impl.rs:310` | `transcript.apply_event` |
| `ToolSideEffect { session_id, effect }` | `ToolSideEffect` | `()` | No | `observer.on_tool_side_effect` `src/agent/tool_executor.rs:214` | buffer -> `handle_fs_event(FileModified)` `src/app/orchestrator.rs:447` + `FileEventProducer::publish_updated` if observer has file_bus |
| `DebugEntry { session_id, entry }` | `AgentDebugEntry` | `()` | No | `publish_debug` `src/agent/agent_impl.rs:59` | `agent.push_debug_entry` |
| `TokenUsage { session_id, usage }` | `TokenUsageInfo` | `()` | No | `emit_usage` `src/agent/agent_impl.rs:176` | `agent.apply_token_usage` `src/agent/session.rs:181` |
| `Failed { session_id, error }` | `String` | `()` | No | `on_failed` `src/agent/agent_impl.rs:140` | `agent.set_running(false)`, `set_status(Error:...)`, `clear_queued_prompts` |

Supporting types:

| Type | File | Variants/Fields |
|---|---|---|
| `AgentStatus` | `src/agent/events.rs:119` | `AwaitingLlm`, `ExecutingTools`, `Done`; `display_string()` |
| `ToolSideEffect` | `src/agent/events.rs:144` | `FileCreated { path, tags }`, `FileChanged { path }` |
| `AgentDebugEntry` | `src/agent/events.rs:49` | `turn`, `timestamp`, `kind: DebugEntryKind(Outgoing|Incoming|ToolResults)`, `summary`, `content: Option<Value>`, `row_type: DebugEntryRow(Entry|SessionBoundary)` |
| `TokenUsageInfo` | `src/agent/events.rs:64` | `prompt_tokens`, `completion_tokens`, `total_tokens`, `cached_tokens?`, `reasoning_tokens?` |
| `AgentPrompt` (UI->driver) | `src/agent/events.rs:95` | `session_id: Uuid`, `text: String`, `system_prompts: Vec<String>`, `active_file/dir: Option<PathBuf>`, `selected_files: HashSet<PathBuf>`, `cancel_flag: Arc<AtomicBool>`; sent via `AgentSession::submit_prompt` `src/agent/session.rs:242` (`prompt_tx.send`), consumed `spawn_driver` `src/agent/session.rs:435` `recv()` loop |
| `AgentObserverEvent` (test) | `src/agent/events.rs:208` | `SessionStarted`, `SessionFinished(Vec<Value>)`, `Status`, `Thinking`, `ContentDelta`, `ToolCallStarted`, `ToolResult`, `ToolSideEffect`, `DebugEntry`, `TokenUsage`, `Failed` |

Cancel: UI `agent.cancel` `src/agent/session.rs:227` sets `AtomicBool`; driver `run_agent_inner` `src/agent/agent_impl.rs:66` polls `cancel_flag.load` each turn.

---

## 5. `ConfigArrived`

`src/app/bus/events/config.rs:10` `ConfigArrived { config: AppConfig }`, `new(config)` helper. `Bus<ConfigArrived>` `config_bus` `src/app/bus/config.rs`. Published once at startup by `main`, subscribed by `Task::new` `src/app/background/task.rs`, `FastMdApp::new` `src/app/ui/app/init.rs:94`, `spawn_config_subscription` `src/app/agent/session/config_subscriber.rs`. Drained `drain_config_bus` `src/app/orchestrator.rs:283` -> `config`, `content_libraries`, `selection.tree_dirty`, `inline_editor_enabled`, `batch_dialog_config.available_dirs`, `agent.set_agent_config`.

---

## 6. Tool executor internal messages (not `Bus`, but thread-crossing)

| Message | File | Args | Return | Await? | Where processed |
|---|---|---|---|---|---|
| `ToolCallRecord` | `src/agent/tool_executor.rs:22` | `call_id`, `name`, `arguments: String`, `result: String` | `(Vec<ToolCallRecord>, Vec<ToolSideEffect>)` from `execute_all` `src/agent/tool_executor.rs:142` | Yes - `run_agent` `process_turn` `src/agent/agent_impl.rs:212` `let (results, side_effects)=executor.execute_all(tc);` blocks driver until tools finish; `execute_parallel` `src/agent/tool_executor.rs:217` `rt.block_on(JoinSet+spawn_blocking)`; `execute_sequential` `src/agent/tool_executor.rs:276` inline | `record_tool_errors` -> `tool_context.rcu`, `extract_side_effects` for `create_note` |
| `BatchConfig` / `BatchJob` | `src/app/agent/batch/types.rs` | `mode`, `concurrency`, `prompt`, `target_path` | `BatchResult { total, completed, failed, cancelled, duration }` | Yes - `BatchCoordinator::execute` `src/app/agent/batch/coordinator.rs:46` returns `BatchHandle { thread, cancel_flag }`; the iced batch dialog reads the handle status (`Dialogs::batch_handle` + `thread.is_finished()`) into its view-model and the reducer joins the finished handle | `BatchJobExecutor::execute_concurrent` `src/app/agent/batch/executor.rs:39` (`rt.block_on` + `Semaphore` + `run_agent_blocking` `src/app/agent/batch/executor.rs:210`) |

---

## 7. Summary: which channel needs `await`/`join`

| Channel | `await`/`join` required? | Why |
|---|---|---|
| `Bus<FileChanged>`, `Bus<DirectoryChanged>`, `Bus<UserCommand>`, `Bus<AgentEvent>`, `Bus<ConfigArrived>` | No | Delivered to the UI as `Message::Inbound` off the iced `Subscription`; `Lagged` is reported, never awaited; no reply |
| `BackgroundEvent` mpsc | No | Forwarded onto the same `Subscription` stream, fire-and-forget |
| `AgentPrompt` mpsc | No (sender) / blocking `recv` on driver | UI `send` returns immediately; driver blocks |
| `BatchHandle` | Yes - status read, then `join` | The reducer joins the finished handle to reclaim the worker thread |
| `AgentSession` driver `JoinHandle` | Yes - on `drop` `src/agent/session.rs:490` | `drop` replaces `prompt_tx` then `handle.join` to avoid use-after-free |
| `ToolExecutor::execute_all` | Yes - caller blocks driver | LLM turn loop cannot continue until tool results are back |

> Filesystem event tables reflect the current event contracts. Update them after changing `src/app/bus/events/*` or `src/agent/events.rs`.
