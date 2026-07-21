use crate::ui::render::{render_markdown, render_yaml_table};
use crate::ui::{generate_format_prompt, open_in_system_editor, show_in_file_explorer, FastMdApp};
use eframe::egui;
use egui::RichText;
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
/// Preconditions: `app.show_agent_results` must be true.
/// Postconditions: Agent results are hidden, history and text buffers are cleared, and any running agent is flagged for cancellation.
pub fn clear_agent_session_state(app: &mut FastMdApp) {
    app.show_agent_results = false;
    app.agent.clear_history();
    app.agent.set_response(String::new());
    app.agent.set_thinking(String::new());
    if app.agent.state().running {
        app.agent.cancel();
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

/// Purpose: Renders the agent session view in the center panel.
/// Inputs: `ui` - Egui UI context, `app` - FastMdApp state.
/// Outputs: None.
/// Purity: Impure (performs UI rendering).
/// Preconditions: `app.show_agent_results` is true.
/// Postconditions: Rendered agent session. State might be mutated if "Back to Document" is clicked.
fn render_agent_session(ui: &mut egui::Ui, app: &mut FastMdApp) {
    ui.horizontal(|ui| {
        ui.heading(
            RichText::new("🤖 FastMD Agent Session")
                .size(18.0)
                .strong()
                .color(egui::Color32::from_rgb(100, 200, 255)),
        );
        ui.separator();
        if ui.button("Back to Document").clicked() {
            clear_agent_session_state(app);
        }
    });
    ui.separator();

    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("Status: {}", app.agent.state().status)).strong());
        if app.agent.state().running {
            ui.spinner();
        }
    });
    ui.add_space(8.0);

    egui::ScrollArea::vertical()
        .id_source("agent_thinking_scroll")
        .stick_to_bottom(true)
        .show(ui, |ui| {
            if !app.agent.state().thinking.is_empty() {
                ui.collapsing("Thinking Process", |ui| {
                    ui.label(
                        egui::RichText::new(&app.agent.state().thinking)
                            .italics()
                            .color(egui::Color32::from_rgb(160, 160, 160)),
                    );
                });
                ui.add_space(8.0);
            }

            if !app.agent.state().response.is_empty() {
                ui.heading("Response");
                ui.separator();
                let agent = &mut app.agent;
                let response = agent.state().response.clone();
                render_markdown(ui, &response, &mut agent.state_mut().scroll_to_id);
                if app.agent.state().running {
                    ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                }
            }
        });
}

