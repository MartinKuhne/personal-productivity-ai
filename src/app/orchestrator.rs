//! Application orchestrator — coordinates domain state, background events, file-watcher events, and agent turn lifecycles.
//! Unit tests live in the sibling `orchestrator_tests.rs` sidecar.

use crate::agent::AgentSession;
use crate::agent::events::ToolSideEffect;
use crate::background::logs::SharedBackgroundLogs;
use crate::background::{BackgroundLogEntry, LogCategory};
use crate::background::{UserFileOperation, UserFileOperationResult, UserFileOperationWorker};
use crate::bus::core::{BroadcastRecvError, Bus, BusReader};
use crate::bus::events::agent::AgentEvent as SeamAgentEvent;
use crate::bus::events::config::ConfigArrived;
use crate::bus::events::directory::{DirectoryChanged, DirectoryChangedKind};
use crate::bus::events::file::{FileChanged, FileChangedKind};
use crate::bus::events::typed::{
    BackgroundEvent, BackgroundEventSender, FsEvent, McpAuthEvent, ProcessEvent,
};
use crate::bus::events::user_command::UserCommand;
use crate::markdown::Document;
use crate::ui::agent::panel_state::AgentPanelState;
use crate::ui::agent::transcript::AgentTranscript;
use crate::ui::{Dialogs, FileSelection, Tabs, TextBuffer};
use crate::workspace::Tags;
use crate::workspace::watcher::Workspace;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

/// Visible truncation marker emitted into the transcript when the UI falls
/// behind the agent's broadcast bus and `BusReader` reports `Lagged(n)`
/// (research.md §1, quickstart scenario 5).
pub const LAG_TRUNCATION_MARKER: &str = "[output truncated — UI fell behind the agent]";

/// Error code for the log line emitted when a tool's file side effect cannot be
/// re-published as `FsEvent::FileModified` ([NFR-004]).
const SIDE_EFFECT_PUBLISH_ERROR_CODE: &str = "FILE-4013";

pub struct AppOrchestrator {
    pub content_libraries: Vec<crate::config::ContentLibrary>,
    pub rx: Receiver<BackgroundEvent>,
    pub tx: BackgroundEventSender,
    /// Background worker owning user-triggered filesystem operations.
    pub user_file_operation_worker: UserFileOperationWorker,
    /// File-event bus (`[FileChanged]`) — the file-change transport shared
    /// with the background workers ([REQ-330..334]).
    pub file_event_bus: Bus<FileChanged>,
    /// Directory-event bus (`[DirectoryChanged]`) — the directory-lifecycle
    /// transport shared with the background workers ([REQ-320..322]).
    pub dir_event_bus: Bus<DirectoryChanged>,
    /// Inert catch-up reader for the file bus: `Some` only on legacy init
    /// paths, `None` under the iced subscription bridge ([REQ-330..334]).
    pub file_event_reader: Option<BusReader<FileChanged>>,
    /// `Workspace` projection — the single source of truth for file and
    /// directory state, fed by both event buses.
    pub workspace: Workspace,
    pub pdf_backing_tracker: crate::agent::session::PdfBackingTracker,
    pub tags: Tags,
    pub selection: FileSelection,
    pub tabs: Tabs,
    pub _watcher: Option<notify::RecommendedWatcher>,
    pub agent: AgentSession,
    pub dialogs: Dialogs,
    pub text_buffer: TextBuffer,
    pub inline_editor_enabled: bool,
    pub background_manager: SharedBackgroundLogs,
    pub config: crate::config::AppConfig,
    pub config_storage: std::sync::Arc<dyn crate::config::ConfigStorageHandler>,
    pub config_reader: Option<BusReader<ConfigArrived>>,
    pub pending_file_load: Option<PathBuf>,
    /// Path of the latest editor-open request; older worker results are ignored.
    pub pending_editor_open: Option<PathBuf>,
    pub finished_watcher_slot: Arc<Mutex<Option<notify::RecommendedWatcher>>>,
    pub tool_context: std::sync::Arc<arc_swap::ArcSwap<crate::agent::AgentToolContext>>,
    /// Event bus for agent events. Owned by the application orchestration layer.
    pub agent_event_bus: Bus<SeamAgentEvent>,
    /// Reader for the `Bus<AgentEvent>` agent→UI channel. Subscribed
    /// once during app init.
    /// Drained each frame in [`Self::drain_agent_event_bus`].
    pub agent_event_reader: Option<BusReader<SeamAgentEvent>>,
    /// True when the last `BusReader::try_recv` on `agent_event_reader`
    /// returned `Lagged(n)`. Cleared on the next `SessionStarted` or
    /// `Status` boundary (re-sync point).
    pub agent_event_lagged: bool,
    /// UI-owned view model accumulating `AgentEvent` deltas into a
    /// displayable transcript. The UI renders agent response and thinking
    /// from this buffer instead of `AgentState::response`/`thinking`.
    pub agent_transcript: AgentTranscript,
    /// UI-owned panel state (show_results, show_debug_window, scroll_to_id,
    /// command_input, etc.).
    pub agent_panel_state: AgentPanelState,
    /// Event bus for centralized UI user commands.
    pub user_command_bus: Bus<UserCommand>,
    /// Reader for the `Bus<UserCommand>` channel.
    pub user_command_reader: Option<BusReader<UserCommand>>,
}

