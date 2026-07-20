use crate::background::{BackgroundProcessManager, SharedProcessManager};
use crate::background_task::Task;
use crate::messages::{BackgroundMessage, TokenUsageInfo};
use crate::ui::modals::{show_create_dir_modal, show_move_modal, show_rename_modal};
use crate::ui::panels::{
    show_bottom_panel, show_center_panel, show_left_panel, show_right_panel, show_top_panel,
};
use crate::utils::parse_front_matter;
use eframe::egui;
use std::collections::{BTreeMap, BTreeSet, HashSet};
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
    pub all_files: Vec<PathBuf>,
    pub all_dirs: Vec<PathBuf>,
    pub file_tags: BTreeMap<PathBuf, Vec<String>>,
    pub all_tags: BTreeSet<String>,
    pub selected_tag: Option<String>,
    pub indexing_finished: bool,
    pub indexing_finished_handled: bool,
    pub left_panel_width: Option<f32>,
    pub left_panel_dirty: bool,

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
    /// Token usage from the most recent LLM turn. `prompt_tokens` here is
    /// the size of the full conversation context the model just saw, so
    /// it is what you'd compare against the model's context window.
    pub agent_token_usage: Option<TokenUsageInfo>,
    /// Cumulative token usage across every LLM turn in the current session.
    /// `prompt_tokens` is the peak seen; `completion_tokens` and the
    /// optional detail fields are summed.
    pub agent_total_usage: TokenUsageInfo,
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
        let inline_editor_enabled = config.inline_editor_enabled;
        let background_manager = Arc::new(Mutex::new(BackgroundProcessManager::new()));

        Self {
            content_libraries: config.content_libraries.clone(),
            rx: background_task.rx,
            tx: background_task.tx,
            inline_editor_enabled,
            background_manager,
            show_background_logs: false,
            config,
            ..Self::empty_state()
        }
    }

    /// Purpose: Build a `FastMdApp` with all UI state cleared and no background channels.
    /// Inputs: None.
    /// Outputs: `FastMdApp` with every collection empty and every optional set to `None`.
    /// Purity: Constructs a new value; no side effects.
    /// Preconditions: None.
    /// Postconditions: Caller still owns a usable `Sender<BackgroundMessage>` paired with `rx`.
    pub fn empty_state() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            content_libraries: Vec::new(),
            rx,
            tx,
            all_files: Vec::new(),
            all_dirs: Vec::new(),
            file_tags: BTreeMap::new(),
            all_tags: BTreeSet::new(),
            selected_tag: None,
            indexing_finished: false,
            indexing_finished_handled: false,
            left_panel_width: None,
            left_panel_dirty: true,
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
            agent_token_usage: None,
            agent_total_usage: TokenUsageInfo::default(),
            left_panel_reset_count: 0,
            submit_prompt: None,
            editor_state: crate::editor::EditorState::default(),
            inline_editor_enabled: true,
            background_manager: Arc::new(Mutex::new(BackgroundProcessManager::new())),
            show_background_logs: false,
            config: crate::config::AppConfig::default(),
        }
    }

    /// Purpose: Submits a prompt to the agent and starts a new session, taking ownership of all relevant state.
    /// Inputs: `prompt` - the prompt text to send to the agent.
    /// Outputs: None.
    /// Purity: Impure (mutates self, spawns the agent thread).
    /// Preconditions: `prompt` should be non-empty.
    /// Postconditions: `command_input` is cleared, `agent_running` is set, `agent_cancel_flag` holds a fresh flag, and the agent thread is launched.
    pub fn start_agent_session(&mut self, prompt: String) {
        self.command_input = prompt;
        self.agent_status = "Initializing agent...".to_string();
        self.agent_thinking.clear();
        if self.agent_history.is_none() || !self.show_agent_results {
            self.agent_response.clear();
            self.agent_history = None;
            self.agent_token_usage = None;
            self.agent_total_usage = TokenUsageInfo::default();
        } else {
            self.agent_response
                .push_str(&format!("> **User:** {}\n\n", self.command_input));
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

/// Purpose: Generates the markdown formatting prompt with a dynamic date.
/// Inputs: `date_str` - The current date string in RFC3339 format.
/// Outputs: A String containing the complete formatting prompt.
/// Purity: Pure.
/// Preconditions: None.
/// Postconditions: Returns a valid prompt string containing the provided date.
pub fn generate_format_prompt(date_str: &str) -> String {
    format!("Format the current document into correct markdown and use this template for the yaml front matter. Focus ONLY on the currently active file, and DO NOT use list_files or search for other files.\n```yaml\n---\ntitle: A brief title\nsummary: A three sentence summary of the contents\ntags: [\"tag1\",\"tag2\"]\nheader-date: {}\n---\n```", date_str)
}

impl eframe::App for FastMdApp {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Ok(mgr) = self.background_manager.lock() {
            let log_path = crate::config::get_config_path()
                .parent()
                .unwrap()
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
                BackgroundMessage::AgentTokenUsage(info) => {
                    // Track the peak prompt size across the session so the
                    // operator can see how close the conversation is to the
                    // model's context window.
                    if info.prompt_tokens > self.agent_total_usage.prompt_tokens {
                        self.agent_total_usage.prompt_tokens = info.prompt_tokens;
                    }
                    self.agent_total_usage.completion_tokens = self
                        .agent_total_usage
                        .completion_tokens
                        .saturating_add(info.completion_tokens);
                    self.agent_total_usage.total_tokens = self
                        .agent_total_usage
                        .total_tokens
                        .saturating_add(info.total_tokens);
                    self.agent_total_usage.cached_tokens = Some(
                        self.agent_total_usage
                            .cached_tokens
                            .unwrap_or(0)
                            .saturating_add(info.cached_tokens.unwrap_or(0)),
                    );
                    self.agent_total_usage.reasoning_tokens = Some(
                        self.agent_total_usage
                            .reasoning_tokens
                            .unwrap_or(0)
                            .saturating_add(info.reasoning_tokens.unwrap_or(0)),
                    );
                    self.agent_token_usage = Some(info);
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
            self.start_agent_session(prompt);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_app() -> FastMdApp {
        FastMdApp::empty_state()
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
    fn test_rebuild_tags() {
        let mut app = create_test_app();
        app.file_tags.insert(
            PathBuf::from("file1.md"),
            vec!["rust".to_string(), "ui".to_string()],
        );
        app.file_tags.insert(
            PathBuf::from("file2.md"),
            vec!["rust".to_string(), "testing".to_string()],
        );

        app.rebuild_tags();

        assert_eq!(app.all_tags.len(), 3);
        assert!(app.all_tags.contains("rust"));
        assert!(app.all_tags.contains("ui"));
        assert!(app.all_tags.contains("testing"));
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

        assert!(app.all_files.contains(&test_file));
        assert!(app.all_dirs.contains(&test_dir));
        assert!(app.indexing_finished);
        assert_eq!(app.agent_status, "Processing...");
        assert_eq!(app.agent_thinking, "Thinking step");
        assert_eq!(app.agent_response, "Done result");
    }

    #[test]
    fn test_background_message_file_modified_and_deleted() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        let file_path = PathBuf::from("modified_file.md");

        app.all_files.push(file_path.clone());
        app.selected_file = Some(file_path.clone());
        app.selected_files.insert(file_path.clone());
        app.loaded_path = Some(file_path.clone());

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

        assert!(app.loaded_path.is_none()); // Trigger reload

        // File deleted message
        app.tx
            .send(BackgroundMessage::FileDeleted {
                path: file_path.clone(),
            })
            .unwrap();

        let _ = ctx.run(Default::default(), |ctx| {
            app.update_ui(ctx);
        });

        assert!(!app.all_files.contains(&file_path));
        assert!(app.selected_file.is_none());
        assert!(!app.selected_files.contains(&file_path));
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

        assert_eq!(app.agent_status, "Error: Network timeout");
        assert!(!app.agent_running);

        app.tx
            .send(BackgroundMessage::AgentFinished(vec![
                serde_json::json!({"ok": true}),
            ]))
            .unwrap();

        let _ = ctx.run(Default::default(), |ctx| {
            app.update_ui(ctx);
        });

        assert!(!app.agent_running);
        assert!(app.agent_history.is_some());
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

        assert_eq!(app.agent_token_usage.as_ref().unwrap().prompt_tokens, 100);
        assert_eq!(
            app.agent_total_usage.prompt_tokens, 100,
            "prompt_tokens should track the peak seen so far"
        );
        assert_eq!(app.agent_total_usage.completion_tokens, 20);
        assert_eq!(app.agent_total_usage.total_tokens, 120);
        assert_eq!(app.agent_total_usage.cached_tokens, Some(0));
        assert_eq!(app.agent_total_usage.reasoning_tokens, Some(0));

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

        assert_eq!(app.agent_token_usage.as_ref().unwrap().prompt_tokens, 250);
        assert_eq!(
            app.agent_total_usage.prompt_tokens, 250,
            "peak should rise with the larger turn"
        );
        assert_eq!(app.agent_total_usage.completion_tokens, 50);
        assert_eq!(app.agent_total_usage.total_tokens, 400);
        assert_eq!(app.agent_total_usage.cached_tokens, Some(50));
        assert_eq!(app.agent_total_usage.reasoning_tokens, Some(5));

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
            app.agent_total_usage.prompt_tokens, 250,
            "peak prompt size must not regress"
        );
        assert_eq!(app.agent_total_usage.completion_tokens, 60);
        assert_eq!(app.agent_total_usage.cached_tokens, Some(50));
        assert_eq!(app.agent_total_usage.reasoning_tokens, Some(5));
    }
}
