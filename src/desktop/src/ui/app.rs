//! Root egui `App` struct — owns all application state and wires together background tasks, panels, agent, and dialogs.

use crate::agent::AgentSessionManager;
use crate::app::background::BackgroundProcessManager;
use crate::app::background_task::Task;
use crate::app::orchestrator::AppOrchestrator;
use crate::app::watcher::directory_tracker::DirectoryTracker;
use crate::app::watcher::file_processor::FileEventProcessor;
use crate::app::{
    DialogManager, PanelLayout, PersistedUiState, SelectionManager, TabManager, TagManager,
    TextBuffer,
};
use crate::bus::core::Bus;
use crate::bus::events::config::ConfigArrived;
use crate::bus::events::file::FileEventProducer;

use crate::config::AppConfig;

use crate::ui::panels::{
    show_bottom_panel, show_center_panel, show_left_panel, show_right_panel, show_top_panel,
};
use eframe::egui;
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use std::sync::{Arc, Mutex};
use std::time::Duration;

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

const PERSISTED_UI_STATE_KEY: &str = "ppai_ui_state";

/// Sanity bounds for the user-chosen font scale multiplier.
///
/// The persisted `font_size_scale` is meant to be a multiplier on
/// top of the OS-reported `pixels_per_point` (e.g. `1.2` for "20%
/// larger than the system default"). Values outside this range are
/// almost certainly the residue of the historical compounding bug
/// (where the absolute ppp was saved as a "scale" and re-applied on
/// top of the OS baseline, multiplying by 1.25×/1.5×/... every
/// launch) or outright corruption. We clamp them on apply and
/// self-heal them to `None` on the next save.
const FONT_SCALE_MIN: f32 = 0.5;
const FONT_SCALE_MAX: f32 = 3.0;

/// Validate that a candidate font scale is a finite, in-range
/// multiplier. Returns `Some(scale)` when valid, `None` otherwise.
fn sanitise_font_scale(scale: f32) -> Option<f32> {
    if !scale.is_finite() || !(FONT_SCALE_MIN..=FONT_SCALE_MAX).contains(&scale) {
        None
    } else {
        Some(scale)
    }
}

pub struct FastMdApp {
    pub orchestrator: AppOrchestrator,
    pub layout: PanelLayout,
    /// Cached flattened tree rows to avoid rebuilding the file tree every frame.
    /// Invalidated when selection.tree_dirty is true.
    pub cached_tree_rows: Option<Vec<crate::ui::tree::FlatRow>>,
    pub persisted_ui_state: PersistedUiState,
    /// Track whether we've applied persisted font scale on first frame.
    persisted_font_applied: bool,
    /// The OS-reported `pixels_per_point` at the start of the
    /// session, captured on the first frame **before** any
    /// persisted scale is applied. Used as the baseline for
    /// computing the user-chosen scale multiplier on save and is
    /// deliberately not persisted — the OS re-reports it on every
    /// launch (and may change if the window moves between monitors
    /// with different DPI).
    os_baseline_ppp: Option<f32>,
    /// The font scale that was applied to the OS baseline on the
    /// first frame. `1.0` means "no user-chosen scale was
    /// applied" (either because `persisted_ui_state.font_size_scale`
    /// was `None` or because it was rejected by
    /// [`sanitise_font_scale`]). This is the value the persist
    /// helper writes back to storage — **not** a fresh
    /// `current_ppp / baseline_ppp` computation — so the persisted
    /// state is stable across frames and across sessions even
    /// though egui 0.35 defers `set_pixels_per_point` until the
    /// next `begin_pass` (so within a single frame
    /// `ctx.pixels_per_point()` still returns the pre-apply
    /// value).
    applied_font_scale: f32,
}

impl FastMdApp {
    pub fn content_libraries(&self) -> &[crate::config::ContentLibrary] {
        &self.orchestrator.content_libraries
    }

    pub fn content_libraries_mut(&mut self) -> &mut Vec<crate::config::ContentLibrary> {
        &mut self.orchestrator.content_libraries
    }

    pub fn file_processor(&self) -> &FileEventProcessor {
        &self.orchestrator.file_processor
    }

    pub fn file_processor_mut(&mut self) -> &mut FileEventProcessor {
        &mut self.orchestrator.file_processor
    }

    pub fn pdf_backing_tracker(&self) -> &crate::app::watcher::PdfBackingTracker {
        &self.orchestrator.pdf_backing_tracker
    }

    pub fn tags(&self) -> &TagManager {
        &self.orchestrator.tag_manager
    }

    pub fn tags_mut(&mut self) -> &mut TagManager {
        &mut self.orchestrator.tag_manager
    }

    pub fn layout(&self) -> &PanelLayout {
        &self.layout
    }

    pub fn layout_mut(&mut self) -> &mut PanelLayout {
        &mut self.layout
    }

    pub fn selection(&self) -> &SelectionManager {
        &self.orchestrator.selection
    }

    pub fn selection_mut(&mut self) -> &mut SelectionManager {
        &mut self.orchestrator.selection
    }

    pub fn tabs(&self) -> &TabManager {
        &self.orchestrator.tab_manager
    }

    pub fn tabs_mut(&mut self) -> &mut TabManager {
        &mut self.orchestrator.tab_manager
    }

    pub fn agent(&self) -> &AgentSessionManager {
        &self.orchestrator.agent
    }

    pub fn agent_mut(&mut self) -> &mut AgentSessionManager {
        &mut self.orchestrator.agent
    }

    pub fn dialogs(&self) -> &DialogManager {
        &self.orchestrator.dialogs
    }

    pub fn dialogs_mut(&mut self) -> &mut DialogManager {
        &mut self.orchestrator.dialogs
    }

    pub fn editor(&self) -> &TextBuffer {
        &self.orchestrator.text_buffer
    }

    pub fn editor_mut(&mut self) -> &mut TextBuffer {
        &mut self.orchestrator.text_buffer
    }

    pub fn config(&self) -> &crate::config::AppConfig {
        &self.orchestrator.config
    }

    pub fn config_mut(&mut self) -> &mut crate::config::AppConfig {
        &mut self.orchestrator.config
    }

    pub fn submit_prompt(&self) -> &Option<String> {
        &self.orchestrator.submit_prompt
    }

    pub fn submit_prompt_mut(&mut self) -> &mut Option<String> {
        &mut self.orchestrator.submit_prompt
    }

    pub fn inline_editor_enabled(&self) -> bool {
        self.orchestrator.inline_editor_enabled
    }

    /// Purpose: Pin the egui context to the dark theme with the FastMD brand
    /// palette (RGB(9, 9, 11) surfaces, indigo selection, 8px window corners,
    /// 4px widget corners, bright off-white text).
    /// Inputs: `ctx` - The egui context whose theme is to be configured.
    /// Outputs: None.
    /// Purity: Impure (mutates the egui context's options).
    /// Preconditions: `ctx` is a valid egui context.
    /// Postconditions: The dark theme is the active theme, and the dark
    /// theme's visuals match the FastMD palette so UI-002 (dark color
    /// scheme) holds even if the system preference reports light mode.
    ///
    /// egui 0.35 split the global style into a Dark and a Light theme,
    /// picked at runtime by `ThemePreference` (default `System`).
    /// `set_visuals` writes to the *currently active* theme only, so on
    /// systems that report a light-mode preference the next frame can
    /// flip the active theme back to the default light visuals and the
    /// carefully tuned dark background is lost. Forcing the dark theme
    /// and applying the visuals to the dark theme explicitly makes the
    /// dark color scheme the source of truth.
    pub fn configure_dark_theme(ctx: &egui::Context) {
        ctx.set_theme(egui::Theme::Dark);

        let mut visuals = egui::Visuals::dark();
        visuals.window_fill = egui::Color32::from_rgb(9, 9, 11);
        visuals.panel_fill = egui::Color32::from_rgb(9, 9, 11);
        visuals.selection.bg_fill = egui::Color32::from_rgb(99, 102, 241);
        visuals.window_corner_radius = 8.0.into();
        visuals.widgets.noninteractive.corner_radius = 4.0.into();
        visuals.widgets.inactive.corner_radius = 4.0.into();
        visuals.widgets.hovered.corner_radius = 4.0.into();
        visuals.widgets.active.corner_radius = 4.0.into();

        let bright_text = egui::Color32::from_gray(210);
        visuals.widgets.noninteractive.fg_stroke.color = bright_text;
        visuals.widgets.inactive.fg_stroke.color = bright_text;
        visuals.widgets.active.fg_stroke.color = egui::Color32::WHITE;
        visuals.widgets.hovered.fg_stroke.color = egui::Color32::WHITE;

        // Apply to the dark theme explicitly so the visuals persist
        // even if the active theme ever flips to Light.
        ctx.set_visuals_of(egui::Theme::Dark, visuals);
    }

