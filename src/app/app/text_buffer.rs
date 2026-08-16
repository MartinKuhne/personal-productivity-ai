//! Egui-free text editing model — `TextBuffer`, `Cursor`, `Selection`, and `UndoStack`.
//!
//! This module is the data layer for the inline markdown editor. It owns the
//! file content, the front matter (preserved verbatim across saves), the
//! cursor / selection state, and the undo history. Nothing in this module
//! imports `eframe::egui`; the rendering layer in
//! [`crate::ui::editor_egui`] adapts the buffer to egui's immediate-mode
//! widgets and writes the resulting cursor position back here.
//!
//! ## Why a dedicated `TextBuffer`?
//!
//! The original `EditorState` was a flat struct with `is_open`, `content`,
//! `file_path`, `error_message`, and the front-matter cache. This module
//! elevates those fields into a small model with proper types
//! ([`Cursor`], [`Selection`], [`UndoStack`]) so that:
//!
//! - the editor's state machine is testable without driving the UI
//!   harness (the unit tests below are pure data assertions),
//! - the rendering code is a thin adapter and doesn't need to know how
//!   the buffer is saved or how undo history is stored, and
//! - future consumers (e.g. a search-and-replace tool, an external
//!   diff view) can read and mutate the buffer without going through
//!   egui's text widget.
//!
//! ## Save path
//!
//! [`TextBuffer::save`] writes the buffer to disk, publishes an
//! `Updated` [`crate::bus::events::file::FileEvent`] so the rest of
//! the app refreshes immediately, and closes the editor. The publish
//! needs a [`FileEventProducer`], which is itself egui-free; the
//! producer reference is passed in rather than held on the buffer so
//! the type stays plain Rust data with no framework-specific slots.
//!
//! Unit tests live in the sibling `text_buffer_tests.rs` sidecar.

use crate::app::document::DocumentContent;
use crate::bus::events::file::FileEventProducer;
use std::fs;
use std::path::{Path, PathBuf};

/// Default maximum number of entries kept in the [`UndoStack`].
///
/// Sized to comfortably cover a few minutes of typing in a markdown
/// document while bounding memory at ~1 MiB for a 256 KiB file.
pub const DEFAULT_UNDO_CAPACITY: usize = 64;

/// A position in the buffer.
///
/// All three fields are kept in sync by the rendering layer:
///
/// - `line` is 0-indexed
/// - `column` is 0-indexed and measured in *characters* (not bytes), to
///   match what users see on screen and what `egui::TextEdit` reports
/// - `char_index` is the char offset into [`TextBuffer::content`], which
///   the renderer uses to look up the byte offset for slicing
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Cursor {
    /// 0-indexed line number.
    pub line: usize,
    /// 0-indexed column in characters (not bytes).
    pub column: usize,
    /// Char index into the underlying buffer.
    pub char_index: usize,
}

impl Cursor {
    /// Cursor at the very start of the buffer.
    pub const fn at_start() -> Self {
        Self {
            line: 0,
            column: 0,
            char_index: 0,
        }
    }
}

/// A selection between two cursors.
///
/// `anchor` is where the selection started, `head` is the moving end.
/// When the two are equal, the selection is collapsed (i.e. it's just a
/// cursor).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Selection {
    /// Stable end of the selection (the side the user clicked first).
    pub anchor: Cursor,
    /// Moving end of the selection (the side being dragged).
    pub head: Cursor,
}

impl Selection {
    /// A collapsed selection (anchor == head).
    pub const fn collapsed(at: Cursor) -> Self {
        Self {
            anchor: at,
            head: at,
        }
    }

    /// `true` if anchor and head are at the same position.
    pub fn is_collapsed(&self) -> bool {
        self.anchor == self.head
    }
}

/// One snapshot in the undo history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UndoEntry {
    content: String,
    cursor: Cursor,
    selection: Selection,
}

