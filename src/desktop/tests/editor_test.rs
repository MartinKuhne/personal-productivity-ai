use fastmd::editor::{EditorColors, EditorState};
use fastmd::file_events::{Bus, FileEvent, FileEventProducer};
use std::path::Path;

/// A producer that publishes to a throwaway bus. Tests don't
/// need to consume the events.
fn noop_producer() -> FileEventProducer<'static> {
    let bus: &'static Bus<FileEvent> = Box::leak(Box::new(Bus::new()));
    FileEventProducer::new(bus)
}

#[test]
fn test_editor_open_close() {
    let mut editor = EditorState::default();
    
    // Test open
    editor.open(Path::new("test.md"), "---\ntitle: test\n---\n# Hello");
    
    assert!(editor.is_open);
    assert_eq!(editor.file_path, Path::new("test.md"));
    assert_eq!(editor.original_front_matter.as_deref(), Some("---\ntitle: test\n---"));
    assert_eq!(editor.content, "\n# Hello");
    assert_eq!(editor.error_message, None);
    
    // Test close
    editor.close();
    
    assert!(!editor.is_open);
    assert!(editor.content.is_empty());
    assert_eq!(editor.original_front_matter, None);
    assert_eq!(editor.error_message, None);
    assert_eq!(editor.file_path.as_os_str(), "");
}

#[test]
fn test_editor_save() {
    let mut editor = EditorState::default();
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join("test_save.md");
    
    // open
    editor.open(&file_path, "---\ntitle: test\n---\n# Hello");
    
    // edit
    editor.content = "\n# Modified".to_string();
    
    // save
    let result = editor.save(&noop_producer());
    assert!(result.is_ok());
    assert!(!editor.is_open);
    
    // verify file contents
    let saved_content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(saved_content, "---\ntitle: test\n---\n# Modified");
    
    std::fs::remove_file(file_path).ok();
}

#[test]
fn test_editor_save_cancel() {
    let mut editor = EditorState::default();
    editor.open(Path::new("test_cancel.md"), "original");
    editor.content = "modified".to_string();
    editor.close();
    
    assert!(!editor.is_open);
    assert!(editor.content.is_empty());
}

// --- REQ-261: inverted (black text on white) colour scheme -------------

#[test]
fn test_req261_editor_uses_inverted_palette() {
    // REQ-261: the inline editor shall have an inverted, black text on
    // white background colour scheme.
    let colors = EditorColors::inverted();
    assert_eq!(colors.background, eframe::egui::Color32::WHITE);
    assert_eq!(colors.text, eframe::egui::Color32::BLACK);
    // The border must also be dark so the inverted surface stands out
    // against the rest of the dark-themed application.
    assert_eq!(colors.border, eframe::egui::Color32::BLACK);
}

#[test]
fn test_req261_editor_default_palette_is_inverted() {
    // Callers that don't pass a palette must still get the inverted one
    // — the default satisfies REQ-261 transparently.
    assert_eq!(EditorColors::default(), EditorColors::inverted());
}

#[test]
fn test_req261_editor_palette_is_copy_and_comparable() {
    // The palette needs to be passed by value into the render path and
    // also be comparable for tests. A regression that turned it into a
    // non-Copy type would force callers to clone it and would break
    // these tests; that's a useful tripwire.
    let a = EditorColors::inverted();
    let b = a; // Copy
    assert_eq!(a, b);

    let mut palette = EditorColors::inverted();
    palette.background = eframe::egui::Color32::BLACK;
    assert_ne!(palette, EditorColors::inverted());
}

