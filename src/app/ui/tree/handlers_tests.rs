use super::super::context::TreeNodeContext;
use super::super::flatten::FlatRow;
use super::{apply_directory_row_click, apply_file_row_click, build_merge_prompt};
use crate::bus::events::user_command::UserCommand;
use std::collections::HashSet;
use std::path::PathBuf;

struct MockEnv {
    selection: crate::ui::FileSelection,
    tabs: crate::ui::Tabs,
    layout: crate::ui::panel_layout::PanelLayout,
    content_libraries: Vec<crate::config::ContentLibrary>,
    file_event_bus: crate::bus::core::Bus<crate::bus::events::file::FileEvent>,
    pdf_backing_tracker: crate::agent::session::PdfBackingTracker,
    user_command_bus: crate::bus::core::Bus<UserCommand>,
}
impl MockEnv {
    fn new() -> Self {
        Self {
            selection: Default::default(),
            tabs: Default::default(),
            layout: Default::default(),
            content_libraries: Default::default(),
            file_event_bus: crate::bus::core::Bus::new(),
            pdf_backing_tracker: crate::agent::session::PdfBackingTracker::new(),
            user_command_bus: crate::bus::core::Bus::new(),
        }
    }
    fn ctx(&self) -> TreeNodeContext {
        TreeNodeContext::from_app_state(
            &self.selection,
            &self.tabs,
            &self.layout,
            &self.content_libraries,
            None,
            self.file_event_bus.clone(),
            true,
            eframe::egui::Modifiers::default(),
            self.pdf_backing_tracker.clone(),
            self.user_command_bus.clone(),
        )
    }
}

#[test]
fn test_apply_file_row_click_returns_command() {
    let row = FlatRow {
        depth: 0,
        name: "test.md".to_string(),
        path: PathBuf::from("test.md"),
        is_dir: false,
        is_expanded: false,
    };
    let env = MockEnv::new();
    let mut mut_ctx = env.ctx();
    let cmd = apply_file_row_click(&mut mut_ctx, &row);
    assert_eq!(
        cmd,
        UserCommand::SelectFile {
            path: PathBuf::from("test.md"),
            multi: false
        }
    );
}

#[test]
fn test_apply_file_row_click_shift_multi() {
    let row = FlatRow {
        depth: 0,
        name: "test.md".to_string(),
        path: PathBuf::from("test.md"),
        is_dir: false,
        is_expanded: false,
    };
    let env = MockEnv::new();
    let mut mut_ctx = env.ctx();
    mut_ctx.ctx.modifiers.shift = true;
    let cmd = apply_file_row_click(&mut mut_ctx, &row);
    assert_eq!(
        cmd,
        UserCommand::SelectFile {
            path: PathBuf::from("test.md"),
            multi: true
        }
    );
}

#[test]
fn test_apply_directory_row_click_returns_command() {
    let row = FlatRow {
        depth: 0,
        name: "dir".to_string(),
        path: PathBuf::from("dir"),
        is_dir: true,
        is_expanded: false,
    };
    let env = MockEnv::new();
    let mut ctx = env.ctx();
    let cmd = apply_directory_row_click(&mut ctx, &row);
    assert!(ctx.tree_dirty);
    assert_eq!(
        cmd,
        UserCommand::SelectDirectory {
            path: PathBuf::from("dir"),
            toggle_expand: true
        }
    );
}

#[test]
fn test_build_merge_prompt() {
    let libs = vec![crate::config::ContentLibrary {
        name: "Notes".to_string(),
        root_folder: "C:/notes".to_string(),
        kind: "text".to_string(),
        readonly: false,
        priority: 0,
    }];
    let mut files = HashSet::new();
    files.insert(PathBuf::from("C:/notes/a.md"));
    files.insert(PathBuf::from("C:/notes/b.md"));
    let prompt = build_merge_prompt(&libs, &files);
    assert!(prompt.contains("Consolidate"));
    assert!(prompt.contains("a.md"));
    assert!(prompt.contains("b.md"));
}
