//! Bottom command/agent panel — prompt intent parsing (`/models`, agent prompt), agent status, response, and token-usage stats.
//!
//! Unit tests live in the sibling `bottom_tests.rs` sidecar.

use crate::ui::FastMdApp;
use eframe::egui;
use egui::RichText;
use egui::containers::Panel;

/// Enum representing the parsed intent from a user command prompt.
#[derive(Debug, PartialEq)]
pub enum CommandIntent {
    ShowModels,
    ShowDeprecatedModelMessage,
    RunAgent(String),
    Empty,
}

/// Purpose: Detects whether the current frame carries an Enter key
/// press, whether it arrives as a regular `Key::Enter` event or as an
/// IME commit of a newline.
/// Inputs: `input` — the egui input state for the current frame.
/// Outputs: `true` if an Enter press is present.
/// Purity: Pure.
/// Preconditions: None.
/// Postconditions: None.
///
/// On Windows, when an IME processes the Enter key the raw
/// `Key::Enter` event is swallowed (winit reports `VK_PROCESSKEY`,
/// which `egui-winit` filters out) and only an `Event::Ime` newline
/// commit reaches the app. The Enter-to-submit path in the bottom
/// panel must recognise both deliveries, otherwise pressing Enter
/// after an IME interaction (e.g. pasting text) silently does nothing.
pub fn is_enter_pressed(input: &egui::InputState) -> bool {
    input.key_pressed(egui::Key::Enter)
        || input.events.iter().any(|event| match event {
            egui::Event::Ime(egui::ImeEvent::Commit(text))
            | egui::Event::Ime(egui::ImeEvent::Preedit {
                text,
                active_range_chars: _,
            }) => text == "\n" || text == "\r",
            _ => false,
        })
}

/// Parses the user prompt to determine the intended command.
pub fn parse_command_intent(prompt: &str) -> CommandIntent {
    let trimmed = prompt.trim();
    if trimmed.starts_with("/models") {
        CommandIntent::ShowModels
    } else if trimmed.starts_with("/model ") {
        CommandIntent::ShowDeprecatedModelMessage
    } else if !trimmed.is_empty() {
        CommandIntent::RunAgent(trimmed.to_string())
    } else {
        CommandIntent::Empty
    }
}

/// Computes the prompt prefix based on the selected directory and available content libraries.
pub fn compute_prompt_prefix(
    selected_dir: Option<&std::path::Path>,
    content_libraries: &[crate::config::ContentLibrary],
) -> String {
    if let Some(dir) = selected_dir {
        let rel_str = crate::config::library_display_label(content_libraries, dir)
            .unwrap_or_else(|| dir.to_string_lossy().to_string());
        if rel_str.is_empty() {
            ">".to_string()
        } else {
            format!("{} >", rel_str)
        }
    } else {
        ">".to_string()
    }
}

pub fn format_models_list(
    models: &std::collections::HashMap<String, crate::config::LlmConfig>,
) -> String {
    let mut output = crate::ui::strings::MODELS_LIST_HEADER.to_string();
    let mut sorted_names: Vec<&String> = models.keys().collect();
    sorted_names.sort();

    for name in sorted_names {
        let model_cfg = &models[name];
        let use_cases = model_cfg.use_case.join(", ");
        output.push_str(&format!(
            "- {} [cost: {}, use_case: {}]\n",
            name,
            model_cfg.get_cost(),
            use_cases
        ));
    }
    if models.is_empty() {
        output.push_str(crate::ui::strings::MODELS_LIST_NO_ADDITIONAL);
    }
    output
}

/// Purpose: Applies the side effect of pressing Enter in the
/// command input (or clicking an equivalent submit trigger).
/// Inputs: app (the application state)
/// Outputs: ()
/// Purity: Impure (mutates `app.orchestrator.agent`, `app.orchestrator.config`,
/// `app.orchestrator.selection`).
/// Preconditions: `app.agent().command_input` contains the user's
/// prompt. The `command_input` is consumed and cleared as part of
/// the call.
/// Postconditions: Dispatches based on `parse_command_intent`:
///   * `ShowModels` — sets status to "Done", response to the
///     formatted model list, and show_results to `true`.
///   * `ShowDeprecatedModelMessage` — sets status to "Error" and
///     response to the deprecation message.
///   * `RunAgent(agent_prompt)` — starts an agent session with
///     the prompt and the current selection context, and sets
///     show_results to `true`.
///   * `Empty` — no-op.
///
/// The Enter-key handler in `show_bottom_panel` calls this
/// function. It is extracted so the dispatch can be unit-tested
/// without driving the egui harness.
pub fn apply_send_click(app: &mut FastMdApp) {
    let prompt = app.agent_mut().command_input.trim_end().to_string();
    app.agent_mut().command_input.clear();

    match parse_command_intent(&prompt) {
        CommandIntent::ShowModels => {
            app.agent_mut().set_status("Done".to_string());
            let models_response = format_models_list(&app.orchestrator.config.models);
            app.agent_mut().set_response(models_response);
            app.agent_mut().set_show_results(true);
        }
        CommandIntent::ShowDeprecatedModelMessage => {
            app.agent_mut().set_status("Error".to_string());
            app.agent_mut()
                .set_response(crate::ui::strings::DEPRECATED_MODEL_MESSAGE.to_string());
            app.agent_mut().set_show_results(true);
        }
        CommandIntent::RunAgent(agent_prompt) => {
            let tx = app.orchestrator.tx.clone();
            let (file, dir, files) = app
                .selection()
                .agent_context(&app.orchestrator.tab_manager.tabs);
            let bus = app.orchestrator.file_event_bus.clone();
            app.agent_mut()
                .start_session(tx, agent_prompt, file, dir, files, bus);
            app.agent_mut().set_show_results(true);
        }
        CommandIntent::Empty => {}
    }
}