impl AppOrchestrator {
    #[tracing::instrument(
        skip_all,
        name = "orchestrator.drain_user_command_bus",
        level = "debug"
    )]
    pub fn drain_user_command_bus(&mut self) {
        let mut events = Vec::new();
        if let Some(reader) = self.user_command_reader.as_mut() {
            loop {
                match reader.try_recv_exposing_lag() {
                    Ok(event) => events.push(event),
                    Err(BroadcastRecvError::Empty) => break,
                    Err(BroadcastRecvError::Closed) => {
                        self.user_command_reader = None;
                        break;
                    }
                    Err(BroadcastRecvError::Lagged(n)) => {
                        tracing::warn!(
                            name = "orchestrator.user_command_bus.lagged",
                            lagged_events = n,
                            "BusReader<UserCommand> lagged — UI fell behind; dropping events"
                        );
                        continue;
                    }
                }
            }
        }
        for event in events {
            self.apply_user_command(event);
        }
    }

    #[tracing::instrument(
        skip_all,
        name = "orchestrator.process_workspace_events",
        level = "debug"
    )]
    pub fn process_file_events(&mut self) -> bool {
        let mut changed = false;

        // The `Workspace` projection drains the directory bus from its own
        // subscriber and keeps the file/directory sets in sync.
        if self.workspace.process_directory_changed() {
            changed = true;
            self.selection.tree_dirty = true;
        }
        // The workspace's own file-drain applies the reduced file state
        // (added/removed files + parent-directory safety net); full
        // application (tabs/tags/PDF backing) happens in `apply_file_changed`
        // for events carried by the subscription bridge.
        if self.workspace.process_file_changed() {
            changed = true;
            self.selection.tree_dirty = true;
        }

        // Collect the backlog before applying it (the same shape
        // `drain_user_command_bus` uses) so the reader borrow ends before the
        // events touch `self`.
        let mut events: Vec<FileChanged> = Vec::new();
        if let Some(reader) = self.file_event_reader.as_mut() {
            while let Ok(event) = reader.try_recv() {
                events.push(event);
            }
        }
        for event in events {
            self.apply_file_changed(event);
            changed = true;
        }

        changed
    }

    /// Applies ONE `[FileChanged]` event to the workspace projections: the
    /// file/directory index, the PDF-backing tracker, the tag manager, the
    /// open tabs and the tree-dirty flag.
    ///
    /// The reducer applies the event carried by
    /// [`InboundMessage::FileChanged`](crate::ui_iced::message::InboundMessage::FileChanged)
    /// through this method. The subscription bridge is the single inbound
    /// path — background work notifies the UI instead of invoking it
    /// synchronously — so the orchestrator's own `file_event_reader` is left
    /// inert and this is the only applier: no event is applied twice.
    pub fn apply_file_changed(&mut self, changed: FileChanged) {
        let paths = changed.paths();
        match changed.kind {
            FileChangedKind::Added => {
                self.pdf_backing_tracker.process_discovered(&paths);
                for p in &paths {
                    if Self::is_workspace_file(p) {
                        self.workspace.add_file(p.clone());
                        if let Some(parent) = p.parent() {
                            let parent = parent.to_path_buf();
                            self.workspace.add_dir(parent);
                        }
                    }
                }
                self.selection.tree_dirty = true;
            }
            FileChangedKind::Changed => {
                for p in &paths {
                    if self.tabs.loaded_path.as_ref() == Some(p) && !self.text_buffer.is_open {
                        self.tabs.loaded_path = None;
                    }
                }
            }
            FileChangedKind::Deleted => {
                self.apply_removed_files(&paths);
            }
        }
    }

    /// Applies ONE `[DirectoryChanged]` event to the workspace projection
    /// ([REQ-320..322]). A `[DirectoryChanged][Deleted]` cascades to all
    /// descendant directories.
    pub fn apply_directory_changed(&mut self, dir_changed: DirectoryChanged) {
        match dir_changed.kind {
            DirectoryChangedKind::Added => {
                self.workspace.add_dir(dir_changed.path);
            }
            DirectoryChangedKind::Deleted => {
                let removed_files: Vec<PathBuf> = self
                    .workspace
                    .all_files
                    .iter()
                    .filter(|path| path.starts_with(&dir_changed.path))
                    .cloned()
                    .collect();
                self.workspace.remove_dir_and_descendants(&dir_changed.path);
                self.apply_removed_files(&removed_files);
            }
        }
        self.selection.tree_dirty = true;
    }

    fn apply_removed_files(&mut self, paths: &[PathBuf]) {
        self.pdf_backing_tracker.process_removed(paths);
        for path in paths {
            self.workspace.remove_file(path);
            if self.tabs.loaded_path.as_ref() == Some(path) {
                self.tabs.loaded_path = None;
            }
            self.tags.remove_file(path);
        }
        self.close_tabs_for_removed_files(paths);
        self.tags.rebuild();
        self.selection.tree_dirty = true;
    }

    pub fn close_tabs_for_removed_files(&mut self, paths: &[PathBuf]) {
        if !self.tabs.close_tabs(paths) {
            return;
        }
        if let Some(selected) = self.selection.selected_file().cloned() {
            if !self.tabs.tabs.contains(&selected) {
                *self.selection.selected_file_mut() = self.tabs.tabs.last().cloned();
            }
        } else if !self.tabs.tabs.is_empty() {
            *self.selection.selected_file_mut() = self.tabs.tabs.last().cloned();
        }
        if self.selection.selected_file().is_none() {
            self.tabs.clear_content();
        }
    }

    pub fn is_workspace_file(path: &std::path::Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| {
                let lower = e.to_lowercase();
                lower == "md" || lower == "markdown" || lower == "txt"
            })
            .unwrap_or(false)
    }

    pub fn start_agent_session(&mut self, prompt: String) {
        let (active_file, active_dir, selected_files) =
            self.selection.agent_context(&self.tabs.tabs);
        let session_id = self
            .agent
            .current_session_id()
            .unwrap_or_else(uuid::Uuid::new_v4);
        let is_new = self.agent.current_session_id().is_none();

        // Assemble the system prompts at submit time — they depend on the
        // global config (user info, system-prompt extension, content
        // libraries) and the active UI selection. The agent run loop
        // doesn't know how to build these; it just forwards the
        // pre-built blocks.
        let system_prompts = crate::agent::prompts::build_system_prompts(
            &self.config,
            active_file.as_deref(),
            active_dir.as_deref(),
            &selected_files,
        );

        let agent_prompt = crate::agent::events::AgentPrompt {
            session_id,
            text: prompt.clone(),
            system_prompts,
            active_file,
            active_dir,
            selected_files,
            cancel_flag: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        self.agent.submit_prompt(agent_prompt);
        self.agent_panel_state.show_results = true;

        if is_new {
            // Clear the transcript immediately so the UI shows empty content
            // between session start and the `SessionStarted` event arrival.
            self.agent_transcript.reset();
        } else {
            // Visually separate the new prompt in the transcript for continuation
            let user_prompt = format!(
                "\n\n{}{}\n\n",
                crate::ui::strings::AGENT_USER_TURN_MARKER,
                prompt
            );
            self.agent_transcript.content.push_str(&user_prompt);
            self.agent_transcript
                .blocks
                .push(crate::ui::agent::transcript::TranscriptBlock::Content { text: user_prompt });
        }
    }

    pub fn task_take_finished_watcher(&self) -> Option<notify::RecommendedWatcher> {
        self.finished_watcher_slot
            .lock()
            .ok()
            .and_then(|mut slot| slot.take())
    }

    #[tracing::instrument(skip_all, name = "orchestrator.drain_config_bus", level = "debug")]
    pub fn drain_config_bus(&mut self) {
        let mut config: Option<ConfigArrived> = None;
        if let Some(reader) = self.config_reader.as_ref() {
            loop {
                match reader.try_recv() {
                    Ok(event) => config = Some(event),
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        self.config_reader = None;
                        break;
                    }
                }
            }
        }

        let Some(event) = config else {
            return;
        };

        let config = event.config;

        // Project the global config to the agent's domain slice and
        // hand it to the agent. The agent's run loop reads only the
        // projected fields; the tool context (built per session)
        // carries the full `AppConfig` separately.
        let agent_cfg = config.to_agent_config();
        self.agent.set_agent_config(agent_cfg);

        self.content_libraries = config.content_libraries.clone();
        self.selection.tree_dirty = true;
        self.inline_editor_enabled = config.inline_editor_enabled;
        self.dialogs.batch_dialog_config.available_dirs = config
            .content_libraries
            .iter()
            .map(|lib| PathBuf::from(&lib.root_folder))
            .collect();
        // The dialog shows the *virtual* label — the library's own name,
        // which is what `ContentLibraryExt::display_label_for` returns for
        // that library's root — while `available_dirs` keeps the real path
        // the batch runner globs. Same iterator order as above, so the two
        // lists stay index-aligned.
        self.dialogs.batch_dialog_config.available_dir_labels = config
            .content_libraries
            .iter()
            .map(|lib| lib.name.clone())
            .collect();
        self.config = config;
        tracing::info!(
            name = "config.arrived",
            "FastMdApp populated from ConfigArrived event"
        );
    }

    #[tracing::instrument(
        skip_all,
        name = "orchestrator.drain_background_channel",
        level = "debug"
    )]
    pub fn drain_background_channel(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                BackgroundEvent::Fs(fs_ev) => {
                    self.handle_fs_event(fs_ev);
                }
                BackgroundEvent::Process(proc_ev) => {
                    self.handle_process_event(proc_ev);
                }
                BackgroundEvent::McpAuth(mcp_ev) => {
                    self.handle_mcp_auth_event(mcp_ev);
                }
            }
        }
    }

    /// Drain the `Bus<AgentEvent>` reader.
    ///
    /// Routes all agent events through the agent event bus: updates
    /// `AgentState` (lifecycle: `running`, `status`, `history`,
    /// `token_usage`, `debug_entries`) and builds the `AgentTranscript`
    /// view model from `ContentDelta`/`Thinking`/`ToolCallStarted`/
    /// `ToolResult` deltas. A `ToolSideEffect` is re-issued as
    /// `FsEvent::FileModified` on the background channel once the drain has
    /// finished, so the UI's tag and workspace index catch up ([FR-007]); the
    /// observer that produced it also told the file-change bus about the path,
    /// and authoritative tag extraction stays delegated to the background tag
    /// manager. Handles `Lagged(n)` by emitting a visible
    /// truncation marker into the transcript.
    #[tracing::instrument(skip_all, name = "orchestrator.drain_agent_event_bus", level = "debug")]
    pub fn drain_agent_event_bus(&mut self) {
        let Some(reader) = self.agent_event_reader.as_mut() else {
            return;
        };
        // Collect events into a buffer so we can release the reader borrow
        // before calling `self.handle_fs_event` (which needs `&mut self`).
        let mut next_prompt: Option<String> = None;
        // Tool side effects are buffered for the same reason: publishing them
        // needs `&self`, which the `agent_event_reader` borrow forbids until the
        // drain has finished ([FR-007]).
        let mut side_effects: Vec<ToolSideEffect> = Vec::new();
        loop {
            match reader.try_recv_exposing_lag() {
                Ok(event) => {
                    // Re-sync on SessionStarted / Status boundary after a lag.
                    if self.agent_event_lagged {
                        match &event {
                            SeamAgentEvent::SessionStarted { .. }
                            | SeamAgentEvent::Status { .. } => {
                                self.agent_event_lagged = false;
                            }
                            _ => {}
                        }
                    }
                    match &event {
                        SeamAgentEvent::SessionStarted { session_id } => {
                            if self.agent_transcript.session_id != *session_id {
                                self.agent_transcript =
                                    crate::ui::agent::transcript::AgentTranscript::new(*session_id);
                            }
                        }
                        SeamAgentEvent::SessionFinished { history, .. } => {
                            self.agent.set_running(false);
                            self.agent.set_history(Some(history.clone()));
                            next_prompt = self.agent.take_next_queued_prompt();
                        }
                        SeamAgentEvent::Status { status, .. } => {
                            self.agent.set_status(status.display_string().to_string());
                        }
                        SeamAgentEvent::Failed { error, .. } => {
                            self.agent.set_running(false);
                            self.agent.set_status(format!("Error: {}", error));
                            self.agent.clear_queued_prompts();
                        }
                        SeamAgentEvent::TokenUsage { usage, .. } => {
                            self.agent.apply_token_usage(usage.clone());
                        }
                        SeamAgentEvent::DebugEntry { entry, .. } => {
                            self.agent.push_debug_entry(entry.clone());
                        }
                        SeamAgentEvent::ToolSideEffect { effect, .. } => {
                            side_effects.push(effect.clone());
                        }
                        // Transcript-building events:
                        SeamAgentEvent::ContentDelta { .. }
                        | SeamAgentEvent::Thinking { .. }
                        | SeamAgentEvent::ToolCallStarted { .. }
                        | SeamAgentEvent::ToolResult { .. } => {
                            self.agent_transcript.apply_event(&event);
                        }
                    }
                }
                Err(BroadcastRecvError::Empty) => break,
                Err(BroadcastRecvError::Closed) => {
                    self.agent_event_reader = None;
                    break;
                }
                Err(BroadcastRecvError::Lagged(n)) => {
                    tracing::warn!(
                        name = "agent.event_bus.lagged",
                        lagged_events = n,
                        "BusReader<AgentEvent> lagged — UI fell behind the agent; truncating output"
                    );
                    self.agent_event_lagged = true;
                    // Emit a visible truncation marker into the transcript.
                    self.agent_transcript.content.push_str("\n\n");
                    self.agent_transcript
                        .content
                        .push_str(LAG_TRUNCATION_MARKER);
                    self.agent_transcript.content.push_str("\n\n");
                    continue;
                }
            }
        }
        for effect in &side_effects {
            self.publish_tool_side_effect(effect);
        }
        if let Some(prompt) = next_prompt {
            self.start_agent_session(prompt);
        }
    }

    /// Re-issue one tool file side effect as `FsEvent::FileModified` ([FR-007]).
    ///
    /// The observer that produced the event already told the *file* bus about the
    /// path, so the background tag manager will deliver authoritative tags a
    /// moment later. This closes the other half of the documented contract: the
    /// UI's own tag and workspace index catch up through the same handler that
    /// serves watcher changes, instead of the event being dropped on the floor.
    ///
    /// `FileCreated` carries the tags the tool read from the new file's front
    /// matter. `FileChanged` carries none, and [`Tags::add_tags`] *replaces* the
    /// entry a path already has — so the tags currently known to the index are
    /// carried forward rather than wiped by an empty list.
    fn publish_tool_side_effect(&self, effect: &ToolSideEffect) {
        let (path, tags) = match effect {
            ToolSideEffect::FileCreated { path, tags } => (path.clone(), tags.clone()),
            ToolSideEffect::FileChanged { path } => (
                path.clone(),
                self.tags.file_tags().get(path).cloned().unwrap_or_default(),
            ),
        };

        if let Err(error) = self
            .tx
            .send(BackgroundEvent::Fs(FsEvent::FileModified { path, tags }))
        {
            self.log_file_operation_error(
                SIDE_EFFECT_PUBLISH_ERROR_CODE,
                "Failed to publish tool file side effect as a file-modified event",
                &error.to_string(),
            );
        }
    }

    pub fn handle_fs_event(&mut self, ev: FsEvent) {
        use FsEvent;
        match ev {
            FsEvent::FileParsed { path, tags } => {
                self.tags.add_tags(path.clone(), tags);
                if Self::is_workspace_file(&path) {
                    self.workspace.add_file(path);
                }
                self.selection.tree_dirty = true;
            }
            FsEvent::DirParsed { path } => {
                self.workspace.add_dir(path);
                self.selection.tree_dirty = true;
            }
            FsEvent::Finished => {
                if let Some(watcher) = self.task_take_finished_watcher() {
                    self._watcher = Some(watcher);
                }
                self.workspace.indexing_finished = true;
                self.tags.rebuild();
                self.selection.tree_dirty = true;
            }
            FsEvent::FinishedWithoutWatcher => {
                self.workspace.indexing_finished = true;
                self.tags.rebuild();
                self.selection.tree_dirty = true;
            }
            FsEvent::FileModified { path, tags } => {
                self.tags.add_tags(path.clone(), tags);
                if Self::is_workspace_file(&path) {
                    self.workspace.add_file(path.clone());
                }
                self.tags.rebuild();
                if self.tabs.loaded_path.as_ref() == Some(&path) {
                    self.tabs.loaded_path = None;
                }
                self.selection.tree_dirty = true;
            }
            FsEvent::TagsResynchronized { files } => {
                self.tags.replace_all(files);
                self.selection.tree_dirty = true;
            }
            FsEvent::FileDeleted { path } => {
                self.workspace.remove_file(&path);
                self.tags.remove_file(&path);
                self.tags.rebuild();
                self.close_tabs_for_removed_files(std::slice::from_ref(&path));
                if self.selection.selected_file().is_some_and(|p| p == &path) {
                    *self.selection.selected_file_mut() = None;
                    self.tabs.current_yaml = None;
                    self.tabs.current_markdown = String::new();
                    self.tabs.invalidate_heading_ids_cache();
                    self.tabs.toc.clear();
                }
                self.selection.selected_files_mut().remove(&path);
                if self.tabs.loaded_path.as_ref() == Some(&path) {
                    self.tabs.loaded_path = None;
                }
                self.selection.tree_dirty = true;
            }
        }
    }

    pub fn handle_process_event(&mut self, ev: ProcessEvent) {
        use ProcessEvent;
        match ev {
            ProcessEvent::LogEntry(entry) => {
                if let Ok(mut mgr) = self.background_manager.lock() {
                    mgr.push_log(entry.clone());
                }
                tracing::info!(
                    target: crate::background::logs::TARGET_BACKGROUND_CHANNEL,
                    category = %entry.category,
                    "{}",
                    entry.message
                );
            }
            ProcessEvent::FileLoaded { path, content } => {
                // [`pending_file_load`](Self::pending_file_load) is a single slot and
                // the read runs on its own thread, so this result may belong to a
                // file the user has already moved on from. Release our own marker
                // either way — leaving it set would block a later re-selection of
                // the same path behind a load that no longer exists — but only
                // project the bytes onto the document the user is actually looking
                // at. A stale landing otherwise overwrites `loaded_path` and `toc`
                // with a document nobody asked for, which is what made the ToC
                // pane flip between two files' heading sets with no user action.
                if self.pending_file_load.as_deref() == Some(path.as_path()) {
                    self.pending_file_load = None;
                }
                if self.selection.selected_file().map(PathBuf::as_path) != Some(path.as_path()) {
                    tracing::debug!(
                        name = "orchestrator.file_load.stale_dropped",
                        path = %path.display(),
                        "Dropped FileLoaded for a file that is no longer selected"
                    );
                    return;
                }
                match content {
                    Ok(content) => {
                        // Parse the file once into a `Document` and
                        // pull both the front matter and the body
                        // from it. Avoids the previous two-step
                        // `parse_front_matter` + manual body copy.
                        let doc = Document::new(content);
                        self.tabs.current_yaml = doc.yaml().cloned();
                        self.tabs.current_markdown = doc.body().to_string();
                        self.tabs.invalidate_heading_ids_cache();
                        self.tabs.loaded_path = Some(path.clone());
                        self.tabs.toc = crate::markdown::build_toc(&self.tabs.current_markdown);
                        self.tabs.scroll_to_header_id = None;
                    }
                    Err(err) => {
                        // Load failed — do not leave stale content or an open tab.
                        self.tabs.close_tab(&path);
                        if self.selection.selected_file() == Some(&path) {
                            *self.selection.selected_file_mut() = None;
                        }
                        self.selection.selected_files_mut().remove(&path);
                        self.tabs.current_yaml = None;
                        self.tabs.current_markdown = String::new();
                        self.tabs.invalidate_heading_ids_cache();
                        self.tabs.toc.clear();
                        self.tabs.scroll_to_header_id = None;
                        self.tabs.scroll_to_search = None;

                        // Log the failure to the background log.
                        let msg = format!("Failed to load file {}: {}", path.display(), err);
                        if let Ok(mut mgr) = self.background_manager.lock() {
                            mgr.push_log(BackgroundLogEntry::new(
                                LogCategory::Watcher,
                                msg.clone(),
                            ));
                        }
                        tracing::error!(
                            target: crate::background::logs::TARGET_BACKGROUND_CHANNEL,
                            category = "Watcher",
                            "{}",
                            msg
                        );
                    }
                }
            }
            ProcessEvent::UserFileOperationCompleted(result) => {
                self.handle_user_file_operation_result(result);
            }
        }
    }

    /// Queue one user-requested file operation for background execution.
    pub fn queue_user_file_operation(&self, operation: UserFileOperation) -> bool {
        match self.user_file_operation_worker.submit(operation) {
            Ok(()) => true,
            Err(error) => {
                self.log_file_operation_error(
                    "FILE-4012",
                    "Could not queue user file operation",
                    &error,
                );
                false
            }
        }
    }

    fn handle_user_file_operation_result(&mut self, result: UserFileOperationResult) {
        match result {
            UserFileOperationResult::PromptLoaded {
                content: Ok(content),
                target_dir,
                target_file,
            } => {
                self.apply_user_command(UserCommand::RunSkillPrompt {
                    content,
                    target_dir,
                    target_file,
                });
            }
            UserFileOperationResult::PromptLoaded {
                content: Err(error),
                ..
            } => self.log_file_operation_error("FILE-4001", "Failed to read prompt file", &error),
            UserFileOperationResult::EditorOpened { path, content } => {
                if self.pending_editor_open.as_ref() != Some(&path) {
                    tracing::debug!(
                        name = "file_operation.editor_open.stale_dropped",
                        path = %path.display(),
                        "Dropped an editor-open result superseded by a newer request."
                    );
                    return;
                }
                self.pending_editor_open = None;
                match content {
                    Ok(Some(content)) => {
                        self.text_buffer
                            .open(&path, &content, Some(&self.pdf_backing_tracker));
                        if !self.tabs.tabs.contains(&path) {
                            self.tabs.open_tab(path.clone());
                        }
                        self.tabs.loaded_path = Some(path);
                    }
                    Ok(None) => {}
                    Err(error) => self.log_file_operation_error(
                        "FILE-4002",
                        &format!("Failed to open {} in editor", path.display()),
                        &error,
                    ),
                }
            }
            UserFileOperationResult::EditorSaved {
                path,
                result: Ok(()),
                ..
            } => {
                self.file_event_bus.publish(FileChanged::updated_one(path));
            }
            UserFileOperationResult::EditorSaved {
                path,
                result: Err(error),
                ..
            } => self.log_file_operation_error(
                "FILE-4003",
                &format!("Failed to save {}", path.display()),
                &error,
            ),
            UserFileOperationResult::Moved {
                source,
                destination,
                is_directory,
                result: Ok(()),
            } => self.apply_path_move(&source, &destination, is_directory),
            UserFileOperationResult::Moved {
                source,
                destination,
                result: Err(error),
                ..
            } => self.log_file_operation_error(
                "FILE-4004",
                &format!(
                    "Failed to move {} to {}",
                    source.display(),
                    destination.display()
                ),
                &error,
            ),
            UserFileOperationResult::Deleted {
                path,
                is_directory,
                result: Ok(()),
            } => {
                if is_directory {
                    crate::bus::events::directory::DirectoryEventProducer::new(
                        self.dir_event_bus.clone(),
                    )
                    .publish_removed(&path);
                } else {
                    crate::bus::events::file::FileEventProducer::new(self.file_event_bus.clone())
                        .publish_removed(&path);
                }
            }
            UserFileOperationResult::Deleted {
                path,
                result: Err(error),
                ..
            } => self.log_file_operation_error(
                "FILE-4005",
                &format!("Failed to delete {} to trash", path.display()),
                &error,
            ),
            UserFileOperationResult::DirectoryCreated {
                path,
                result: Ok(()),
            } => {
                self.workspace.add_dir(path.clone());
                crate::bus::events::directory::DirectoryEventProducer::new(
                    self.dir_event_bus.clone(),
                )
                .publish_discovered(&path);
                if let Some(watcher) = &mut self._watcher {
                    use notify::Watcher;
                    if let Err(error) = watcher.watch(&path, notify::RecursiveMode::Recursive) {
                        self.log_file_operation_error(
                            "FILE-4006",
                            &format!("Failed to watch {}", path.display()),
                            &error.to_string(),
                        );
                    }
                }
            }
            UserFileOperationResult::DirectoryCreated {
                path,
                result: Err(error),
            } => self.log_file_operation_error(
                "FILE-4007",
                &format!("Failed to create directory {}", path.display()),
                &error,
            ),
            UserFileOperationResult::DocumentCreated { path: Ok(path) } => {
                crate::bus::events::file::FileEventProducer::new(self.file_event_bus.clone())
                    .publish_discovered(&path);
            }
            UserFileOperationResult::DocumentCreated { path: Err(error) } => self
                .log_file_operation_error(
                    "FILE-4008",
                    "Failed to create Markdown document",
                    &error,
                ),
            UserFileOperationResult::PdfSaved {
                markdown_path,
                result: Err(error),
            } => self.log_file_operation_error(
                "FILE-4010",
                &format!("Failed to export {} as PDF", markdown_path.display()),
                &error,
            ),
            UserFileOperationResult::PdfSaved { result: Ok(_), .. } => {}
            UserFileOperationResult::Revealed {
                path,
                result: Err(error),
            } => self.log_file_operation_error(
                "FILE-4011",
                &format!("Failed to reveal {} in Explorer", path.display()),
                &error,
            ),
            UserFileOperationResult::Revealed { result: Ok(()), .. } => {}
        }
    }

    fn apply_path_move(&mut self, source: &Path, destination: &Path, is_directory: bool) {
        if is_directory {
            crate::bus::events::directory::DirectoryEventProducer::new(self.dir_event_bus.clone())
                .publish_rename(source, destination);
            self.workspace.remove_dir(source);
            self.workspace.add_dir(destination.to_path_buf());
        } else {
            crate::bus::events::file::FileEventProducer::new(self.file_event_bus.clone())
                .publish_rename(source, destination);
            self.workspace.remove_file(source);
            let extension = destination
                .extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or("");
            if extension.eq_ignore_ascii_case("md") || extension.eq_ignore_ascii_case("markdown") {
                self.workspace.add_file(destination.to_path_buf());
            }
            let tags = self.tags.file_tags().get(source).cloned();
            self.tags.remove_file(source);
            if let Some(tags) = tags {
                self.tags.add_tags(destination.to_path_buf(), tags);
                self.tags.rebuild();
            }
        }
        if self.tabs.loaded_path.as_deref() == Some(source) {
            self.tabs.loaded_path = Some(destination.to_path_buf());
        }
        if self.selection.selected_file.as_deref() == Some(source) {
            self.selection.selected_file = Some(destination.to_path_buf());
        }
        if self.selection.selected_dir.as_deref() == Some(source) {
            self.selection.selected_dir = Some(destination.to_path_buf());
        }
        for tab in &mut self.tabs.tabs {
            if tab == source {
                *tab = destination.to_path_buf();
            }
        }
        if self.selection.expanded_dirs.remove(source) {
            self.selection
                .expanded_dirs
                .insert(destination.to_path_buf());
        }
        self.selection.tree_dirty = true;
    }

    fn log_file_operation_error(&self, code: &str, message: &str, error: &str) {
        tracing::error!(
            error_code = code,
            error = %error,
            "{message}"
        );
        if let Ok(mut manager) = self.background_manager.lock() {
            manager.push_log(BackgroundLogEntry::new(
                LogCategory::Watcher,
                format!("{message}: {error}"),
            ));
        }
    }

    pub fn handle_mcp_auth_event(&mut self, ev: McpAuthEvent) {
        use crate::agent::tools::registry::{ToolErrorKind, ToolGroupError};
        match ev {
            McpAuthEvent::Completed { server_name, error } => {
                self.dialogs.set_oauth_idle(&server_name);
                match error {
                    None => {
                        tracing::info!(
                            server = %server_name,
                            "OAuth flow completed; clearing in-progress flag"
                        );
                    }
                    Some(msg) => {
                        tracing::warn!(
                            server = %server_name,
                            error = %msg,
                            "OAuth flow failed; recording error on group row"
                        );
                        self.tool_context.rcu(|bundle| {
                            let mut new_bundle = (**bundle).clone();
                            new_bundle.registry.record_error(
                                &crate::agent::tools::registry::ToolGroupId::Mcp(
                                    server_name.clone(),
                                ),
                                ToolGroupError::now(ToolErrorKind::Authentication, msg.clone()),
                            );
                            new_bundle
                        });
                    }
                }
            }
        }
    }

    #[tracing::instrument(skip_all, name = "orchestrator.handle_file_selection", level = "debug")]
    pub fn handle_file_selection(&mut self) {
        if let Some(selected_path) = self.selection.selected_file()
            && self.tabs.loaded_path.as_ref() != Some(selected_path)
            && self.pending_file_load.as_ref() != Some(selected_path)
        {
            self.pending_file_load = Some(selected_path.clone());
            let tx = self.tx.clone();
            let path = selected_path.clone();
            std::thread::spawn(move || {
                let content =
                    crate::agent::utils::encoding::read_text_file(&path).map_err(|e| e.to_string());
                if tx
                    .send(ProcessEvent::FileLoaded { path, content }.into())
                    .is_err()
                {
                    tracing::warn!("Background channel closed, file load result dropped");
                }
            });
        }
    }
}

#[cfg(test)]
#[path = "orchestrator_tests.rs"]
mod tests;
