use crate::background_task::Task;
use crate::messages::BackgroundMessage;
use crate::ui::panels::{show_bottom_panel, show_center_panel, show_left_panel, show_right_panel, show_top_panel};
use crate::ui::modals::{show_create_dir_modal, show_move_modal, show_rename_modal};
use crate::utils::parse_front_matter;
use eframe::egui;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use crate::background::{BackgroundProcessManager, SharedProcessManager};
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
    pub all_files: Vec<PathBuf>,
    pub all_dirs: Vec<PathBuf>,
    pub file_tags: BTreeMap<PathBuf, Vec<String>>,
    pub all_tags: BTreeSet<String>,
    pub selected_tag: Option<String>,
    pub indexing_finished: bool,
    pub indexing_finished_handled: bool,
    pub left_panel_width: Option<f32>,

    pub selected_file: Option<PathBuf>,
    pub selected_files: HashSet<PathBuf>,
    pub selected_dir: Option<PathBuf>,
    pub expanded_dirs: HashSet<PathBuf>,

    pub loaded_path: Option<PathBuf>,
    pub current_yaml: Option<serde_yaml::Value>,
    pub current_markdown: String,

    pub tabs: Vec<PathBuf>,

    pub move_dialog_open: bool,
    pub file_to_move: Option<PathBuf>,
    pub selected_move_folder: Option<PathBuf>,

    pub create_dir_dialog_open: bool,
    pub create_dir_parent: Option<PathBuf>,
    pub create_dir_name: String,

    pub rename_dialog_open: bool,
    pub file_to_rename: Option<PathBuf>,
    pub rename_new_name: String,

    pub command_input: String,
    pub toc: Vec<ToCEntry>,
    pub scroll_to_header_id: Option<egui::Id>,
    pub _watcher: Option<notify::RecommendedWatcher>,

    pub show_agent_results: bool,
    pub agent_running: bool,
    pub agent_status: String,
    pub agent_thinking: String,
    pub agent_response: String,
    pub agent_scroll_to_id: Option<egui::Id>,
    pub agent_cancel_flag: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    pub agent_history: Option<Vec<serde_json::Value>>,
    pub left_panel_reset_count: u32,
    pub submit_prompt: Option<String>,
    pub editor_state: crate::editor::EditorState,
    pub inline_editor_enabled: bool,
    pub background_manager: SharedProcessManager,
    pub show_background_logs: bool,
    pub config: crate::config::AppConfig,
}

