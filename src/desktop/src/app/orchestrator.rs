use crate::agent::AgentSessionManager;
use crate::app::background::{BackgroundLogEntry, LogCategory, SharedProcessManager};
use crate::app::watcher::directory_tracker::DirectoryTracker;
use crate::app::watcher::file_processor::FileEventProcessor;
use crate::app::{DialogManager, SelectionManager, TabManager, TagManager, TextBuffer};
use crate::bus::core::{Bus, BusReader};
use crate::bus::events::config::ConfigArrived;
use crate::bus::events::file::FileEvent;
use crate::bus::events::typed::{BackgroundEvent, FsEvent, McpAuthEvent, ProcessEvent};
use crate::markdown::Document;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct AppOrchestrator {
    pub content_libraries: Vec<crate::config::ContentLibrary>,
    pub rx: Receiver<BackgroundEvent>,
    pub tx: std::sync::mpsc::Sender<BackgroundEvent>,
    pub file_event_bus: Bus<FileEvent>,
    pub file_event_reader: Option<BusReader<FileEvent>>,
    pub file_processor: FileEventProcessor,
    pub pdf_backing_tracker: crate::app::session::PdfBackingTracker,
    pub tag_manager: TagManager,
    pub directory_tracker: DirectoryTracker,
    pub selection: SelectionManager,
    pub tab_manager: TabManager,
    pub _watcher: Option<notify::RecommendedWatcher>,
    pub agent: AgentSessionManager,
    pub dialogs: DialogManager,
    pub submit_prompt: Option<String>,
    pub text_buffer: TextBuffer,
    pub inline_editor_enabled: bool,
    pub background_manager: SharedProcessManager,
    pub config: crate::config::AppConfig,
    pub config_reader: Option<BusReader<ConfigArrived>>,
    pub pending_file_load: Option<PathBuf>,
    pub repaint_interval: Duration,
    pub finished_watcher_slot: Arc<Mutex<Option<notify::RecommendedWatcher>>>,
}

impl AppOrchestrator {
    pub fn process_file_events(&mut self) -> bool {
        use crate::bus::events::file::FileEventKind;

        let mut changed = false;
        let mut needs_rebuild = false;
        let mut tree_dirty = false;

        // Let DirectoryTracker consume directory events from its own subscriber.
        if self.directory_tracker.process_events() {
            changed = true;
            tree_dirty = true;
        }

        if let Some(reader) = &self.file_event_reader {
            let mut removed_paths: Vec<PathBuf> = Vec::new();
            while let Ok(event) = reader.try_recv() {
                changed = true;
                match event.kind {
                    FileEventKind::Discovered => {
                        self.pdf_backing_tracker.process_discovered(&event.paths);
                        for p in &event.paths {
                            if Self::is_workspace_file(p) {
                                self.file_processor.add_file(p.clone());
                                if let Some(parent) = p.parent() {
                                    let parent = parent.to_path_buf();
                                    self.file_processor.add_dir(parent);
                                }
                            }
                        }
                        tree_dirty = true;
                    }
                    FileEventKind::Updated => {
                        for p in &event.paths {
                            if self.tab_manager.loaded_path.as_ref() == Some(p)
                                && !self.text_buffer.is_open
                            {
                                self.tab_manager.loaded_path = None;
                            }
                        }
                    }
                    FileEventKind::Removed => {
                        self.pdf_backing_tracker.process_removed(&event.paths);
                        for p in &event.paths {
                            self.file_processor.remove_file(p);
                            if self.tab_manager.loaded_path.as_ref() == Some(p) {
                                self.tab_manager.loaded_path = None;
                            }
                            self.tag_manager.remove_file(p);
                        }
                        removed_paths.extend(event.paths);
                        needs_rebuild = true;
                        tree_dirty = true;
                    }
                    FileEventKind::DirDiscovered => {
                        for p in &event.paths {
                            self.file_processor.add_dir(p.clone());
                        }
                        tree_dirty = true;
                    }
                    FileEventKind::DirRemoved => {
                        for p in &event.paths {
                            self.file_processor.remove_dir(p);
                        }
                        tree_dirty = true;
                    }
                }
            }
            if !removed_paths.is_empty() {
                self.close_tabs_for_removed_files(&removed_paths);
            }
        }

        if needs_rebuild {
            self.tag_manager.rebuild();
        }

        if tree_dirty {
            self.selection.tree_dirty = true;
        }

        changed
    }

