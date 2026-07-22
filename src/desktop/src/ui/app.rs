//! Root egui `App` struct — owns all application state and wires together background tasks, panels, agent, and dialogs.

use crate::agent::AgentSessionManager;
use crate::background::{BackgroundProcessManager, SharedProcessManager};
use crate::background_task::Task;
use crate::directory_tracker::DirectoryTracker;
use crate::file_processor::FileEventProcessor;
use crate::messages::BackgroundMessage;
use crate::tag_manager::TagManager;
use crate::ui::dialog_manager::DialogManager;
use crate::ui::panel_layout::PanelLayout;
use crate::ui::panels::{
    show_bottom_panel, show_center_panel, show_left_panel, show_right_panel, show_top_panel,
};
use crate::ui::selection_manager::SelectionManager;
use crate::ui::tab_manager::TabManager;
use crate::utils::parse_front_matter;
use eframe::egui;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct TreeNode {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub children: BTreeMap<String, TreeNode>,
}

impl TreeNode {
    pub fn new(name: String, path: PathBuf, is_dir: bool) -> Self {
        Self {
            name,
            path,
            is_dir,
            children: BTreeMap::new(),
        }
    }
}

#[derive(Clone)]
pub struct ToCEntry {
    pub title: String,
    pub level: u32,
    pub id: egui::Id,
}

pub struct FastMdApp {
    pub content_libraries: Vec<crate::config::ContentLibrary>,
    pub rx: Receiver<BackgroundMessage>,
    pub tx: std::sync::mpsc::Sender<BackgroundMessage>,
    pub file_event_bus: crate::file_events::Bus<crate::file_events::FileEvent>,
    pub file_event_reader: Option<crate::file_events::BusReader<crate::file_events::FileEvent>>,
    pub file_processor: FileEventProcessor,
    pub tag_manager: TagManager,
    pub directory_tracker: DirectoryTracker,
    pub layout: PanelLayout,
    pub selection: SelectionManager,
    pub tab_manager: TabManager,

    pub _watcher: Option<notify::RecommendedWatcher>,

    /// Agent session manager - encapsulates all agent state and lifecycle.
    pub agent: AgentSessionManager,
    /// Dialog manager - owns all modal state and rendering.
    pub dialogs: DialogManager,
    pub submit_prompt: Option<String>,
    pub editor_state: crate::editor::EditorState,
    pub inline_editor_enabled: bool,
    pub background_manager: SharedProcessManager,
    pub config: crate::config::AppConfig,
}

impl FastMdApp {
    pub fn content_libraries(&self) -> &[crate::config::ContentLibrary] {
        &self.content_libraries
    }

    pub fn content_libraries_mut(&mut self) -> &mut Vec<crate::config::ContentLibrary> {
        &mut self.content_libraries
    }

    pub fn file_processor(&self) -> &FileEventProcessor {
        &self.file_processor
    }

    pub fn file_processor_mut(&mut self) -> &mut FileEventProcessor {
        &mut self.file_processor
    }

    pub fn tags(&self) -> &TagManager {
        &self.tag_manager
    }

    pub fn tags_mut(&mut self) -> &mut TagManager {
        &mut self.tag_manager
    }

    pub fn layout(&self) -> &PanelLayout {
        &self.layout
    }

    pub fn layout_mut(&mut self) -> &mut PanelLayout {
        &mut self.layout
    }

    pub fn selection(&self) -> &SelectionManager {
        &self.selection
    }

    pub fn selection_mut(&mut self) -> &mut SelectionManager {
        &mut self.selection
    }

    pub fn tabs(&self) -> &TabManager {
        &self.tab_manager
    }

    pub fn tabs_mut(&mut self) -> &mut TabManager {
        &mut self.tab_manager
    }

    pub fn agent(&self) -> &AgentSessionManager {
        &self.agent
    }

    pub fn agent_mut(&mut self) -> &mut AgentSessionManager {
        &mut self.agent
    }

    pub fn dialogs(&self) -> &DialogManager {
        &self.dialogs
    }

    pub fn dialogs_mut(&mut self) -> &mut DialogManager {
        &mut self.dialogs
    }

    pub fn editor(&self) -> &crate::editor::EditorState {
        &self.editor_state
    }

    pub fn editor_mut(&mut self) -> &mut crate::editor::EditorState {
        &mut self.editor_state
    }