    pub fn new(cc: &eframe::CreationContext<'_>, config_bus: Bus<ConfigArrived>) -> Self {
        // egui 0.35 split the global style into a Dark and a Light
        // theme, picked at runtime by `ThemePreference` (default
        // `System`). `set_visuals` writes to the *currently active*
        // theme only, so on systems that report a light-mode
        // preference the next frame can flip the active theme back
        // to the default light visuals and the carefully tuned
        // dark background is lost. Force the dark theme and apply
        // our custom visuals to the dark theme explicitly.
        Self::configure_dark_theme(&cc.egui_ctx);

        // Subscribe before any worker is spawned so the first
        // `ConfigArrived` publish reaches every reader.
        let config_reader = config_bus.subscribe();

        // `Task::new` and `AgentSessionManager::new` each subscribe
        // to the same bus, then defer their own work until the
        // event arrives. The background `Task` spawns a thread that
        // waits on its own reader; the agent's reader is drained on
        // the UI thread in `update_ui`.
        let background_task = Task::new(config_bus.clone());
        // The tools manager subscribes to the same bus and performs
        // the one-time MCP startup ping / tool discovery on its own
        // background thread, so the UI thread never blocks on MCP
        // network I/O at startup.
        crate::agent::tools::manager::spawn_config_subscription(
            config_bus.clone(),
            background_task.tx.clone(),
        );
        // The file-watcher thread writes the `RecommendedWatcher`
        // handle into this slot before sending `FsEvent::Finished`;
        // the UI takes ownership from `task_take_finished_watcher`
        // in `handle_fs_event`.
        let finished_watcher_slot = background_task.finished_watcher.clone();
        let file_processor = FileEventProcessor::new(background_task.file_event_bus.subscribe());
        let background_manager = Arc::new(Mutex::new(BackgroundProcessManager::new()));
        // One BrowserSession for the whole app lifetime; shared
        // with the agent and (read-only) with the Tools dialog
        // so the UI can call `tick()` / `forget()`. Lazily
        // launches a Firefox process on first browser tool call.
        let browser_session = std::sync::Arc::new(crate::app::browser::BrowserSession::new(
            &crate::config::AppConfig::default(),
        ));
        let pdf_backing_tracker = crate::app::watcher::PdfBackingTracker::new();
        let agent = AgentSessionManager::new(
            config_bus,
            browser_session.clone(),
            Arc::new(pdf_backing_tracker.clone()),
        );

        let event_bus = background_task.file_event_bus;
        let dir_tracker = DirectoryTracker::new(event_bus.subscribe());

        let mut persisted_ui_state: PersistedUiState = cc
            .storage
            .and_then(|s| s.get_string(PERSISTED_UI_STATE_KEY))
            .map(|json| {
                serde_json::from_str(&json).unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "Failed to parse persisted UI state, using defaults");
                    PersistedUiState::default()
                })
            })
            .unwrap_or_default();

        // Schema migration: state written by the pre-fix build
        // has no `schema_version` field (deserialises to 0) and
        // its `font_size_scale` actually holds the absolute
        // OS-reported ppp — treating it as a multiplier on top
        // of the same ppp was the historical compounding bug.
        // Clear the field on the first launch after the fix so
        // the user starts at the OS default; other fields
        // (panel widths, expanded dirs) are preserved.
        if persisted_ui_state.schema_version < crate::app::persisted::CURRENT_SCHEMA_VERSION {
            tracing::info!(
                from = persisted_ui_state.schema_version,
                to = crate::app::persisted::CURRENT_SCHEMA_VERSION,
                "migrating persisted UI state: clearing legacy font_size_scale"
            );
            persisted_ui_state.font_size_scale = None;
            persisted_ui_state.schema_version = crate::app::persisted::CURRENT_SCHEMA_VERSION;
        }

        let mut layout = PanelLayout::new();
        if let Some(w) = persisted_ui_state.left_panel_width {
            layout.left_panel_width = Some(w);
        }
        if let Some(w) = persisted_ui_state.right_panel_width {
            layout.right_panel_width = Some(w);
        }

        let mut selection = SelectionManager::new();
        for dir in &persisted_ui_state.expanded_dirs {
            selection.expanded_dirs.insert(dir.clone());
        }

        let dialogs = DialogManager::new();
        // `batch_dialog_config.available_dirs` is populated from
        // the published config in `drain_config_bus` on the first
        // frame.

        Self {
            orchestrator: AppOrchestrator {
                content_libraries: Vec::new(),
                rx: background_task.rx,
                tx: background_task.tx,
                file_event_reader: Some(event_bus.subscribe()),
                file_event_bus: event_bus,
                file_processor,
                pdf_backing_tracker,
                directory_tracker: dir_tracker,
                tag_manager: TagManager::new(),
                selection,
                tab_manager: TabManager::new(),
                _watcher: None,
                agent,
                dialogs,
                submit_prompt: None,
                text_buffer: TextBuffer::new(),
                inline_editor_enabled: false,
                background_manager,
                config: AppConfig::default(),
                config_reader: Some(config_reader),
                pending_file_load: None,
                repaint_interval: Duration::from_millis(16),
                finished_watcher_slot,
            },
            layout,
            cached_tree_rows: None,
            persisted_ui_state,
            persisted_font_applied: false,
            os_baseline_ppp: None,
            applied_font_scale: 1.0,
        }
    }

    /// Purpose: Build a `FastMdApp` with all UI state cleared and no background channels.
    /// Inputs: None.
    /// Outputs: `FastMdApp` with every collection empty and every optional set to `None`.
    /// Purity: Constructs a new value; no side effects.
    /// Preconditions: None.
    /// Postconditions: Caller still owns a usable `Sender<BackgroundEvent>` paired with `rx`.
    pub fn empty_state(config: crate::config::AppConfig) -> Self {
        // Publish the supplied config into a private bus and let
        // `empty_state_via_bus` build the struct through the same
        // bus-driven init path. This keeps a single code path for
        // tests that don't need a real `CreationContext`.
        let bus = crate::bus::config::config_bus();
        bus.publish(ConfigArrived::new(config.clone()));
        Self::empty_state_via_bus(bus, config)
    }

    /// Build an `empty_state` `FastMdApp` by publishing `config` into
    /// a private bus and draining it synchronously. This is the test
    /// counterpart of [`Self::new`]: no `CreationContext` is needed
    /// because we don't apply egui visuals.
    fn empty_state_via_bus(bus: Bus<ConfigArrived>, config: crate::config::AppConfig) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let background_task = Task::new(bus.clone());
        // The `empty_state` test path creates the background `Task`
        // (so it subscribes to the config bus) but uses its own
        // fresh `tx`/`rx` channel for the UI side. As a result the
        // file-watcher thread never has a chance to deposit a
        // `RecommendedWatcher` into the slot the UI knows about, so
        // we point the slot at a fresh empty one.
        let finished_watcher_slot = background_task.finished_watcher.clone();
        let file_processor = FileEventProcessor::new(background_task.file_event_bus.subscribe());
        let background_manager = Arc::new(Mutex::new(BackgroundProcessManager::new()));
        let test_browser_session = std::sync::Arc::new(crate::app::browser::BrowserSession::new(
            &crate::config::AppConfig::default(),
        ));
        let pdf_backing_tracker = crate::app::watcher::PdfBackingTracker::new();
        let mut agent = AgentSessionManager::new(
            bus.clone(),
            test_browser_session,
            Arc::new(pdf_backing_tracker.clone()),
        );
        agent.set_config(config.clone());

        let event_bus = background_task.file_event_bus;
        let dir_tracker = DirectoryTracker::new(event_bus.subscribe());

        let selection = SelectionManager::new();
        let mut dialogs = DialogManager::new();
        let batch_dialog_config = crate::app::batch::types::BatchDialogConfig {
            available_dirs: config
                .content_libraries
                .iter()
                .map(|lib| PathBuf::from(&lib.root_folder))
                .collect(),
            ..Default::default()
        };
        dialogs.batch_dialog_config = batch_dialog_config;

        let content_libraries = config.content_libraries.clone();
        let inline_editor_enabled = config.inline_editor_enabled;

        Self {
            orchestrator: AppOrchestrator {
                content_libraries,
                rx,
                tx,
                file_event_bus: event_bus.clone(),
                file_event_reader: Some(event_bus.subscribe()),
                file_processor,
                pdf_backing_tracker,
                tag_manager: TagManager::new(),
                selection,
                tab_manager: TabManager::new(),
                _watcher: None,
                agent,
                dialogs,
                submit_prompt: None,
                text_buffer: TextBuffer::new(),
                inline_editor_enabled,
                background_manager,
                directory_tracker: dir_tracker,
                config,
                config_reader: None,
                pending_file_load: None,
                repaint_interval: Duration::from_millis(16),
                finished_watcher_slot,
            },
            layout: PanelLayout::new(),
            cached_tree_rows: None,
            persisted_ui_state: PersistedUiState::default(),
            persisted_font_applied: false,
            os_baseline_ppp: None,
            applied_font_scale: 1.0,
        }
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
        crate::app::print::cleanup_temp_files();
        if let Ok(mgr) = self.orchestrator.background_manager.lock() {
            let log_path = crate::config::get_config_path()
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .join("logs/background-process.log");
            let _ = mgr.save_logs(&log_path);
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.persisted_ui_state.left_panel_width = self.layout.left_panel_width;
        self.persisted_ui_state.right_panel_width = self.layout.right_panel_width;

        // Get window size and position from egui context
        // Note: This is called from the egui App trait, so we don't have direct access to ctx here.
        // The window state will be saved via the viewport commands in update_ui.

        let all_dirs: HashSet<PathBuf> = self
            .orchestrator
            .selection
            .expanded_dirs
            .iter()
            .cloned()
            .collect();
        self.persisted_ui_state.expanded_dirs = all_dirs;
        if let Ok(json) = serde_json::to_string(&self.persisted_ui_state) {
            storage.set_string(PERSISTED_UI_STATE_KEY, json);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.update_ui(ui);
    }
}

