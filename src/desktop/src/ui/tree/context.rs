//! Merged tree-operation context — flat [`TreeOpsContext`] struct and its accessor methods.

use crate::app::panel_layout::PanelLayout;
use crate::bus::events::file::FileEventProducer;
use crate::bus::events::typed::BackgroundEvent;
use eframe::egui;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

/// Flat context for all tree-rendering operations.
///
/// Previously the state was split across four intermediate structs
/// (`FileOpsContext`, `DirOpsContext`, `SelectionContext`,
/// `AppIntegrationContext`) that were nested inside a wrapping
/// `TreeNodeContext`. Any new field required touching all five
/// definitions. The four intermediate types are now collapsed into
/// this single struct; a `TreeNodeContext` type alias preserves
/// every call site unchanged.
pub struct TreeOpsContext<'a> {
    // ---- Selection state -----------------------------------------------
    /// Currently selected file (single selection).
    pub selected_file: &'a mut Option<PathBuf>,
    /// Currently selected files (multi-selection).
    pub selected_files: &'a mut HashSet<PathBuf>,
    /// Set of expanded directory paths.
    pub expanded_dirs: &'a mut HashSet<PathBuf>,
    /// Open tabs (files opened in editor).
    pub tabs: &'a mut Vec<PathBuf>,

    // ---- Directory operations ------------------------------------------
    /// Selected directory (for context menu operations and bottom-panel prefix).
    pub selected_dir: &'a mut Option<PathBuf>,
    /// Whether create-directory dialog is open.
    pub create_dir_dialog_open: &'a mut bool,
    /// Parent directory for new directory creation.
    pub create_dir_parent: &'a mut Option<PathBuf>,

    // ---- File operations -----------------------------------------------
    /// File currently queued for move operation.
    pub file_to_move: &'a mut Option<PathBuf>,
    /// Whether move dialog is open.
    pub move_dialog_open: &'a mut bool,
    /// File currently queued for rename operation.
    pub file_to_rename: &'a mut Option<PathBuf>,
    /// Whether rename dialog is open.
    pub rename_dialog_open: &'a mut bool,
    /// New name input for rename dialog.
    pub rename_new_name: &'a mut String,
    /// Whether create-document dialog is open.
    pub create_document_dialog_open: &'a mut bool,
    /// Parent directory for new document creation.
    pub create_document_parent: &'a mut Option<PathBuf>,

    // ---- Application integration ---------------------------------------
    /// Panel layout state (widths, dirty flags).
    pub layout: &'a mut PanelLayout,
    /// Prompt to submit to agent (from context menu actions).
    pub submit_prompt: &'a mut Option<String>,
    /// Content libraries configuration.
    pub content_libraries: &'a [crate::config::ContentLibrary],
    /// File path to open in inline editor.
    pub open_editor: &'a mut Option<PathBuf>,
    /// Keyboard modifiers state (shift, ctrl, command).
    pub modifiers: egui::Modifiers,
    /// Whether inline editor is enabled.
    pub inline_editor_enabled: bool,
    /// Background event sender (for print jobs, etc.).
    pub bg_tx: &'a Option<Sender<BackgroundEvent>>,
    /// Optional file-event producer for immediate UI updates.
    pub file_event_producer: Option<FileEventProducer>,

    // ---- Cache invalidation ---------------------------------------------
    /// Tree-rows cache invalidation flag. Borrowed from
    /// `SelectionManager::tree_dirty`. Click handlers that change
    /// which rows are visible (e.g. directory expand/collapse) must
    /// set this to `true` so the next `show_left_panel` pass rebuilds
    /// the cached `Vec<FlatRow>`. Click handlers that only change
    /// *which* row is selected but not *which* rows are visible (e.g.
    /// file row click) leave this flag alone.
    pub tree_dirty: &'a mut bool,
    /// Tracker for PDF backed files
    pub pdf_backing_tracker: crate::app::session::PdfBackingTracker,
}

/// Type alias for backward compatibility.
///
/// All public functions and call sites that accepted `TreeNodeContext`
/// continue to compile without change. New code should prefer
/// `TreeOpsContext` directly.
pub type TreeNodeContext<'a> = TreeOpsContext<'a>;

impl<'a> TreeOpsContext<'a> {
    /// Access expanded directories set.
    pub fn expanded_dirs(&mut self) -> &mut HashSet<PathBuf> {
        self.expanded_dirs
    }

    /// Access selected file.
    pub fn selected_file(&mut self) -> &mut Option<PathBuf> {
        self.selected_file
    }

    /// Access selected files set.
    pub fn selected_files(&mut self) -> &mut HashSet<PathBuf> {
        self.selected_files
    }

    /// Access tabs vector.
    pub fn tabs(&mut self) -> &mut Vec<PathBuf> {
        self.tabs
    }

    /// Access file to move.
    pub fn file_to_move(&mut self) -> &mut Option<PathBuf> {
        self.file_to_move
    }

    /// Access move dialog open flag.
    pub fn move_dialog_open(&mut self) -> &mut bool {
        self.move_dialog_open
    }

    /// Access selected directory.
    pub fn selected_dir(&mut self) -> &mut Option<PathBuf> {
        self.selected_dir
    }

    /// Access create directory dialog open flag.
    pub fn create_dir_dialog_open(&mut self) -> &mut bool {
        self.create_dir_dialog_open
    }

    /// Access create directory parent.
    pub fn create_dir_parent(&mut self) -> &mut Option<PathBuf> {
        self.create_dir_parent
    }

    /// Access layout.
    pub fn layout(&mut self) -> &mut PanelLayout {
        self.layout
    }

    /// Access rename dialog open flag.
    pub fn rename_dialog_open(&mut self) -> &mut bool {
        self.rename_dialog_open
    }

    /// Access file to rename.
    pub fn file_to_rename(&mut self) -> &mut Option<PathBuf> {
        self.file_to_rename
    }

    /// Access rename new name.
    pub fn rename_new_name(&mut self) -> &mut String {
        self.rename_new_name
    }

    /// Access create-document dialog open flag.
    pub fn create_document_dialog_open(&mut self) -> &mut bool {
        self.create_document_dialog_open
    }

    /// Access create-document parent directory.
    pub fn create_document_parent(&mut self) -> &mut Option<PathBuf> {
        self.create_document_parent
    }

    /// Access modifiers.
    pub fn modifiers(&self) -> egui::Modifiers {
        self.modifiers
    }

    /// Access submit prompt.
    pub fn submit_prompt(&mut self) -> &mut Option<String> {
        self.submit_prompt
    }

    /// Access content libraries.
    pub fn content_libraries(&self) -> &[crate::config::ContentLibrary] {
        self.content_libraries
    }

    /// Access open editor.
    pub fn open_editor(&mut self) -> &mut Option<PathBuf> {
        self.open_editor
    }

    /// Access inline editor enabled flag.
    pub fn inline_editor_enabled(&self) -> bool {
        self.inline_editor_enabled
    }

    /// Access background sender.
    pub fn bg_tx(&self) -> &Option<Sender<BackgroundEvent>> {
        self.bg_tx
    }

    /// Access file event producer.
    pub fn file_event_producer(&self) -> &Option<FileEventProducer> {
        &self.file_event_producer
    }

    /// Access tree-rows cache invalidation flag.
    pub fn tree_dirty(&mut self) -> &mut bool {
        self.tree_dirty
    }
}