/// Purpose: Renders the file tabs and the selected file content in the center panel.
/// Inputs: `ui` - Egui UI context, `app` - FastMdApp state.
/// Outputs: None.
/// Purity: Impure (performs UI rendering).
/// Preconditions: `app.tab_manager.tabs` is not empty.
/// Postconditions: Rendered tabs and file content.
fn render_tabs_and_content(ui: &mut egui::Ui, app: &mut FastMdApp) {
    ui.horizontal(|ui| {
        let mut tab_action = None;

        for (i, tab_path) in app.tab_manager.tabs.iter().enumerate() {
            let is_selected = app.selection.selected_file() == Some(tab_path);
            let title = tab_path.file_name().unwrap_or_default().to_string_lossy();

            let response = ui.selectable_label(is_selected, title);
            if response.clicked() {
                *app.selection.selected_file_mut() = Some(tab_path.clone());
            }
            if response.middle_clicked() {
                tab_action = Some(TabAction::Close(i));
            }
            response.context_menu(|ui| {
                if ui.button("Edit").clicked() {
                    if app.inline_editor_enabled {
                        if let Ok(content) = std::fs::read_to_string(tab_path) {
                            app.editor_state.open(tab_path, &content);
                        }
                    } else {
                        open_in_system_editor(tab_path);
                    }
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Close").clicked() {
                    tab_action = Some(TabAction::Close(i));
                    ui.close_menu();
                }
                if ui.button("Close Others").clicked() {
                    tab_action = Some(TabAction::CloseOthers(i));
                    ui.close_menu();
                }
                if ui.button("Close All").clicked() {
                    tab_action = Some(TabAction::CloseAll);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Copy Path").clicked() {
                    ui.output_mut(|o| o.copied_text = tab_path.to_string_lossy().to_string());
                    ui.close_menu();
                }
                if ui.button("Show in File Explorer").clicked() {
                    show_in_file_explorer(tab_path);
                    ui.close_menu();
                }
                if ui.button("Open in Editor").clicked() {
                    open_in_system_editor(tab_path);
                    ui.close_menu();
                }
                if ui.button("Format Markdown").clicked() {
                    let now = chrono::Local::now();
                    let date_str = now.to_rfc3339();
                    app.submit_prompt = Some(generate_format_prompt(&date_str));
                    *app.selection.selected_file_mut() = Some(tab_path.clone());
                    ui.close_menu();
                }
            });

            if ui.button("❌").clicked() {
                tab_action = Some(TabAction::Close(i));
            }
            ui.separator();
        }

        if let Some(action) = tab_action {
            apply_tab_action(
                &mut app.tab_manager.tabs,
                app.selection.selected_file_mut(),
                action,
            );
        }
    });
    ui.separator();

    if let Some(selected_path) = app.selection.selected_file() {
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
        ui.separator();

        egui::ScrollArea::vertical()
            .id_source("main_markdown_scroll")
            .show(ui, |ui| {
                if let Some(yaml) = &app.tab_manager.current_yaml {
                    render_yaml_table(ui, yaml);
                }
                render_markdown(
                    ui,
                    &app.tab_manager.current_markdown,
                    &mut app.tab_manager.scroll_to_header_id,
                );
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
            RichText::new("Select a markdown file from the left pane to view its content")
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
pub fn show_center_panel(app: &mut FastMdApp, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        if app.show_agent_results {
            render_agent_session(ui, app);
        } else if !app.tab_manager.tabs.is_empty() {
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

        // Setup agent state via manager
        app.agent.state_mut().history =
            Some(vec![serde_json::json!({"role": "user", "content": "hi"})]);
        app.agent.set_response("response text".to_string());
        app.agent.set_thinking("thinking process".to_string());
        app.agent.state_mut().running = true;
        app.show_agent_results = true;

        clear_agent_session_state(&mut app);

        assert!(!app.show_agent_results);
        assert!(app.agent.state().history.is_none());
        assert!(app.agent.state().response.is_empty());
        assert!(app.agent.state().thinking.is_empty());
        assert!(!app.agent.state().running); // cancel() sets running false
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
        let ctx = egui::Context::default();
        let mut app = create_test_app();

        // Mode 1: Empty state
        let _ = ctx.run(Default::default(), |ctx| {
            show_center_panel(&mut app, ctx);
        });

        // Mode 2: Tabs and content state
        app.tab_manager.tabs = vec![PathBuf::from("doc1.md"), PathBuf::from("doc2.md")];
        *app.selection.selected_file_mut() = Some(PathBuf::from("doc1.md"));
        app.tab_manager.current_markdown = "# Document 1 Header".to_string();
        app.tab_manager.current_yaml = Some(serde_yaml::from_str("title: Doc 1").unwrap());

        let _ = ctx.run(Default::default(), |ctx| {
            show_center_panel(&mut app, ctx);
        });

        // Mode 3: Agent results state
        app.show_agent_results = true;
        app.agent.set_running(true);
        app.agent.set_status("Thinking...".to_string());
        app.agent.set_thinking("Reasoning step 1".to_string());
        app.agent
            .set_response("Final agent summary answer".to_string());

        let _ = ctx.run(Default::default(), |ctx| {
            show_center_panel(&mut app, ctx);
        });
    }
}
