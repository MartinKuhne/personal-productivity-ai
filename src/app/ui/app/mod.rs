//! Root egui `App` struct — owns all application state and wires together
//! background tasks, panels, agent, and dialogs.
//!
//! The lifecycle is split across three sibling files (one phase each):
//!
//! - `init`   — `FastMdApp::new`, `FastMdApp::empty_state`, and the
//!   dark-theme pinning. Runs once at app start (or once per test).
//! - `update` — `FastMdApp::update_ui` and the helpers it drives
//!   (`process_file_events_and_repaint`, `handle_deferred_actions`,
//!   `update_persisted_ui_state`). Runs every frame before paint.
//! - `render` — `show_editor_overlay`, `show_modals`, `render_panels`.
//!   Runs every frame to paint the editor window, dialogs, and the
//!   five top-level panels in the order top → bottom → right → left
//!   → center.
//!
//! This file holds the data definitions (`FastMdApp`, `TreeNode`, the
//! persisted-UI-state key) plus the cross-cutting concerns that don't
//! fit a single phase: the pass-through accessors used by every panel
//! and modal, the `eframe::App` trait impl (`on_exit` / `save` / `ui`),
//! the `generate_format_prompt` helper, and the test module.
//!
//! This is the same split the `tools/manager/` directory uses:
//! `mod.rs` is a thin facade that re-exports the public types and
//! declares the submodules; each submodule owns a single concern.
//!
//! Unit tests live in the sibling `tests.rs` sidecar.

mod init;
mod render;
mod update;

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use eframe::egui;

use crate::agent::AgentSession;
use crate::app::{Dialogs, FileSelection, PanelLayout, PersistedUiState, Tabs, Tags, TextBuffer};
use crate::workspace::watcher::FileEventProcessor;

/// The key used to persist the eframe UI state.
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
/// The maximum font scaling factor allowed.
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

pub struct FastMdApp {
    pub orchestrator: crate::app::orchestrator::AppOrchestrator,
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

    pub fn pdf_backing_tracker(&self) -> &crate::app::session::PdfBackingTracker {
        &self.orchestrator.pdf_backing_tracker
    }

    pub fn tags(&self) -> &Tags {
        &self.orchestrator.tags
    }

    pub fn tags_mut(&mut self) -> &mut Tags {
        &mut self.orchestrator.tags
    }

    pub fn layout(&self) -> &PanelLayout {
        &self.layout
    }

    pub fn layout_mut(&mut self) -> &mut PanelLayout {
        &mut self.layout
    }

    pub fn selection(&self) -> &FileSelection {
        &self.orchestrator.selection
    }

    pub fn selection_mut(&mut self) -> &mut FileSelection {
        &mut self.orchestrator.selection
    }

    pub fn tabs(&self) -> &Tabs {
        &self.orchestrator.tabs
    }

    pub fn tabs_mut(&mut self) -> &mut Tabs {
        &mut self.orchestrator.tabs
    }

    pub fn agent(&self) -> &AgentSession {
        &self.orchestrator.agent
    }

    pub fn agent_mut(&mut self) -> &mut AgentSession {
        &mut self.orchestrator.agent
    }

    pub fn dialogs(&self) -> &Dialogs {
        &self.orchestrator.dialogs
    }

    pub fn dialogs_mut(&mut self) -> &mut Dialogs {
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
}

/// Purpose: Generates the markdown formatting prompt with a dynamic date.
/// Inputs: `date_str` - The current date string in RFC3339 format.
/// Outputs: A String containing the complete formatting prompt.
/// Purity: Pure.
/// Preconditions: None.
/// Postconditions: Returns a valid prompt string containing the provided date.
pub fn generate_format_prompt(date_str: &str) -> String {
    format!(
        "Format the current document into correct markdown and use this template for the yaml front matter. Focus ONLY on the currently active file, and DO NOT use list_notes or search for other files.\n```yaml\n---\ntitle: A brief title\nsummary: A three sentence summary of the contents\ntags: [\"tag1\",\"tag2\"]\nheader-date: {}\n---\n```",
        date_str
    )
}

impl eframe::App for FastMdApp {
    fn on_exit(&mut self) {
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

        // Window size/position are persisted by eframe's built-in
        // `persistence` feature (enabled on the `eframe` dep in
        // `Cargo.toml`) — we do not duplicate that here.

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

// ---------------------------------------------------------------------------
// Tests live in the sibling `tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
