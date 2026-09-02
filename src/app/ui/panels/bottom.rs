//! Bottom command/agent panel — prompt intent parsing (`/models`, agent prompt), agent status, response, and token-usage stats.
//!
//! Unit tests live in the sibling `bottom_tests.rs` sidecar.

use crate::ui::FastMdApp;
use eframe::egui;
use egui::RichText;
use egui::containers::Panel;

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
/// Windows newlines are often CRLF (`\r\n`), so we accept `\n`, `\r`,
/// or `\r\n` as the IME commit text.
pub fn is_enter_pressed(input: &egui::InputState) -> bool {
    input.key_pressed(egui::Key::Enter)
        || input.events.iter().any(|event| match event {
            egui::Event::Ime(egui::ImeEvent::Commit(text))
            | egui::Event::Ime(egui::ImeEvent::Preedit {
                text,
                active_range_chars: _,
            }) => matches!(text.as_str(), "\n" | "\r" | "\r\n"),
            _ => false,
        })
}

/// Parses the user prompt to determine the intended command.
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

/// Computes the file name context display from the active file.
/// Returns `None` when there is no active file context.
pub fn compute_file_context(
    active_file: Option<&std::path::Path>,
    prompt_prefix: &str,
) -> Option<(String, f32)> {
    let file_name = active_file?.file_name()?.to_string_lossy().to_string();
    if file_name.is_empty() {
        return None;
    }

    let dir_display = prompt_prefix.trim_end_matches(" >").trim_end_matches('>');
    let dir_len = dir_display.chars().count();
    let file_len = file_name.chars().count();

    let base_size = 12.0;
    let min_size = 8.0;

    let font_size = if file_len > dir_len && dir_len > 0 {
        let ratio = dir_len as f32 / file_len as f32;
        (base_size * ratio).max(min_size)
    } else {
        base_size
    };

    Some((file_name, font_size))
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
/// Preconditions: `app.orchestrator.agent_panel_state.command_input` contains the user's
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
pub fn apply_send_click(
    app: &mut FastMdApp,
) -> Option<crate::bus::events::user_command::UserCommand> {
    let prompt = app
        .orchestrator
        .agent_panel_state
        .command_input
        .trim_end()
        .to_string();

    // Since we clear the prompt in the UI when the user clicks send,
    // this mutation remains inline. (Note: could be deferred to CommandExecutor if desired)
    app.orchestrator.agent_panel_state.command_input.clear();

    if prompt.is_empty() {
        return None;
    }

    let trimmed = prompt.trim();
    if trimmed.starts_with("/models") {
        Some(crate::bus::events::user_command::UserCommand::ShowModels)
    } else if trimmed.starts_with("/model ") {
        Some(crate::bus::events::user_command::UserCommand::ShowDeprecatedModelMessage)
    } else {
        Some(crate::bus::events::user_command::UserCommand::RunAgent(
            trimmed.to_string(),
        ))
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
#[tracing::instrument(skip_all, name = "ui.panel.bottom", level = "debug")]
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

            // Get the file context (active file) for display
            let (active_file, _, _) = app.selection().agent_context(&app.orchestrator.tabs.tabs);

            let prompt_prefix = compute_prompt_prefix(
                app.selection()
                    .prompt_dir(&app.orchestrator.tabs.tabs)
                    .as_deref(),
                app.content_libraries(),
            );

            let file_context = compute_file_context(active_file.as_deref(), &prompt_prefix);

            // Get the full available rect for the panel row BEFORE any horizontal layout
            let full_row = ui.available_rect_before_wrap();

            // === RIGHT COLUMN: Stop button (laid out first at far right of panel) ===
            let mut stop_clicked = false;
            let mut stop_button_width = 0.0;
            if app.agent().state().running {
                let button_text = crate::ui::strings::STOP_AGENT_BUTTON;
                let button_response = ui.put(
                    egui::Rect::from_min_max(
                        egui::pos2(full_row.max.x - 100.0, full_row.min.y), // approximate width
                        full_row.max,
                    ),
                    egui::Button::new(RichText::new(button_text).color(egui::Color32::RED)),
                );
                stop_clicked = button_response.clicked();
                stop_button_width = button_response.rect.width();
            }

            // Now do horizontal layout for left column + prompt in remaining space
            let remaining_rect = egui::Rect::from_min_max(
                full_row.min,
                egui::pos2(full_row.max.x - stop_button_width, full_row.max.y),
            );

            ui.scope_builder(egui::UiBuilder::new().max_rect(remaining_rect), |ui| {
                ui.horizontal(|ui| {
                    // === LEFT COLUMN: Directory + File context (vertical stack) ===
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&prompt_prefix).monospace().strong());
                        if let Some((file_name, font_size)) = file_context {
                            ui.label(
                                RichText::new(&file_name)
                                    .monospace()
                                    .size(font_size)
                                    .color(egui::Color32::GRAY),
                            );
                        }
                    });

                    // === MIDDLE COLUMN: Prompt input (takes all remaining space) ===
                    let prompt_rect = ui.available_rect_before_wrap();
                    let response = ui.put(
                        prompt_rect,
                        egui::TextEdit::multiline(
                            &mut app.orchestrator.agent_panel_state.command_input,
                        )
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
                        app.orchestrator
                            .user_command_bus
                            .publish(crate::bus::events::user_command::UserCommand::CancelAgent);
                    }

                    if submit {
                        if app.agent().state().running {
                            // Agent is running - queue the prompt
                            let prompt = app
                                .orchestrator
                                .agent_panel_state
                                .command_input
                                .trim_end()
                                .to_string();
                            if !prompt.is_empty() {
                                app.orchestrator.user_command_bus.publish(
                                    crate::bus::events::user_command::UserCommand::QueueAgentPrompt(
                                        prompt,
                                    ),
                                );
                                app.orchestrator.agent_panel_state.command_input.clear();
                            }
                        } else {
                            // Agent is idle - submit normally
                            if let Some(cmd) = apply_send_click(app) {
                                app.orchestrator.user_command_bus.publish(cmd);
                            }
                            on_click("send");
                        }
                    }
                });
            });
        });
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `bottom_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "bottom_tests.rs"]
mod tests;
