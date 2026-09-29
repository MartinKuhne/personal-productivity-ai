use std::path::PathBuf;

use eframe::egui;

use super::*;
use crate::config::AppConfig;
use crate::ui::test_helpers::run_ui_test;

#[test]
fn test_count_document_matches_empty_query() {
    let md = "# Title\n\nParagraph text.\n";
    assert_eq!(count_document_matches(md, ""), 0);
    assert_eq!(count_document_matches(md, "   "), 0);
}

#[test]
fn test_count_document_matches_empty_doc() {
    assert_eq!(count_document_matches("", "test"), 0);
}

#[test]
fn test_count_document_matches_case_insensitive() {
    let md = "# Hello World\n\nhello again\n\nHELLO there\n";
    assert_eq!(count_document_matches(md, "hello"), 3);
    assert_eq!(count_document_matches(md, "HELLO"), 3);
    assert_eq!(count_document_matches(md, "HeLLo"), 3);
}

#[test]
fn test_count_document_matches_across_element_types() {
    let md = r#"# Matching Heading

A paragraph with matching term.

```rust
let matching = true;
```

| Header | Matching Col |
| --- | --- |
| row1 | cell |
"#;
    assert_eq!(count_document_matches(md, "matching"), 4);
}

#[test]
fn test_next_match_index() {
    assert_eq!(next_match_index(0, 0), 0);
    assert_eq!(next_match_index(0, 1), 0);
    assert_eq!(next_match_index(0, 3), 1);
    assert_eq!(next_match_index(1, 3), 2);
    assert_eq!(next_match_index(2, 3), 0); // wrap-around
}

#[test]
fn test_prev_match_index() {
    assert_eq!(prev_match_index(0, 0), 0);
    assert_eq!(prev_match_index(0, 1), 0);
    assert_eq!(prev_match_index(0, 3), 2); // wrap-around
    assert_eq!(prev_match_index(2, 3), 1);
    assert_eq!(prev_match_index(1, 3), 0);
}

#[test]
fn test_format_match_status() {
    assert_eq!(format_match_status(0, 0), "No matches");
    assert_eq!(format_match_status(0, 5), "1 of 5");
    assert_eq!(format_match_status(2, 5), "3 of 5");
}

#[test]
fn test_show_find_dialog_closed_does_not_render() {
    let mut app = FastMdApp::empty_state(AppConfig::default());
    app.orchestrator.dialogs.find_dialog_open = false;

    let ctx = egui::Context::default();
    let raw_input = egui::RawInput::default();
    let _ = run_ui_test(&ctx, raw_input, |_| {
        show_find_dialog(&ctx, &mut app);
    });

    assert!(!app.orchestrator.dialogs.find_dialog_open);
}

#[test]
fn test_show_find_dialog_renders_when_open() {
    let mut app = FastMdApp::empty_state(AppConfig::default());
    let path = PathBuf::from("test.md");
    app.orchestrator.tabs.tabs.push(path.clone());
    app.orchestrator.selection.selected_file = Some(path);
    app.orchestrator.tabs.current_markdown = "# Hello\n\nWorld\n".to_string();
    app.orchestrator.dialogs.find_dialog_open = true;
    app.orchestrator.dialogs.find_query = "Hello".to_string();

    let ctx = egui::Context::default();
    let raw_input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1024.0, 768.0),
        )),
        ..egui::RawInput::default()
    };

    let _ = run_ui_test(&ctx, raw_input, |_| {
        show_find_dialog(&ctx, &mut app);
    });

    assert!(app.orchestrator.dialogs.find_dialog_open);
}