impl FastMdApp {
    /// Purpose: Drive one frame of the app.
    /// Inputs: `ui` - The root [`egui::Ui`] supplied by eframe.
    /// Outputs: None.
    /// Purity: Impure (mutates `self`, paints to `ui`).
    /// Preconditions: None.
    /// Postconditions: The root view has been rendered for this frame.
    ///
    /// egui 0.35 changed `App::update` to `App::ui`, and the
    /// `eframe::App` entry point now hands us a `&mut egui::Ui`
    /// rather than a `&Context`. We use the inner `Ui` to draw
    /// all panels, and pluck out the [`egui::Context`] for the
    /// non-rendering bookkeeping (file-event drain, repaint
    /// scheduling, etc).
    pub fn update_ui(&mut self, ui: &mut egui::Ui) {
        #[cfg(feature = "profiling")]
        puffin::GlobalProfiler::lock().new_frame();

        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        let ctx = ui.ctx();

        // Apply persisted font size scale on first frame
        self.apply_persisted_font_scale(ctx);

        self.orchestrator.drain_config_bus();
        self.process_file_events_and_repaint(ctx);
        self.orchestrator.drain_background_channel();
        self.orchestrator.handle_file_selection();
        self.show_editor_overlay(ui);
        self.show_modals(ui);
        self.render_panels(ui);
        self.handle_deferred_actions();

        // Update persisted UI state with current values for saving on exit
        self.update_persisted_ui_state(ui.ctx());

        #[cfg(feature = "profiling")]
        {
            egui::Window::new("Profiler")
                .vscroll(true)
                .resizable(true)
                .default_size([400.0, 300.0])
                .show(ui.ctx(), |ui| {
                    puffin_egui::profiler_ui(ui);
                });
        }
    }

    fn show_editor_overlay(&mut self, ui: &mut egui::Ui) {
        let producer = FileEventProducer::new(&self.orchestrator.file_event_bus);
        // The editor opens its own top-level `egui::Window` from
        // the context pulled out of `ui`. After it returns we
        // check whether the buffer was closed (either by a
        // successful save or a manual cancel) and clear the
        // loaded path so the centre panel reloads the file on
        // the next frame.
        let was_open = self.orchestrator.text_buffer.is_open;
        let _ = crate::ui::editor_egui::show_text_editor(
            ui,
            &mut self.orchestrator.text_buffer,
            &producer,
        );
        if was_open && !self.orchestrator.text_buffer.is_open {
            self.orchestrator.tab_manager.loaded_path = None;
        }
    }

    fn show_modals(&mut self, parent_ui: &mut egui::Ui) {
        // egui 0.35: modal dialogs are still rendered through
        // `egui::Window`, which can take the context directly. We
        // pull the `Context` off the root `Ui` so the existing
        // `show_*_modal` helpers (which take `&Context`) keep working.
        let ctx = parent_ui.ctx();
        if self.orchestrator.dialogs.move_dialog_open {
            crate::ui::modals::show_move_modal_dialog(
                &mut self.orchestrator.dialogs,
                &self.orchestrator.content_libraries,
                &self.orchestrator.file_processor,
                &self.orchestrator.file_event_bus,
                ctx,
            );
        }
        if self.orchestrator.dialogs.create_dir_dialog_open {
            crate::ui::modals::show_create_dir_dialog(
                &mut self.orchestrator.dialogs,
                &mut self.orchestrator.file_processor,
                &mut self.orchestrator._watcher,
                &self.orchestrator.file_event_bus,
                ctx,
            );
        }
        if self.orchestrator.dialogs.rename_dialog_open {
            let selection = &mut self.orchestrator.selection;
            crate::ui::modals::show_rename_dialog(crate::ui::modals::RenameDialogCtx {
                dialog_manager: &mut self.orchestrator.dialogs,
                file_event_bus: &self.orchestrator.file_event_bus,
                loaded_path: &mut self.orchestrator.tab_manager.loaded_path,
                selected_file: &mut selection.selected_file,
                selected_dir: &mut selection.selected_dir,
                tabs: &mut self.orchestrator.tab_manager.tabs,
                file_processor: &mut self.orchestrator.file_processor,
                tag_manager: &mut self.orchestrator.tag_manager,
                expanded_dirs: &mut selection.expanded_dirs,
                ctx,
            });
        }
        if self.orchestrator.dialogs.create_document_dialog_open {
            crate::ui::modals::show_create_document_dialog(
                &mut self.orchestrator.dialogs,
                &self.orchestrator.file_event_bus,
                ctx,
            );
        }

        crate::ui::background_logs::show_background_logs_window(self, ctx);

        if self.orchestrator.dialogs.tools_dialog_open {
            crate::ui::tools_dialog::show_tools_dialog(ctx, self);
        }

        if self.orchestrator.dialogs.batch_dialog_open {
            let mut dialog_config = self.orchestrator.dialogs.batch_dialog_config.clone();

            let prev_selected = dialog_config
                .selected_dir_idx
                .and_then(|i| dialog_config.available_dirs.get(i).cloned());
            dialog_config.available_dirs = self.orchestrator.directory_tracker.dirs_sorted();
            dialog_config.selected_dir_idx = prev_selected
                .as_ref()
                .and_then(|p| dialog_config.available_dirs.iter().position(|d| d == p));

            if let Some(result) =
                crate::ui::batch_dialog::show_batch_modal(self, ctx, &mut dialog_config)
            {
                match result {
                    crate::app::batch::types::BatchDialogResult::Process(config) => {
                        if self.orchestrator.dialogs.batch_handle.is_none() {
                            let prompt_text = dialog_config
                                .available_prompts
                                .get(dialog_config.selected_prompt_idx.unwrap_or(0))
                                .map(|p| p.content.clone())
                                .unwrap_or_default();

                            let (coordinator, cancel_flag) =
                                crate::app::batch::coordinator::BatchCoordinator::new(
                                    config,
                                    self.orchestrator.config.clone(),
                                    self.orchestrator.tx.clone(),
                                    self.orchestrator.file_event_bus.clone(),
                                    prompt_text,
                                );
                            let handle = coordinator.execute();
                            self.orchestrator.dialogs.batch_handle = Some(handle);
                            self.orchestrator.dialogs.batch_cancel_flag = Some(cancel_flag);
                        }
                    }
                    crate::app::batch::types::BatchDialogResult::Cancel => {
                        self.orchestrator.dialogs.batch_dialog_open = false;
                        dialog_config.available_prompts.clear();
                        dialog_config.selected_prompt_idx = None;
                    }
                }
            }
            self.orchestrator.dialogs.batch_dialog_config = dialog_config;
        }
    }

    fn render_panels(&mut self, parent_ui: &mut egui::Ui) {
        // egui 0.35: each `*Panel` allocates itself from a parent
        // `&mut Ui`; pass the root `Ui` from `App::ui` straight
        // through. The order is preserved from 0.27: top → bottom →
        // right → left → center. Panels must be allocated directly from
        // the parent_ui container, not nested within child_ui scopes.
        #[cfg(feature = "profiling")]
        puffin::profile_scope!("panel_top");
        show_top_panel(self, parent_ui);
        #[cfg(feature = "profiling")]
        puffin::profile_scope!("panel_bottom");
        show_bottom_panel(self, parent_ui);
        #[cfg(feature = "profiling")]
        puffin::profile_scope!("panel_right");
        show_right_panel(self, parent_ui);
        #[cfg(feature = "profiling")]
        puffin::profile_scope!("panel_left");
        show_left_panel(self, parent_ui);
        #[cfg(feature = "profiling")]
        puffin::profile_scope!("panel_center");
        show_center_panel(self, parent_ui);
    }

