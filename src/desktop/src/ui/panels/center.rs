//! Center content panel Ã¢â‚¬â€ tab bar, markdown preview, YAML front-matter editor, agent chat output, and inline editor.

use crate::ui::render::{render_markdown, render_yaml_table};
use crate::ui::{FastMdApp, generate_format_prompt, open_in_system_editor, show_in_file_explorer};
use eframe::egui;
use egui::RichText;
use egui::containers::CentralPanel;
use std::path::PathBuf;

/// Action that can be applied to tabs.
#[derive(Debug, PartialEq, Clone)]
pub enum TabAction {
    Close(usize),
    CloseOthers(usize),
    CloseAll,
}

/// Purpose: Clears all agent-related session state from the application state.
/// Inputs: `app` - A mutable reference to the `FastMdApp` state.
/// Outputs: None
/// Purity: Impure (mutates application state).
/// Preconditions: `app.agent.show_results()` must be true.
/// Postconditions: Agent results are hidden, history and text buffers are cleared, and any running agent is flagged for cancellation.
pub fn clear_agent_session_state(app: &mut FastMdApp) {
    app.agent_mut().set_show_results(false);
    app.agent_mut().clear_history();
    app.agent_mut().set_response(String::new());
    app.agent_mut().set_thinking(String::new());
    if app.agent().state().running {
        app.agent_mut().cancel();
    }
}

/// Purpose: Modifies the list of open tabs and the currently selected file based on a tab action.
/// Inputs: `tabs` - Mutable list of tab paths, `selected_file` - Mutable selected file option, `action` - The tab action to perform.
/// Outputs: None.
/// Purity: Impure (mutates arguments).
/// Preconditions: `tabs` must not be empty if `CloseOthers` or `Close` is called with an index.
/// Postconditions: `tabs` is updated according to the action. `selected_file` falls back to the last tab if the currently selected file was closed.
pub fn apply_tab_action(
    tabs: &mut Vec<PathBuf>,
    selected_file: &mut Option<PathBuf>,
    action: TabAction,
) {
    match action {
        TabAction::Close(i) => {
            if i < tabs.len() {
                tabs.remove(i);
            }
        }
        TabAction::CloseOthers(i) => {
            if i < tabs.len() {
                let keep = tabs[i].clone();
                tabs.clear();
                tabs.push(keep);
            }
        }
        TabAction::CloseAll => {
            tabs.clear();
        }
    }

    if let Some(selected) = selected_file {
        if !tabs.contains(selected) {
            *selected_file = tabs.last().cloned();
        }
    } else if !tabs.is_empty() {
        *selected_file = tabs.last().cloned();
    } else {
        *selected_file = None;
    }
}

/// Purpose: Applies the side effect of clicking the tab close `×`
/// button in the center panel.
/// Inputs: app (the application state), i (the index of the tab to
/// close, as it appeared in the tab strip on the frame the button
/// was clicked)
/// Outputs: ()
/// Purity: Impure (mutates `app.tab_manager.tabs` and
/// `app.selection.selected_file`).
/// Preconditions: None — `i` is bounds-checked inside
/// `apply_tab_action`; an out-of-range index is a silent no-op.
/// Postconditions: If `i` was in range, the tab at that index is
/// removed. If the closed tab was the selected file, the selection
/// falls back to the new last tab (or `None` if no tabs remain).
///
/// The `×` button click in `render_tabs_and_content` calls this
/// function. It is extracted so the side effect can be unit-tested
/// without driving the egui harness. Existing unit tests for
/// `apply_tab_action` cover the underlying logic; this wrapper just
/// adapts the `&mut FastMdApp` API to the field-level `apply_tab_action`
/// API.
pub fn apply_tab_close_click(app: &mut FastMdApp, i: usize) {
    apply_tab_action(
        &mut app.tab_manager.tabs,
        app.selection.selected_file_mut(),
        TabAction::Close(i),
    );
}