#[test]
fn test_find_command_lifecycle_and_navigation() {
    let mut app = FastMdApp::empty_state(AppConfig::default());
    let path = PathBuf::from("doc.md");
    app.orchestrator.tabs.tabs.push(path.clone());
    app.orchestrator.selection.selected_file = Some(path);
    app.orchestrator.tabs.current_markdown =
        "# Target Heading\n\nFirst target paragraph.\n\nSecond target paragraph.\n".to_string();

    // 1. Open Find Dialog
    app.orchestrator
        .apply_user_command(UserCommand::OpenFindDialog);
    assert!(app.orchestrator.dialogs.find_dialog_open);
    assert!(app.orchestrator.dialogs.find_focus_requested);

    // 2. Set Find Query (jumps to first match index 0)
    app.orchestrator
        .apply_user_command(UserCommand::SetFindQuery("target".to_string()));
    assert_eq!(app.orchestrator.dialogs.find_query, "target");
    assert_eq!(app.orchestrator.dialogs.find_match_index, 0);
    assert_eq!(
        app.orchestrator.tabs.scroll_to_search,
        Some(crate::ui::tabs::SearchJump::new("target", 0))
    );

    // 3. Find Next (jumps to second match index 1)
    app.orchestrator.apply_user_command(UserCommand::FindNext);
    assert_eq!(app.orchestrator.dialogs.find_match_index, 1);
    assert_eq!(
        app.orchestrator.tabs.scroll_to_search,
        Some(crate::ui::tabs::SearchJump::new("target", 1))
    );

    // 4. Find Next (jumps to third match index 2)
    app.orchestrator.apply_user_command(UserCommand::FindNext);
    assert_eq!(app.orchestrator.dialogs.find_match_index, 2);
    assert_eq!(
        app.orchestrator.tabs.scroll_to_search,
        Some(crate::ui::tabs::SearchJump::new("target", 2))
    );

    // 5. Find Next (wraps around back to match index 0)
    app.orchestrator.apply_user_command(UserCommand::FindNext);
    assert_eq!(app.orchestrator.dialogs.find_match_index, 0);
    assert_eq!(
        app.orchestrator.tabs.scroll_to_search,
        Some(crate::ui::tabs::SearchJump::new("target", 0))
    );

    // 6. Find Previous (wraps backwards to match index 2)
    app.orchestrator
        .apply_user_command(UserCommand::FindPrevious);
    assert_eq!(app.orchestrator.dialogs.find_match_index, 2);
    assert_eq!(
        app.orchestrator.tabs.scroll_to_search,
        Some(crate::ui::tabs::SearchJump::new("target", 2))
    );

    // 7. Close Find Dialog
    app.orchestrator
        .apply_user_command(UserCommand::CloseFindDialog);
    assert!(!app.orchestrator.dialogs.find_dialog_open);
}

#[test]
fn test_find_command_no_match_does_not_jump() {
    let mut app = FastMdApp::empty_state(AppConfig::default());
    let path = PathBuf::from("doc.md");
    app.orchestrator.tabs.tabs.push(path.clone());
    app.orchestrator.selection.selected_file = Some(path);
    app.orchestrator.tabs.current_markdown = "# Plain text\n".to_string();

    app.orchestrator
        .apply_user_command(UserCommand::SetFindQuery("nonexistent".to_string()));
    assert_eq!(app.orchestrator.tabs.scroll_to_search, None);

    app.orchestrator.apply_user_command(UserCommand::FindNext);
    assert_eq!(app.orchestrator.tabs.scroll_to_search, None);

    app.orchestrator
        .apply_user_command(UserCommand::FindPrevious);
    assert_eq!(app.orchestrator.tabs.scroll_to_search, None);
}

#[test]
fn test_ctrl_f_shortcut_opens_find_dialog() {
    let mut app = FastMdApp::empty_state(AppConfig::default());
    assert!(!app.orchestrator.dialogs.find_dialog_open);

    let ctx = egui::Context::default();
    let raw_input = egui::RawInput {
        events: vec![egui::Event::Key {
            key: egui::Key::F,
            physical_key: None,
            modifiers: egui::Modifiers::CTRL,
            pressed: true,
            repeat: false,
        }],
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1024.0, 768.0),
        )),
        ..egui::RawInput::default()
    };

    let _ = run_ui_test(&ctx, raw_input, |ui| {
        app.update_ui(ui);
    });

    app.orchestrator.drain_user_command_bus();
    assert!(app.orchestrator.dialogs.find_dialog_open);
}