    pub fn close_tabs_for_removed_files(&mut self, paths: &[PathBuf]) {
        let mut closed_any = false;
        for path in paths {
            if self.tab_manager.tabs.contains(path) {
                self.tab_manager.tabs.retain(|p| p != path);
                closed_any = true;
            }
        }
        if !closed_any {
            return;
        }
        if let Some(selected) = self.selection.selected_file().cloned() {
            if !self.tab_manager.tabs.contains(&selected) {
                *self.selection.selected_file_mut() = self.tab_manager.tabs.last().cloned();
            }
        } else if !self.tab_manager.tabs.is_empty() {
            *self.selection.selected_file_mut() = self.tab_manager.tabs.last().cloned();
        }
        if self.selection.selected_file().is_none() {
            self.tab_manager.clear_content();
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
            self.selection.agent_context(&self.tab_manager.tabs);
        self.agent.start_session(
            self.tx.clone(),
            prompt,
            active_file,
            active_dir,
            selected_files,
            self.file_event_bus.clone(),
        );
        self.agent.set_show_results(true);
    }

    pub fn task_take_finished_watcher(&self) -> Option<notify::RecommendedWatcher> {
        self.finished_watcher_slot
            .lock()
            .ok()
            .and_then(|mut slot| slot.take())
    }

    pub fn drain_config_bus(&mut self) {
        let mut config: Option<ConfigArrived> = None;
        if let Some(reader) = self.config_reader.as_ref() {
            match reader.try_recv() {
                Ok(event) => config = Some(event),
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    return;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.config_reader = None;
                    return;
                }
            }
        }
        self.config_reader = None;

        let Some(event) = config else {
            return;
        };

        let mut config = event.config;
        let args: Vec<String> = std::env::args().collect();
        if args.len() > 1 {
            let path = PathBuf::from(&args[1]);
            if path.exists() && path.is_dir() {
                let mut path_str = path
                    .canonicalize()
                    .unwrap_or(path)
                    .to_string_lossy()
                    .to_string();
                if path_str.starts_with(r"\\?\") {
                    path_str = path_str[4..].to_string();
                }
                let found = config
                    .content_libraries
                    .iter()
                    .any(|lib| lib.root_folder == path_str);
                if !found {
                    config
                        .content_libraries
                        .push(crate::config::ContentLibrary {
                            root_folder: path_str,
                            name: "Workspace".to_string(),
                            kind: "text".to_string(),
                            readonly: false,
                            priority: 0,
                        });
                }
            }
        }
        if config.content_libraries.is_empty() {
            let mut current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            if let Ok(canon) = std::fs::canonicalize(&current_dir) {
                current_dir = canon;
            }
            let mut path_str = current_dir.to_string_lossy().to_string();
            if path_str.starts_with(r"\\?\") {
                path_str = path_str[4..].to_string();
            }
            config
                .content_libraries
                .push(crate::config::ContentLibrary {
                    root_folder: path_str,
                    name: "Workspace".to_string(),
                    kind: "text".to_string(),
                    readonly: false,
                    priority: 0,
                });
        }

        self.agent.set_config(config.clone());

        let prompt = crate::agent::prompt_builder::SystemPromptBuilder::new(&config).build(&config);
        tracing::info!(
            name = "app.startup",
            system_prompt = %prompt,
            "Application started successfully. Emitted system prompt for diagnostics."
        );

        self.content_libraries = config.content_libraries.clone();
        self.selection.tree_dirty = true;
        self.inline_editor_enabled = config.inline_editor_enabled;
        self.dialogs.batch_dialog_config.available_dirs = config
            .content_libraries
            .iter()
            .map(|lib| PathBuf::from(&lib.root_folder))
            .collect();
        self.config = config;
        tracing::info!(
            name = "config.arrived",
            "FastMdApp populated from ConfigArrived event"
        );
    }

