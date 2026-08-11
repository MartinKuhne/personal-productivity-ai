//! Center content panel — tab bar, markdown preview, YAML front-matter editor, agent chat output, and inline editor.
//!
//! Unit tests live in the sibling `center_tests.rs` sidecar.

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
/// Preconditions: `app.orchestrator.agent_panel_state.show_results` must be true.
/// Postconditions: Agent results are hidden, history and text buffers are cleared, and any running agent is flagged for cancellation.
pub fn clear_agent_session_state(app: &mut FastMdApp) {
    app.orchestrator.agent_panel_state.show_results = false;
    app.orchestrator.agent_panel_state.scroll_to_id = None;
    app.agent_mut().clear_history();
    app.agent_mut().set_response(String::new());
    app.agent_mut().set_thinking(String::new());
    app.orchestrator.agent_transcript.reset();
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
/// Purity: Impure (mutates `app.orchestrator.tab_manager.tabs` and
/// `app.orchestrator.selection.selected_file`).
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
        &mut app.orchestrator.tab_manager.tabs,
        app.orchestrator.selection.selected_file_mut(),
        TabAction::Close(i),
    );
}

/// Purpose: Applies the side effect of clicking the context menu's
/// "Close Other Tabs" item.
/// Inputs: app (the application state), i (the index of the tab to
/// keep, as it appeared in the tab strip on the frame the menu
/// item was clicked)
/// Outputs: ()
/// Purity: Impure (mutates `app.orchestrator.tab_manager.tabs` and
/// `app.orchestrator.selection.selected_file`).
/// Preconditions: None — `i` is bounds-checked inside
/// `apply_tab_action`.
/// Postconditions: All tabs except the one at index `i` are
/// removed. The kept tab is `app.orchestrator.tab_manager.tabs[0]` after the
/// call. `selected_file` is updated if it was a closed tab.
pub fn apply_tab_close_others_click(app: &mut FastMdApp, i: usize) {
    apply_tab_action(
        &mut app.orchestrator.tab_manager.tabs,
        app.orchestrator.selection.selected_file_mut(),
        TabAction::CloseOthers(i),
    );
}

/// Purpose: Applies the side effect of clicking the context menu's
/// "Close All Tabs" item.
/// Inputs: app (the application state)
/// Outputs: ()
/// Purity: Impure (mutates `app.orchestrator.tab_manager.tabs` and
/// `app.orchestrator.selection.selected_file`).
/// Preconditions: None.
/// Postconditions: All tabs are removed. `selected_file` is `None`.
pub fn apply_tab_close_all_click(app: &mut FastMdApp) {
    apply_tab_action(
        &mut app.orchestrator.tab_manager.tabs,
        app.orchestrator.selection.selected_file_mut(),
        TabAction::CloseAll,
    );
}