    pub fn config(&self) -> &crate::config::AppConfig {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut crate::config::AppConfig {
        &mut self.config
    }

    pub fn submit_prompt(&self) -> &Option<String> {
        &self.submit_prompt
    }

    pub fn submit_prompt_mut(&mut self) -> &mut Option<String> {
        &mut self.submit_prompt
    }

    pub fn inline_editor_enabled(&self) -> bool {
        self.inline_editor_enabled
    }

    /// Drain pending `FileEvent`s from the bus and update.
    ///
    /// Returns `true` if any event was processed, so callers can
    /// schedule a follow-up UI repaint.
    fn process_file_events(&mut self) -> bool {
        use crate::file_events::FileEventKind;

        let mut changed = false;
        let mut needs_rebuild = false;

        // Let DirectoryTracker consume directory events from its own subscriber.
        if self.directory_tracker.process_events() {
            changed = true;
        }

        if let Some(reader) = &self.file_event_reader {
            while let Ok(event) = reader.try_recv() {
                changed = true;
                match event.kind {
                    FileEventKind::Discovered => {
                        for p in &event.paths {
                            if Self::is_workspace_file(p) {
                                self.file_processor.add_file(p.clone());
                                if let Some(parent) = p.parent() {
                                    let parent = parent.to_path_buf();
                                    self.file_processor.add_dir(parent);
                                }
                            }
                        }
                    }
                    FileEventKind::Updated => {
                        for p in &event.paths {
                            if self.tab_manager.loaded_path.as_ref() == Some(p)
                                && !self.editor_state.is_open
                            {
                                self.tab_manager.loaded_path = None;
                            }
                        }
                    }
                    FileEventKind::Removed => {
                        for p in &event.paths {
                            self.file_processor.remove_file(p);
                            if self.tab_manager.loaded_path.as_ref() == Some(p) {
                                self.tab_manager.loaded_path = None;
                            }
                            self.tag_manager.remove_file(p);
                        }
                        needs_rebuild = true;
                    }
                    FileEventKind::DirDiscovered => {
                        for p in &event.paths {
                            self.file_processor.add_dir(p.clone());
                        }
                    }
                    FileEventKind::DirRemoved => {
                        for p in &event.paths {
                            self.file_processor.remove_dir(p);
                        }
                    }
                }
            }
        }

        if needs_rebuild {
            self.tag_manager.rebuild();
        }

        changed
    }

    /// Returns `true` if `path` is a user-editable workspace file
    /// (i.e. one that should appear in the directory tree).
    ///
    /// The current rule is: markdown (`.md` / `.markdown`) and
    /// plain-text (`.txt`). PDFs and images are inputs to the
    /// PDF-converter and image-vision workers and stay out of the
    /// tree. If we ever want to surface other text types
    /// (e.g. `.org`, `.adoc`), add them here.
    fn is_workspace_file(path: &std::path::Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| {
                let lower = e.to_lowercase();
                lower == "md" || lower == "markdown" || lower == "txt"
            })
            .unwrap_or(false)
    }

    pub fn new(cc: &eframe::CreationContext<'_>, mut config: crate::config::AppConfig) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.window_fill = egui::Color32::from_rgb(9, 9, 11);
        visuals.panel_fill = egui::Color32::from_rgb(9, 9, 11);
        visuals.selection.bg_fill = egui::Color32::from_rgb(99, 102, 241);
        visuals.window_rounding = 8.0.into();
        visuals.widgets.noninteractive.rounding = 4.0.into();
        visuals.widgets.inactive.rounding = 4.0.into();
        visuals.widgets.hovered.rounding = 4.0.into();
        visuals.widgets.active.rounding = 4.0.into();

        let bright_text = egui::Color32::from_gray(210);
        visuals.widgets.noninteractive.fg_stroke.color = bright_text;
        visuals.widgets.inactive.fg_stroke.color = bright_text;
        visuals.widgets.active.fg_stroke.color = egui::Color32::WHITE;
        visuals.widgets.hovered.fg_stroke.color = egui::Color32::WHITE;
        cc.egui_ctx.set_visuals(visuals);

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
                let mut found = false;
                for lib in &config.content_libraries {
                    if lib.root_folder == path_str {
                        found = true;
                        break;
                    }
                }
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

        let background_task = Task::new(config.clone());
        let file_processor = FileEventProcessor::new(background_task.file_event_bus.subscribe());
        let background_manager = Arc::new(Mutex::new(BackgroundProcessManager::new()));
        let inline_editor_enabled = config.inline_editor_enabled;

        let mut batch_dialog_config = crate::batch::types::BatchDialogConfig::default();
        batch_dialog_config.available_dirs = config
            .content_libraries
            .iter()
            .map(|lib| PathBuf::from(&lib.root_folder))
            .collect();
        let mut dialogs = DialogManager::new();
        dialogs.batch_dialog_config = batch_dialog_config;

        let event_bus = background_task.file_event_bus;
        let dir_tracker = DirectoryTracker::new(event_bus.subscribe());
        Self {
            content_libraries: config.content_libraries.clone(),
            rx: background_task.rx,
            tx: background_task.tx,
            file_event_reader: Some(event_bus.subscribe()),
            file_event_bus: event_bus,
            file_processor,
            directory_tracker: dir_tracker,
            tag_manager: TagManager::new(),
            layout: PanelLayout::new(),
            selection: SelectionManager::new(),
            tab_manager: TabManager::new(),
            _watcher: None,
            agent: AgentSessionManager::new(config.clone()),
            dialogs,
            submit_prompt: None,
            editor_state: crate::editor::EditorState::default(),
            inline_editor_enabled,
            background_manager,
            config,
        }
    }

