//! Root application state struct — owns all application state and wires
//! together background tasks, panels, agent, and dialogs.
//!
//! `FastMdApp` is the framework-free domain object the iced UI
//! ([`crate::ui_iced`]) wraps: the orchestrator handle, the pane layout,
//! the persisted-UI snapshot and the pass-through accessors every panel
//! and modal reads. Construction lives in the sibling `init` module
//! (`FastMdApp::from_parts`, [`FastMdApp::empty_state`]).
//!
//! The iced reducer in [`crate::ui_iced::update`] drives this state.
//!
//! Unit tests live in the sibling `tests.rs` sidecar.

mod init;

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use crate::agent::AgentSession;
use crate::ui::{Dialogs, FileSelection, PanelLayout, PersistedUiState, Tabs, TextBuffer};
use crate::workspace::Tags;
use crate::workspace::watcher::FileEventProcessor;

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

    /// Recursively collects all directory paths present in this tree node and its descendants.
    pub fn collect_dirs(&self, out: &mut HashSet<PathBuf>) {
        if self.is_dir && !self.path.as_os_str().is_empty() {
            out.insert(self.path.clone());
        }
        for child in self.children.values() {
            child.collect_dirs(out);
        }
    }
}

pub struct FastMdApp {
    pub orchestrator: crate::orchestrator::AppOrchestrator,
    pub layout: PanelLayout,
    /// Cached flattened tree rows to avoid rebuilding the file tree every frame.
    /// Invalidated when selection.tree_dirty is true.
    pub cached_tree_rows: Option<Arc<Vec<crate::ui::tree::FlatRow>>>,
    pub persisted_ui_state: PersistedUiState,
    /// Workspace content search state (UI-050).
    pub search: crate::ui::TreeSearch,
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

    pub fn pdf_backing_tracker(&self) -> &crate::agent::session::PdfBackingTracker {
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

    pub fn inline_editor_enabled(&self) -> bool {
        self.orchestrator.inline_editor_enabled
    }

    pub fn search(&self) -> &crate::ui::TreeSearch {
        &self.search
    }

    pub fn search_mut(&mut self) -> &mut crate::ui::TreeSearch {
        &mut self.search
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

// ---------------------------------------------------------------------------
// Tests live in the sibling `tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
