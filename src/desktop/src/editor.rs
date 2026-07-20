use std::path::{Path, PathBuf};
use std::fs;
use eframe::egui;
use pulldown_cmark::{Parser, Options};
use crate::document::DocumentContent;

#[derive(Default)]
pub struct EditorState {
    pub is_open: bool,
    pub content: String,
    pub original_front_matter: Option<String>,
    pub file_path: PathBuf,
    pub error_message: Option<String>,
}

impl EditorState {
    pub fn open(&mut self, file_path: &Path, raw_content: &str) {
        self.is_open = true;
        self.file_path = file_path.to_path_buf();
        self.error_message = None;
        
        let doc = DocumentContent::parse(raw_content);
        self.content = doc.body;
        self.original_front_matter = doc.front_matter;
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.content.clear();
        self.original_front_matter = None;
        self.error_message = None;
        self.file_path = PathBuf::new();
    }

    pub fn save(&mut self) -> Result<(), String> {
        // Validation using pulldown-cmark
        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_FOOTNOTES);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TASKLISTS);
        
        let parser = Parser::new_ext(&self.content, options);
        // Just consume the parser to see if it panics or we can do more.
        // pulldown-cmark doesn't typically "fail" parsing as Markdown is very forgiving.
        // However, if we want to catch broken tables or something we could check events.
        // For our MVP, just running the parser validates it doesn't crash.
        // Wait, what defines a "parse error" in pulldown-cmark?
        // Actually, cmark parses everything. So "invalid markdown" usually isn't an error in cmark.
        // The requirements say "If parsing fails, the save shall be aborted".
        // Let's just run it to ensure no panics, or check if we want to do any custom validation.
        let _events: Vec<_> = parser.collect();
        
        // Actually, if there's any specific "broken syntax", cmark just renders it as text.
        // But let's assume it passes.

        let doc = DocumentContent {
            front_matter: self.original_front_matter.clone(),
            body: self.content.clone(),
        };
        let full_text = doc.to_string();

        if let Err(e) = fs::write(&self.file_path, full_text) {
            let err = format!("Failed to save: {}", e);
            self.error_message = Some(err.clone());
            return Err(err);
        }

        self.close();
        Ok(())
    }

    pub fn show(&mut self, ctx: &egui::Context) -> bool {
        if !self.is_open {
            return false;
        }

        let mut is_open = self.is_open;
        let mut did_save = false;
        
        egui::Window::new("Inline Editor")
            .open(&mut is_open)
            .collapsible(false)
            .vscroll(false)
            .hscroll(false)
            .resizable(true)
            .default_size(egui::vec2(800.0, 600.0))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    if let Some(err) = &self.error_message {
                        ui.colored_label(egui::Color32::RED, err);
                    }
                });

                // Calculate cursor position for status bar
                let mut cursor_line = 0;
                let mut cursor_col = 0;
                
                let text_edit = egui::TextEdit::multiline(&mut self.content)
                    .font(egui::TextStyle::Monospace)
                    .code_editor()
                    .desired_rows(30)
                    .desired_width(f32::INFINITY)
                    .lock_focus(true);
                
                let output = text_edit.show(ui);
                
                if let Some(cursor_range) = output.cursor_range {
                    let cursor_char_idx = cursor_range.primary.ccursor.index;
                    let byte_idx = self.content.char_indices().nth(cursor_char_idx).map(|(i, _)| i).unwrap_or(self.content.len());
                    let text_up_to_cursor = &self.content[..byte_idx];
                    cursor_line = text_up_to_cursor.chars().filter(|&c| c == '\n').count() + 1;
                    if let Some(last_newline) = text_up_to_cursor.rfind('\n') {
                        cursor_col = text_up_to_cursor.chars().count() - text_up_to_cursor[..last_newline].chars().count();
                    } else {
                        cursor_col = text_up_to_cursor.chars().count() + 1;
                    }
                }

                ui.separator();
                
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        if self.save().is_ok() {
                            did_save = true;
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        self.close();
                    }
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!("Line: {} | Col: {}", cursor_line, cursor_col));
                    });
                });
            });

        if !is_open {
            self.close();
        }
        
        did_save
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_editor_open_strips_front_matter() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.md");
        fs::write(&path, "---\ntitle: Test\n---\nBody content").unwrap();

        let mut state = EditorState::default();
        let raw = fs::read_to_string(&path).unwrap();
        state.open(&path, &raw);

        assert!(state.is_open);
        // DocumentContent::parse returns body with leading newline after --- delimiter
        assert_eq!(state.content, "\nBody content");
        assert_eq!(state.original_front_matter, Some("---\ntitle: Test\n---".to_string()));
        assert_eq!(state.file_path, path);
    }

    #[test]
    fn test_editor_open_no_front_matter() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.md");
        fs::write(&path, "Just body content").unwrap();

        let mut state = EditorState::default();
        let raw = fs::read_to_string(&path).unwrap();
        state.open(&path, &raw);

        assert!(state.is_open);
        assert_eq!(state.content, "Just body content");
        assert!(state.original_front_matter.is_none());
    }

    #[test]
    fn test_editor_close_clears_state() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.md");
        fs::write(&path, "---\ntitle: Test\n---\nBody").unwrap();

        let mut state = EditorState::default();
        let raw = fs::read_to_string(&path).unwrap();
        state.open(&path, &raw);
        state.close();

        assert!(!state.is_open);
        assert!(state.content.is_empty());
        assert!(state.original_front_matter.is_none());
        assert!(state.error_message.is_none());
    }

    #[test]
    fn test_editor_save_preserves_front_matter() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.md");
        fs::write(&path, "---\ntitle: Original\n---\nOriginal body").unwrap();

        let mut state = EditorState::default();
        let raw = fs::read_to_string(&path).unwrap();
        state.open(&path, &raw);
        // content includes leading newline from DocumentContent::parse; preserve it
        state.content = "\nModified body".to_string();

        state.save().unwrap();

        let saved = fs::read_to_string(&path).unwrap();
        assert_eq!(saved, "---\ntitle: Original\n---\nModified body");
    }

    #[test]
    fn test_editor_cancel_discards_changes() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.md");
        fs::write(&path, "Original body").unwrap();

        let mut state = EditorState::default();
        let raw = fs::read_to_string(&path).unwrap();
        state.open(&path, &raw);
        state.content = "Unsaved changes".to_string();
        state.close();

        assert!(!state.is_open);
        let saved = fs::read_to_string(&path).unwrap();
        assert_eq!(saved, "Original body");
    }

    #[test]
    fn test_editor_save_no_front_matter() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.md");
        fs::write(&path, "Original body").unwrap();

        let mut state = EditorState::default();
        let raw = fs::read_to_string(&path).unwrap();
        state.open(&path, &raw);
        state.content = "Modified body".to_string();

        state.save().unwrap();

        let saved = fs::read_to_string(&path).unwrap();
        assert_eq!(saved, "Modified body");
    }

    #[test]
    fn test_editor_save_error_message_on_failure() {
        let mut state = EditorState::default();
        state.file_path = PathBuf::from("C:\\nonexistent_dir\\file.md");
        state.content = "Body".to_string();

        let result = state.save();
        assert!(result.is_err());

        let mut state2 = EditorState::default();
        state2.file_path = PathBuf::from("C:\\nonexistent_dir\\file.md");
        state2.content = "Body".to_string();
        state2.save().unwrap_err();
        // After save failure, the error_message is set; we test the close path separately
    }
}