    /// Purpose: Build a `FastMdApp` with all UI state cleared and no background channels.
    /// Inputs: None.
    /// Outputs: `FastMdApp` with every collection empty and every optional set to `None`.
    /// Purity: Constructs a new value; no side effects.
    /// Preconditions: None.
    /// Postconditions: Caller still owns a usable `Sender<BackgroundMessage>` paired with `rx`.
    pub fn empty_state(config: crate::config::AppConfig) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            content_libraries: Vec::new(),
            rx,
            tx,
            file_event_bus: crate::file_events::Bus::new(),
            file_event_reader: None,
            file_processor: FileEventProcessor::new(crate::file_events::BusReader::new(
                std::sync::mpsc::channel().1,
            )),
            tag_manager: TagManager::new(),
            layout: PanelLayout::new(),
            selection: SelectionManager::new(),
            tab_manager: TabManager::new(),
            _watcher: None,
            agent: AgentSessionManager::new(config.clone()),
            dialogs: DialogManager::new(),
            submit_prompt: None,
            editor_state: crate::editor::EditorState::default(),
            inline_editor_enabled: true,
            background_manager: Arc::new(Mutex::new(BackgroundProcessManager::new())),
            directory_tracker: DirectoryTracker::new(crate::file_events::BusReader::new(
                std::sync::mpsc::channel().1,
            )),
            config,
        }
    }

    /// Purpose: Submits a prompt to the agent and starts a new session, taking ownership of all relevant state.
    /// Inputs: `prompt` - the prompt text to send to the agent.
    /// Outputs: None.
    /// Purity: Impure (mutates self, spawns the agent thread).
    /// Preconditions: `prompt` should be non-empty.
    /// Postconditions: agent state reflects running, cancel flag is set, agent thread launched.
    pub fn start_agent_session(&mut self, prompt: String) {
        self.agent.start_session(
            self.tx.clone(),
            prompt,
            self.selection.selected_file().cloned(),
            self.selection.selected_dir().cloned(),
            self.selection.selected_files().clone(),
            self.file_event_bus.clone(),
        );
        self.agent.set_show_results(true);
    }
}

/// Purpose: Generates the markdown formatting prompt with a dynamic date.
/// Inputs: `date_str` - The current date string in RFC3339 format.
/// Outputs: A String containing the complete formatting prompt.
/// Purity: Pure.
/// Preconditions: None.
/// Postconditions: Returns a valid prompt string containing the provided date.
pub fn generate_format_prompt(date_str: &str) -> String {
    format!(
        "Format the current document into correct markdown and use this template for the yaml front matter. Focus ONLY on the currently active file, and DO NOT use list_files or search for other files.\n```yaml\n---\ntitle: A brief title\nsummary: A three sentence summary of the contents\ntags: [\"tag1\",\"tag2\"]\nheader-date: {}\n---\n```",
        date_str
    )
}

impl eframe::App for FastMdApp {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Ok(mgr) = self.background_manager.lock() {
            let log_path = crate::config::get_config_path()
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .join("logs/background-process.log");
            let _ = mgr.save_logs(&log_path);
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_ui(ctx);
    }
}

