//! Iced port of the bottom command band ([TASK-016]; UI-001's "Bottom Pane
//! (command prompt)").
//!
//! Replaces `src/app/ui/panels/bottom.rs`. Like [`top`](super::top) and
//! [`left`](super::left), the band is a **pure view**: it renders a
//! [`BottomPanelViewModel`] snapshot into an [`iced::Element`] of
//! [`Message`] and never touches the `UserCommand` bus, the agent session or
//! `FastMdApp` itself — the [TASK-022]/[TASK-023] reducer publishes what the
//! band emits ([FMD-068]).
//!
//! # Shape
//!
//! ```text
//! ~/Notes >            [ Type command (Enter to submit…)      ] [⏹ Stop]
//! note.md
//! ```
//!
//! * **Prompt prefix** — the library-relative directory the prompt will run
//!   against, [`compute_prompt_prefix`].
//! * **File context** — the active file, sized down against the prefix so the
//!   pair stays readable, [`compute_file_context`].
//! * **Command input** — [`strings::COMMAND_INPUT_HINT`] placeholder; emits
//!   [`AgentMessage::InputChanged`] / [`AgentMessage::Submit`].
//! * **Stop** — [`strings::STOP_AGENT_BUTTON`], live only while the agent is
//!   running; emits [`UserCommand::CancelAgent`].
//!
//! # Reused, not reimplemented ([RUST-051])
//!
//! The band's framework-free helpers were **relocated into this module** for
//! the framework-free migration ([TASK-029]): [`compute_prompt_prefix`],
//! [`compute_file_context`], [`format_models_list`],
//! [`parse_command_intent`] and [`clear_command_input_command`] used to be
//! `pub use`d from `src/app/ui/panels/bottom.rs`, a file [TASK-029b]
//! deletes. They are copied here verbatim — every one is framework-free (pure
//! `Path`/`String`/`UserCommand` signatures, no framework type anywhere in the
//! body), so the iced view, the [TASK-022] reducer and the iced sidecar
//! tests all run the same rules the legacy UI band ran. The legacy UI panel keeps its
//! own copy until Phase 5; the transient duplication dies with it.
//!
//! The one piece of *behaviour* that the legacy UI kept inline inside the widget
//! closure — "running → queue the prompt, idle → parse the intent, then clear
//! the input" — is extracted into [`submit_commands`] so it is testable
//! without a runtime. The reducer calls it and publishes the returned
//! commands in order.
//!
//! # Not ported, with reasons
//!
//! * **The IME Enter workaround** (`ui/panels/bottom.rs:13-40`). the legacy UI needed
//!   `is_enter_pressed` because `the legacy winit backend` swallows the raw `Key::Enter`
//!   when an IME claims it (`VK_PROCESSKEY`) and only an
//!   `Event::Ime(Commit("\n"))` arrives. iced 0.14 does not surface IME
//!   commits to the application at all — `text_input` publishes its
//!   `on_submit` message from its own key handling
//!   (`iced_widget-0.14.2/src/text_input.rs:1050-1056`), so there is no
//!   second delivery path to reconcile and no equivalent helper to write.
//! * **Shift+Enter = newline.** The legacy UI band submitted on Enter *unless*
//!   Shift was held (`ui/panels/bottom.rs:238-243`). iced's `text_input`
//!   publishes `on_submit` on Enter with **no modifier check**
//!   (`text_input.rs:1050-1056`), and the message is a fixed `Message`, so
//!   the view cannot branch on Shift. A faithful multi-line field would need
//!   `text_editor` (DEC-007, [TASK-014]) whose `Action::Edit(Edit::Enter)`
//!   likewise carries no modifiers — the gap is framework-wide, not a shortcut
//!   taken here. Flagged for [TASK-018]/[TASK-022]: if multi-line prompts
//!   matter, the reducer must own a `text_editor` buffer and treat Enter as
//!   submit unconditionally, or the hint text must change.
//! * **The Tier-4 `on_click("send")` sink** (`ui/panels/bottom.rs:148-159`)
//!   is `legacy-snapshot-harness` plumbing. CON-003 rules it out; the iced equivalent is
//!   the `iced_test::simulator` tier of [TASK-025]/[TASK-026].
//! * **Hand-rolled right-alignment.** the legacy UI computed a 100 pt rect by hand to
//!   park the Stop button at the far right (`ui/panels/bottom.rs:194-209`).
//!   iced's `Row` lays it out, so neither the magic number nor the
//!   `available_rect_before_wrap` bookkeeping is needed.
//!
//! Unit tests live in the sibling `bottom_tests.rs` sidecar.

#[cfg(test)]
#[path = "bottom_tests.rs"]
mod tests;

use iced::widget::{Column, Row, button, container, text, text_input};
use iced::{Alignment, Color, Element, Font, Length};

use crate::bus::events::user_command::UserCommand;
use crate::ui::app::FastMdApp;
use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::message::{AgentMessage, Message};

