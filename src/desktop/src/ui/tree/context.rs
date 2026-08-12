//! Merged tree-operation context — flat [`TreeOpsContext`] struct and its accessor methods.
//!
//! # Lifetime design
//!
//! `TreeOpsContext` is `'static + Send`: every field is owned
//! (no `&'a T` or `&'a mut T`). The previous version borrowed
//! every field from a parent struct, which forced a lifetime
//! parameter on the type and a `Box::leak` per test fixture
//! (the test harness wanted `'static` for re-borrow across
//! frames). Owning the fields is the same shape the agent's
//! `ToolContext` was rewritten to; the same compile-time
//! advantages apply here (cargo-fuzz targets, async work,
//! simpler test fixtures).
//!
//! # Usage pattern (view object)
//!
//! The context is built once per `show_left_panel` call from
//! the orchestrator's state, rendered into (mutating the
//! owned values), then `write_back`-ed to the orchestrator so
//! the changes persist. The `from_app_state` constructor and
//! the `write_back` method are the two ends of this swap.
//!
//! ```ignore
//! let mut ctx = TreeOpsContext::from_app_state(&mut app, &ui);
//! for row in &tree_rows {
//!     render_flat_row(ui, row, &mut ctx);
//! }
//! ctx.write_back(&mut app);
//! ```

use crate::app::panel_layout::PanelLayout;
use crate::bus::core::Bus;
use crate::bus::events::file::{FileEvent, FileEventProducer};
use crate::bus::events::typed::BackgroundEvent;
use crate::config::ContentLibrary;
use eframe::egui;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

/// Flat context for all tree-rendering operations. Owns every
/// field so the struct is `'static + Send` and embeddable in a
/// long-lived `FastMdApp` or any test fixture without lifetime
/// juggling.
///
/// The struct was previously broken into four intermediate
/// types (`FileOpsContext`, `DirOpsContext`,
/// `SelectionContext`, `AppIntegrationContext`) nested inside
/// a wrapping `TreeNodeContext`. The four intermediate types
/// are now collapsed into this single struct; a
/// `TreeNodeContext` type alias preserves every call site
/// unchanged.
///
/// # Why owned fields
///
/// The render function takes `&mut TreeNodeContext`. When the
/// fields were borrowed, every test had to `Box::leak` 18
/// separate values to satisfy the `'static` lifetime the
/// egui_kittest harness required. Owning the fields drops the
/// lifetime parameter and lets tests construct the context
/// directly, e.g. `TreeNodeContext { selected_file: None, ... }`.
pub struct TreeOpsContext {
    // ---- Selection state -----------------------------------------------
    /// Currently selected file (single selection).
    pub selected_file: Option<PathBuf>,
    /// Currently selected files (multi-selection).
    pub selected_files: HashSet<PathBuf>,
    /// Set of expanded directory paths.
    pub expanded_dirs: HashSet<PathBuf>,
    /// Open tabs (files opened in editor).
    pub tabs: Vec<PathBuf>,

    // ---- Directory operations ------------------------------------------
    /// Selected directory (for context menu operations and bottom-panel prefix).
    pub selected_dir: Option<PathBuf>,
    /// Whether create-directory dialog is open.
    pub create_dir_dialog_open: bool,
    /// Parent directory for new directory creation.
    pub create_dir_parent: Option<PathBuf>,

    // ---- File operations -----------------------------------------------
    /// File currently queued for move operation.
    pub file_to_move: Option<PathBuf>,
    /// Whether move dialog is open.
    pub move_dialog_open: bool,
    /// File currently queued for rename operation.
    pub file_to_rename: Option<PathBuf>,
    /// Whether rename dialog is open.
    pub rename_dialog_open: bool,
    /// New name input for rename dialog.
    pub rename_new_name: String,
    /// Whether create-document dialog is open.
    pub create_document_dialog_open: bool,
    /// Parent directory for new document creation.
    pub create_document_parent: Option<PathBuf>,

    // ---- Application integration ---------------------------------------
    /// Panel layout state (widths, dirty flags).
    pub layout: PanelLayout,
    /// Prompt to submit to agent (from context menu actions).
    pub submit_prompt: Option<String>,
    /// Content libraries configuration. Read-only in render
    /// passes; cloned in `from_app_state` because the per-frame
    /// diff is bounded (1-3 items) and avoids the lifetime
    /// question entirely.
    pub content_libraries: Vec<ContentLibrary>,
    /// File path to open in inline editor.
    pub open_editor: Option<PathBuf>,
    /// Keyboard modifiers state (shift, ctrl, command).
    pub modifiers: egui::Modifiers,
    /// Whether inline editor is enabled.
    pub inline_editor_enabled: bool,
    /// Background event sender (for print jobs, etc.).
    pub bg_tx: Option<Sender<BackgroundEvent>>,
    /// Optional file-event producer for immediate UI updates.
    pub file_event_producer: Option<FileEventProducer>,

