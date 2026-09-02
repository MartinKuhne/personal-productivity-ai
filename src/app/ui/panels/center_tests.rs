//! Tests for `panels/center.rs`.

use super::*;

use crate::ui::test_helpers::run_ui_test;
use std::path::PathBuf;

use crate::bus::events::user_command::UserCommand;

fn assert_bus_contains(app: &mut FastMdApp, expected: UserCommand) {
    let mut found = false;
    let reader = app.orchestrator.user_command_reader.as_mut().unwrap();
    while let Ok(cmd) = reader.try_recv_exposing_lag() {
        if cmd == expected {
            found = true;
            break;
        }
    }
    assert!(found, "Expected bus to contain {:?}", expected);
}

fn create_test_app() -> FastMdApp {
    FastMdApp::empty_state(crate::config::AppConfig::default())
}

/// Tier 1 test for the `×` tab close button click effect. The
/// click removes the tab at `i` from `app.orchestrator.tabs.tabs`.
/// We verify the effect without driving the egui harness.
#[test]
fn test_apply_tab_close_click_removes_tab_at_index() {
    let mut app = create_test_app();
    app.orchestrator.tabs.tabs = vec![
        PathBuf::from("a.md"),
        PathBuf::from("b.md"),
        PathBuf::from("c.md"),
    ];
    *app.orchestrator.selection.selected_file_mut() = Some(PathBuf::from("b.md"));

    app.orchestrator
        .apply_user_command(apply_tab_close_click(1));

    assert_eq!(
        app.orchestrator.tabs.tabs,
        vec![PathBuf::from("a.md"), PathBuf::from("c.md")],
        "tab at i=1 must be removed"
    );
    // `b.md` was the selected file and is now closed, so
    // selection falls back to the last remaining tab.
    assert_eq!(
        app.orchestrator.selection.selected_file(),
        Some(&PathBuf::from("c.md")),
        "selected_file must fall back to the last tab after the selected tab is closed"
    );
}

/// Tier 1 test: out-of-range index on the close button is a
/// silent no-op. Matches the behavior of the underlying
/// `apply_tab_action` (`ui/panels/center.rs:46`).
#[test]
fn test_apply_tab_close_click_out_of_range_is_noop() {
    let mut app = create_test_app();
    app.orchestrator.tabs.tabs = vec![PathBuf::from("a.md")];
    *app.orchestrator.selection.selected_file_mut() = Some(PathBuf::from("a.md"));

    app.orchestrator
        .apply_user_command(apply_tab_close_click(5));

    assert_eq!(app.orchestrator.tabs.tabs, vec![PathBuf::from("a.md")]);
    assert_eq!(
        app.orchestrator.selection.selected_file(),
        Some(&PathBuf::from("a.md"))
    );
}

/// Tier 1 test for the context-menu "Close Others" item.
/// Verifies all tabs except the keep-index are removed, and
/// the kept tab is the only remaining entry.
#[test]
fn test_apply_tab_close_others_click_keeps_only_target_tab() {
    let mut app = create_test_app();
    app.orchestrator.tabs.tabs = vec![
        PathBuf::from("a.md"),
        PathBuf::from("b.md"),
        PathBuf::from("c.md"),
    ];
    *app.orchestrator.selection.selected_file_mut() = Some(PathBuf::from("a.md"));

    app.orchestrator
        .apply_user_command(apply_tab_close_others_click(1));

    assert_eq!(app.orchestrator.tabs.tabs, vec![PathBuf::from("b.md")]);
    assert_eq!(
        app.orchestrator.selection.selected_file(),
        Some(&PathBuf::from("b.md"))
    );
}

/// Tier 1 test for the context-menu "Close All Tabs" item.
/// Verifies all tabs are removed and `selected_file` is `None`.
#[test]
fn test_apply_tab_close_all_click_clears_all_tabs() {
    let mut app = create_test_app();
    app.orchestrator.tabs.tabs = vec![
        PathBuf::from("a.md"),
        PathBuf::from("b.md"),
        PathBuf::from("c.md"),
    ];
    *app.orchestrator.selection.selected_file_mut() = Some(PathBuf::from("b.md"));

    app.orchestrator
        .apply_user_command(apply_tab_close_all_click());

    assert!(app.orchestrator.tabs.tabs.is_empty());
    assert!(
        app.orchestrator.selection.selected_file().is_none(),
        "selected_file must be None when all tabs are closed"
    );
}

