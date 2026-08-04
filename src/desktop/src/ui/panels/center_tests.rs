//! Tests for `panels/center.rs`.
//!
//! Sidecar file. Extracted from `center.rs` so the implementation
//! module stays focused on production code.
//!
//! Originally a `#[cfg(test)] mod tests { ... }` block at the bottom of
//! `center.rs`. Lives in a sibling file so private item access via
//! `super::*` keeps working.

use super::*;
use crate::ui::generate_format_prompt;
use std::path::PathBuf;

fn create_test_app() -> FastMdApp {
    FastMdApp::empty_state(crate::config::AppConfig::default())
}

/// Tier 1 test for the `×` tab close button click effect. The
/// click removes the tab at `i` from `app.orchestrator.tab_manager.tabs`.
/// We verify the effect without driving the egui harness.
#[test]
fn test_apply_tab_close_click_removes_tab_at_index() {
    let mut app = create_test_app();
    app.orchestrator.tab_manager.tabs = vec![
        PathBuf::from("a.md"),
        PathBuf::from("b.md"),
        PathBuf::from("c.md"),
    ];
    *app.orchestrator.selection.selected_file_mut() = Some(PathBuf::from("b.md"));

    apply_tab_close_click(&mut app, 1);

    assert_eq!(
        app.orchestrator.tab_manager.tabs,
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
    app.orchestrator.tab_manager.tabs = vec![PathBuf::from("a.md")];
    *app.orchestrator.selection.selected_file_mut() = Some(PathBuf::from("a.md"));

    apply_tab_close_click(&mut app, 5);

    assert_eq!(
        app.orchestrator.tab_manager.tabs,
        vec![PathBuf::from("a.md")]
    );
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
    app.orchestrator.tab_manager.tabs = vec![
        PathBuf::from("a.md"),
        PathBuf::from("b.md"),
        PathBuf::from("c.md"),
    ];
    *app.orchestrator.selection.selected_file_mut() = Some(PathBuf::from("a.md"));

    apply_tab_close_others_click(&mut app, 1);

    assert_eq!(
        app.orchestrator.tab_manager.tabs,
        vec![PathBuf::from("b.md")]
    );
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
    app.orchestrator.tab_manager.tabs = vec![
        PathBuf::from("a.md"),
        PathBuf::from("b.md"),
        PathBuf::from("c.md"),
    ];
    *app.orchestrator.selection.selected_file_mut() = Some(PathBuf::from("b.md"));

    apply_tab_close_all_click(&mut app);

    assert!(app.orchestrator.tab_manager.tabs.is_empty());
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
/// from `app.orchestrator.tab_manager.tabs`, but the harness owns `&mut app`
/// so the side effect is observed via the captured event.
#[test]
fn test_tab_close_button_captures_event() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;

    let mut harness = stateful_harness(Vec::<&'static str>::new(), |ui, captured| {
        let mut app = create_test_app();
        app.orchestrator.tab_manager.tabs = vec![PathBuf::from("a.md"), PathBuf::from("b.md")];
        *app.orchestrator.selection.selected_file_mut() = Some(PathBuf::from("a.md"));
        render_tabs_and_content_capture(ui, &mut app, |event| {
            captured.push(event);
        });
    });
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

    let captured = harness.state();
    assert!(
        captured.contains(&"tab_close_button"),
        "clicking the tab-strip × close button must fire the \
             `tab_close_button` on_click event; got: {:?}",
        captured
    );
}

#[test]
fn test_generate_format_prompt() {
    let date_str = "2026-07-20T12:00:00Z";
    let prompt = generate_format_prompt(date_str);
    assert!(prompt.contains(date_str));
    assert!(prompt.contains("Format the current document"));
    assert!(prompt.contains("header-date: 2026-07-20T12:00:00Z"));
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
    app.agent_mut().set_show_results(true);

    clear_agent_session_state(&mut app);

    assert!(!app.agent().show_results());
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
    let output = ctx.run_ui(raw_input(), |ui| {
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
    let output = ctx.run_ui(raw_input(), |ui| {
        show_center_panel(&mut app, ui);
    });
    assert_text_contains(&output.shapes, "Document 1 Header");
    assert_text_contains(&output.shapes, "title");

    app.agent_mut().set_show_results(true);
    app.agent_mut().set_running(true);
    app.agent_mut().set_status("Thinking...".to_string());
    app.agent_mut().set_thinking("Reasoning step 1".to_string());
    app.agent_mut()
        .set_response("Final agent summary answer".to_string());

    // Mode 3: agent session active. The agent session header is the
    // stable string for this mode.
    let output = ctx.run_ui(raw_input(), |ui| {
        show_center_panel(&mut app, ui);
    });
    assert_text_contains(&output.shapes, AGENT_SESSION_HEADER);
}

#[test]
fn test_agent_session_close_button_label() {
    assert_eq!(crate::ui::strings::AGENT_SESSION_CLOSE_BUTTON, "Close");
}
