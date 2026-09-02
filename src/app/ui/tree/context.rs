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
//! the orchestrator's state and rendered into. UI intents are
//! published via `user_command_bus` rather than mutated through a
//! `write_back` swap — the `CommandExecutor` drains the bus and
//! applies the mutations centrally.
//!
//! ```ignore
//! let mut ctx = TreeNodeContext::from_app_state(
//!     &app.orchestrator.selection,
//!     &app.orchestrator.tabs,
//!     &app.layout,
//!     &app.orchestrator.content_libraries,
//!     Some(app.orchestrator.tx.clone()),
//!     app.orchestrator.file_event_bus.clone(),
//!     app.orchestrator.inline_editor_enabled,
//!     ui.input(|i| i.modifiers),
//!     app.pdf_backing_tracker().clone(),
//!     app.orchestrator.user_command_bus.clone(),
//! );
//! for row in &tree_rows {
//!     render_flat_row(ui, row, &mut ctx);
//! }
//! // No write_back — intents were published to `Bus<UserCommand>`.
//! ```

use crate::bus::core::Bus;
use crate::bus::events::file::{FileEvent, FileEventProducer};
use crate::bus::events::typed::BackgroundEventSender;
use crate::config::ContentLibrary;
use crate::ui::panel_layout::PanelLayout;
use crate::ui::{FileSelection, Tabs};
use eframe::egui;
use std::collections::HashSet;
use std::path::PathBuf;

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
    /// Parent directory for new directory creation.
    // ---- File operations -----------------------------------------------
    /// File currently queued for move operation.
    /// Whether move dialog is open.
    /// File currently queued for rename operation.
    /// Whether rename dialog is open.
    /// New name input for rename dialog.
    /// Whether create-document dialog is open.
    /// Parent directory for new document creation.
    // ---- Application integration ---------------------------------------
    /// Panel layout state (widths, dirty flags).
    pub layout: PanelLayout,
    /// Prompt to submit to agent (from context menu actions).
    /// Content libraries configuration. Read-only in render
    /// passes; cloned in `from_app_state` because the per-frame
    /// diff is bounded (1-3 items) and avoids the lifetime
    /// question entirely.
    pub content_libraries: Vec<ContentLibrary>,
    /// File path to open in inline editor.
    /// Keyboard modifiers state (shift, ctrl, command).
    pub modifiers: egui::Modifiers,
    /// Whether inline editor is enabled.
    pub inline_editor_enabled: bool,
    /// Background event sender (for print jobs, etc.).
    pub bg_tx: Option<BackgroundEventSender>,
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
    pub pdf_backing_tracker: crate::agent::session::PdfBackingTracker,
}

/// Type alias for backward compatibility.
///
/// All public functions and call sites that accepted `TreeNodeContext`
/// continue to compile without change. New code should prefer
/// `TreeOpsContext` directly.
pub struct TreeNodeContext {
    pub ctx: TreeOpsContext,
    pub user_command_bus: Bus<crate::bus::events::user_command::UserCommand>,
}

impl std::ops::Deref for TreeNodeContext {
    type Target = TreeOpsContext;
    fn deref(&self) -> &Self::Target {
        &self.ctx
    }
}

impl std::ops::DerefMut for TreeNodeContext {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ctx
    }
}

impl TreeNodeContext {
    #[allow(clippy::too_many_arguments)]
    pub fn from_app_state(
        selection: &crate::ui::FileSelection,
        tabs: &crate::ui::Tabs,
        layout: &crate::ui::panel_layout::PanelLayout,
        content_libraries: &[crate::config::ContentLibrary],
        bg_tx: Option<crate::bus::events::typed::BackgroundEventSender>,
        file_event_bus: Bus<crate::bus::events::file::FileEvent>,
        inline_editor_enabled: bool,
        modifiers: eframe::egui::Modifiers,
        pdf_backing_tracker: crate::agent::session::PdfBackingTracker,
        user_command_bus: Bus<crate::bus::events::user_command::UserCommand>,
    ) -> Self {
        Self {
            ctx: TreeOpsContext::from_app_state(
                selection,
                tabs,
                layout,
                content_libraries,
                bg_tx,
                file_event_bus,
                inline_editor_enabled,
                modifiers,
                pdf_backing_tracker,
            ),
            user_command_bus,
        }
    }
}

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
            layout: PanelLayout::default(),
            content_libraries: Vec::new(),
            modifiers: egui::Modifiers::default(),
            inline_editor_enabled: false,
            bg_tx: None,
            file_event_producer: None,
            tree_dirty: false,
            pdf_backing_tracker: crate::agent::session::PdfBackingTracker::default(),
        }
    }
}

impl TreeOpsContext {
    /// Build a context from the orchestrator's current state.
    /// Clones the small per-frame state (`Vec<PathBuf>`,
    /// `HashSet<PathBuf>`, etc.) into owned fields. The
    /// orchestrator's state is not modified directly; UI
    /// intents are published to `Bus<UserCommand>` and applied
    /// centrally by `CommandExecutor::apply_user_command`.
    #[allow(clippy::too_many_arguments)]
    pub fn from_app_state(
        selection: &FileSelection,
        tabs: &Tabs,
        layout: &PanelLayout,
        content_libraries: &[ContentLibrary],
        bg_tx: Option<BackgroundEventSender>,
        file_event_bus: Bus<FileEvent>,
        inline_editor_enabled: bool,
        modifiers: egui::Modifiers,
        pdf_backing_tracker: crate::agent::session::PdfBackingTracker,
    ) -> Self {
        Self {
            selected_file: selection.selected_file.clone(),
            selected_files: selection.selected_files.clone(),
            expanded_dirs: selection.expanded_dirs.clone(),
            tabs: tabs.tabs.clone(),
            selected_dir: selection.selected_dir.clone(),
            layout: layout.clone(),
            content_libraries: content_libraries.to_vec(),
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

    /// Access selected directory.
    pub fn selected_dir(&mut self) -> &mut Option<PathBuf> {
        &mut self.selected_dir
    }

    /// Access layout.
    pub fn layout(&mut self) -> &mut PanelLayout {
        &mut self.layout
    }

    /// Access modifiers.
    pub fn modifiers(&self) -> egui::Modifiers {
        self.modifiers
    }

    /// Access content libraries.
    pub fn content_libraries(&self) -> &[ContentLibrary] {
        &self.content_libraries
    }

    /// Access inline editor enabled flag.
    pub fn inline_editor_enabled(&self) -> bool {
        self.inline_editor_enabled
    }

    /// Access background sender.
    pub fn bg_tx(&self) -> Option<&BackgroundEventSender> {
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