    fn process_file_events_and_repaint(&mut self, ctx: &egui::Context) {
        if self.orchestrator.process_file_events()
            || !self.orchestrator.file_processor.indexing_finished
            || !ctx.input(|i| i.raw.events.is_empty())
        {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(self.orchestrator.repaint_interval);
        }
    }

    fn handle_deferred_actions(&mut self) {
        if let Some(prompt) = self.orchestrator.submit_prompt.take() {
            self.orchestrator.start_agent_session(prompt);
        }

        if let Some(handle) = self.orchestrator.dialogs.batch_handle.take() {
            if handle.thread.is_finished() {
                let result = handle.join();
                self.orchestrator.dialogs.batch_cancel_flag = None;
                tracing::info!("Batch completed: {:?}", result);
            } else {
                self.orchestrator.dialogs.batch_handle = Some(handle);
            }
        }
    }

    /// Update persisted UI state with current window size, position, font scale, and panel widths.
    fn update_persisted_ui_state(&mut self, ctx: &egui::Context) {
        // Update panel widths from layout
        self.persisted_ui_state.left_panel_width = self.layout.left_panel_width;
        self.persisted_ui_state.right_panel_width = self.layout.right_panel_width;

        // Update font size scale (relative to the OS-reported
        // baseline captured on the first frame — see
        // [`Self::apply_persisted_font_scale`]).
        self.persist_font_scale(ctx);
    }

    /// Capture the OS-reported `pixels_per_point` as the baseline
    /// and apply any persisted user-chosen scale on top of it.
    ///
    /// Must run on the first frame of the session, **before** any
    /// widget paints, so the rest of the UI sees the scaled ppp.
    /// After this call the persisted scale is treated as a
    /// multiplier on top of the baseline; never as the absolute
    /// ppp.
    ///
    /// The baseline is the OS-reported value at the start of the
    /// session and is **not persisted** — the OS re-reports it on
    /// every launch (and may differ between monitors on a
    /// multi-DPI Windows setup).
    ///
    /// Records the chosen scale into [`Self::applied_font_scale`]
    /// so [`Self::persist_font_scale`] can write a stable value
    /// even though egui 0.35 defers `set_pixels_per_point` until
    /// the next `begin_pass` (so `ctx.pixels_per_point()` still
    /// returns the pre-apply value within the same frame).
    fn apply_persisted_font_scale(&mut self, ctx: &egui::Context) {
        if self.persisted_font_applied {
            return;
        }
        let baseline = ctx.pixels_per_point();
        self.os_baseline_ppp = Some(baseline);
        let scale = self
            .persisted_ui_state
            .font_size_scale
            .and_then(sanitise_font_scale)
            .unwrap_or(1.0);
        self.applied_font_scale = scale;
        if scale != 1.0 {
            ctx.set_pixels_per_point(baseline * scale);
        }
        self.persisted_font_applied = true;
    }

    /// Persist the font scale that was actually applied on the
    /// first frame (see [`Self::applied_font_scale`]) back into
    /// [`Self::persisted_ui_state`]. This is a pure
    /// "remember what we did" write — it does **not** re-read
    /// `ctx.pixels_per_point()` to recompute the multiplier,
    /// because egui 0.35 defers `set_pixels_per_point` until the
    /// next `begin_pass` and recomputing from the still-old ppp
    /// would silently reset the persisted value to `None` on the
    /// same frame the scale was applied.
    ///
    /// A near-unity applied scale (≈ 1.0) is stored as `None` so
    /// a fresh install doesn't carry a redundant `1.0` and risk
    /// a future bug misinterpreting it. Out-of-range or
    /// non-finite applied scales are also stored as `None` so a
    /// single corrupt entry self-heals on the next launch.
    fn persist_font_scale(&mut self, _ctx: &egui::Context) {
        if !self.applied_font_scale.is_finite()
            || self.applied_font_scale < FONT_SCALE_MIN
            || self.applied_font_scale > FONT_SCALE_MAX
        {
            self.persisted_ui_state.font_size_scale = None;
            return;
        }
        if (self.applied_font_scale - 1.0).abs() < 1e-3 {
            self.persisted_ui_state.font_size_scale = None;
        } else {
            self.persisted_ui_state.font_size_scale = Some(self.applied_font_scale);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::events::file::FileEvent;
    use crate::bus::events::messages::TokenUsageInfo;
    use crate::bus::events::typed::AgentEvent;
    use crate::bus::events::typed::FsEvent;
    use crate::ui::test_helpers::assert::assert_no_id_change_in_shapes;
    use std::path::PathBuf;

    fn create_test_app() -> FastMdApp {
        FastMdApp::empty_state(crate::config::AppConfig::default())
    }

    /// UI-002 (dark color scheme): `configure_dark_theme` must pin the
    /// active theme to Dark and apply the FastMD brand palette
    /// (RGB(9, 9, 11) surface, indigo selection) to the dark theme.
    /// Regression guard: the egui 0.27 → 0.35 upgrade silently fell
    /// back to the active theme's default visuals on systems reporting
    /// light mode, losing the black background.
    #[test]
    fn test_configure_dark_theme_pins_dark_with_brand_palette() {
        let ctx = egui::Context::default();

        // First, flip the active theme to Light to simulate a host
        // that reports light mode as a preference. The fix must hold
        // even in that case.
        ctx.set_theme(egui::Theme::Light);
        assert_eq!(ctx.theme(), egui::Theme::Light);

        FastMdApp::configure_dark_theme(&ctx);

        // Theme is forced to Dark regardless of the prior preference.
        assert_eq!(
            ctx.theme(),
            egui::Theme::Dark,
            "configure_dark_theme must force the active theme to Dark"
        );

        // The dark theme's visuals are the FastMD brand palette,
        // not the default `Visuals::dark()` (which is RGB(27, 27, 27)
        // for both window_fill and panel_fill).
        let dark_visuals = ctx.style_of(egui::Theme::Dark).visuals.clone();
        let expected_panel = egui::Color32::from_rgb(9, 9, 11);
        let expected_window = egui::Color32::from_rgb(9, 9, 11);
        assert_eq!(
            dark_visuals.panel_fill, expected_panel,
            "dark theme's panel_fill must be the FastMD brand RGB(9, 9, 11)"
        );
        assert_eq!(
            dark_visuals.window_fill, expected_window,
            "dark theme's window_fill must be the FastMD brand RGB(9, 9, 11)"
        );
        assert_eq!(
            dark_visuals.selection.bg_fill,
            egui::Color32::from_rgb(99, 102, 241),
            "selection.bg_fill must be the FastMD indigo RGB(99, 102, 241)"
        );
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
        app.orchestrator.tag_manager.add_tags(
            PathBuf::from("file1.md"),
            vec!["rust".to_string(), "ui".to_string()],
        );
        app.orchestrator.tag_manager.add_tags(
            PathBuf::from("file2.md"),
            vec!["rust".to_string(), "testing".to_string()],
        );

        assert_eq!(app.orchestrator.tag_manager.all_tags().len(), 3);
        assert!(app.orchestrator.tag_manager.all_tags().contains("rust"));
        assert!(app.orchestrator.tag_manager.all_tags().contains("ui"));
        assert!(app.orchestrator.tag_manager.all_tags().contains("testing"));
    }

    #[test]
    fn test_background_messages_handling() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        let test_file = PathBuf::from("test_doc.md");
        let test_dir = PathBuf::from("test_dir");

        // 1. FileParsed
        app.orchestrator
            .tx
            .send(
                FsEvent::FileParsed {
                    path: test_file.clone(),
                    tags: vec!["tag1".to_string()],
                }
                .into(),
            )
            .unwrap();

        // 2. DirParsed
        app.orchestrator
            .tx
            .send(
                FsEvent::DirParsed {
                    path: test_dir.clone(),
                }
                .into(),
            )
            .unwrap();

        // 3. FinishedWithoutWatcher
        app.orchestrator
            .tx
            .send(FsEvent::FinishedWithoutWatcher.into())
            .unwrap();

        // 4. Agent Status & Response
        app.orchestrator
            .tx
            .send(AgentEvent::Status("Processing...".to_string()).into())
            .unwrap();
        app.orchestrator
            .tx
            .send(AgentEvent::Thinking("Thinking step".to_string()).into())
            .unwrap();
        app.orchestrator
            .tx
            .send(AgentEvent::Response("Done result".to_string()).into())
            .unwrap();

        let _ = ctx.run_ui(Default::default(), |ui| {
            app.update_ui(ui);
        });

        assert!(
            app.orchestrator
                .file_processor
                .all_files
                .contains(&test_file)
        );
        assert!(app.orchestrator.file_processor.all_dirs.contains(&test_dir));
        assert!(app.orchestrator.file_processor.indexing_finished);
        assert_eq!(app.orchestrator.agent.state().status, "Processing...");
        assert_eq!(app.orchestrator.agent.state().thinking, "Thinking step");
        assert_eq!(app.orchestrator.agent.state().response, "Done result");
    }

    #[test]
    fn test_background_message_file_modified_and_deleted() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        let file_path = PathBuf::from("modified_file.md");

        app.orchestrator
            .file_processor
            .all_files
            .push(file_path.clone());
        *app.orchestrator.selection.selected_file_mut() = Some(file_path.clone());
        app.orchestrator
            .selection
            .selected_files_mut()
            .insert(file_path.clone());
        app.orchestrator.tab_manager.loaded_path = Some(file_path.clone());

        // File modified message
        app.orchestrator
            .tx
            .send(
                FsEvent::FileModified {
                    path: file_path.clone(),
                    tags: vec!["updated".to_string()],
                }
                .into(),
            )
            .unwrap();

        let _ = ctx.run_ui(Default::default(), |ui| {
            app.update_ui(ui);
        });

        assert!(app.orchestrator.tab_manager.loaded_path.is_none()); // Trigger reload

        // File deleted message
        app.orchestrator
            .tx
            .send(
                FsEvent::FileDeleted {
                    path: file_path.clone(),
                }
                .into(),
            )
            .unwrap();

        let _ = ctx.run_ui(Default::default(), |ui| {
            app.update_ui(ui);
        });

        assert!(
            !app.orchestrator
                .file_processor
                .all_files
                .contains(&file_path)
        );
        assert!(app.orchestrator.selection.selected_file().is_none());
        assert!(
            !app.orchestrator
                .selection
                .selected_files()
                .contains(&file_path)
        );
    }