pub fn show_bottom_panel(app: &mut FastMdApp, parent_ui: &mut egui::Ui) {
    show_bottom_panel_capture(app, parent_ui, |_| {});
}

/// Tier 4 test variant of [`show_bottom_panel`]. The `on_click`
/// callback is invoked after every dispatch trigger (Enter key on
/// the command input) with a stable event name. The production
/// caller ([`show_bottom_panel`]) passes a no-op closure; the
/// test caller in `tests::test_send_enter_key_captures_event`
/// passes a closure that pushes the event into the harness's
/// persistent state.
pub fn show_bottom_panel_capture(
    app: &mut FastMdApp,
    parent_ui: &mut egui::Ui,
    mut on_click: impl FnMut(&'static str),
) {
    // egui 0.35 unified `TopBottomPanel`/`SidePanel` into `Panel`,
    // and panels now allocate within a parent `&mut Ui` (using
    // `show_inside`) rather than from a `&Context`.
    Panel::bottom("bottom_panel")
        .resizable(true)
        // egui 0.35: `min_height` was replaced by `min_size` (and the
        // same `size` parameter is used for all four sides).
        .min_size(32.0)
        .show(parent_ui, |ui| {
            // Some branches below still need the context for input
            // polling — pull it from the inner Ui.
            let ctx = ui.ctx().clone();
            ui.horizontal(|ui| {
                let prompt_prefix = compute_prompt_prefix(
                    app.selection()
                        .prompt_dir(&app.orchestrator.tab_manager.tabs)
                        .as_deref(),
                    app.content_libraries(),
                );
                ui.label(RichText::new(prompt_prefix).monospace().strong());

                // Lay the Stop button out right-to-left first so it hugs
                // the panel's right edge, then measure where it ends to
                // give the prompt every remaining pixel. This replaces the
                // old fixed 130px reserve, which left dead space between
                // the prompt and the button.
                let row = ui.available_rect_before_wrap();
                let mut stop_clicked = false;
                let mut prompt_right = row.max.x;
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if app.agent().state().running {
                        let response = ui.button(
                            RichText::new(crate::ui::strings::STOP_AGENT_BUTTON)
                                .color(egui::Color32::RED),
                        );
                        stop_clicked = response.clicked();
                        prompt_right = response.rect.left();
                    }
                });

                let prompt_rect = egui::Rect::from_min_max(
                    row.min,
                    egui::pos2(prompt_right.max(row.min.x), row.max.y),
                );
                let response = ui.put(
                    prompt_rect,
                    egui::TextEdit::multiline(&mut app.agent_mut().command_input)
                        .desired_width(f32::INFINITY)
                        .hint_text(crate::ui::strings::COMMAND_INPUT_HINT),
                );

                let mut submit = false;
                if response.has_focus()
                    && ctx.input(is_enter_pressed)
                    && !ctx.input(|i| i.modifiers.shift)
                {
                    submit = true;
                }

                // The bottom panel used to host a `⚡ Quick Tasks`
                // menu button whose only item was `Format Markdown`.
                // That entry point duplicated the file context
                // menu's [Format Markdown] action, so the button
                // was removed. Quick Actions now live in the
                // file context menu — see
                // `doc/planning/quick-actions-into-context-menu.md`.

                if stop_clicked {
                    app.agent_mut().cancel();
                }

                if submit {
                    if app.agent().state().running {
                        // Agent is running - queue the prompt
                        let prompt = app.agent_mut().command_input.trim_end().to_string();
                        if !prompt.is_empty() {
                            app.agent_mut().queue_prompt(prompt);
                            app.agent_mut().command_input.clear();
                        }
                    } else {
                        // Agent is idle - submit normally
                        apply_send_click(app);
                        on_click("send");
                    }
                }
            });
        });
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `bottom_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "bottom_tests.rs"]
mod tests;