/// Purpose: Renders the agent session view in the center panel.
/// Inputs: `ui` - Egui UI context, `app` - FastMdApp state.
/// Outputs: None.
/// Purity: Impure (performs UI rendering).
/// Preconditions: `app.orchestrator.agent_panel_state.show_results` is true.
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
            let thinking = app.orchestrator.agent_transcript.thinking.clone();
            if !thinking.is_empty() {
                ui.collapsing(crate::ui::strings::AGENT_THINKING_PROCESS, |ui| {
                    ui.label(
                        egui::RichText::new(&thinking)
                            .italics()
                            .color(egui::Color32::from_rgb(160, 160, 160)),
                    );
                });
                ui.add_space(8.0);
            }

            let content = app.orchestrator.agent_transcript.content.clone();
            if !content.is_empty() {
                ui.heading(crate::ui::strings::AGENT_RESPONSE);
                ui.separator();
                let strategy = app.orchestrator.config.deficit_strategy();
                let mut toggles = Vec::new();
                render_markdown(
                    ui,
                    &content,
                    &mut app.orchestrator.agent_panel_state.scroll_to_id,
                    &mut toggles,
                    strategy,
                    None,
                );
                // P0-2: Apply task checkbox toggles to the transcript
                // content (the new render source, replacing
                // `AgentState::response`).
                if !toggles.is_empty() {
                    for (idx, checked) in toggles {
                        crate::ui::render::apply_task_toggle(
                            &mut app.orchestrator.agent_transcript.content,
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
        let tabs_snapshot: Vec<PathBuf> = app.orchestrator.tab_manager.tabs.clone();
        let tab_titles = app.orchestrator.tab_manager.tab_titles().to_vec();

        for (i, (tab_path, title)) in tabs_snapshot.iter().zip(tab_titles.iter()).enumerate() {
            let is_selected = app.orchestrator.selection.selected_file() == Some(tab_path);

            let response = ui.push_id((i, tab_path, "tab_label"), |ui| {
                ui.selectable_label(is_selected, title)
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
                    if app.orchestrator.inline_editor_enabled {
                        if let Ok(content) = std::fs::read_to_string(tab_path) {
                            let is_pdf_backed = app.pdf_backing_tracker().is_pdf_backed(tab_path);
                            if !is_pdf_backed {
                                app.orchestrator.text_buffer.open(tab_path, &content, None);
                            }
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
                    *app.submit_prompt_mut() = Some(generate_format_prompt(&date_str));
                    *app.orchestrator.selection.selected_file_mut() = Some(tab_path.clone());
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

        let pdf_backed = app.pdf_backing_tracker().is_pdf_backed(selected_path);
        let frame_fill = pdf_backed.then_some(egui::Color32::from_rgb(35, 30, 20));
        let deficit_strategy = app.orchestrator.config.deficit_strategy();
        render_markdown_content(
            ui,
            &mut app.orchestrator.tab_manager,
            frame_fill,
            deficit_strategy,
        );
    }
}

/// Purpose: Renders the shared markdown preview content (YAML table, rendered
/// markdown, task toggles) inside an optional sepia frame and a vertical
/// scroll area.
/// Inputs: `ui` - Egui UI context, `tab_manager` - The tab manager holding the
///   current markdown, YAML, and scroll state, `frame_fill` - Optional frame
///   background color (sepia tint for PDF-backed files, `None` otherwise),
///   `deficit_strategy` - The table width deficit strategy for rendering.
/// Outputs: None.
/// Purity: Impure (performs UI rendering and mutates pending_task_toggles).
/// Preconditions: None.
/// Postconditions: Renders the markdown preview with YAML front-matter table
///   (if present) and applies any pending task-checkbox toggles to the markdown
///   source.
fn render_markdown_content(
    ui: &mut egui::Ui,
    tab_manager: &mut crate::app::TabManager,
    frame_fill: Option<egui::Color32>,
    deficit_strategy: crate::ui::table_width::DeficitStrategy,
) {
    if let Some(fill) = frame_fill {
        egui::Frame::new().fill(fill).show(ui, |ui| {
            show_markdown_scroll_area(ui, tab_manager, deficit_strategy);
        });
    } else {
        show_markdown_scroll_area(ui, tab_manager, deficit_strategy);
    }
}

/// Renders the scroll area containing YAML table, markdown, and task toggles.
fn show_markdown_scroll_area(
    ui: &mut egui::Ui,
    tab_manager: &mut crate::app::TabManager,
    deficit_strategy: crate::ui::table_width::DeficitStrategy,
) {
    egui::ScrollArea::vertical()
        .id_salt("main_markdown_scroll")
        .show(ui, |ui| {
            if let Some(yaml) = &tab_manager.current_yaml {
                render_yaml_table(ui, yaml);
            }
            let heading_ids = tab_manager.heading_ids().to_vec();
            render_markdown(
                ui,
                &tab_manager.current_markdown,
                &mut tab_manager.scroll_to_header_id,
                &mut tab_manager.pending_task_toggles,
                deficit_strategy,
                Some(&heading_ids),
            );
            // P0-2: Apply task checkbox toggles to the markdown source.
            if !tab_manager.pending_task_toggles.is_empty() {
                for (idx, checked) in tab_manager.pending_task_toggles.drain(..) {
                    crate::ui::render::apply_task_toggle(
                        &mut tab_manager.current_markdown,
                        idx,
                        checked,
                    );
                }
            }
        });
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
        if app.orchestrator.agent_panel_state.show_results {
            render_agent_session(ui, app);
        } else if !app.tabs().tabs.is_empty() {
            render_tabs_and_content(ui, app);
        } else {
            render_empty_state(ui);
        }
    });
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `center_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "center_tests.rs"]
mod tests;