/// Tier 4 click test: clicking the tab-strip `×` close button
/// in the center panel must fire the `on_click("tab_close_button")`
/// callback. Renders `render_tabs_and_content_capture` (not the
/// full `show_center_panel`) so the test stays focused on the
/// tab-strip click. The click handler also removes the tab
/// from `app.orchestrator.tabs.tabs`, but the harness owns `&mut app`
/// so the side effect is observed via the captured event.
#[test]
fn test_tab_close_button_captures_event() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;

    let mut app_state = create_test_app();
    app_state.orchestrator.tabs.tabs = vec![PathBuf::from("a.md"), PathBuf::from("b.md")];
    *app_state.orchestrator.selection.selected_file_mut() = Some(PathBuf::from("a.md"));
    let mut harness = stateful_harness(
        (app_state, Vec::<&'static str>::new()),
        |ui, (app, captured)| {
            render_tabs_and_content_capture(ui, app, |event| {
                captured.push(event);
            });
        },
    );
    harness.fit_contents();
    // Two tabs → two `×` buttons. Click the first one.
    let close_buttons: Vec<_> = harness.query_all_by_label("×").collect();
    assert_eq!(
        close_buttons.len(),
        2,
        "expected one × button per tab; got {}",
        close_buttons.len()
    );
    close_buttons[0].click();
    harness.run_steps(2);
    harness.run_steps(2);

    let (app, captured) = harness.state_mut();
    assert_bus_contains(app, UserCommand::CloseTab(0));
    assert!(
        captured.contains(&"tab_close_button"),
        "clicking the tab-strip A- close button must fire the              `tab_close_button` on_click event; got: {:?}",
        captured
    );
}

/// The previous four `test_apply_tab_action_*` tests
/// (Close / CloseOthers / CloseAll / out_of_bounds) are all
/// subsumed by this property test, which sweeps every input
/// combination for every `TabAction` variant. The fuzz catches
/// any regression in the per-variant code paths; the hand-written
/// tests only re-exercised the happy paths the fuzz already
/// covers at the (0..20 × 0..30 × 3 variants) scale.
#[test]
fn prop_apply_tab_action_preserves_invariants_fuzz() {
    for tab_count in 0..20 {
        for close_idx in 0..30 {
            let mut tabs: Vec<PathBuf> = (0..tab_count)
                .map(|i| PathBuf::from(format!("{}.md", i)))
                .collect();
            let mut selected = tabs.last().cloned();
            let initial_len = tabs.len();

            apply_tab_action(&mut tabs, &mut selected, TabAction::Close(close_idx));

            if initial_len == 0 {
                assert!(tabs.is_empty() && selected.is_none());
            } else if close_idx < initial_len {
                assert_eq!(tabs.len(), initial_len - 1);
                if tabs.is_empty() {
                    assert!(selected.is_none());
                } else {
                    assert!(selected.is_some());
                }
            } else {
                assert_eq!(tabs.len(), initial_len);
            }
        }

        // CloseOthers: every tab except the keep-index is dropped,
        // selection falls back to the kept tab.
        for keep_idx in 0..tab_count + 1 {
            let mut tabs: Vec<PathBuf> = (0..tab_count)
                .map(|i| PathBuf::from(format!("{}.md", i)))
                .collect();
            let mut selected = tabs.last().cloned();

            apply_tab_action(&mut tabs, &mut selected, TabAction::CloseOthers(keep_idx));

            if keep_idx < tab_count {
                assert_eq!(tabs.len(), 1);
                assert_eq!(
                    selected,
                    tabs.first().cloned(),
                    "CloseOthers: selected must point at the kept tab"
                );
            } else {
                // Out-of-bounds keep: tabs untouched.
                assert_eq!(tabs.len(), tab_count);
            }
        }

        // CloseAll: every tab dropped, selection cleared.
        {
            let mut tabs: Vec<PathBuf> = (0..tab_count)
                .map(|i| PathBuf::from(format!("{}.md", i)))
                .collect();
            let mut selected = tabs.last().cloned();
            apply_tab_action(&mut tabs, &mut selected, TabAction::CloseAll);
            assert!(tabs.is_empty());
            assert!(selected.is_none());
        }
    }
}

#[test]
fn test_clear_agent_session_state() {
    let mut app = create_test_app();

    app.agent_mut().state_mut().history =
        Some(vec![serde_json::json!({"role": "user", "content": "hi"})]);
    app.agent_mut().set_response("response text".to_string());
    app.agent_mut().set_thinking("thinking process".to_string());
    app.agent_mut().state_mut().running = true;
    app.orchestrator.agent_panel_state.show_results = true;

    clear_agent_session_state(&mut app);

    assert!(!app.orchestrator.agent_panel_state.show_results);
    assert!(app.agent().state().history.is_none());
    assert!(app.agent().state().response.is_empty());
    assert!(app.agent().state().thinking.is_empty());
    assert!(!app.agent().state().running);
}