// The legacy UI band's framework-free helpers, **relocated into this module**
// for the framework-free migration ([TASK-029]) from
// `src/app/ui/panels/bottom.rs:44-142`.
//
// Each is copied verbatim and is framework-free: no framework type appears
// in any signature or body. See the individual items below.
//
// The legacy UI panel keeps its own copy until [TASK-029b] removes it; the
// transient duplication is intentional.

/// Parses the user prompt to determine the intended command.
/// Computes the prompt prefix based on the selected directory and available content libraries.
///
/// Relocated from the legacy UI bottom band for the framework-free migration
/// ([TASK-029]).
#[must_use]
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
///
/// Relocated from the legacy UI bottom band for the framework-free migration
/// ([TASK-029]).
#[must_use]
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

/// Renders the chat-model catalogue the `/models` command reports.
///
/// Relocated from the legacy UI bottom band for the framework-free migration
/// ([TASK-029]).
#[must_use]
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

/// Purpose: Parses the user prompt and returns the appropriate
/// `UserCommand` to dispatch. Does NOT mutate state.
/// Inputs: `prompt` - the user's input text (already trimmed)
/// Outputs: `Option<UserCommand>` - the command to publish, or `None` if empty
/// Purity: Pure (no state mutation).
/// Preconditions: None.
/// Postconditions: Returns the command corresponding to the parsed intent.
///
/// Relocated from the legacy UI bottom band for the framework-free migration
/// ([TASK-029]).
#[must_use]
pub fn parse_command_intent(prompt: &str) -> Option<UserCommand> {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.starts_with("/models") {
        Some(UserCommand::ShowModels)
    } else if trimmed.starts_with("/model ") {
        Some(UserCommand::ShowDeprecatedModelMessage)
    } else {
        Some(UserCommand::RunAgent(trimmed.to_string()))
    }
}

/// Purpose: Returns a command to clear the command input.
/// Inputs: None
/// Outputs: `UserCommand::ClearCommandInput`
/// Purity: Pure (returns command, no state mutation).
///
/// Relocated from the legacy UI bottom band for the framework-free migration
/// ([TASK-029]).
#[must_use]
pub fn clear_command_input_command() -> UserCommand {
    UserCommand::ClearCommandInput
}

/// Logical point size of the prompt prefix and the command input.
pub const PROMPT_TEXT_SIZE: f32 = 14.0;

/// Logical point size of the Stop button label.
pub const STOP_BUTTON_TEXT_SIZE: f32 = 14.0;

/// Spacing between the band's items.
pub const BAND_SPACING: f32 = 8.0;

/// Vertical spacing between the prompt prefix and the file-context label.
pub const CONTEXT_SPACING: f32 = 2.0;

/// Inner padding of the band.
pub const BAND_PADDING: f32 = 6.0;

/// Fallback size for the file-context label when the reused
/// [`compute_file_context`] helper produced none.
pub const FILE_CONTEXT_FALLBACK_SIZE: f32 = 12.0;

/// What the bottom band needs to render, snapshotted off `FastMdApp`.
///
/// Mirrors [`super::top::TopBarViewModel`]: the widget tree reads only this
/// struct, so the band is unit-testable without a live iced runtime and stays
/// decoupled from where each field comes from.
#[derive(Clone, Debug, PartialEq)]
pub struct BottomPanelViewModel {
    /// The command input's current text (`AgentPanelState::command_input`).
    pub command_input: String,
    /// Whether the agent is currently running — the gate on the Stop button
    /// and on queue-vs-submit ([`submit_commands`]).
    pub agent_running: bool,
    /// The prompt prefix: the library-relative directory the next prompt
    /// runs against.
    pub prompt_prefix: String,
    /// The active file's display name plus its de-emphasised font size,
    /// `None` when there is no active file context.
    pub file_context: Option<(String, f32)>,
}

impl BottomPanelViewModel {
    /// Snapshots the band's inputs off `app`, reading exactly the accessors
    /// the legacy UI band used: `selection().agent_context(..)`,
    /// `selection().prompt_dir(..)`, `content_libraries()`,
    /// `agent().state().running` and
    /// `orchestrator.agent_panel_state.command_input`.
    pub fn from_app(app: &FastMdApp) -> Self {
        let tabs = &app.orchestrator.tabs.tabs;
        let (active_file, _, _) = app.selection().agent_context(tabs);

        let prompt_prefix = compute_prompt_prefix(
            app.selection().prompt_dir(tabs).as_deref(),
            app.content_libraries(),
        );

        Self {
            command_input: app.orchestrator.agent_panel_state.command_input.clone(),
            agent_running: app.agent().state().running,
            prompt_prefix: prompt_prefix.clone(),
            file_context: compute_file_context(active_file.as_deref(), &prompt_prefix),
        }
    }

    /// The commands submitting [`Self::command_input`] produces right now.
    ///
    /// Delegates to [`submit_commands`] with this snapshot's running state.
    #[must_use]
    pub fn submit_commands(&self) -> Vec<UserCommand> {
        submit_commands(self.agent_running, &self.command_input)
    }
}

