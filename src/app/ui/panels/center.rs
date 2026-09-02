//! Center content panel — tab bar, markdown preview, YAML front-matter editor, agent chat output, and inline editor.
//!
//! Unit tests live in the sibling `center_tests.rs` sidecar.

use crate::ui::FastMdApp;
use crate::ui::render::{render_markdown, render_yaml_table};
use eframe::egui;
use egui::RichText;
use egui::containers::CentralPanel;
use std::path::PathBuf;

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

/// Purpose: Applies the side effect of clicking the tab close `×`
/// button in the center panel.
/// Inputs: app (the application state), i (the index of the tab to
/// close, as it appeared in the tab strip on the frame the button
/// was clicked)
/// Outputs: ()
/// Purity: Impure (mutates `app.orchestrator.tabs.tabs` and
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
use crate::bus::events::user_command::UserCommand;

pub fn apply_tab_close_click(i: usize) -> UserCommand {
    UserCommand::CloseTab(i)
}

pub fn apply_tab_close_others_click(i: usize) -> UserCommand {
    UserCommand::CloseOtherTabs(i)
}

pub fn apply_tab_close_all_click() -> UserCommand {
    UserCommand::CloseAllTabs
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

/// Tier 4 test variant of `render_tabs_and_content`. The
/// `on_click` callback is invoked after every tab-strip click
/// (`×` close button, label click, context-menu items) with a
/// stable event name. The production caller
/// (`render_tabs_and_content`) passes a no-op closure; the
/// test caller in `tests::test_tab_close_button_captures_event`
/// passes a closure that pushes the event into the harness's
/// persistent state.
#[tracing::instrument(skip_all, name = "ui.center.tabs_and_content", level = "debug")]
pub fn render_tabs_and_content_capture(
    ui: &mut egui::Ui,
    app: &mut FastMdApp,
    mut on_click: impl FnMut(&'static str),
) {
    ui.horizontal(|ui| {
        let tabs_snapshot: Vec<PathBuf> = app.orchestrator.tabs.tabs.clone();
        let tab_titles = app.orchestrator.tabs.tab_titles().to_vec();

        for (i, (tab_path, title)) in tabs_snapshot.iter().zip(tab_titles.iter()).enumerate() {
            let is_selected = app.orchestrator.selection.selected_file() == Some(tab_path);

            let response = ui.push_id((i, tab_path, "tab_label"), |ui| {
                ui.selectable_label(is_selected, title)
            });
            if response.inner.clicked() {
                app.orchestrator.user_command_bus.publish(
                    crate::bus::events::user_command::UserCommand::SelectFile {
                        path: tab_path.clone(),
                        multi: false,
                    },
                );
            }
            if response.inner.middle_clicked() {
                app.orchestrator
                    .user_command_bus
                    .publish(apply_tab_close_click(i));
                on_click("tab_middle_click");
            }
            response.inner.context_menu(|ui| {
                if ui.button(crate::ui::strings::EDIT_BUTTON).clicked() {
                    app.orchestrator.user_command_bus.publish(
                        crate::bus::events::user_command::UserCommand::OpenInEditor(
                            tab_path.clone(),
                        ),
                    );
                    ui.close();
                }
                ui.separator();
                if ui.button(crate::ui::strings::CLOSE_TAB_MENU).clicked() {
                    app.orchestrator
                        .user_command_bus
                        .publish(apply_tab_close_click(i));
                    on_click("tab_close_menu");
                    ui.close();
                }
                if ui.button(crate::ui::strings::CLOSE_OTHERS_MENU).clicked() {
                    app.orchestrator
                        .user_command_bus
                        .publish(apply_tab_close_others_click(i));
                    on_click("tab_close_others_menu");
                    ui.close();
                }
                if ui.button(crate::ui::strings::CLOSE_ALL_TABS_MENU).clicked() {
                    app.orchestrator
                        .user_command_bus
                        .publish(apply_tab_close_all_click());
                    on_click("tab_close_all_menu");
                    ui.close();
                }
                ui.separator();
                if ui.button(crate::ui::strings::COPY_PATH_ACTION).clicked() {
                    app.orchestrator.user_command_bus.publish(
                        crate::bus::events::user_command::UserCommand::CopyPath(tab_path.clone()),
                    );
                    ui.close();
                }
                if ui
                    .button(crate::ui::strings::SHOW_IN_EXPLORER_ACTION)
                    .clicked()
                {
                    app.orchestrator.user_command_bus.publish(
                        crate::bus::events::user_command::UserCommand::ShowInExplorer(
                            tab_path.clone(),
                        ),
                    );
                    ui.close();
                }
                if ui
                    .button(crate::ui::strings::OPEN_IN_EDITOR_ACTION)
                    .clicked()
                {
                    app.orchestrator.user_command_bus.publish(
                        crate::bus::events::user_command::UserCommand::OpenInEditor(
                            tab_path.clone(),
                        ),
                    );
                    ui.close();
                }

                // Note skills (VFS-121)
                let note_skills = app.config().list_note_skills();
                if !note_skills.is_empty() {
                    ui.separator();
                    for skill in note_skills {
                        if ui.button(&skill.name).clicked() {
                            if let Ok(raw_content) = std::fs::read_to_string(&skill.path) {
                                let content =
                                    crate::markdown::DocumentContent::parse(&raw_content).body;
                                app.orchestrator.user_command_bus.publish(
                                    crate::bus::events::user_command::UserCommand::RunSkillPrompt {
                                        content,
                                        target_dir: None,
                                        target_file: Some(tab_path.clone()),
                                    },
                                );
                            } else {
                                tracing::error!(
                                    name = "ui.tab.skill_prompt_failed",
                                    path = %skill.path.display(),
                                    "Failed to read skill file content to run as prompt."
                                );
                            }
                            ui.close();
                        }
                    }
                }
            });

            if ui
                .push_id((i, tab_path, "tab_close"), |ui| ui.button("×"))
                .inner
                .clicked()
            {
                app.orchestrator
                    .user_command_bus
                    .publish(apply_tab_close_click(i));
                on_click("tab_close_button");
            }
            ui.separator();
        }
    });
    ui.separator();

    if let Some(selected_path) = app.selection().selected_file() {
        let pdf_backed = app.pdf_backing_tracker().is_pdf_backed(selected_path);
        let frame_fill = pdf_backed.then_some(egui::Color32::from_rgb(35, 30, 20));
        let deficit_strategy = app.orchestrator.config.deficit_strategy();
        render_markdown_content(ui, &mut app.orchestrator.tabs, frame_fill, deficit_strategy);
    }
}

/// Purpose: Renders the shared markdown preview content (YAML table, rendered
/// markdown, task toggles) inside an optional sepia frame and a vertical
/// scroll area.
/// Inputs: `ui` - Egui UI context, `tabs` - The tab manager holding the
///   current markdown, YAML, and scroll state, `frame_fill` - Optional frame
///   background color (sepia tint for PDF-backed files, `None` otherwise),
///   `deficit_strategy` - The table width deficit strategy for rendering.
/// Outputs: None.
/// Purity: Impure (performs UI rendering and mutates pending_task_toggles).
/// Preconditions: None.
/// Postconditions: Renders the markdown preview with YAML front-matter table
///   (if present) and applies any pending task-checkbox toggles to the markdown
///   source.
#[tracing::instrument(skip_all, name = "ui.center.render_markdown_content", level = "debug")]
fn render_markdown_content(
    ui: &mut egui::Ui,
    tabs: &mut crate::ui::Tabs,
    frame_fill: Option<egui::Color32>,
    deficit_strategy: crate::ui::table_width::DeficitStrategy,
) {
    if let Some(fill) = frame_fill {
        egui::Frame::new().fill(fill).show(ui, |ui| {
            show_markdown_scroll_area(ui, tabs, deficit_strategy);
        });
    } else {
        show_markdown_scroll_area(ui, tabs, deficit_strategy);
    }
}

/// Renders the scroll area containing YAML table, markdown, and task toggles.
#[tracing::instrument(
    skip_all,
    name = "ui.center.show_markdown_scroll_area",
    level = "debug"
)]
fn show_markdown_scroll_area(
    ui: &mut egui::Ui,
    tabs: &mut crate::ui::Tabs,
    deficit_strategy: crate::ui::table_width::DeficitStrategy,
) {
    egui::ScrollArea::vertical()
        .id_salt("main_markdown_scroll")
        .show(ui, |ui| {
            if let Some(yaml) = &tabs.current_yaml {
                render_yaml_table(ui, yaml);
            }
            let heading_ids = tabs.heading_ids().to_vec();
            render_markdown(
                ui,
                &tabs.current_markdown,
                &mut tabs.scroll_to_header_id,
                &mut tabs.pending_task_toggles,
                deficit_strategy,
                Some(&heading_ids),
            );
            // P0-2: Apply task checkbox toggles to the markdown source.
            if !tabs.pending_task_toggles.is_empty() {
                for (idx, checked) in tabs.pending_task_toggles.drain(..) {
                    crate::ui::render::apply_task_toggle(&mut tabs.current_markdown, idx, checked);
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
#[tracing::instrument(skip_all, name = "ui.center.render_empty_state", level = "debug")]
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
#[tracing::instrument(skip_all, name = "ui.panel.center", level = "debug")]
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