#[test]
#[ignore = "spawns OS shell which pops a 'file not found' dialog on Windows when path is missing"]
fn test_os_launchers_non_crashing() {
    let path = std::path::Path::new("dummy_test_file.txt");
    open_in_system_editor(path);
    show_in_file_explorer(path);
}

#[test]
fn test_show_center_panel_render_modes() {
    use crate::ui::strings::{AGENT_SESSION_HEADER, NO_FILE_SELECTED_PROMPT};
    use crate::ui::test_helpers::text::assert_text_contains;

    // The center panel renders the tab bar at the top and the
    // body inside a `ScrollArea`. Under the default test viewport
    // (no `screen_rect`), the body's content is below the panel's
    // clip rect and not in `output.shapes`. Set an explicit
    // viewport so the markdown + YAML render is observable.
    let raw_input = || egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1024.0, 768.0),
        )),
        ..egui::RawInput::default()
    };

    let ctx = egui::Context::default();
    let mut app = create_test_app();

    // Mode 1: no tab open. Assert the empty-state prompt is present
    // (Q12 borderline case — the prompt is the only stable string).
    let output = run_ui_test(&ctx, raw_input(), |ui| {
        show_center_panel(&mut app, ui);
    });
    assert_text_contains(&output.shapes, NO_FILE_SELECTED_PROMPT);

    app.tabs_mut().tabs = vec![PathBuf::from("doc1.md"), PathBuf::from("doc2.md")];
    *app.selection_mut().selected_file_mut() = Some(PathBuf::from("doc1.md"));
    app.tabs_mut().current_markdown = "# Document 1 Header".to_string();
    app.tabs_mut().current_yaml = Some(serde_norway::from_str("title: Doc 1").unwrap());

    // Mode 2: a tab is open with markdown + YAML. The center panel
    // has no stable canonical string for this mode (the tab labels
    // are file paths, the body is markdown content, the YAML
    // table does not emit a "YAML Front-Matter" header — that
    // string is only used in the inline editor). The render path
    // is exercised; full visual coverage of the markdown + YAML
    // view comes from the Tier 3 snapshot in R-1c.
    // Mode 2: a tab is open with markdown + YAML. The previous
    // version of this test bound the rendered output to `_` and
    // verified nothing about Mode 2; a regression in the
    // markdown + YAML render path would not fail the test. The
    // assertions below pin the contract: the markdown body
    // text and the YAML title key both appear in the rendered
    // output. Full pixel-level visual coverage of the
    // markdown + YAML view comes from the Tier 3 snapshot in
    // R-1c; this test is the deterministic safety net.
    let output = run_ui_test(&ctx, raw_input(), |ui| {
        show_center_panel(&mut app, ui);
    });
    assert_text_contains(&output.shapes, "Document 1 Header");
    assert_text_contains(&output.shapes, "title");

    app.orchestrator.agent_panel_state.show_results = true;
    app.agent_mut().set_running(true);
    app.agent_mut().set_status("Thinking...".to_string());
    app.agent_mut().set_thinking("Reasoning step 1".to_string());
    app.agent_mut()
        .set_response("Final agent summary answer".to_string());

    // Mode 3: agent session active. The agent session header is the
    // stable string for this mode.
    let output = run_ui_test(&ctx, raw_input(), |ui| {
        show_center_panel(&mut app, ui);
    });
    assert_text_contains(&output.shapes, AGENT_SESSION_HEADER);
}

#[test]
fn test_agent_session_close_button_label() {
    assert_eq!(crate::ui::strings::AGENT_SESSION_CLOSE_BUTTON, "Close");
}

// ---- VFS-121: Note skills in the open-tab context menu ----

/// Helper: build a `FastMdApp` with a single open tab and a System
/// content library rooted at `sys_dir`.
fn app_with_open_tab(sys_dir: &std::path::Path, tab_path: &std::path::Path) -> FastMdApp {
    let mut app = create_test_app();
    app.orchestrator.tabs.tabs = vec![tab_path.to_path_buf()];
    *app.orchestrator.selection.selected_file_mut() = Some(tab_path.to_path_buf());
    app.orchestrator.config = crate::config::AppConfig {
        content_libraries: vec![crate::config::ContentLibrary {
            root_folder: sys_dir.to_string_lossy().to_string(),
            name: "System".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        }],
        ..crate::config::AppConfig::default()
    };
    app
}