impl FastMdApp {
    fn rebuild_tags(&mut self) {
        self.all_tags.clear();
        for tags in self.file_tags.values() {
            for tag in tags {
                self.all_tags.insert(tag.clone());
            }
        }
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
                let mut path_str = path.canonicalize().unwrap_or(path).to_string_lossy().to_string();
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
                    config.content_libraries.push(crate::config::ContentLibrary {
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
            config.content_libraries.push(crate::config::ContentLibrary {
                root_folder: path_str,
                name: "Workspace".to_string(),
                kind: "text".to_string(),
                readonly: false,
                priority: 0,
            });
        }

        let background_task = Task::new(config.clone());
        let inline_editor_enabled = config.inline_editor_enabled;
        let background_manager = Arc::new(Mutex::new(BackgroundProcessManager::new()));

        Self {
            content_libraries: config.content_libraries.clone(),
            rx: background_task.rx,
            tx: background_task.tx,
            all_files: Vec::new(),
            all_dirs: Vec::new(),
            file_tags: BTreeMap::new(),
            all_tags: BTreeSet::new(),
            selected_tag: None,
            indexing_finished: false,
            indexing_finished_handled: false,
            left_panel_width: None,
            selected_file: None,
            selected_files: HashSet::new(),
            selected_dir: None,
            expanded_dirs: HashSet::new(),
            loaded_path: None,
            current_yaml: None,
            current_markdown: String::new(),
            tabs: Vec::new(),
            move_dialog_open: false,
            file_to_move: None,
            selected_move_folder: None,
            create_dir_dialog_open: false,
            create_dir_parent: None,
            create_dir_name: String::new(),
            rename_dialog_open: false,
            file_to_rename: None,
            rename_new_name: String::new(),
            command_input: String::new(),
            toc: Vec::new(),
            scroll_to_header_id: None,
            _watcher: None,
            show_agent_results: false,
            agent_running: false,
            agent_status: String::new(),
            agent_thinking: String::new(),
            agent_response: String::new(),
            agent_scroll_to_id: None,
            agent_cancel_flag: None,
            agent_history: None,
            left_panel_reset_count: 0,
            submit_prompt: None,
            editor_state: crate::editor::EditorState::default(),
            inline_editor_enabled,
            background_manager,
            show_background_logs: false,
            config,
        }
    }
}

impl eframe::App for FastMdApp {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Ok(mgr) = self.background_manager.lock() {
            let log_path = crate::config::get_config_path().parent().unwrap().join("logs/background-process.log");
            let _ = mgr.save_logs(&log_path);
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle background messages
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                BackgroundMessage::FileParsed { path, tags } => {
                    self.file_tags.insert(path.clone(), tags.clone());
                    for tag in tags {
                        self.all_tags.insert(tag);
                    }
                    if !self.all_files.contains(&path) {
                        self.all_files.push(path);
                    }
                }
                BackgroundMessage::DirParsed { path } => {
                    if !self.all_dirs.contains(&path) {
                        self.all_dirs.push(path);
                    }
                }
                BackgroundMessage::Finished(watcher) => {
                    self._watcher = Some(watcher);
                    self.indexing_finished = true;
                    self.rebuild_tags();
                }
                BackgroundMessage::FinishedWithoutWatcher => {
                    self.indexing_finished = true;
                    self.rebuild_tags();
                }
                BackgroundMessage::FileModified { path, tags } => {
                    self.file_tags.insert(path.clone(), tags);
                    if !self.all_files.contains(&path) {
                        self.all_files.push(path.clone());
                    }
                    self.rebuild_tags();
                    if self.loaded_path.as_ref() == Some(&path) {
                        self.loaded_path = None; // Trigger reload
                    }
                }
                BackgroundMessage::FileDeleted { path } => {
                    self.all_files.retain(|p| p != &path);
                    self.file_tags.remove(&path);
                    self.rebuild_tags();
                    if self.selected_file.as_ref() == Some(&path) {
                        self.selected_file = None;
                        self.current_yaml = None;
                        self.current_markdown = String::new();
                        self.toc.clear();
                    }
                    self.selected_files.remove(&path);
                    if self.loaded_path.as_ref() == Some(&path) {
                        self.loaded_path = None;
                    }
                }
                BackgroundMessage::AgentStatus(status) => {
                    self.agent_status = status;
                }
                BackgroundMessage::AgentThinking(thinking) => {
                    self.agent_thinking = thinking;
                }
                BackgroundMessage::AgentResponse(resp) => {
                    self.agent_response = resp;
                }
                BackgroundMessage::AgentFinished(history) => {
                    self.agent_running = false;
                    self.agent_history = Some(history);
                }
                BackgroundMessage::AgentFailed(err) => {
                    self.agent_status = format!("Error: {}", err);
                    self.agent_running = false;
                }
                BackgroundMessage::LogEntry(entry) => {
                    if let Ok(mut mgr) = self.background_manager.lock() {
                        mgr.push_log(entry);
                    }
                }
            }
        }

        // Repaint if still indexing
        if !self.indexing_finished {
            ctx.request_repaint();
        }

        // Handle file selection and dynamic content loading
        if let Some(selected_path) = &self.selected_file {
            if self.loaded_path.as_ref() != Some(selected_path) {
                if let Ok(content) = std::fs::read_to_string(selected_path) {
                    if let Some((yaml_val, md_content)) = parse_front_matter(&content) {
                        self.current_yaml = Some(yaml_val);
                        self.current_markdown = md_content.to_string();
                    } else {
                        self.current_yaml = None;
                        self.current_markdown = content;
                    }
                    self.loaded_path = Some(selected_path.clone());
                    self.toc = crate::ui::render::build_toc(&self.current_markdown);
                    self.scroll_to_header_id = None;
                }
            }
        }

        // Show inline editor overlay
        if self.editor_state.show(ctx) {
            // Force reload if we edited the active document
            self.loaded_path = None;
        }

        // Show modals
        show_move_modal(self, ctx);
        show_create_dir_modal(self, ctx);
        show_rename_modal(self, ctx);
        crate::ui::background_logs::show_background_logs_window(self, ctx);

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
            self.command_input = prompt;
            // The prompt will be submitted next frame, or we can trigger it immediately.
            // But we actually need to replicate the agent trigger logic here.
            self.agent_status = "Initializing agent...".to_string();
            self.agent_thinking.clear();
            if self.agent_history.is_none() || !self.show_agent_results {
                self.agent_response.clear();
                self.agent_history = None;
            } else {
                self.agent_response.push_str(&format!("> **User:** {}\n\n", self.command_input));
            }
            self.show_agent_results = true;
            self.agent_running = true;

            let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            self.agent_cancel_flag = Some(cancel_flag.clone());
            
            crate::agent::run_agent(
                self.config.clone(),
                self.tx.clone(),
                self.selected_file.clone(),
                self.selected_dir.clone(),
                self.selected_files.clone(),
                self.command_input.clone(),
                cancel_flag,
                self.agent_history.clone(),
                self.agent_response.clone(),
            );
            self.command_input.clear();
        }
    }
}