/// A linear undo / redo history.
///
/// The stack is bounded by `capacity`; pushing a new entry when full
/// evicts the oldest entry. The redo branch is cleared whenever a new
/// edit is pushed.
#[derive(Clone, Debug)]
pub struct UndoStack {
    entries: Vec<UndoEntry>,
    /// `Some(i)` = the index of the entry that is the buffer's current
    /// state. `None` = the buffer has never been pushed.
    position: Option<usize>,
    capacity: usize,
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new()
    }
}

impl UndoStack {
    /// Create an empty undo stack with [`DEFAULT_UNDO_CAPACITY`].
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_UNDO_CAPACITY)
    }

    /// Create an empty undo stack with the given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            position: None,
            capacity: capacity.max(1),
        }
    }

    /// Push a new entry onto the stack. Any redo branch is discarded.
    ///
    /// Consecutive pushes of identical content are coalesced into a
    /// single entry (so a stream of `TextChanged` events from
    /// `egui::TextEdit` does not flood the history).
    pub fn push(&mut self, content: String, cursor: Cursor, selection: Selection) {
        // Coalesce with the current position if the content is unchanged
        // (egui emits a TextChanged per keystroke, most of which are
        // no-ops in terms of the buffer's byte stream).
        if let Some(pos) = self.position
            && let Some(current) = self.entries.get(pos)
            && current.content == content
        {
            // Just update the cursor / selection; the entry stays put.
            self.entries[pos].cursor = cursor;
            self.entries[pos].selection = selection;
            return;
        }

        // Drop the redo branch.
        if let Some(pos) = self.position {
            self.entries.truncate(pos + 1);
        }

        self.entries.push(UndoEntry {
            content,
            cursor,
            selection,
        });
        if self.entries.len() > self.capacity {
            let excess = self.entries.len() - self.capacity;
            self.entries.drain(0..excess);
        }
        self.position = Some(self.entries.len() - 1);
    }

    /// `true` if [`undo`](Self::undo) would return an entry.
    pub fn can_undo(&self) -> bool {
        self.position.is_some_and(|p| p > 0)
    }

    /// `true` if [`redo`](Self::redo) would return an entry.
    pub fn can_redo(&self) -> bool {
        match self.position {
            Some(pos) => pos + 1 < self.entries.len(),
            None => false,
        }
    }

    /// Move the cursor one step back in history and return the entry
    /// the buffer should restore.
    pub fn undo(&mut self) -> Option<&UndoEntry> {
        let pos = self.position?;
        if pos == 0 {
            return None;
        }
        self.position = Some(pos - 1);
        self.entries.get(self.position?)
    }

    /// Move the cursor one step forward in history and return the
    /// entry the buffer should restore.
    pub fn redo(&mut self) -> Option<&UndoEntry> {
        let pos = self.position?;
        if pos + 1 >= self.entries.len() {
            return None;
        }
        self.position = Some(pos + 1);
        self.entries.get(self.position?)
    }

    /// Drop all entries and reset the position cursor.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.position = None;
    }

    /// Number of entries currently held.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` if no entries are held.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// The text buffer that backs the inline markdown editor.
///
/// `TextBuffer` is plain Rust data: it does not know about egui. The
/// rendering layer in [`crate::ui::editor_egui`] reads the buffer's
/// content, displays it, and writes the resulting cursor position back
/// via [`set_cursor`](Self::set_cursor) before returning.
#[derive(Debug, Default)]
pub struct TextBuffer {
    /// Whether the editor window should currently be rendered.
    pub is_open: bool,
    /// The body of the document (front matter stripped, if any).
    pub content: String,
    /// The original front matter text, preserved verbatim for round-trip
    /// on save.
    pub front_matter: Option<String>,
    /// The file this buffer is editing.
    pub file_path: PathBuf,
    /// Most recent save / open error, surfaced to the editor UI.
    pub error_message: Option<String>,
    /// The buffer's primary cursor.
    pub cursor: Cursor,
    /// The buffer's current selection.
    pub selection: Selection,
    /// Linear undo / redo history.
    pub undo_stack: UndoStack,
}