/// The commands a submit produces, in publish order.
///
/// Extracted from the legacy UI band's inline submit branch
/// (`src/app/ui/panels/bottom.rs:259-294`), which ran the same two paths
/// inside the widget closure:
///
/// * **Agent running** — the prompt is queued behind the current turn
///   ([`UserCommand::QueueAgentPrompt`]) and the input is cleared. A
///   blank/whitespace prompt queues nothing, so no stray clear is published
///   either (the legacy UI's `if !prompt.is_empty()` guard).
/// * **Agent idle** — [`parse_command_intent`] decides the command
///   (`/models` → `ShowModels`, `/model …` → the deprecation notice,
///   anything else → `RunAgent`), followed by
///   [`clear_command_input_command`]. A blank prompt parses to `None`, so
///   nothing is published at all.
///
/// The clear command travels with the intent so the reducer's result is
/// replayable from the returned vector alone ([FMD-068]).
///
/// Purity: pure function of `(agent_running, raw)`.
#[must_use]
pub fn submit_commands(agent_running: bool, raw: &str) -> Vec<UserCommand> {
    let prompt = raw.trim_end();

    if agent_running {
        if prompt.is_empty() {
            return Vec::new();
        }
        return vec![
            UserCommand::QueueAgentPrompt(prompt.to_string()),
            clear_command_input_command(),
        ];
    }

    match parse_command_intent(prompt) {
        Some(command) => vec![command, clear_command_input_command()],
        None => Vec::new(),
    }
}

/// Whether the Stop button can cancel something right now.
///
/// Purity: pure.
#[must_use]
pub fn stop_enabled(agent_running: bool) -> bool {
    agent_running
}

/// Colour of the always-allocated Stop button's label.
///
/// A disabled `Button::on_press_maybe(None)` still renders in its enabled
/// look, which made the idle band present a red "Stop" over nothing running
/// (the button-visibility defect UI-001 reports). The label therefore
/// drops to the `text_muted` token while idle and returns to `error` the
/// moment a turn is cancellable — same palette rule as
/// [`crate::ui_iced::panels::top::indexing_status_color`] ([GUD-003]).
///
/// Purity: pure function of `running`.
#[must_use]
pub fn stop_button_color(running: bool) -> Color {
    let palette = Palette::fastmd_dark();
    if running {
        palette.error
    } else {
        palette.text_muted
    }
}

/// Renders the iced bottom command band from a view-model snapshot
/// ([TASK-016]).
///
/// Emitted messages:
/// * typing → [`Message::Agent`] [`AgentMessage::InputChanged`]
/// * Enter → [`Message::Agent`] [`AgentMessage::Submit`] (the reducer runs
///   [`submit_commands`] on it)
/// * Stop while running → [`Message::Command`] [`UserCommand::CancelAgent`]
///
/// Every item is **always allocated**: with no file context the label is an
/// empty string rather than a missing widget, and the Stop button is always
/// present but inert unless the agent is running
/// (`Button::on_press_maybe`). The widget-tree shape therefore does not
/// change across the idle ⇄ running or no-file ⇄ file transitions
/// (AGENTS §6).
#[must_use]
pub fn bottom_panel(view_model: &BottomPanelViewModel) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();

    let prompt_prefix = text(&view_model.prompt_prefix)
        .font(Font::MONOSPACE)
        .size(PROMPT_TEXT_SIZE)
        .color(palette.text_strong);

    let (file_name, file_size) = view_model
        .file_context
        .clone()
        .unwrap_or_else(|| (String::new(), FILE_CONTEXT_FALLBACK_SIZE));

    let file_label = text(file_name)
        .font(Font::MONOSPACE)
        .size(file_size)
        .color(palette.text_muted);

    // the legacy UI stacked the prefix above the file name
    // (`ui/panels/bottom.rs:214-224`); the Column keeps that shape.
    let context_group = Column::with_children(vec![prompt_prefix.into(), file_label.into()])
        .spacing(CONTEXT_SPACING);

    let command_field = text_input(strings::COMMAND_INPUT_HINT, &view_model.command_input)
        .on_input(|text| Message::Agent(AgentMessage::InputChanged { text }))
        .on_submit(Message::Agent(AgentMessage::Submit {
            text: view_model.command_input.clone(),
        }));

    let stop_button = button(
        text(strings::STOP_AGENT_BUTTON)
            .size(STOP_BUTTON_TEXT_SIZE)
            .color(stop_button_color(view_model.agent_running)),
    )
    .on_press_maybe(
        stop_enabled(view_model.agent_running)
            .then_some(Message::Command(UserCommand::CancelAgent)),
    );

    Row::with_children(vec![
        context_group.into(),
        command_field.width(Length::Fill).into(),
        container(stop_button)
            .width(Length::Fill)
            .align_right(Length::Shrink)
            .into(),
    ])
    .spacing(BAND_SPACING)
    .padding(BAND_PADDING)
    .align_y(Alignment::Center)
    .into()
}