    pub fn drain_background_channel(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                BackgroundEvent::Agent(agent_ev) => {
                    if let Some(next_prompt) = self.agent.handle_agent_event(agent_ev) {
                        // There's a queued prompt - start the next session
                        self.start_agent_session(next_prompt);
                    }
                }
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

    pub fn handle_fs_event(&mut self, ev: FsEvent) {
        use FsEvent;
        match ev {
            FsEvent::FileParsed { path, tags } => {
                self.tag_manager.add_tags(path.clone(), tags);
                self.file_processor.add_file(path);
                self.selection.tree_dirty = true;
            }
            FsEvent::DirParsed { path } => {
                self.file_processor.add_dir(path);
                self.selection.tree_dirty = true;
            }
            FsEvent::Finished => {
                if let Some(watcher) = self.task_take_finished_watcher() {
                    self._watcher = Some(watcher);
                }
                self.file_processor.indexing_finished = true;
                self.tag_manager.rebuild();
                self.selection.tree_dirty = true;
            }
            FsEvent::FinishedWithoutWatcher => {
                self.file_processor.indexing_finished = true;
                self.tag_manager.rebuild();
                self.selection.tree_dirty = true;
            }
            FsEvent::FileModified { path, tags } => {
                self.tag_manager.add_tags(path.clone(), tags);
                self.file_processor.add_file(path.clone());
                self.tag_manager.rebuild();
                if self.tab_manager.loaded_path.as_ref() == Some(&path) {
                    self.tab_manager.loaded_path = None;
                }
                self.selection.tree_dirty = true;
            }
            FsEvent::FileDeleted { path } => {
                self.file_processor.remove_file(&path);
                self.tag_manager.remove_file(&path);
                self.tag_manager.rebuild();
                self.close_tabs_for_removed_files(std::slice::from_ref(&path));
                if self.selection.selected_file().is_some_and(|p| p == &path) {
                    *self.selection.selected_file_mut() = None;
                    self.tab_manager.current_yaml = None;
                    self.tab_manager.current_markdown = String::new();
                    self.tab_manager.invalidate_heading_ids_cache();
                    self.tab_manager.toc.clear();
                }
                self.selection.selected_files_mut().remove(&path);
                if self.tab_manager.loaded_path.as_ref() == Some(&path) {
                    self.tab_manager.loaded_path = None;
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
                    mgr.push_log(entry);
                }
            }
            ProcessEvent::FileLoaded { path, content } => {
                self.pending_file_load = None;
                match content {
                    Ok(content) => {
                        // Parse the file once into a `Document` and
                        // pull both the front matter and the body
                        // from it. Avoids the previous two-step
                        // `parse_front_matter` + manual body copy.
                        let doc = Document::new(content);
                        self.tab_manager.current_yaml = doc.yaml().cloned();
                        self.tab_manager.current_markdown = doc.body().to_string();
                        self.tab_manager.invalidate_heading_ids_cache();
                        self.tab_manager.loaded_path = Some(path.clone());
                        self.tab_manager.toc =
                            crate::ui::render::build_toc(&self.tab_manager.current_markdown);
                        self.tab_manager.scroll_to_header_id = None;
                    }
                    Err(err) => {
                        // Load failed — do not leave stale content or an open tab.
                        self.tab_manager.close_tab(&path);
                        if self.selection.selected_file() == Some(&path) {
                            *self.selection.selected_file_mut() = None;
                        }
                        self.selection.selected_files_mut().remove(&path);
                        self.tab_manager.current_yaml = None;
                        self.tab_manager.current_markdown = String::new();
                        self.tab_manager.invalidate_heading_ids_cache();
                        self.tab_manager.toc.clear();
                        self.tab_manager.scroll_to_header_id = None;

                        // Log the failure to the background log.
                        if let Ok(mut mgr) = self.background_manager.lock() {
                            mgr.push_log(BackgroundLogEntry::new(
                                LogCategory::Watcher,
                                format!("Failed to load file {}: {}", path.display(), err),
                            ));
                        }
                    }
                }
            }
        }
    }

    pub fn handle_mcp_auth_event(&mut self, ev: McpAuthEvent) {
        use crate::agent::tools::manager::{
            self as tool_manager, ToolErrorKind, ToolGroupError, ToolGroupId,
        };
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
                        tool_manager::record_mcp_error(
                            &ToolGroupId::Mcp(server_name),
                            ToolGroupError::now(ToolErrorKind::Authentication, msg),
                        );
                    }
                }
            }
        }
    }

    pub fn handle_file_selection(&mut self) {
        if let Some(selected_path) = self.selection.selected_file()
            && self.tab_manager.loaded_path.as_ref() != Some(selected_path)
            && self.pending_file_load.as_ref() != Some(selected_path)
        {
            self.pending_file_load = Some(selected_path.clone());
            let tx = self.tx.clone();
            let path = selected_path.clone();
            std::thread::spawn(move || {
                let content = crate::utils::read_text_file(&path).map_err(|e| e.to_string());
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
