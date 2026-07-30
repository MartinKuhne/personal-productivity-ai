//! Egui-free text editing model — `TextBuffer`, `Cursor`, `Selection`, and `UndoStack`.
//!
//! This module is the data layer for the inline markdown editor. It owns the
//! file content, the front matter (preserved verbatim across saves), the
//! cursor / selection state, and the undo history. Nothing in this module
//! imports `eframe::egui`; the rendering layer in
//! [`crate::editor_egui`] adapts the buffer to egui's immediate-mode
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
/// rendering layer in [`crate::editor_egui`] reads the buffer's
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
    pub fn open(&mut self, file_path: &Path, raw_content: &str) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::core::Bus;
    use crate::bus::events::file::FileEvent;
    use std::fs;
    use tempfile::tempdir;

    /// A producer that publishes to a throwaway bus. Tests don't need
    /// to consume the events — they only care about the file I/O
    /// outcome.
    fn noop_producer() -> FileEventProducer<'static> {
        let bus: &'static Bus<FileEvent> = Box::leak(Box::new(Bus::new()));
        FileEventProducer::new(bus)
    }

    // ---- Cursor ----

    #[test]
    fn test_cursor_at_start_is_origin() {
        let c = Cursor::at_start();
        assert_eq!(c.line, 0);
        assert_eq!(c.column, 0);
        assert_eq!(c.char_index, 0);
    }

    #[test]
    fn test_cursor_default_matches_at_start() {
        assert_eq!(Cursor::default(), Cursor::at_start());
    }

    // ---- Selection ----

    #[test]
    fn test_selection_collapsed_at_origin() {
        let s = Selection::collapsed(Cursor::at_start());
        assert!(s.is_collapsed());
    }

    #[test]
    fn test_selection_with_distinct_ends_is_not_collapsed() {
        let s = Selection {
            anchor: Cursor::at_start(),
            head: Cursor {
                line: 1,
                column: 0,
                char_index: 5,
            },
        };
        assert!(!s.is_collapsed());
    }

    // ---- UndoStack ----

    #[test]
    fn test_undo_stack_starts_empty() {
        let stack = UndoStack::new();
        assert!(stack.is_empty());
        assert_eq!(stack.len(), 0);
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
    }

    #[test]
    fn test_undo_stack_push_then_undo_restores_previous() {
        let mut stack = UndoStack::new();
        stack.push(
            "a".to_string(),
            Cursor::at_start(),
            Selection::collapsed(Cursor::at_start()),
        );
        stack.push(
            "ab".to_string(),
            Cursor::at_start(),
            Selection::collapsed(Cursor::at_start()),
        );
        assert!(stack.can_undo());
        assert!(!stack.can_redo());

        let prev = stack.undo().unwrap();
        assert_eq!(prev.content, "a");
    }

    #[test]
    fn test_undo_stack_push_clears_redo_branch() {
        let mut stack = UndoStack::new();
        stack.push(
            "a".to_string(),
            Cursor::at_start(),
            Selection::collapsed(Cursor::at_start()),
        );
        stack.push(
            "ab".to_string(),
            Cursor::at_start(),
            Selection::collapsed(Cursor::at_start()),
        );
        // Move back, then push a new entry. The redo branch is gone.
        let _ = stack.undo();
        stack.push(
            "z".to_string(),
            Cursor::at_start(),
            Selection::collapsed(Cursor::at_start()),
        );
        assert!(!stack.can_redo());
    }

    #[test]
    fn test_undo_stack_coalesces_identical_content() {
        // egui emits a TextChanged per keystroke; many of those keep the
        // content unchanged. UndoStack should coalesce them so the
        // history doesn't fill up with no-op entries.
        let mut stack = UndoStack::new();
        for _ in 0..10 {
            stack.push(
                "same".to_string(),
                Cursor::at_start(),
                Selection::collapsed(Cursor::at_start()),
            );
        }
        assert_eq!(stack.len(), 1, "identical content should coalesce");
    }

    #[test]
    fn test_undo_stack_capacity_evicts_oldest() {
        let mut stack = UndoStack::with_capacity(2);
        stack.push(
            "a".to_string(),
            Cursor::at_start(),
            Selection::collapsed(Cursor::at_start()),
        );
        stack.push(
            "b".to_string(),
            Cursor::at_start(),
            Selection::collapsed(Cursor::at_start()),
        );
        stack.push(
            "c".to_string(),
            Cursor::at_start(),
            Selection::collapsed(Cursor::at_start()),
        );
        assert_eq!(stack.len(), 2, "oldest entry should be evicted");
    }

    #[test]
    fn test_undo_stack_cannot_undo_below_zero() {
        let mut stack = UndoStack::new();
        stack.push(
            "a".to_string(),
            Cursor::at_start(),
            Selection::collapsed(Cursor::at_start()),
        );
        let _ = stack.undo(); // already at position 0
        assert!(stack.undo().is_none());
    }

    #[test]
    fn test_undo_stack_clear_resets_state() {
        let mut stack = UndoStack::new();
        stack.push(
            "a".to_string(),
            Cursor::at_start(),
            Selection::collapsed(Cursor::at_start()),
        );
        stack.clear();
        assert!(stack.is_empty());
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
    }

    // ---- TextBuffer: open / close ----

    #[test]
    fn test_text_buffer_open_strips_front_matter() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.md");
        fs::write(&path, "---\ntitle: Test\n---\nBody content").unwrap();

        let mut buf = TextBuffer::new();
        let raw = fs::read_to_string(&path).unwrap();
        buf.open(&path, &raw);

        assert!(buf.is_open);
        // DocumentContent::parse returns body with leading newline after --- delimiter
        assert_eq!(buf.content, "\nBody content");
        assert_eq!(buf.front_matter, Some("---\ntitle: Test\n---".to_string()));
        assert_eq!(buf.file_path, path);
        // Cursor resets to origin on open.
        assert_eq!(buf.cursor, Cursor::at_start());
    }

    #[test]
    fn test_text_buffer_open_no_front_matter() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.md");
        fs::write(&path, "Just body content").unwrap();

        let mut buf = TextBuffer::new();
        let raw = fs::read_to_string(&path).unwrap();
        buf.open(&path, &raw);

        assert!(buf.is_open);
        assert_eq!(buf.content, "Just body content");
        assert!(buf.front_matter.is_none());
    }

    #[test]
    fn test_text_buffer_close_clears_state() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.md");
        fs::write(&path, "---\ntitle: Test\n---\nBody").unwrap();

        let mut buf = TextBuffer::new();
        let raw = fs::read_to_string(&path).unwrap();
        buf.open(&path, &raw);
        buf.close();

        assert!(!buf.is_open);
        assert!(buf.content.is_empty());
        assert!(buf.front_matter.is_none());
        assert!(buf.error_message.is_none());
        assert!(buf.file_path.as_os_str().is_empty());
    }

    #[test]
    fn test_text_buffer_close_clears_undo_stack() {
        let mut buf = TextBuffer::new();
        buf.undo_stack.push(
            "x".to_string(),
            Cursor::at_start(),
            Selection::collapsed(Cursor::at_start()),
        );
        buf.close();
        assert!(buf.undo_stack.is_empty());
    }

    // ---- TextBuffer: save ----

    #[test]
    fn test_text_buffer_save_preserves_front_matter() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.md");
        fs::write(&path, "---\ntitle: Original\n---\nOriginal body").unwrap();

        let mut buf = TextBuffer::new();
        let raw = fs::read_to_string(&path).unwrap();
        buf.open(&path, &raw);
        buf.content = "\nModified body".to_string();

        buf.save(&noop_producer()).unwrap();

        let saved = fs::read_to_string(&path).unwrap();
        assert_eq!(saved, "---\ntitle: Original\n---\nModified body");
    }

    #[test]
    fn test_text_buffer_save_no_front_matter() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.md");
        fs::write(&path, "Original body").unwrap();

        let mut buf = TextBuffer::new();
        let raw = fs::read_to_string(&path).unwrap();
        buf.open(&path, &raw);
        buf.content = "Modified body".to_string();

        buf.save(&noop_producer()).unwrap();

        let saved = fs::read_to_string(&path).unwrap();
        assert_eq!(saved, "Modified body");
    }

    #[test]
    fn test_text_buffer_cancel_discards_changes() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.md");
        fs::write(&path, "Original body").unwrap();

        let mut buf = TextBuffer::new();
        let raw = fs::read_to_string(&path).unwrap();
        buf.open(&path, &raw);
        buf.content = "Unsaved changes".to_string();
        buf.close();

        assert!(!buf.is_open);
        let saved = fs::read_to_string(&path).unwrap();
        assert_eq!(saved, "Original body");
    }

    #[test]
    fn test_text_buffer_save_error_message_on_failure() {
        // Build a "missing parent" path inside a tempdir we never
        // create a subdir under. The previous test hardcoded
        // `C:\nonexistent_dir\...` which was a flake waiting to
        // happen: on the GitHub Windows runner the path resolved to
        // a writable location, so `fs::write` succeeded and `save`
        // returned `Ok(())`, breaking the assertion. Building the
        // bad path off a fresh `tempdir()` makes the missing parent
        // deterministic on every platform and every runner.
        let dir = tempdir().unwrap();
        let bad_path = dir.path().join("missing_subdir").join("file.md");

        let mut buf = TextBuffer {
            file_path: bad_path.clone(),
            content: "Body".to_string(),
            ..TextBuffer::default()
        };

        let producer = noop_producer();
        let result = buf.save(&producer);
        assert!(result.is_err(), "expected save to fail, got {:?}", result);
        assert!(
            buf.error_message.is_some(),
            "expected error_message to be set after a failed save"
        );

        let mut buf2 = TextBuffer {
            file_path: dir.path().join("missing_subdir_2").join("file.md"),
            content: "Body".to_string(),
            ..TextBuffer::default()
        };
        let producer2 = noop_producer();
        buf2.save(&producer2).unwrap_err();
        // After save failure, the error_message is set; we test the
        // close path separately.
    }

    #[test]
    fn test_text_buffer_save_closes_buffer_on_success() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.md");
        fs::write(&path, "body").unwrap();

        let mut buf = TextBuffer::new();
        buf.open(&path, "body");
        buf.content = "modified".to_string();
        buf.save(&noop_producer()).unwrap();
        assert!(!buf.is_open, "successful save should close the buffer");
    }

    #[test]
    fn test_text_buffer_save_keeps_buffer_open_on_failure() {
        let dir = tempdir().unwrap();
        let bad_path = dir.path().join("missing").join("file.md");

        let mut buf = TextBuffer {
            is_open: true,
            file_path: bad_path,
            content: "body".to_string(),
            ..TextBuffer::default()
        };
        buf.save(&noop_producer()).unwrap_err();
        assert!(buf.is_open, "failed save should leave the buffer open");
    }

    // ---- TextBuffer: byte_index / line_count ----

    #[test]
    fn test_text_buffer_byte_index_handles_multibyte() {
        let mut buf = TextBuffer::new();
        // "héllo" — 'h'=1 byte, 'é'=2 bytes, 'l'=1, 'l'=1, 'o'=1. 5 chars, 6 bytes.
        buf.content = "héllo".to_string();

        assert_eq!(buf.byte_index(0), 0); // start of 'h'
        assert_eq!(buf.byte_index(1), 1); // start of 'é'
        assert_eq!(buf.byte_index(2), 3); // start of first 'l'
        assert_eq!(buf.byte_index(5), 6); // past end
    }

    #[test]
    fn test_text_buffer_line_count() {
        let mut buf = TextBuffer::new();
        assert_eq!(buf.line_count(), 0);
        buf.content = "one".to_string();
        assert_eq!(buf.line_count(), 1);
        buf.content = "one\ntwo".to_string();
        assert_eq!(buf.line_count(), 2);
        buf.content = "a\nb\nc".to_string();
        assert_eq!(buf.line_count(), 3);
    }

    #[test]
    fn test_text_buffer_set_cursor_updates_position() {
        let mut buf = TextBuffer::new();
        buf.set_cursor(2, 5, 17);
        assert_eq!(buf.cursor.line, 2);
        assert_eq!(buf.cursor.column, 5);
        assert_eq!(buf.cursor.char_index, 17);
    }
}