    // ---- Cache invalidation ---------------------------------------------
    /// Tree-rows cache invalidation flag. Click handlers that change
    /// which rows are visible (e.g. directory expand/collapse) must
    /// set this to `true` so the next `show_left_panel` pass rebuilds
    /// the cached `Vec<FlatRow>`. Click handlers that only change
    /// *which* row is selected but not *which* rows are visible (e.g.
    /// file row click) leave this flag alone.
    pub tree_dirty: bool,
    /// Tracker for PDF backed files
    pub pdf_backing_tracker: crate::app::session::PdfBackingTracker,
}

/// Type alias for backward compatibility.
///
/// All public functions and call sites that accepted `TreeNodeContext`
/// continue to compile without change. New code should prefer
/// `TreeOpsContext` directly.
pub type TreeNodeContext = TreeOpsContext;

impl Default for TreeOpsContext {
    /// All-defaults construction. Used by tests to build a
    /// context with `..Default::default()` struct-update syntax
    /// so each test only specifies the fields it actually
    /// exercises. The previous `TreeNodeContext<'a>` carried
    /// `&'a mut T` references and could not be `Default`.
    fn default() -> Self {
        Self {
            selected_file: None,
            selected_files: HashSet::new(),
            expanded_dirs: HashSet::new(),
            tabs: Vec::new(),
            selected_dir: None,
            create_dir_dialog_open: false,
            create_dir_parent: None,
            file_to_move: None,
            move_dialog_open: false,
            file_to_rename: None,
            rename_dialog_open: false,
            rename_new_name: String::new(),
            create_document_dialog_open: false,
            create_document_parent: None,
            layout: PanelLayout::default(),
            submit_prompt: None,
            content_libraries: Vec::new(),
            open_editor: None,
            modifiers: egui::Modifiers::default(),
            inline_editor_enabled: false,
            bg_tx: None,
            file_event_producer: None,
            tree_dirty: false,
            pdf_backing_tracker: crate::app::session::PdfBackingTracker::default(),
        }
    }
}

impl TreeOpsContext {
    /// Build a context from the orchestrator's current state.
    /// Clones the small per-frame state (`Vec<PathBuf>`,
    /// `HashSet<PathBuf>`, etc.) into owned fields. The
    /// orchestrator's state is not modified; the caller is
    /// expected to call [`Self::write_back`] after the render
    /// pass to commit any changes the render function made.
    ///
    /// `open_editor` is taken as a value (not cloned) because
    /// the call site owns the consumer — `None` starts the
    /// frame with no pending open, and the render-row click
    /// handlers in `tree/render.rs` write into the owned
    /// `ctx.open_editor` field directly.
    #[allow(clippy::too_many_arguments)]
    pub fn from_app_state(
        selection: &crate::app::selection_manager::SelectionManager,
        tab_manager: &crate::app::tab_manager::TabManager,
        dialogs: &crate::app::dialog_manager::DialogManager,
        layout: &crate::app::panel_layout::PanelLayout,
        submit_prompt: &Option<String>,
        content_libraries: &[ContentLibrary],
        bg_tx: Option<Sender<BackgroundEvent>>,
        file_event_bus: Bus<FileEvent>,
        inline_editor_enabled: bool,
        modifiers: egui::Modifiers,
        open_editor: Option<PathBuf>,
        pdf_backing_tracker: crate::app::session::PdfBackingTracker,
    ) -> Self {
        Self {
            selected_file: selection.selected_file.clone(),
            selected_files: selection.selected_files.clone(),
            expanded_dirs: selection.expanded_dirs.clone(),
            tabs: tab_manager.tabs.clone(),
            selected_dir: selection.selected_dir.clone(),
            create_dir_dialog_open: dialogs.create_dir_dialog_open,
            create_dir_parent: dialogs.create_dir_parent.clone(),
            file_to_move: dialogs.file_to_move.clone(),
            move_dialog_open: dialogs.move_dialog_open,
            file_to_rename: dialogs.file_to_rename.clone(),
            rename_dialog_open: dialogs.rename_dialog_open,
            rename_new_name: dialogs.rename_new_name.clone(),
            create_document_dialog_open: dialogs.create_document_dialog_open,
            create_document_parent: dialogs.create_document_parent.clone(),
            layout: layout.clone(),
            submit_prompt: submit_prompt.clone(),
            content_libraries: content_libraries.to_vec(),
            open_editor,
            modifiers,
            inline_editor_enabled,
            bg_tx,
            file_event_producer: Some(FileEventProducer::new(file_event_bus)),
            tree_dirty: selection.tree_dirty,
            pdf_backing_tracker,
        }
    }