    #[test]
    fn test_agent_failure_and_finish_messages() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();

        app.orchestrator
            .tx
            .send(AgentEvent::Failed("Network timeout".to_string()).into())
            .unwrap();

        let _ = ctx.run_ui(Default::default(), |ui| {
            app.update_ui(ui);
        });

        assert_eq!(
            app.orchestrator.agent.state().status,
            "Error: Network timeout"
        );
        assert!(!app.orchestrator.agent.state().running);

        app.orchestrator
            .tx
            .send(AgentEvent::Finished(vec![serde_json::json!({"ok": true})]).into())
            .unwrap();

        let _ = ctx.run_ui(Default::default(), |ui| {
            app.update_ui(ui);
        });

        assert!(!app.orchestrator.agent.state().running);
        assert!(app.orchestrator.agent.state().history.is_some());
    }

    #[test]
    fn test_agent_token_usage_message_accumulates() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();

        // First turn: small context, no cached or reasoning tokens.
        app.orchestrator
            .tx
            .send(
                AgentEvent::TokenUsage(TokenUsageInfo {
                    prompt_tokens: 100,
                    completion_tokens: 20,
                    total_tokens: 120,
                    ..Default::default()
                })
                .into(),
            )
            .unwrap();

        let _ = ctx.run_ui(Default::default(), |ui| {
            app.update_ui(ui);
        });

        assert_eq!(
            app.orchestrator
                .agent
                .state()
                .token_usage
                .as_ref()
                .unwrap()
                .prompt_tokens,
            100
        );
        assert_eq!(
            app.orchestrator.agent.state().total_usage.prompt_tokens,
            100,
            "prompt_tokens should track the peak seen so far"
        );
        assert_eq!(
            app.orchestrator.agent.state().total_usage.completion_tokens,
            20
        );
        assert_eq!(app.orchestrator.agent.state().total_usage.total_tokens, 120);
        assert_eq!(
            app.orchestrator.agent.state().total_usage.cached_tokens,
            Some(0)
        );
        assert_eq!(
            app.orchestrator.agent.state().total_usage.reasoning_tokens,
            Some(0)
        );

        // Second turn: context grew, completion + reasoning added.
        app.orchestrator
            .tx
            .send(
                AgentEvent::TokenUsage(TokenUsageInfo {
                    prompt_tokens: 250,
                    completion_tokens: 30,
                    total_tokens: 280,
                    cached_tokens: Some(50),
                    reasoning_tokens: Some(5),
                })
                .into(),
            )
            .unwrap();

        let _ = ctx.run_ui(Default::default(), |ui| {
            app.update_ui(ui);
        });

        assert_eq!(
            app.orchestrator
                .agent
                .state()
                .token_usage
                .as_ref()
                .unwrap()
                .prompt_tokens,
            250
        );
        assert_eq!(
            app.orchestrator.agent.state().total_usage.prompt_tokens,
            250,
            "peak should rise with the larger turn"
        );
        assert_eq!(
            app.orchestrator.agent.state().total_usage.completion_tokens,
            50
        );
        assert_eq!(app.orchestrator.agent.state().total_usage.total_tokens, 400);
        assert_eq!(
            app.orchestrator.agent.state().total_usage.cached_tokens,
            Some(50)
        );
        assert_eq!(
            app.orchestrator.agent.state().total_usage.reasoning_tokens,
            Some(5)
        );

        // Third turn: smaller context — peak should NOT shrink.
        app.orchestrator
            .tx
            .send(
                AgentEvent::TokenUsage(TokenUsageInfo {
                    prompt_tokens: 80,
                    completion_tokens: 10,
                    total_tokens: 90,
                    ..Default::default()
                })
                .into(),
            )
            .unwrap();

        let _ = ctx.run_ui(Default::default(), |ui| {
            app.update_ui(ui);
        });

        assert_eq!(
            app.orchestrator.agent.state().total_usage.prompt_tokens,
            250,
            "peak prompt size must not regress"
        );
        assert_eq!(
            app.orchestrator.agent.state().total_usage.completion_tokens,
            60
        );
        assert_eq!(
            app.orchestrator.agent.state().total_usage.cached_tokens,
            Some(50)
        );
        assert_eq!(
            app.orchestrator.agent.state().total_usage.reasoning_tokens,
            Some(5)
        );
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

        *app.orchestrator.selection.selected_file_mut() = Some(path.clone());
        app.orchestrator.tab_manager.loaded_path = Some(path.clone());
        app.orchestrator.file_processor.all_files.push(path.clone());

        // Subscribe a reader to the bus so we can publish into it
        // and have process_file_events pick up the event.
        app.orchestrator.file_event_reader = Some(app.orchestrator.file_event_bus.subscribe());

        // Use a separate clone of the bus to publish; both clones
        // share the same subscriber list.
        let publisher = app.orchestrator.file_event_bus.clone();
        publisher.publish(FileEvent::updated_one(path.clone()));

        let changed = app.orchestrator.process_file_events();
        assert!(changed, "process_file_events should report a change");
        assert!(
            app.orchestrator.tab_manager.loaded_path.is_none(),
            "loaded_path must be cleared so the renderer reloads on the next frame"
        );
        // selected_file must be preserved so the renderer knows
        // what to render.
        assert_eq!(app.orchestrator.selection.selected_file(), Some(&path));
    }

    #[test]
    fn test_process_file_events_updated_preserves_loaded_when_editor_open() {
        // If the inline editor is open on the file, the user's
        // unsaved changes must not be clobbered by an external
        // update. The reload should be skipped.
        let mut app = create_test_app();
        let path = PathBuf::from("/tmp/being_edited.md");

        *app.orchestrator.selection.selected_file_mut() = Some(path.clone());
        app.orchestrator.tab_manager.loaded_path = Some(path.clone());
        app.orchestrator.file_processor.all_files.push(path.clone());
        app.orchestrator
            .text_buffer
            .open(&path, "old content", None);
        assert!(app.orchestrator.text_buffer.is_open);

        app.orchestrator.file_event_reader = Some(app.orchestrator.file_event_bus.subscribe());
        let publisher = app.orchestrator.file_event_bus.clone();
        publisher.publish(FileEvent::updated_one(path.clone()));

        let _ = app.orchestrator.process_file_events();
        assert!(
            app.orchestrator.tab_manager.loaded_path.is_some(),
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

        *app.orchestrator.selection.selected_file_mut() = Some(path.clone());
        app.orchestrator.tab_manager.loaded_path = Some(path.clone());
        app.orchestrator.file_processor.all_files.push(path.clone());

        app.orchestrator.file_event_reader = Some(app.orchestrator.file_event_bus.subscribe());
        let publisher = app.orchestrator.file_event_bus.clone();
        publisher.publish(FileEvent::removed_one(path.clone()));

        let _ = app.orchestrator.process_file_events();
        assert!(app.orchestrator.tab_manager.loaded_path.is_none());
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

        app.orchestrator.file_event_reader = Some(app.orchestrator.file_event_bus.subscribe());
        let publisher = app.orchestrator.file_event_bus.clone();
        publisher.publish(FileEvent::discovered_one(pdf.clone()));
        publisher.publish(FileEvent::discovered_one(img.clone()));
        publisher.publish(FileEvent::discovered_one(md.clone()));
        publisher.publish(FileEvent::discovered_one(pdf_in_pdf_only_dir.clone()));

        let _ = app.orchestrator.process_file_events();

        // The markdown file should be in the tree and its
        // parent should be in `all_dirs`.
        assert!(
            app.orchestrator.file_processor.all_files.contains(&md),
            "markdown files must appear in the workspace tree"
        );
        assert!(
            app.orchestrator
                .file_processor
                .all_dirs
                .contains(&PathBuf::from("/tmp/lib")),
            "directories containing workspace files must appear in the tree"
        );

        // The PDF and image must NOT be in the tree, even though
        // they were published to the bus (the converters need
        // them).
        assert!(
            !app.orchestrator.file_processor.all_files.contains(&pdf),
            "PDFs must not appear in the workspace tree"
        );
        assert!(
            !app.orchestrator.file_processor.all_files.contains(&img),
            "images must not appear in the workspace tree"
        );

        // A directory that contains only a PDF must not be added
        // to `all_dirs`.
        assert!(
            !app.orchestrator
                .file_processor
                .all_dirs
                .contains(&pdf_only_dir),
            "directories that contain only non-workspace files must not appear in the tree"
        );
    }

    #[test]
    fn test_is_workspace_file_predicate() {
        // Direct unit test for the predicate that drives the
        // filter. Markdown (case-insensitive) and plain text
        // are workspace files; everything else (PDFs, images,
        // no extension) is not.
        assert!(AppOrchestrator::is_workspace_file(&PathBuf::from(
            "/a/b/note.md"
        )));
        assert!(AppOrchestrator::is_workspace_file(&PathBuf::from(
            "/a/b/note.MD"
        )));
        assert!(AppOrchestrator::is_workspace_file(&PathBuf::from(
            "/a/b/note.markdown"
        )));
        assert!(AppOrchestrator::is_workspace_file(&PathBuf::from(
            "/a/b/note.txt"
        )));
        assert!(!AppOrchestrator::is_workspace_file(&PathBuf::from(
            "/a/b/doc.pdf"
        )));
        assert!(!AppOrchestrator::is_workspace_file(&PathBuf::from(
            "/a/b/photo.png"
        )));
        assert!(!AppOrchestrator::is_workspace_file(&PathBuf::from(
            "/a/b/photo.jpg"
        )));
        assert!(!AppOrchestrator::is_workspace_file(&PathBuf::from(
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

        app.orchestrator.file_event_reader = Some(app.orchestrator.file_event_bus.subscribe());
        let publisher = app.orchestrator.file_event_bus.clone();
        publisher.publish(FileEvent::discovered_one(PathBuf::from("/lib/notes.md")));
        publisher.publish(FileEvent::discovered_one(PathBuf::from("/lib/extra.md")));
        publisher.publish(FileEvent::updated_one(PathBuf::from("/lib/notes.md")));

        let _ = app.orchestrator.process_file_events();
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
        app.orchestrator
            .tag_manager
            .add_tags(PathBuf::from("/lib/notes.md"), vec!["work".to_string()]);
        app.orchestrator
            .file_processor
            .all_files
            .push(PathBuf::from("/lib/notes.md"));

        // A `Removed` event must trigger `rebuid`, which
        // evicts the file's tags.
        app.orchestrator.file_event_reader = Some(app.orchestrator.file_event_bus.subscribe());
        app.orchestrator
            .file_event_bus
            .publish(FileEvent::removed_one(PathBuf::from("/lib/notes.md")));
        let _ = app.orchestrator.process_file_events();
        assert!(
            !app.orchestrator.tag_manager.all_tags().contains("work"),
            "Removed events must trigger rebuild so stale tags are evicted"
        );

        // A `Discovered` event must NOT call rebuild (which
        // would clear all_tags and lose the tag we just
        // added).
        app.orchestrator
            .tag_manager
            .add_tags(PathBuf::from("/lib/other.md"), vec!["keep".to_string()]);
        app.orchestrator
            .file_event_bus
            .publish(FileEvent::discovered_one(PathBuf::from("/lib/other.md")));
        let _ = app.orchestrator.process_file_events();
        assert!(
            app.orchestrator.tag_manager.all_tags().contains("keep"),
            "Discovered events must NOT call rebuild — the FileParsed path \
             updates all_tags incrementally"
        );
    }

    /// Regression: rendering a document with a Table of Contents (such as
    /// `Laptop.md`) shows `Panel::right("toc_panel")`. When the TOC panel is
    /// active, all 5 side panels must produce a stable widget tree across
    /// multi-pass renders (0 red-stroke ID-change warning shapes in egui).
    #[test]
    fn test_render_panels_no_id_change_warnings_on_toc_transition() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        let file = PathBuf::from("Laptop.md");

        app.orchestrator.tab_manager.tabs = vec![file.clone()];
        *app.orchestrator.selection.selected_file_mut() = Some(file.clone());
        app.layout.left_panel_width = Some(200.0);
        app.layout.left_panel_dirty = false;

        // Populate TOC (simulating rendering a document with headings like Laptop.md).
        app.orchestrator.tab_manager.toc = vec![
            crate::ui::ToCEntry {
                title: "Introduction".to_string(),
                level: 1,
                id: "intro".to_string(),
            },
            crate::ui::ToCEntry {
                title: "Specifications".to_string(),
                level: 2,
                id: "specs".to_string(),
            },
        ];

        // Pass 1: Initial render pass with TOC active.
        let _ = ctx.run_ui(Default::default(), |ui| {
            app.render_panels(ui);
        });

        // Pass 2: Second render pass with TOC active — must produce 0 ID change warnings.
        let output = ctx.run_ui(Default::default(), |ui| {
            app.render_panels(ui);
        });

        let shapes: Vec<egui::Shape> = output.shapes.into_iter().map(|cs| cs.shape).collect();
        assert_no_id_change_in_shapes(&shapes);
    }

    /// High-level layout integration test ensuring all 5 top-level UI panels
    /// (Top, Left, Right, Center, Bottom) allocate non-zero, full-window layout
    /// rects and render their expected child elements without collapsing or
    /// disappearing.
    #[test]
    fn test_all_top_level_panels_visible_and_rendered() {
        let ctx = egui::Context::default();
        let mut app = create_test_app();
        let file = PathBuf::from("Laptop.md");

        app.orchestrator.tab_manager.tabs = vec![file.clone()];
        *app.orchestrator.selection.selected_file_mut() = Some(file.clone());
        app.layout.left_panel_width = Some(200.0);
        app.layout.left_panel_dirty = false;
        app.file_processor_mut().indexing_finished = true;
        app.orchestrator.tab_manager.current_markdown =
            "# Laptop Specifications\n\n- CPU: 8 Cores\n- RAM: 32GB".to_string();

        // Populate TOC so the right panel is active.
        app.orchestrator.tab_manager.toc = vec![crate::ui::ToCEntry {
            title: "Laptop Specifications".to_string(),
            level: 1,
            id: "laptop_specs".to_string(),
        }];

        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1024.0, 768.0),
            )),
            ..egui::RawInput::default()
        };

        // Execute render_panels
        let output = ctx.run_ui(raw_input, |ui| {
            app.render_panels(ui);
        });

        // Extract (text, rect) for every text shape, plus the
        // overall bounding rect of the rendered output. The positional
        // assertions below use each panel's stable text marker plus
        // its expected spatial region, so a regression that swaps two
        // panels (e.g. TOC appears on the left) fails the test.
        //
        // We use the text shape's `visual_bounding_rect` for the
        // position. `text_shape.galley.rect` is in local widget
        // coordinates and `clipped.clip_rect` is the parent panel's
        // full allocation; neither is what we want. The visual
        // bounding rect is the rect of the actually-rendered glyphs
        // in root-Ui coordinates, which is where the text sits on
        // screen.
        let mut text_rects: Vec<(String, egui::Rect)> = Vec::new();
        let mut min_pos = egui::Pos2::new(f32::MAX, f32::MAX);
        let mut max_pos = egui::Pos2::new(f32::MIN, f32::MIN);

        fn collect_text_rects(shape: &egui::Shape, acc: &mut Vec<(String, egui::Rect)>) {
            match shape {
                egui::Shape::Text(text_shape) => {
                    let text = text_shape.galley.text().to_string();
                    if !text.trim().is_empty() {
                        let rect = text_shape.visual_bounding_rect();
                        if rect.is_finite() && !rect.is_negative() {
                            acc.push((text, rect));
                        }
                    }
                }
                egui::Shape::Vec(shapes) => {
                    for s in shapes {
                        collect_text_rects(s, acc);
                    }
                }
                _ => {}
            }
        }

        for clipped in &output.shapes {
            let rect = clipped.shape.visual_bounding_rect();
            if rect.is_finite() && !rect.is_negative() {
                min_pos.x = min_pos.x.min(rect.min.x);
                min_pos.y = min_pos.y.min(rect.min.y);
                max_pos.x = max_pos.x.max(rect.max.x);
                max_pos.y = max_pos.y.max(rect.max.y);
            }
            collect_text_rects(&clipped.shape, &mut text_rects);
        }

        let rendered_width = max_pos.x - min_pos.x;
        let rendered_height = max_pos.y - min_pos.y;

        // 1. Overall bounding box covers most of the 1024x768 viewport.
        assert!(
            rendered_width >= 800.0,
            "UI layout must span the window width (expected >= 800px, got {}px)",
            rendered_width
        );
        assert!(
            rendered_height >= 600.0,
            "UI layout must span the window height (expected >= 600px, got {}px)",
            rendered_height
        );

        // 2. Each panel's stable text marker is rendered AND sits in
        // the expected spatial region. P1-7: reference
        // `crate::ui::strings::*` constants rather than hardcoding
        // literals so copy changes flow through one place.
        use crate::ui::strings::{APP_TITLE, TABLE_OF_CONTENTS_HEADER, WORKSPACE_HEADER};

        let find_marker = |marker: &str| -> Option<egui::Rect> {
            text_rects
                .iter()
                .find(|(t, _)| t.contains(marker))
                .map(|(_, r)| *r)
        };

        // Top panel: header sits at the very top of the viewport.
        let title_rect = find_marker(APP_TITLE)
            .unwrap_or_else(|| panic!("Top panel content ({APP_TITLE:?}) not rendered"));
        assert!(
            title_rect.min.y < 50.0,
            "Top panel must sit at the top of the viewport (min.y < 50, got {})",
            title_rect.min.y
        );

        // Left panel: header is in the leftmost ~250px column.
        let left_rect = find_marker(WORKSPACE_HEADER)
            .unwrap_or_else(|| panic!("Left panel content ({WORKSPACE_HEADER:?}) not rendered"));
        assert!(
            left_rect.min.x < 250.0,
            "Left panel must sit in the leftmost column (min.x < 250, got {})",
            left_rect.min.x
        );

        // Right panel (TOC): header is on the right half of the
        // viewport. The right panel anchors to the right edge, so
        // its right side should be near the viewport's right edge.
        let right_rect = find_marker(TABLE_OF_CONTENTS_HEADER).unwrap_or_else(|| {
            panic!("Right panel content ({TABLE_OF_CONTENTS_HEADER:?}) not rendered")
        });
        assert!(
            right_rect.max.x > 500.0,
            "Right panel must sit on the right half of the viewport (max.x > 500, got {})",
            right_rect.max.x
        );

        // Center panel: the markdown heading is the body marker. The
        // `Laptop Specifications` literal is set by the test's
        // `current_markdown` and is therefore not a canonical copy
        // string; keep the literal but add a comment.
        //
        // We only assert the left edge of the center text. The
        // right edge can extend into the right panel's x range
        // because the right panel renders on top of the center
        // panel (it's the topmost layer in the 5-pane layout);
        // checking max.x would couple the test to the heading's
        // glyph width rather than the panel's actual position.
        let center_rect = find_marker("Laptop Specifications")
            .unwrap_or_else(|| panic!("Center panel content (markdown heading) not rendered"));
        assert!(
            center_rect.min.x > left_rect.max.x - 5.0,
            "Center panel content must start after (or at) the left panel's right edge (left.max={}, center.min={})",
            left_rect.max.x,
            center_rect.min.x
        );

        // Bottom/status bar: at least one of "Indexing finished" or
        // "files" appears in the rendered text.
        let all_text: String = text_rects
            .iter()
            .map(|(t, _)| t.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            all_text.contains("Indexing finished") || all_text.contains("files"),
            "Bottom/Top status bar content must be rendered, text: {}",
            all_text
        );
    }

    /// UI-051: closing a tab when its file is deleted via the bus
    /// `Removed` event must fall the selection back to the last
    /// remaining tab (or to `None` when no tabs remain).
    #[test]
    fn test_process_file_events_removed_closes_open_tab() {
        let mut app = create_test_app();
        let gone = PathBuf::from("/tmp/gone.md");
        let keep = PathBuf::from("/tmp/keep.md");
        app.orchestrator.tab_manager.tabs = vec![gone.clone(), keep.clone()];
        app.orchestrator.tab_manager.loaded_path = Some(gone.clone());
        *app.orchestrator.selection.selected_file_mut() = Some(gone.clone());

        app.orchestrator.file_event_reader = Some(app.orchestrator.file_event_bus.subscribe());
        app.orchestrator
            .file_event_bus
            .publish(FileEvent::removed_one(gone.clone()));
        let _ = app.orchestrator.process_file_events();

        assert!(
            !app.orchestrator.tab_manager.tabs.contains(&gone),
            "tab for deleted file must be closed"
        );
        assert!(
            app.orchestrator.tab_manager.tabs.contains(&keep),
            "tab for remaining file must stay open"
        );
        assert_eq!(
            app.orchestrator.selection.selected_file(),
            Some(&keep),
            "selection must fall back to the last remaining tab"
        );
        assert!(
            app.orchestrator.tab_manager.loaded_path.is_none(),
            "loaded_path must be cleared"
        );
    }

    /// UI-051: closing the last tab when its file is deleted must
    /// clear the selection and the displayed content.
    #[test]
    fn test_process_file_events_removed_closes_last_tab_clears_content() {
        let mut app = create_test_app();
        let gone = PathBuf::from("/tmp/gone.md");
        app.orchestrator.tab_manager.tabs = vec![gone.clone()];
        app.orchestrator.tab_manager.loaded_path = Some(gone.clone());
        app.orchestrator.tab_manager.current_markdown = "some content".to_string();
        *app.orchestrator.selection.selected_file_mut() = Some(gone.clone());

        app.orchestrator.file_event_reader = Some(app.orchestrator.file_event_bus.subscribe());
        app.orchestrator
            .file_event_bus
            .publish(FileEvent::removed_one(gone.clone()));
        let _ = app.orchestrator.process_file_events();

        assert!(
            app.orchestrator.tab_manager.tabs.is_empty(),
            "all tabs must be closed"
        );
        assert!(
            app.orchestrator.selection.selected_file().is_none(),
            "selection must be None when no tabs remain"
        );
        assert!(
            app.orchestrator.tab_manager.loaded_path.is_none(),
            "loaded_path must be cleared"
        );
        assert!(
            app.orchestrator.tab_manager.current_markdown.is_empty(),
            "content must be cleared when no tab remains"
        );
    }

    /// UI-051: closing a tab when its file is deleted via the typed
    /// `FsEvent::FileDeleted` event must fall the selection back to
    /// the last remaining tab.
    #[test]
    fn test_handle_fs_event_file_deleted_closes_open_tab() {
        let mut app = create_test_app();
        let gone = PathBuf::from("gone.md");
        let keep = PathBuf::from("keep.md");
        app.orchestrator.tab_manager.tabs = vec![gone.clone(), keep.clone()];
        app.orchestrator.tab_manager.loaded_path = Some(gone.clone());
        *app.orchestrator.selection.selected_file_mut() = Some(gone.clone());

        app.orchestrator
            .handle_fs_event(FsEvent::FileDeleted { path: gone.clone() });

        assert!(
            !app.orchestrator.tab_manager.tabs.contains(&gone),
            "tab for deleted file must be closed"
        );
        assert_eq!(
            app.orchestrator.selection.selected_file(),
            Some(&keep),
            "selection must fall back to the last remaining tab"
        );
    }

    /// REGRESSION (font scale compounding): `pixels_per_point` is the
    /// OS-reported device pixel ratio (e.g. 1.5 on a 150% DPI display).
    /// The persisted `font_size_scale` must be a user-chosen
    /// **multiplier** relative to that baseline — not the absolute
    /// ppp. The pre-fix code divided the current ppp by a hard-coded
    /// 1.0 to compute the scale, so a 150% DPI display saved
    /// `Some(1.5)`, then on the next launch multiplied the OS-reported
    /// 1.5 by that "scale" 1.5 → 2.25, then saved `Some(2.25)`, then
    /// 3.375, 5.06, ... The font visibly grew every launch.
    ///
    /// This test exercises the **real per-frame order** used by
    /// `update_ui`: `apply_persisted_font_scale` and
    /// `persist_font_scale` run in the same frame, **before**
    /// egui 0.35's deferred `set_pixels_per_point` update takes
    /// effect. The earlier version of this test masked a bug by
    /// inserting a `run_ui` between apply and persist.
    #[test]
    fn test_font_scale_does_not_compound_across_launches() {
        // === Session 1: simulated 150% DPI display, user has chosen 1.2x zoom ===
        let mut app1 = create_test_app();
        app1.persisted_ui_state.font_size_scale = Some(1.2);

        let (ctx1, _raw1) = ctx_with_native_ppp(1.5);
        app1.apply_persisted_font_scale(&ctx1);
        // Persist in the SAME frame as apply — the real app's
        // `update_ui` does this. The deferred zoom-factor update
        // from `set_pixels_per_point` has not been applied yet
        // (it only takes effect on the next `begin_pass`), so
        // `ctx.pixels_per_point()` still reports the OS baseline
        // (1.5), not the target 1.8. The persist must NOT
        // recompute the scale from this stale ppp or it would
        // silently reset the stored value to `None`.
        app1.persist_font_scale(&ctx1);
        assert_eq!(
            app1.persisted_ui_state.font_size_scale,
            Some(1.2),
            "session 1 persist (same frame as apply): must store the user's \
             1.2 multiplier, not the pre-apply ppp / baseline ratio"
        );

        // After a follow-up frame, the deferred zoom-factor is
        // applied and the on-screen ppp matches the user's choice.
        let (ctx1_after, _) = ctx_with_native_ppp(1.5);
        let _ = ctx1_after.run_ui(egui::RawInput::default(), |_ui| {});
        let _ = ctx1_after; // not asserted — we only care about persistence here.

        // === Session 2: restart, reload persisted state, same OS baseline ===
        let persisted_json = serde_json::to_string(&app1.persisted_ui_state).unwrap();
        let mut app2 = create_test_app();
        app2.persisted_ui_state = serde_json::from_str(&persisted_json).unwrap();

        let (ctx2, _raw2) = ctx_with_native_ppp(1.5);
        app2.apply_persisted_font_scale(&ctx2);
        app2.persist_font_scale(&ctx2);
        assert_eq!(
            app2.persisted_ui_state.font_size_scale,
            Some(1.2),
            "session 2 persist: must remain 1.2, not 1.8 or higher"
        );

        // === Session 3..N: the value must stay stable for any number of restarts ===
        for _ in 0..5 {
            let json = serde_json::to_string(&app2.persisted_ui_state).unwrap();
            let mut next = create_test_app();
            next.persisted_ui_state = serde_json::from_str(&json).unwrap();
            let (ctx, _raw) = ctx_with_native_ppp(1.5);
            next.apply_persisted_font_scale(&ctx);
            next.persist_font_scale(&ctx);
            assert_eq!(
                next.persisted_ui_state.font_size_scale,
                Some(1.2),
                "loop: persisted scale drifted after multiple restarts"
            );
            app2 = next;
        }
    }

    /// REGRESSION (same-frame persist under a non-trivial OS
    /// baseline): the persist must store the *applied* scale,
    /// not a freshly-computed `current_ppp / baseline_ppp`. With
    /// the old logic, running apply + persist in the same frame
    /// on a 150% DPI display with `Some(1.2)` would compute
    /// `scale = 1.5 / 1.5 = 1.0` and silently reset the
    /// persisted value to `None`. The user's font would then
    /// snap back to the OS default on every restart, shrinking
    /// the UI each time.
    #[test]
    fn test_font_scale_persist_in_same_frame_as_apply_keeps_value() {
        let mut app = create_test_app();
        app.persisted_ui_state.font_size_scale = Some(1.2);

        let (ctx, _raw) = ctx_with_native_ppp(1.5);
        app.apply_persisted_font_scale(&ctx);
        // Same-frame persist: ctx.pixels_per_point() is still 1.5
        // (the OS baseline) because the deferred zoom-factor from
        // `set_pixels_per_point(1.8)` has not been applied yet.
        app.persist_font_scale(&ctx);

        assert_eq!(
            app.persisted_ui_state.font_size_scale,
            Some(1.2),
            "same-frame persist must not silently reset the scale to None"
        );
    }

    /// REGRESSION (legacy corruption): A user who upgraded from the
    /// buggy build may have a pre-fix persisted scale that is the
    /// absolute ppp (e.g., `Some(1.5)` on a 1.0 DPI display, or
    /// worse — `Some(5.0)` after several compounding launches).
    /// The apply helper must clamp out-of-range values to a no-op
    /// and the next persist must self-heal the stored value back
    /// to `None`.
    #[test]
    fn test_font_scale_clamps_legacy_corrupt_value() {
        let mut app = create_test_app();
        // Pretend the old buggy code persisted the absolute ppp
        // (or a compounded value) as a "scale".
        app.persisted_ui_state.font_size_scale = Some(5.0);

        let (ctx, _raw) = ctx_with_native_ppp(1.5);
        app.apply_persisted_font_scale(&ctx);
        app.persist_font_scale(&ctx);

        // The corrupt 5.0 must NOT be applied on top of the OS
        // baseline (that would yield 7.5 ppp). The baseline is
        // left untouched, and the next persist self-heals the
        // stored value to None (the user has not actually
        // chosen a zoom).
        assert_eq!(
            app.persisted_ui_state.font_size_scale, None,
            "corrupt scale must self-heal to None after one save"
        );
    }

    /// REGRESSION (NaN / infinity guard): a corrupt persisted scale
    /// that is not finite must also be ignored, not propagated into
    /// `set_pixels_per_point`, which would otherwise produce a
    /// runtime panic in egui.
    #[test]
    fn test_font_scale_rejects_non_finite_value() {
        let mut app = create_test_app();
        app.persisted_ui_state.font_size_scale = Some(f32::NAN);

        let (ctx, _raw) = ctx_with_native_ppp(1.5);
        app.apply_persisted_font_scale(&ctx);
        app.persist_font_scale(&ctx);

        // The NaN must not be applied; the persist self-heals to None.
        assert_eq!(
            app.persisted_ui_state.font_size_scale, None,
            "NaN scale must self-heal to None"
        );
    }

    /// REGRESSION (schema migration): a persisted state written by
    /// the pre-fix build (no `schema_version` field) must be
    /// migrated to the current schema on load — specifically,
    /// `font_size_scale` is cleared so the absolute-ppp value
    /// that the old bug used to compound is not carried forward
    /// as a multiplier.
    #[test]
    fn test_persisted_state_migration_clears_legacy_font_size_scale() {
        // Hand-written JSON mimicking the pre-fix on-disk shape:
        // no `schema_version` field; `font_size_scale` holds the
        // absolute ppp from a 150% DPI display.
        let legacy_json = r#"{
            "left_panel_width": null,
            "right_panel_width": null,
            "window_width": null,
            "window_height": null,
            "window_x": null,
            "window_y": null,
            "font_size_scale": 1.5,
            "expanded_dirs": []
        }"#;
        let mut state: PersistedUiState = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(state.schema_version, 0);
        assert_eq!(state.font_size_scale, Some(1.5));

        // Apply the same migration the production `FastMdApp::new`
        // runs. (We do it inline here because the helper would
        // require an eframe::CreationContext.)
        if state.schema_version < crate::app::persisted::CURRENT_SCHEMA_VERSION {
            state.font_size_scale = None;
            state.schema_version = crate::app::persisted::CURRENT_SCHEMA_VERSION;
        }

        assert_eq!(
            state.font_size_scale, None,
            "migration must clear the legacy font_size_scale"
        );
        assert_eq!(
            state.schema_version,
            crate::app::persisted::CURRENT_SCHEMA_VERSION,
            "migration must bump schema_version to the current value"
        );
    }

    /// Build an `egui::Context` whose input state reports
    /// `native_pixels_per_point = Some(ppp)`, so
    /// `ctx.pixels_per_point()` returns `ppp` before any zoom
    /// change is applied. Returns the context together with the
    /// matching `RawInput` so the caller can drive a follow-up
    /// `run_ui` that preserves the high-DPI viewport info.
    fn ctx_with_native_ppp(ppp: f32) -> (egui::Context, egui::RawInput) {
        let ctx = egui::Context::default();
        let viewports = std::iter::once((
            egui::ViewportId::ROOT,
            egui::ViewportInfo {
                native_pixels_per_point: Some(ppp),
                ..Default::default()
            },
        ))
        .collect();
        let raw_input = egui::RawInput {
            viewports,
            ..Default::default()
        };
        // Drive a single empty pass to seed the input state. We
        // immediately call `end_pass` to leave the viewport
        // stack balanced, so the next `begin_pass` (driven by
        // `run_ui`) is the "outermost" pass and the deferred
        // `new_zoom_factor` written by `set_pixels_per_point` is
        // actually applied.
        ctx.begin_pass(raw_input.clone());
        let _ = ctx.end_pass();
        (ctx, raw_input)
    }
}
