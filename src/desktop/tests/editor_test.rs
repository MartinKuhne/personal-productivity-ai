use fastmd::editor::EditorState;
use std::path::Path;

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
    let result = editor.save();
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