    /// Write the context's mutable fields back to the
    /// orchestrator. The render pass may have changed any of
    /// the selection or dialog state; this method commits
    /// those changes. `bg_tx`, `file_event_producer`,
    /// `modifiers`, `inline_editor_enabled`, `layout`, and
    /// `content_libraries` are not written back because the
    /// orchestrator is the source of truth for them.
    pub fn write_back(
        &self,
        selection: &mut crate::app::selection_manager::SelectionManager,
        tab_manager: &mut crate::app::tab_manager::TabManager,
        dialogs: &mut crate::app::dialog_manager::DialogManager,
        submit_prompt: &mut Option<String>,
    ) {
        selection.selected_file = self.selected_file.clone();
        selection.selected_files = self.selected_files.clone();
        selection.expanded_dirs = self.expanded_dirs.clone();
        selection.selected_dir = self.selected_dir.clone();
        selection.tree_dirty = self.tree_dirty;
        tab_manager.tabs = self.tabs.clone();
        dialogs.create_dir_dialog_open = self.create_dir_dialog_open;
        dialogs.create_dir_parent = self.create_dir_parent.clone();
        dialogs.file_to_move = self.file_to_move.clone();
        dialogs.move_dialog_open = self.move_dialog_open;
        dialogs.file_to_rename = self.file_to_rename.clone();
        dialogs.rename_dialog_open = self.rename_dialog_open;
        dialogs.rename_new_name = self.rename_new_name.clone();
        dialogs.create_document_dialog_open = self.create_document_dialog_open;
        dialogs.create_document_parent = self.create_document_parent.clone();
        *submit_prompt = self.submit_prompt.clone();
    }

    /// Access expanded directories set.
    pub fn expanded_dirs(&mut self) -> &mut HashSet<PathBuf> {
        &mut self.expanded_dirs
    }

    /// Access selected file.
    pub fn selected_file(&mut self) -> &mut Option<PathBuf> {
        &mut self.selected_file
    }

    /// Access selected files set.
    pub fn selected_files(&mut self) -> &mut HashSet<PathBuf> {
        &mut self.selected_files
    }

    /// Access tabs vector.
    pub fn tabs(&mut self) -> &mut Vec<PathBuf> {
        &mut self.tabs
    }

    /// Access file to move.
    pub fn file_to_move(&mut self) -> &mut Option<PathBuf> {
        &mut self.file_to_move
    }

    /// Access move dialog open flag.
    pub fn move_dialog_open(&mut self) -> &mut bool {
        &mut self.move_dialog_open
    }

    /// Access selected directory.
    pub fn selected_dir(&mut self) -> &mut Option<PathBuf> {
        &mut self.selected_dir
    }

    /// Access create directory dialog open flag.
    pub fn create_dir_dialog_open(&mut self) -> &mut bool {
        &mut self.create_dir_dialog_open
    }

    /// Access create directory parent.
    pub fn create_dir_parent(&mut self) -> &mut Option<PathBuf> {
        &mut self.create_dir_parent
    }

    /// Access layout.
    pub fn layout(&mut self) -> &mut PanelLayout {
        &mut self.layout
    }

    /// Access rename dialog open flag.
    pub fn rename_dialog_open(&mut self) -> &mut bool {
        &mut self.rename_dialog_open
    }

    /// Access file to rename.
    pub fn file_to_rename(&mut self) -> &mut Option<PathBuf> {
        &mut self.file_to_rename
    }

    /// Access rename new name.
    pub fn rename_new_name(&mut self) -> &mut String {
        &mut self.rename_new_name
    }

    /// Access create-document dialog open flag.
    pub fn create_document_dialog_open(&mut self) -> &mut bool {
        &mut self.create_document_dialog_open
    }

    /// Access create-document parent directory.
    pub fn create_document_parent(&mut self) -> &mut Option<PathBuf> {
        &mut self.create_document_parent
    }

    /// Access modifiers.
    pub fn modifiers(&self) -> egui::Modifiers {
        self.modifiers
    }

    /// Access submit prompt.
    pub fn submit_prompt(&mut self) -> &mut Option<String> {
        &mut self.submit_prompt
    }

    /// Access content libraries.
    pub fn content_libraries(&self) -> &[ContentLibrary] {
        &self.content_libraries
    }

    /// Access open editor.
    pub fn open_editor(&mut self) -> &mut Option<PathBuf> {
        &mut self.open_editor
    }

    /// Access inline editor enabled flag.
    pub fn inline_editor_enabled(&self) -> bool {
        self.inline_editor_enabled
    }

    /// Access background sender.
    pub fn bg_tx(&self) -> Option<&Sender<BackgroundEvent>> {
        self.bg_tx.as_ref()
    }

    /// Access file event producer.
    pub fn file_event_producer(&self) -> Option<&FileEventProducer> {
        self.file_event_producer.as_ref()
    }

    /// Access tree-rows cache invalidation flag.
    pub fn tree_dirty(&mut self) -> &mut bool {
        &mut self.tree_dirty
    }
}