/// Purpose: Applies the side effect of clicking the context menu's
/// "Close Other Tabs" item.
/// Inputs: app (the application state), i (the index of the tab to
/// keep, as it appeared in the tab strip on the frame the menu
/// item was clicked)
/// Outputs: ()
/// Purity: Impure (mutates `app.tab_manager.tabs` and
/// `app.selection.selected_file`).
/// Preconditions: None — `i` is bounds-checked inside
/// `apply_tab_action`.
/// Postconditions: All tabs except the one at index `i` are
/// removed. The kept tab is `app.tab_manager.tabs[0]` after the
/// call. `selected_file` is updated if it was a closed tab.
pub fn apply_tab_close_others_click(app: &mut FastMdApp, i: usize) {
    apply_tab_action(
        &mut app.tab_manager.tabs,
        app.selection.selected_file_mut(),
        TabAction::CloseOthers(i),
    );
}

/// Purpose: Applies the side effect of clicking the context menu's
/// "Close All Tabs" item.
/// Inputs: app (the application state)
/// Outputs: ()
/// Purity: Impure (mutates `app.tab_manager.tabs` and
/// `app.selection.selected_file`).
/// Preconditions: None.
/// Postconditions: All tabs are removed. `selected_file` is `None`.
pub fn apply_tab_close_all_click(app: &mut FastMdApp) {
    apply_tab_action(
        &mut app.tab_manager.tabs,
        app.selection.selected_file_mut(),
        TabAction::CloseAll,
    );
}

/// Purpose: Renders the agent session view in the center panel.
/// Inputs: `ui` - Egui UI context, `app` - FastMdApp state.
/// Outputs: None.
/// Purity: Impure (performs UI rendering).
/// Preconditions: `app.agent.show_results()` is true.
/// Postconditions: Rendered agent session. State might be mutated if "Close" is clicked.
fn render_agent_session(ui: &mut egui::Ui, app: &mut FastMdApp) {
    ui.horizontal_wrapped(|ui| {
        ui.heading(
            RichText::new(crate::ui::strings::AGENT_SESSION_HEADER)
                .size(18.0)
                .strong()
                .color(egui::Color32::from_rgb(100, 200, 255)),
        );
        ui.separator();
        if ui
            .button(crate::ui::strings::AGENT_SESSION_CLOSE_BUTTON)
            .clicked()
        {
            clear_agent_session_state(app);
        }
    });
    ui.separator();

    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(format!(
                "{}{}",
                crate::ui::strings::AGENT_STATUS_PREFIX,
                app.agent().state().status
            ))
            .strong(),
        );
        if app.agent().state().running {
            ui.spinner();
        }
    });
    ui.add_space(8.0);

    egui::ScrollArea::vertical()
        .id_salt("agent_thinking_scroll")
        .stick_to_bottom(true)
        .show(ui, |ui| {
            if !app.agent().state().thinking.is_empty() {
                ui.collapsing(crate::ui::strings::AGENT_THINKING_PROCESS, |ui| {
                    ui.label(
                        egui::RichText::new(&app.agent().state().thinking)
                            .italics()
                            .color(egui::Color32::from_rgb(160, 160, 160)),
                    );
                });
                ui.add_space(8.0);
            }

            if !app.agent().state().response.is_empty() {
                ui.heading(crate::ui::strings::AGENT_RESPONSE);
                ui.separator();
                let agent = app.agent_mut();
                let response = agent.state().response.clone();
                let mut toggles = Vec::new();
                render_markdown(
                    ui,
                    &response,
                    &mut agent.state_mut().scroll_to_id,
                    &mut toggles,
                );
                // P0-2: Apply task checkbox toggles to the response source.
                if !toggles.is_empty() {
                    for (idx, checked) in toggles {
                        crate::ui::render::apply_task_toggle(
                            &mut agent.state_mut().response,
                            idx,
                            checked,
                        );
                    }
                }
                // `stick_to_bottom(true)` on the ScrollArea handles
                // auto-scroll; an explicit `scroll_to_cursor` here would
                // compete with it and cause jitter.
            }
        });
}

/// Purpose: Renders the file tabs and the selected file content in the center panel.
/// Inputs: `ui` - Egui UI context, `app` - FastMdApp state.
/// Outputs: None.
/// Purity: Impure (performs UI rendering).
/// Preconditions: `app.tabs().tabs` is not empty.
/// Postconditions: Rendered tabs and file content.
fn render_tabs_and_content(ui: &mut egui::Ui, app: &mut FastMdApp) {
    render_tabs_and_content_capture(ui, app, |_| {});
}