impl FastMdApp {
    pub fn update_ui(&mut self, ctx: &egui::Context) {
        // Process file events from the bus. This is the directory
        // tree's "consumer" of the file-event bus — the
        // tag manager/indexer has its own consumers in
        // `background_task.rs`.
        if self.process_file_events() {
            // At least one tab needs to reload because its file
            // changed on disk. Force a repaint so the change
            // shows up immediately.
            ctx.request_repaint();
        }

        // Handle background messages
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                BackgroundMessage::FileParsed { path, tags } => {
                    self.tag_manager.add_tags(path.clone(), tags);
                    self.file_processor.add_file(path);
                }
                BackgroundMessage::DirParsed { path } => {
                    self.file_processor.add_dir(path);
                }
                BackgroundMessage::Finished(watcher) => {
                    self._watcher = Some(watcher);
                    self.file_processor.indexing_finished = true;
                    self.tag_manager.rebuild();
                }
                BackgroundMessage::FinishedWithoutWatcher => {
                    self.file_processor.indexing_finished = true;
                    self.tag_manager.rebuild();
                }
                BackgroundMessage::FileModified { path, tags } => {
                    self.tag_manager.add_tags(path.clone(), tags);
                    self.file_processor.add_file(path.clone());
                    self.tag_manager.rebuild();
                    if self.tab_manager.loaded_path.as_ref() == Some(&path) {
                        self.tab_manager.loaded_path = None;
                    }
                }
                BackgroundMessage::FileDeleted { path } => {
                    self.file_processor.remove_file(&path);
                    self.tag_manager.remove_file(&path);
                    self.tag_manager.rebuild();
                    if self.selection.selected_file().is_some_and(|p| p == &path) {
                        *self.selection.selected_file_mut() = None;
                        self.tab_manager.current_yaml = None;
                        self.tab_manager.current_markdown = String::new();
                        self.tab_manager.toc.clear();
                    }
                    self.selection.selected_files_mut().remove(&path);
                    if self.tab_manager.loaded_path.as_ref() == Some(&path) {
                        self.tab_manager.loaded_path = None;
                    }
                }
                // Agent messages are delegated to AgentSessionManager
                BackgroundMessage::AgentStatus(_)
                | BackgroundMessage::AgentThinking(_)
                | BackgroundMessage::AgentResponse(_)
                | BackgroundMessage::AgentFinished(_)
                | BackgroundMessage::AgentFailed(_)
                | BackgroundMessage::AgentTokenUsage(_) => {
                    self.agent.handle_background_message(msg);
                }
                BackgroundMessage::LogEntry(entry) => {
                    if let Ok(mut mgr) = self.background_manager.lock() {
                        mgr.push_log(entry);
                    }
                }
            }
        }

        // Repaint if still indexing
        if !self.file_processor.indexing_finished {
            ctx.request_repaint();
        }

        // Handle file selection and dynamic content loading
        if let Some(selected_path) = self.selection.selected_file() {
            if self.tab_manager.loaded_path.as_ref() != Some(selected_path) {
                if let Ok(content) = std::fs::read_to_string(selected_path) {
                    if let Some((yaml_val, md_content)) = parse_front_matter(&content) {
                        self.tab_manager.current_yaml = Some(yaml_val);
                        self.tab_manager.current_markdown = md_content.to_string();
                    } else {
                        self.tab_manager.current_yaml = None;
                        self.tab_manager.current_markdown = content;
                    }
                    self.tab_manager.loaded_path = Some(selected_path.clone());
                    self.tab_manager.toc =
                        crate::ui::render::build_toc(&self.tab_manager.current_markdown);
                    self.tab_manager.scroll_to_header_id = None;
                }
            }
        }

        // Show inline editor overlay
        let producer = crate::file_events::FileEventProducer::new(&self.file_event_bus);
        if self.editor_state.show(ctx, &producer) {
            // Force reload if we edited the active document
            self.tab_manager.loaded_path = None;
        }

        if self.dialogs.move_dialog_open {
            crate::ui::modals::show_move_modal_dialog(
                &mut self.dialogs,
                &self.content_libraries,
                &self.file_processor,
                &self.file_event_bus,
                ctx,
            );
        }
        if self.dialogs.create_dir_dialog_open {
            crate::ui::modals::show_create_dir_dialog(
                &mut self.dialogs,
                &mut self.file_processor,
                &mut self._watcher,
                &self.file_event_bus,
                ctx,
            );
        }
        if self.dialogs.rename_dialog_open {
            let selection = &mut self.selection;
            crate::ui::modals::show_rename_dialog(
                &mut self.dialogs,
                &self.file_event_bus,
                &mut self.tab_manager.loaded_path,
                &mut selection.selected_file,
                &mut selection.selected_dir,
                &mut self.tab_manager.tabs,
                &mut self.file_processor,
                &mut self.tag_manager,
                &mut selection.expanded_dirs,
                ctx,
            );
        }

        // Show background logs window (separate, not part of DialogManager)
        crate::ui::background_logs::show_background_logs_window(self, ctx);

        // Show batch processing modal
        if self.dialogs.batch_dialog_open {
            let mut dialog_config = self.dialogs.batch_dialog_config.clone();

            // Populate available directories from DirectoryTracker (all known directories, including subdirectories).
            // Preserve selection by path if the previously selected dir still exists in the updated list.
            let prev_selected = dialog_config
                .selected_dir_idx
                .and_then(|i| dialog_config.available_dirs.get(i).cloned());
            dialog_config.available_dirs = self.directory_tracker.dirs_sorted();
            dialog_config.selected_dir_idx = prev_selected
                .as_ref()
                .and_then(|p| dialog_config.available_dirs.iter().position(|d| d == p));

            if let Some(result) =
                crate::batch::dialog::show_batch_modal(self, ctx, &mut dialog_config)
            {
                match result {
                    crate::batch::types::BatchDialogResult::Process(config) => {
                        if self.dialogs.batch_handle.is_none() {
                            let prompt_text = dialog_config
                                .available_prompts
                                .get(dialog_config.selected_prompt_idx.unwrap_or(0))
                                .map(|p| p.content.clone())
                                .unwrap_or_default();

                            let (coordinator, cancel_flag) =
                                crate::batch::coordinator::BatchCoordinator::new(
                                    config,
                                    self.config.clone(),
                                    self.tx.clone(),
                                    self.file_event_bus.clone(),
                                    prompt_text,
                                );
                            let handle = coordinator.execute();
                            self.dialogs.batch_handle = Some(handle);
                            self.dialogs.batch_cancel_flag = Some(cancel_flag);
                        }
                    }
                    crate::batch::types::BatchDialogResult::Cancel => {
                        self.dialogs.batch_dialog_open = false;
                        dialog_config.available_prompts.clear();
                        dialog_config.selected_prompt_idx = None;
                    }
                }
            }
            self.dialogs.batch_dialog_config = dialog_config;
        }

        // Top panel
        show_top_panel(self, ctx);

        // Bottom panel
        show_bottom_panel(self, ctx);

        // Right panel (Table of Contents)
        show_right_panel(self, ctx);

        // Left panel (Directory tree)
        show_left_panel(self, ctx);

        // Center panel (Markdown content or Agent)
        show_center_panel(self, ctx);

        // Handle programmatic prompt submission
        if let Some(prompt) = self.submit_prompt.take() {
            self.start_agent_session(prompt);
        }

        if let Some(handle) = self.dialogs.batch_handle.take() {
            if handle.thread.is_finished() {
                let result = handle.join();
                self.dialogs.batch_cancel_flag = None;
                eprintln!("Batch completed: {:?}", result);
            } else {
                self.dialogs.batch_handle = Some(handle);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::TokenUsageInfo;
    use std::path::PathBuf;

    fn create_test_app() -> FastMdApp {
        FastMdApp::empty_state(crate::config::AppConfig::default())
    }

    #[test]
    fn test_treenode_new() {
        let node = TreeNode::new("Docs".to_string(), PathBuf::from("/docs"), true);
        assert_eq!(node.name, "Docs");
        assert_eq!(node.path, PathBuf::from("/docs"));
        assert!(node.is_dir);
        assert!(node.children.is_empty());
    }

    #[test]
    fn test_tag_manager_tracks_tags_correctly() {
        let mut app = create_test_app();
        app.tag_manager.add_tags(
            PathBuf::from("file1.md"),
            vec!["rust".to_string(), "ui".to_string()],
        );
        app.tag_manager.add_tags(
            PathBuf::from("file2.md"),
            vec!["rust".to_string(), "testing".to_string()],
        );

        assert_eq!(app.tag_manager.all_tags().len(), 3);
        assert!(app.tag_manager.all_tags().contains("rust"));
        assert!(app.tag_manager.all_tags().contains("ui"));
        assert!(app.tag_manager.all_tags().contains("testing"));
    }

    #[test]
    fn test_background_messages_handling() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        let test_file = PathBuf::from("test_doc.md");
        let test_dir = PathBuf::from("test_dir");

        // 1. FileParsed
        app.tx
            .send(BackgroundMessage::FileParsed {
                path: test_file.clone(),
                tags: vec!["tag1".to_string()],
            })
            .unwrap();

        // 2. DirParsed
        app.tx
            .send(BackgroundMessage::DirParsed {
                path: test_dir.clone(),
            })
            .unwrap();

        // 3. FinishedWithoutWatcher
        app.tx
            .send(BackgroundMessage::FinishedWithoutWatcher)
            .unwrap();

        // 4. Agent Status & Response
        app.tx
            .send(BackgroundMessage::AgentStatus("Processing...".to_string()))
            .unwrap();
        app.tx
            .send(BackgroundMessage::AgentThinking(
                "Thinking step".to_string(),
            ))
            .unwrap();
        app.tx
            .send(BackgroundMessage::AgentResponse("Done result".to_string()))
            .unwrap();

        let _ = ctx.run(Default::default(), |ctx| {
            app.update_ui(ctx);
        });

        assert!(app.file_processor.all_files.contains(&test_file));
        assert!(app.file_processor.all_dirs.contains(&test_dir));
        assert!(app.file_processor.indexing_finished);
        assert_eq!(app.agent.state().status, "Processing...");
        assert_eq!(app.agent.state().thinking, "Thinking step");
        assert_eq!(app.agent.state().response, "Done result");
    }

    #[test]
    fn test_background_message_file_modified_and_deleted() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        let file_path = PathBuf::from("modified_file.md");

        app.file_processor.all_files.push(file_path.clone());
        *app.selection.selected_file_mut() = Some(file_path.clone());
        app.selection.selected_files_mut().insert(file_path.clone());
        app.tab_manager.loaded_path = Some(file_path.clone());

        // File modified message
        app.tx
            .send(BackgroundMessage::FileModified {
                path: file_path.clone(),
                tags: vec!["updated".to_string()],
            })
            .unwrap();

        let _ = ctx.run(Default::default(), |ctx| {
            app.update_ui(ctx);
        });

        assert!(app.tab_manager.loaded_path.is_none()); // Trigger reload

        // File deleted message
        app.tx
            .send(BackgroundMessage::FileDeleted {
                path: file_path.clone(),
            })
            .unwrap();

        let _ = ctx.run(Default::default(), |ctx| {
            app.update_ui(ctx);
        });

        assert!(!app.file_processor.all_files.contains(&file_path));
        assert!(app.selection.selected_file().is_none());
        assert!(!app.selection.selected_files().contains(&file_path));
    }

    #[test]
    fn test_agent_failure_and_finish_messages() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();

        app.tx
            .send(BackgroundMessage::AgentFailed(
                "Network timeout".to_string(),
            ))
            .unwrap();

        let _ = ctx.run(Default::default(), |ctx| {
            app.update_ui(ctx);
        });

        assert_eq!(app.agent.state().status, "Error: Network timeout");
        assert!(!app.agent.state().running);

        app.tx
            .send(BackgroundMessage::AgentFinished(vec![
                serde_json::json!({"ok": true}),
            ]))
            .unwrap();

        let _ = ctx.run(Default::default(), |ctx| {
            app.update_ui(ctx);
        });

        assert!(!app.agent.state().running);
        assert!(app.agent.state().history.is_some());
    }

    #[test]
    fn test_agent_token_usage_message_accumulates() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();

        // First turn: small context, no cached or reasoning tokens.
        app.tx
            .send(BackgroundMessage::AgentTokenUsage(TokenUsageInfo {
                prompt_tokens: 100,
                completion_tokens: 20,
                total_tokens: 120,
                cached_tokens: None,
                reasoning_tokens: None,
            }))
            .unwrap();

        let _ = ctx.run(Default::default(), |ctx| {
            app.update_ui(ctx);
        });

        assert_eq!(
            app.agent
                .state()
                .token_usage
                .as_ref()
                .unwrap()
                .prompt_tokens,
            100
        );
        assert_eq!(
            app.agent.state().total_usage.prompt_tokens,
            100,
            "prompt_tokens should track the peak seen so far"
        );
        assert_eq!(app.agent.state().total_usage.completion_tokens, 20);
        assert_eq!(app.agent.state().total_usage.total_tokens, 120);
        assert_eq!(app.agent.state().total_usage.cached_tokens, Some(0));
        assert_eq!(app.agent.state().total_usage.reasoning_tokens, Some(0));

        // Second turn: context grew, completion + reasoning added.
        app.tx
            .send(BackgroundMessage::AgentTokenUsage(TokenUsageInfo {
                prompt_tokens: 250, // larger than first turn
                completion_tokens: 30,
                total_tokens: 280,
                cached_tokens: Some(50),
                reasoning_tokens: Some(5),
            }))
            .unwrap();

        let _ = ctx.run(Default::default(), |ctx| {
            app.update_ui(ctx);
        });

        assert_eq!(
            app.agent
                .state()
                .token_usage
                .as_ref()
                .unwrap()
                .prompt_tokens,
            250
        );
        assert_eq!(
            app.agent.state().total_usage.prompt_tokens,
            250,
            "peak should rise with the larger turn"
        );
        assert_eq!(app.agent.state().total_usage.completion_tokens, 50);
        assert_eq!(app.agent.state().total_usage.total_tokens, 400);
        assert_eq!(app.agent.state().total_usage.cached_tokens, Some(50));
        assert_eq!(app.agent.state().total_usage.reasoning_tokens, Some(5));

        // Third turn: smaller context — peak should NOT shrink.
        app.tx
            .send(BackgroundMessage::AgentTokenUsage(TokenUsageInfo {
                prompt_tokens: 80,
                completion_tokens: 10,
                total_tokens: 90,
                cached_tokens: None,
                reasoning_tokens: None,
            }))
            .unwrap();

        let _ = ctx.run(Default::default(), |ctx| {
            app.update_ui(ctx);
        });

        assert_eq!(
            app.agent.state().total_usage.prompt_tokens,
            250,
            "peak prompt size must not regress"
        );
        assert_eq!(app.agent.state().total_usage.completion_tokens, 60);
        assert_eq!(app.agent.state().total_usage.cached_tokens, Some(50));
        assert_eq!(app.agent.state().total_usage.reasoning_tokens, Some(5));
    }

    // -- process_file_events: tab reload on file Updated --

    #[test]
    fn test_process_file_events_updated_resets_loaded_path() {
        // When the bus reports a Discovered/Updated event for a
        // file that is currently loaded into the renderer, the
        // next frame must reload it from disk. We model "currently
        // loaded" by setting `loaded_path = Some(path)` while
        // leaving `selected_file` alone — `load_selected_file`
        // (the actual reload driver) only fires when
        // `selected_file.is_some() && loaded_path != selected_file`.
        let mut app = create_test_app();
        let path = PathBuf::from("/tmp/active_doc.md");

        *app.selection.selected_file_mut() = Some(path.clone());
        app.tab_manager.loaded_path = Some(path.clone());
        app.file_processor.all_files.push(path.clone());

        // Subscribe a reader to the bus so we can publish into it
        // and have process_file_events pick up the event.
        app.file_event_reader = Some(app.file_event_bus.subscribe());

        // Use a separate clone of the bus to publish; both clones
        // share the same subscriber list.
        let publisher = app.file_event_bus.clone();
        publisher.publish(crate::file_events::FileEvent::updated_one(path.clone()));

        let changed = app.process_file_events();
        assert!(changed, "process_file_events should report a change");
        assert!(
            app.tab_manager.loaded_path.is_none(),
            "loaded_path must be cleared so the renderer reloads on the next frame"
        );
        // selected_file must be preserved so the renderer knows
        // what to render.
        assert_eq!(app.selection.selected_file(), Some(&path));
    }

    #[test]
    fn test_process_file_events_updated_preserves_loaded_when_editor_open() {
        // If the inline editor is open on the file, the user's
        // unsaved changes must not be clobbered by an external
        // update. The reload should be skipped.
        let mut app = create_test_app();
        let path = PathBuf::from("/tmp/being_edited.md");

        *app.selection.selected_file_mut() = Some(path.clone());
        app.tab_manager.loaded_path = Some(path.clone());
        app.file_processor.all_files.push(path.clone());
        app.editor_state.open(&path, "old content");
        assert!(app.editor_state.is_open);

        app.file_event_reader = Some(app.file_event_bus.subscribe());
        let publisher = app.file_event_bus.clone();
        publisher.publish(crate::file_events::FileEvent::updated_one(path.clone()));

        let _ = app.process_file_events();
        assert!(
            app.tab_manager.loaded_path.is_some(),
            "loaded_path must NOT be cleared while the inline editor is open"
        );
    }

    #[test]
    fn test_process_file_events_removed_clears_loaded_path() {
        // Sanity check: a Removed event still clears `loaded_path`
        // regardless of whether the editor is open. (We accept
        // losing unsaved edits in the editor if the file was
        // deleted out from under us — that's the user's action.)
        let mut app = create_test_app();
        let path = PathBuf::from("/tmp/gone.md");

        *app.selection.selected_file_mut() = Some(path.clone());
        app.tab_manager.loaded_path = Some(path.clone());
        app.file_processor.all_files.push(path.clone());

        app.file_event_reader = Some(app.file_event_bus.subscribe());
        let publisher = app.file_event_bus.clone();
        publisher.publish(crate::file_events::FileEvent::removed_one(path.clone()));

        let _ = app.process_file_events();
        assert!(app.tab_manager.loaded_path.is_none());
    }

    #[test]
    fn test_process_file_events_filters_out_non_workspace_files() {
        // PDFs and images are inputs to the PDF-converter and
        // image-vision workers. They still flow through the bus
        // (so the workers see them) but they must NOT be added
        // to `all_files` or `all_dirs`, which feed the directory
        // tree. A directory that contains only PDFs / images
        // must not appear in the tree either.
        let mut app = create_test_app();

        let pdf = PathBuf::from("/tmp/lib/doc.pdf");
        let img = PathBuf::from("/tmp/lib/photo.png");
        let md = PathBuf::from("/tmp/lib/notes.md");
        let pdf_only_dir = PathBuf::from("/tmp/pdf_only");
        let pdf_in_pdf_only_dir = PathBuf::from("/tmp/pdf_only/thing.pdf");

        app.file_event_reader = Some(app.file_event_bus.subscribe());
        let publisher = app.file_event_bus.clone();
        publisher.publish(crate::file_events::FileEvent::discovered_one(pdf.clone()));
        publisher.publish(crate::file_events::FileEvent::discovered_one(img.clone()));
        publisher.publish(crate::file_events::FileEvent::discovered_one(md.clone()));
        publisher.publish(crate::file_events::FileEvent::discovered_one(
            pdf_in_pdf_only_dir.clone(),
        ));

        let _ = app.process_file_events();

        // The markdown file should be in the tree and its
        // parent should be in `all_dirs`.
        assert!(
            app.file_processor.all_files.contains(&md),
            "markdown files must appear in the workspace tree"
        );
        assert!(
            app.file_processor
                .all_dirs
                .contains(&PathBuf::from("/tmp/lib")),
            "directories containing workspace files must appear in the tree"
        );

        // The PDF and image must NOT be in the tree, even though
        // they were published to the bus (the converters need
        // them).
        assert!(
            !app.file_processor.all_files.contains(&pdf),
            "PDFs must not appear in the workspace tree"
        );
        assert!(
            !app.file_processor.all_files.contains(&img),
            "images must not appear in the workspace tree"
        );

        // A directory that contains only a PDF must not be added
        // to `all_dirs`.
        assert!(
            !app.file_processor.all_dirs.contains(&pdf_only_dir),
            "directories that contain only non-workspace files must not appear in the tree"
        );
    }

    #[test]
    fn test_is_workspace_file_predicate() {
        // Direct unit test for the predicate that drives the
        // filter. Markdown (case-insensitive) and plain text
        // are workspace files; everything else (PDFs, images,
        // no extension) is not.
        assert!(FastMdApp::is_workspace_file(&PathBuf::from("/a/b/note.md")));
        assert!(FastMdApp::is_workspace_file(&PathBuf::from("/a/b/note.MD")));
        assert!(FastMdApp::is_workspace_file(&PathBuf::from(
            "/a/b/note.markdown"
        )));
        assert!(FastMdApp::is_workspace_file(&PathBuf::from(
            "/a/b/note.txt"
        )));
        assert!(!FastMdApp::is_workspace_file(&PathBuf::from(
            "/a/b/doc.pdf"
        )));
        assert!(!FastMdApp::is_workspace_file(&PathBuf::from(
            "/a/b/photo.png"
        )));
        assert!(!FastMdApp::is_workspace_file(&PathBuf::from(
            "/a/b/photo.jpg"
        )));
        assert!(!FastMdApp::is_workspace_file(&PathBuf::from(
            "/a/b/no_extension"
        )));
    }

    // -- process_file_events: performance invariants (regression) --

    #[test]
    fn test_process_file_events_does_not_set_left_panel_dirty() {
        // Regression: `process_file_events` used to set
        // `left_panel_dirty = true` on every event, which made
        // `show_left_panel` run `calc_max_width` (a recursive
        // O(n) text-layout pass) once per event during the
        // initial scan. With many files this saturated the UI
        // thread and the app felt unresponsive on startup. The
        // fix: the bus consumer no longer touches
        // `left_panel_dirty`. The width is calculated once,
        // when indexing finishes, in `show_left_panel`.
        let mut app = create_test_app();
        assert!(!app.layout.left_panel_dirty);

        app.file_event_reader = Some(app.file_event_bus.subscribe());
        let publisher = app.file_event_bus.clone();
        publisher.publish(crate::file_events::FileEvent::discovered_one(
            PathBuf::from("/lib/notes.md"),
        ));
        publisher.publish(crate::file_events::FileEvent::discovered_one(
            PathBuf::from("/lib/extra.md"),
        ));
        publisher.publish(crate::file_events::FileEvent::updated_one(PathBuf::from(
            "/lib/notes.md",
        )));

        let _ = app.process_file_events();
        assert!(
            !app.layout.left_panel_dirty,
            "process_file_events must not set left_panel_dirty — the width is \
             calculated once when indexing finishes, not per bus event"
        );
    }

    #[test]
    fn test_process_file_events_rebuild_only_on_removal() {
        // `rebuild` is O(n) in the tag manager. Calling it on
        // every bus event (Discovered or Updated) made the UI
        // thread do unnecessary work during the initial scan.
        // The `FileParsed` handler keeps tags up to date
        // incrementally, so rebuild is only needed when a file
        // actually leaves (`Removed`).
        let mut app = create_test_app();

        // Pre-populate tag manager so the tag exists.
        app.tag_manager
            .add_tags(PathBuf::from("/lib/notes.md"), vec!["work".to_string()]);
        app.file_processor
            .all_files
            .push(PathBuf::from("/lib/notes.md"));

        // A `Removed` event must trigger `rebuid`, which
        // evicts the file's tags.
        app.file_event_reader = Some(app.file_event_bus.subscribe());
        app.file_event_bus
            .publish(crate::file_events::FileEvent::removed_one(PathBuf::from(
                "/lib/notes.md",
            )));
        let _ = app.process_file_events();
        assert!(
            !app.tag_manager.all_tags().contains("work"),
            "Removed events must trigger rebuild so stale tags are evicted"
        );

        // A `Discovered` event must NOT call rebuild (which
        // would clear all_tags and lose the tag we just
        // added).
        app.tag_manager
            .add_tags(PathBuf::from("/lib/other.md"), vec!["keep".to_string()]);
        app.file_event_bus
            .publish(crate::file_events::FileEvent::discovered_one(
                PathBuf::from("/lib/other.md"),
            ));
        let _ = app.process_file_events();
        assert!(
            app.tag_manager.all_tags().contains("keep"),
            "Discovered events must NOT call rebuild — the FileParsed path \
             updates all_tags incrementally"
        );
    }
}