impl TextBuffer {
    /// Create a new, empty, closed buffer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Open a file for editing.
    ///
    /// Splits the raw file into front matter (preserved verbatim) and
    /// body (placed in `content`), resets the cursor to the start, and
    /// clears the undo history.
    ///
    /// PDF-backed Markdown files (those with a same-stem `.pdf` sibling)
    /// are blocked from editing — `is_open` remains `false` and
    /// `error_message` is set to explain the restriction.
    ///
    /// When `pdf_backing` is provided, it is used for the PDF-backing check
    /// instead of the per-access filesystem stat.
    pub fn open(
        &mut self,
        file_path: &Path,
        raw_content: &str,
        pdf_backing: Option<&crate::app::session::PdfBackingTracker>,
    ) {
        let is_pdf_backed = pdf_backing
            .map(|t| t.is_pdf_backed(file_path))
            .unwrap_or_else(|| crate::utils::path::has_pdf_backing(file_path));
        if is_pdf_backed {
            self.error_message = Some(
                "This file is auto-generated from a PDF and cannot be edited \
                 directly. Use write_yaml_header to modify front-matter."
                    .to_string(),
            );
            return;
        }
        self.is_open = true;
        self.file_path = file_path.to_path_buf();
        self.error_message = None;

        let doc = DocumentContent::parse(raw_content);
        self.content = doc.body;
        self.front_matter = doc.front_matter;
        self.cursor = Cursor::at_start();
        self.selection = Selection::collapsed(self.cursor);
        self.undo_stack.clear();
    }

    /// Close the editor, discarding all unsaved state.
    pub fn close(&mut self) {
        self.is_open = false;
        self.content.clear();
        self.front_matter = None;
        self.error_message = None;
        self.file_path = PathBuf::new();
        self.cursor = Cursor::at_start();
        self.selection = Selection::collapsed(self.cursor);
        self.undo_stack.clear();
    }

    /// Persist the buffer to disk and notify the rest of the app.
    ///
    /// On success: writes the file (re-attaching the preserved front
    /// matter), publishes a `FileEvent::Updated` via `producer`, and
    /// closes the editor. On failure: sets `error_message` and returns
    /// the error to the caller; the editor stays open so the user can
    /// retry.
    pub fn save(&mut self, producer: &FileEventProducer) -> Result<(), String> {
        let doc = DocumentContent {
            front_matter: self.front_matter.clone(),
            body: self.content.clone(),
        };
        let full_text = doc.to_string();

        if let Err(e) = fs::write(&self.file_path, full_text) {
            let err = format!("Failed to save: {}", e);
            tracing::error!(
                name = "editor.file.save_failed",
                path = %self.file_path.display(),
                error = %e,
                "Failed to save file from inline editor. Likely cause: disk full or permission denied. Operator should verify disk space and write permissions."
            );
            self.error_message = Some(err.clone());
            return Err(err);
        }

        // Tell the rest of the app this file changed so the
        // directory tree and tag manager refresh immediately,
        // without waiting for the next OS-level notify event.
        producer.publish_updated(&self.file_path);

        self.close();
        Ok(())
    }

    /// Update the cursor position. Called by the rendering layer at the
    /// end of each frame.
    pub fn set_cursor(&mut self, line: usize, column: usize, char_index: usize) {
        self.cursor = Cursor {
            line,
            column,
            char_index,
        };
    }

    /// Convert a char index into a byte offset for slicing `content`.
    ///
    /// Returns `content.len()` if the char index is past the end of the
    /// buffer.
    pub fn byte_index(&self, char_index: usize) -> usize {
        self.content
            .char_indices()
            .nth(char_index)
            .map(|(i, _)| i)
            .unwrap_or(self.content.len())
    }

    /// Number of lines in the buffer (1 + number of `\n`s).
    pub fn line_count(&self) -> usize {
        if self.content.is_empty() {
            0
        } else {
            self.content.chars().filter(|&c| c == '\n').count() + 1
        }
    }
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `text_buffer_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "text_buffer_tests.rs"]
mod tests;