/// Tier 4 test variant of [`render_tabs_and_content`]. The
/// `on_click` callback is invoked after every tab-strip click
/// (`×` close button, label click, context-menu items) with a
/// stable event name. The production caller
/// ([`render_tabs_and_content`]) passes a no-op closure; the
/// test caller in `tests::test_tab_close_button_captures_event`
/// passes a closure that pushes the event into the harness's
/// persistent state.
pub fn render_tabs_and_content_capture(
    ui: &mut egui::Ui,
    app: &mut FastMdApp,
    mut on_click: impl FnMut(&'static str),
) {
    ui.horizontal(|ui| {
        let tabs_snapshot: Vec<PathBuf> = app.tab_manager.tabs.clone();

        for (i, tab_path) in tabs_snapshot.iter().enumerate() {
            let is_selected = app.selection.selected_file() == Some(tab_path);
            let title: String = tab_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into();

            let response = ui.push_id((i, tab_path, "tab_label"), |ui| {
                ui.selectable_label(is_selected, &title)
            });
            if response.inner.clicked() {
                *app.selection_mut().selected_file_mut() = Some(tab_path.clone());
            }
            if response.inner.middle_clicked() {
                apply_tab_close_click(app, i);
                on_click("tab_middle_click");
            }
            response.inner.context_menu(|ui| {
                if ui.button(crate::ui::strings::EDIT_BUTTON).clicked() {
                    if app.inline_editor_enabled {
                        if let Ok(content) = std::fs::read_to_string(tab_path) {
                            app.editor_state.open(tab_path, &content);
                        }
                    } else {
                        open_in_system_editor(tab_path);
                    }
                    ui.close();
                }
                ui.separator();
                if ui.button(crate::ui::strings::CLOSE_TAB_MENU).clicked() {
                    apply_tab_close_click(app, i);
                    on_click("tab_close_menu");
                    ui.close();
                }
                if ui.button(crate::ui::strings::CLOSE_OTHERS_MENU).clicked() {
                    apply_tab_close_others_click(app, i);
                    on_click("tab_close_others_menu");
                    ui.close();
                }
                if ui.button(crate::ui::strings::CLOSE_ALL_TABS_MENU).clicked() {
                    apply_tab_close_all_click(app);
                    on_click("tab_close_all_menu");
                    ui.close();
                }
                ui.separator();
                if ui.button(crate::ui::strings::COPY_PATH_ACTION).clicked() {
                    // egui 0.35: `PlatformOutput::copied_text` was
                    // removed; use the dedicated `Ui::copy_text` helper.
                    ui.copy_text(tab_path.to_string_lossy().to_string());
                    ui.close();
                }
                if ui
                    .button(crate::ui::strings::SHOW_IN_EXPLORER_ACTION)
                    .clicked()
                {
                    show_in_file_explorer(tab_path);
                    ui.close();
                }
                if ui
                    .button(crate::ui::strings::OPEN_IN_EDITOR_ACTION)
                    .clicked()
                {
                    open_in_system_editor(tab_path);
                    ui.close();
                }
                if ui
                    .button(crate::ui::strings::FORMAT_MARKDOWN_ACTION)
                    .clicked()
                {
                    let now = chrono::Local::now();
                    let date_str = now.to_rfc3339();
                    app.submit_prompt = Some(generate_format_prompt(&date_str));
                    *app.selection.selected_file_mut() = Some(tab_path.clone());
                    ui.close();
                }
            });

            if ui
                .push_id((i, tab_path, "tab_close"), |ui| ui.button("×"))
                .inner
                .clicked()
            {
                apply_tab_close_click(app, i);
                on_click("tab_close_button");
            }
            ui.separator();
        }
    });
    ui.separator();

    if let Some(selected_path) = app.selection().selected_file() {
        ui.push_id("selected_file_header", |ui| {
            ui.horizontal(|ui| {
                ui.heading(
                    RichText::new(
                        selected_path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy(),
                    )
                    .size(18.0)
                    .strong(),
                );
                ui.label(
                    RichText::new(format!("({})", selected_path.to_string_lossy()))
                        .size(11.0)
                        .italics()
                        .color(egui::Color32::GRAY),
                );
            });
        });
        ui.separator();

        egui::ScrollArea::vertical()
            .id_salt("main_markdown_scroll")
            .show(ui, |ui| {
                if let Some(yaml) = &app.tab_manager.current_yaml {
                    render_yaml_table(ui, yaml);
                }
                render_markdown(
                    ui,
                    &app.tab_manager.current_markdown,
                    &mut app.tab_manager.scroll_to_header_id,
                    &mut app.tab_manager.pending_task_toggles,
                );
                // P0-2: Apply task checkbox toggles to the markdown source.
                if !app.tab_manager.pending_task_toggles.is_empty() {
                    for (idx, checked) in app.tab_manager.pending_task_toggles.drain(..) {
                        crate::ui::render::apply_task_toggle(
                            &mut app.tab_manager.current_markdown,
                            idx,
                            checked,
                        );
                    }
                }
            });
    }
}

/// Purpose: Renders the empty state when no files are open.
/// Inputs: `ui` - Egui UI context.
/// Outputs: None.
/// Purity: Impure (performs UI rendering).
/// Preconditions: None.
/// Postconditions: Rendered empty state message.
fn render_empty_state(ui: &mut egui::Ui) {
    ui.centered_and_justified(|ui| {
        ui.label(
            RichText::new(crate::ui::strings::NO_FILE_SELECTED_PROMPT)
                .size(15.0)
                .italics()
                .color(egui::Color32::GRAY),
        );
    });
}

/// Purpose: Main adapter for rendering the center panel in the UI. Routes to specific sub-renderers based on app state.
/// Inputs: `app` - FastMdApp state, `ctx` - Egui context.
/// Outputs: None.
/// Purity: Impure (performs UI rendering and routes side effects).
/// Preconditions: None.
/// Postconditions: Renders the central panel content.
pub fn show_center_panel(app: &mut FastMdApp, parent_ui: &mut egui::Ui) {
    // egui 0.35: `CentralPanel::show` now takes the parent
    // `&mut Ui` rather than a `&Context`; allocate within the
    // root Ui we got from `App::ui`.
    CentralPanel::default().show(parent_ui, |ui| {
        if app.agent().show_results() {
            render_agent_session(ui, app);
        } else if !app.tabs().tabs.is_empty() {
            render_tabs_and_content(ui, app);
        } else {
            render_empty_state(ui);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::generate_format_prompt;
    use std::path::PathBuf;

    fn create_test_app() -> FastMdApp {
        FastMdApp::empty_state(crate::config::AppConfig::default())
    }

    /// Tier 1 test for the `×` tab close button click effect. The
    /// click removes the tab at `i` from `app.tab_manager.tabs`.
    /// We verify the effect without driving the egui harness.
    #[test]
    fn test_apply_tab_close_click_removes_tab_at_index() {
        let mut app = create_test_app();
        app.tab_manager.tabs = vec![
            PathBuf::from("a.md"),
            PathBuf::from("b.md"),
            PathBuf::from("c.md"),
        ];
        *app.selection.selected_file_mut() = Some(PathBuf::from("b.md"));

        apply_tab_close_click(&mut app, 1);

        assert_eq!(
            app.tab_manager.tabs,
            vec![PathBuf::from("a.md"), PathBuf::from("c.md")],
            "tab at i=1 must be removed"
        );
        // `b.md` was the selected file and is now closed, so
        // selection falls back to the last remaining tab.
        assert_eq!(
            app.selection.selected_file(),
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
        app.tab_manager.tabs = vec![PathBuf::from("a.md")];
        *app.selection.selected_file_mut() = Some(PathBuf::from("a.md"));

        apply_tab_close_click(&mut app, 5);

        assert_eq!(app.tab_manager.tabs, vec![PathBuf::from("a.md")]);
        assert_eq!(app.selection.selected_file(), Some(&PathBuf::from("a.md")));
    }

    /// Tier 1 test for the context-menu "Close Others" item.
    /// Verifies all tabs except the keep-index are removed, and
    /// the kept tab is the only remaining entry.
    #[test]
    fn test_apply_tab_close_others_click_keeps_only_target_tab() {
        let mut app = create_test_app();
        app.tab_manager.tabs = vec![
            PathBuf::from("a.md"),
            PathBuf::from("b.md"),
            PathBuf::from("c.md"),
        ];
        *app.selection.selected_file_mut() = Some(PathBuf::from("a.md"));

        apply_tab_close_others_click(&mut app, 1);

        assert_eq!(app.tab_manager.tabs, vec![PathBuf::from("b.md")]);
        assert_eq!(app.selection.selected_file(), Some(&PathBuf::from("b.md")));
    }

    /// Tier 1 test for the context-menu "Close All Tabs" item.
    /// Verifies all tabs are removed and `selected_file` is `None`.
    #[test]
    fn test_apply_tab_close_all_click_clears_all_tabs() {
        let mut app = create_test_app();
        app.tab_manager.tabs = vec![
            PathBuf::from("a.md"),
            PathBuf::from("b.md"),
            PathBuf::from("c.md"),
        ];
        *app.selection.selected_file_mut() = Some(PathBuf::from("b.md"));

        apply_tab_close_all_click(&mut app);

        assert!(app.tab_manager.tabs.is_empty());
        assert!(
            app.selection.selected_file().is_none(),
            "selected_file must be None when all tabs are closed"
        );
    }

    /// Tier 4 click test: clicking the tab-strip `×` close button
    /// in the center panel must fire the `on_click("tab_close_button")`
    /// callback. Renders `render_tabs_and_content_capture` (not the
    /// full `show_center_panel`) so the test stays focused on the
    /// tab-strip click. The click handler also removes the tab
    /// from `app.tab_manager.tabs`, but the harness owns `&mut app`
    /// so the side effect is observed via the captured event.
    #[test]
    fn test_tab_close_button_captures_event() {
        use crate::ui::test_helpers::interact::stateful_harness;
        use egui_kittest::kittest::Queryable;

        let mut harness = stateful_harness(Vec::<&'static str>::new(), |ui, captured| {
            let mut app = create_test_app();
            app.tab_manager.tabs = vec![PathBuf::from("a.md"), PathBuf::from("b.md")];
            *app.selection.selected_file_mut() = Some(PathBuf::from("a.md"));
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

    #[test]
    fn test_apply_tab_action_close() {
        let mut tabs = vec![
            PathBuf::from("a.md"),
            PathBuf::from("b.md"),
            PathBuf::from("c.md"),
        ];
        let mut selected = Some(PathBuf::from("a.md"));

        apply_tab_action(&mut tabs, &mut selected, TabAction::Close(0));
        assert_eq!(tabs, vec![PathBuf::from("b.md"), PathBuf::from("c.md")]);
        // selected was a.md, so it falls back to the last tab (c.md)
        assert_eq!(selected, Some(PathBuf::from("c.md")));
    }

    #[test]
    fn test_apply_tab_action_close_others() {
        let mut tabs = vec![
            PathBuf::from("a.md"),
            PathBuf::from("b.md"),
            PathBuf::from("c.md"),
        ];
        let mut selected = Some(PathBuf::from("a.md"));

        apply_tab_action(&mut tabs, &mut selected, TabAction::CloseOthers(1));
        assert_eq!(tabs, vec![PathBuf::from("b.md")]);
        assert_eq!(selected, Some(PathBuf::from("b.md")));
    }

    #[test]
    fn test_apply_tab_action_close_all() {
        let mut tabs = vec![PathBuf::from("a.md"), PathBuf::from("b.md")];
        let mut selected = Some(PathBuf::from("b.md"));

        apply_tab_action(&mut tabs, &mut selected, TabAction::CloseAll);
        assert!(tabs.is_empty());
        assert_eq!(selected, None);
    }

    #[test]
    fn test_apply_tab_action_out_of_bounds() {
        let mut tabs = vec![PathBuf::from("a.md")];
        let mut selected = Some(PathBuf::from("a.md"));

        // Invalid index should do nothing
        apply_tab_action(&mut tabs, &mut selected, TabAction::Close(5));
        assert_eq!(tabs.len(), 1);
        assert_eq!(selected, Some(PathBuf::from("a.md")));
    }

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
        app.tabs_mut().current_yaml = Some(serde_yaml::from_str("title: Doc 1").unwrap());

        // Mode 2: a tab is open with markdown + YAML. The center panel
        // has no stable canonical string for this mode (the tab labels
        // are file paths, the body is markdown content, the YAML
        // table does not emit a "YAML Front-Matter" header — that
        // string is only used in the inline editor). The render path
        // is exercised; full visual coverage of the markdown + YAML
        // view comes from the Tier 3 snapshot in R-1c.
        let _ = ctx.run_ui(raw_input(), |ui| {
            show_center_panel(&mut app, ui);
        });

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
}