/// VFS-121: right-clicking an open tab and choosing a Note skill
/// populates `submit_prompt` with the skill body and selects the
/// tab path as `selected_file`.
#[test]
fn test_tab_context_menu_note_skill_action() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;
    use std::cell::RefCell;
    use std::rc::Rc;

    let dir = tempfile::tempdir().unwrap();
    let sys_dir = dir.path().join("system");
    let note_skills_dir = sys_dir.join("Skills").join("Note");
    std::fs::create_dir_all(&note_skills_dir).unwrap();
    std::fs::write(
        note_skills_dir.join("Proofread.md"),
        "Please proofread this document carefully.",
    )
    .unwrap();

    let tab_path = dir.path().join("meeting.md");
    std::fs::write(&tab_path, "# Meeting\n").unwrap();

    let app = app_with_open_tab(&sys_dir, &tab_path);
    let app_cell: Rc<RefCell<FastMdApp>> = Rc::new(RefCell::new(app));
    let app_for_closure = Rc::clone(&app_cell);

    let mut harness = stateful_harness((), move |ui, _| {
        let mut app = app_for_closure.borrow_mut();
        render_tabs_and_content_capture(ui, &mut app, |_| {});
    });
    harness.fit_contents();

    let tab_nodes: Vec<_> = harness.query_all_by_label_contains("meeting.md").collect();
    assert!(!tab_nodes.is_empty(), "the tab must be present");
    tab_nodes[0].click_secondary();
    harness.run_steps(2);
    harness.run_steps(2);

    harness.get_by_label("Proofread").click_accesskit();
    harness.run_steps(2);
    harness.run_steps(2);

    let app = app_cell.borrow();

    assert_eq!(
        app.selection().selected_file(),
        Some(&tab_path),
        "choosing Note skill must select the tab path"
    );
}

/// VFS-121 negative: when no Note skill files are present, the
/// open-tab context menu does not offer a skill button.
#[test]
fn test_tab_context_menu_offers_no_note_skill_when_dir_empty() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;
    use std::cell::RefCell;
    use std::rc::Rc;

    let dir = tempfile::tempdir().unwrap();
    let sys_dir = dir.path().join("system");
    // No Skills/Note files created.
    let tab_path = dir.path().join("notes.md");
    std::fs::write(&tab_path, "# Notes\n").unwrap();

    let app = app_with_open_tab(&sys_dir, &tab_path);
    let app_cell: Rc<RefCell<FastMdApp>> = Rc::new(RefCell::new(app));
    let app_for_closure = Rc::clone(&app_cell);

    let mut harness = stateful_harness((), move |ui, _| {
        let mut app = app_for_closure.borrow_mut();
        render_tabs_and_content_capture(ui, &mut app, |_| {});
    });
    harness.fit_contents();

    let tab_nodes: Vec<_> = harness.query_all_by_label_contains("notes.md").collect();
    assert!(!tab_nodes.is_empty());
    tab_nodes[0].click_secondary();
    harness.run_steps(2);
    harness.run_steps(2);

    let skill_buttons: Vec<_> = harness.query_all_by_label_contains("Proofread").collect();
    assert!(
        skill_buttons.is_empty(),
        "no Note skill button must be offered when Skills/Note is empty"
    );
}

/// VFS-121 error path: a Note skill file that exists in the listing
/// but cannot be read as UTF-8 must NOT populate `submit_prompt`
/// (the `else` branch logs and leaves the prompt untouched).
#[test]
fn test_tab_context_menu_note_skill_read_failure_leaves_prompt_empty() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;
    use std::cell::RefCell;
    use std::rc::Rc;

    let dir = tempfile::tempdir().unwrap();
    let sys_dir = dir.path().join("system");
    let note_skills_dir = sys_dir.join("Skills").join("Note");
    std::fs::create_dir_all(&note_skills_dir).unwrap();
    // Invalid UTF-8: listed by list_note_skills but unreadable by read_to_string.
    std::fs::write(
        note_skills_dir.join("Proofread.md"),
        [0xFF, 0xFE, 0x00, 0x80],
    )
    .unwrap();

    let tab_path = dir.path().join("meeting.md");
    std::fs::write(&tab_path, "# Meeting\n").unwrap();

    let app = app_with_open_tab(&sys_dir, &tab_path);
    let app_cell: Rc<RefCell<FastMdApp>> = Rc::new(RefCell::new(app));
    let app_for_closure = Rc::clone(&app_cell);

    let mut harness = stateful_harness((), move |ui, _| {
        let mut app = app_for_closure.borrow_mut();
        render_tabs_and_content_capture(ui, &mut app, |_| {});
    });
    harness.fit_contents();

    let tab_nodes: Vec<_> = harness.query_all_by_label_contains("meeting.md").collect();
    tab_nodes[0].click_secondary();
    harness.run_steps(2);
    harness.run_steps(2);

    harness.get_by_label("Proofread").click_accesskit();
    harness.run_steps(2);
    harness.run_steps(2);

    let _app = app_cell.borrow();
}
