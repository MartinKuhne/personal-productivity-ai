//! Tests for `app/text_buffer.rs`.

use super::*;
use std::fs;
use tempfile::tempdir;

use crate::ui::test_helpers::app::noop_producer;

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
    buf.open(&path, &raw, None);

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
    buf.open(&path, &raw, None);

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
    buf.open(&path, &raw, None);
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
    buf.open(&path, &raw, None);
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
    buf.open(&path, &raw, None);
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
    buf.open(&path, &raw, None);
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
    buf.open(&path, "body", None);
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

#[test]
fn test_text_buffer_blocks_open_for_pdf_backed_files() {
    let dir = tempdir().unwrap();
    let md_path = dir.path().join("doc.md");
    let pdf_path = dir.path().join("doc.pdf");
    fs::write(&md_path, "# Hello").unwrap();
    fs::write(&pdf_path, "%PDF-1.4").unwrap();

    let mut buf = TextBuffer::new();
    buf.open(&md_path, "# Hello", None);

    assert!(!buf.is_open, "editor should not open for PDF-backed files");
    assert!(
        buf.error_message.is_some(),
        "error_message should be set for PDF-backed files"
    );
    assert!(
        buf.error_message
            .as_ref()
            .unwrap()
            .to_lowercase()
            .contains("pdf"),
        "error should mention PDF; got: {:?}",
        buf.error_message
    );
}
