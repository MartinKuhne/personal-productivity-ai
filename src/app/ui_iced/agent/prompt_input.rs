//! The iced agent prompt row — prompt box, send / queue / cancel controls and
//! the chat-model selector ([TASK-018], UI-001 / UI-064 / UI-065).
//!
//! This is the agent-panel-composed prompt surface. It shares its intent
//! vocabulary with the pane-level command band
//! ([`crate::ui_iced::panels::bottom`]) rather than re-inventing one: the
//! Send and Queue buttons both emit [`AgentMessage::Submit`], and the
//! reducer runs the reused [`submit_commands`] helper to decide
//! `RunAgent` (idle) vs `QueueAgentPrompt` (running) — exactly the
//! running-vs-idle branch the legacy UI kept inline in its widget closure
//! (`src/app/ui/panels/bottom.rs:259-294`). Cancel emits
//! [`UserCommand::CancelAgent`]; the model pick emits
//! [`UserCommand::SelectChatModel`]; Clear session emits
//! [`UserCommand::ClearAgentSession`].
//!
//! # Reused, not reimplemented ([RUST-051])
//!
//! * [`submit_commands`] and [`clear_command_input_command`] — the legacy UI
//!   band's framework-free submit solver, re-exported.
//! * [`ChatModelEntry`] — the top bar's model row type, reused for the
//!   selector so both surfaces format and carry the model identically.
//!
//! # Send / queue gating
//!
//! The two buttons are **always allocated** and toggled by
//! [`send_enabled`] (non-empty prompt **and** idle) and [`queue_enabled`]
//! (non-empty prompt **and** running) via `Button::on_press_maybe`, so the
//! row's shape never changes across the idle ⇄ running or empty ⇄ filled
//! transitions (AGENTS §6).
//!
//! Unit tests live in the sibling `prompt_input_tests.rs` sidecar.

use iced::widget::{Column, Row, button, pick_list, text, text_input};
use iced::{Alignment, Element, Length};

use crate::bus::events::user_command::UserCommand;
use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::message::{AgentMessage, Message};
use crate::ui_iced::panels::top::ChatModelEntry;

use super::panel_state::AgentPanelViewModel;

/// The legacy UI command band's framework-free submit solver, reused rather
/// than duplicated ([RUST-051]) — `src/app/ui/panels/bottom.rs:185-202`.
/// The reducer calls it on [`AgentMessage::Submit`].
pub use crate::ui_iced::panels::bottom::{clear_command_input_command, submit_commands};

/// Logical point size of the prompt controls.
pub const CONTROL_TEXT_SIZE: f32 = 14.0;

/// Spacing between the controls in the row.
pub const ROW_SPACING: f32 = 6.0;

/// Padding inside the row.
pub const ROW_PADDING: f32 = 6.0;

/// Whether the Send button can fire: a non-blank prompt and the agent idle.
///
/// Purity: pure.
#[must_use]
pub fn send_enabled(prompt: &str, running: bool) -> bool {
    !prompt.trim().is_empty() && !running
}

/// Whether the Queue button can fire: a non-blank prompt and the agent
/// already running (the prompt parks behind the current turn).
///
/// Purity: pure.
#[must_use]
pub fn queue_enabled(prompt: &str, running: bool) -> bool {
    !prompt.trim().is_empty() && running
}

/// Whether the Cancel (Stop) button can fire: only while the agent runs.
///
/// Purity: pure.
#[must_use]
pub fn cancel_enabled(running: bool) -> bool {
    running
}

/// The queued-prompt count badge label, `None` when nothing is parked.
///
/// Purity: pure function of `queued`.
#[must_use]
pub fn queue_display(queued: &[String]) -> Option<String> {
    if queued.is_empty() {
        None
    } else {
        Some(strings::format_queued_prompts_label(queued.len()))
    }
}

/// The [`Message`] a chat-model pick produces (UI-065).
///
/// Purity: pure function of `name`.
#[must_use]
pub fn select_model_message(name: &str) -> Message {
    Message::Command(UserCommand::SelectChatModel(name.to_string()))
}

/// The [`Message`] the Clear-session control produces — the same
/// `ClearAgentSession` intent the agent header's Close button carries
/// (`src/app/ui/panels/center.rs:112-119`).
///
/// Purity: pure.
#[must_use]
pub fn clear_session_message() -> Message {
    Message::Command(crate::ui_iced::panels::center::clear_agent_session_command())
}

/// The [`ChatModelEntry`] in `options` matching `current`, if still offered.
///
/// Borrows out of the snapshot's owned list so the reference is valid for
/// the view's lifetime — what `pick_list` needs (`V: Borrow<T> + 'a`,
/// `iced_widget-0.14.2/src/pick_list.rs:179-195`).
///
/// Purity: pure.
#[must_use]
pub fn current_model_option<'a>(
    options: &'a [ChatModelEntry],
    current: Option<&str>,
) -> Option<&'a ChatModelEntry> {
    let current = current?;
    options.iter().find(|entry| entry.name == current)
}

/// Renders the agent prompt row from a snapshot ([TASK-018]).
///
/// Emitted messages:
/// * typing → [`AgentMessage::InputChanged`]
/// * Enter / Send → [`AgentMessage::Submit`] (reducer runs
///   [`submit_commands`])
/// * Queue while running → [`AgentMessage::Submit`] (reducer queues)
/// * Stop while running → [`UserCommand::CancelAgent`]
/// * model pick → [`UserCommand::SelectChatModel`]
/// * Clear session → [`UserCommand::ClearAgentSession`]
///
/// Every control is always allocated and gated with `on_press_maybe`, so the
/// row's widget shape is stable across the idle ⇄ running and empty ⇄
/// filled transitions (AGENTS §6).
#[must_use]
pub fn agent_prompt_row(view_model: &AgentPanelViewModel) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();

    let command_field = text_input(strings::COMMAND_INPUT_HINT, &view_model.command_input)
        .on_input(|text| Message::Agent(AgentMessage::InputChanged { text }))
        .on_submit(Message::Agent(AgentMessage::Submit {
            text: view_model.command_input.clone(),
        }));

    let send_button = button(
        text(strings::AGENT_SEND_BUTTON)
            .size(CONTROL_TEXT_SIZE)
            .color(palette.text),
    )
    .on_press_maybe(
        send_enabled(&view_model.command_input, view_model.running).then(|| {
            Message::Agent(AgentMessage::Submit {
                text: view_model.command_input.clone(),
            })
        }),
    );

    let queue_button = button(
        text(strings::AGENT_QUEUE_BUTTON)
            .size(CONTROL_TEXT_SIZE)
            .color(palette.text),
    )
    .on_press_maybe(
        queue_enabled(&view_model.command_input, view_model.running).then(|| {
            Message::Agent(AgentMessage::Submit {
                text: view_model.command_input.clone(),
            })
        }),
    );

    let cancel_button = button(
        text(strings::STOP_AGENT_BUTTON)
            .size(CONTROL_TEXT_SIZE)
            .color(palette.error),
    )
    .on_press_maybe(
        cancel_enabled(view_model.running).then_some(Message::Command(UserCommand::CancelAgent)),
    );

    let model_label = text(strings::AGENT_MODEL_LABEL)
        .size(CONTROL_TEXT_SIZE)
        .color(palette.text_muted);

    let model_pick: Element<'_, Message> = pick_list(
        view_model.chat_models.clone(),
        current_model_option(
            &view_model.chat_models,
            view_model.current_chat_model.as_deref(),
        ),
        |entry: ChatModelEntry| select_model_message(&entry.name),
    )
    .placeholder(strings::NO_CHAT_MODELS_CONFIGURED)
    .into();

    let clear_button = button(
        text(strings::AGENT_SESSION_CLOSE_BUTTON)
            .size(CONTROL_TEXT_SIZE)
            .color(palette.text_muted),
    )
    .on_press(clear_session_message());

    let queue_badge: Element<'_, Message> = match queue_display(&view_model.queued_prompts) {
        Some(label) => text(label)
            .size(CONTROL_TEXT_SIZE)
            .color(palette.warning)
            .into(),
        None => Column::<Message>::new().into(),
    };

    Row::with_children(vec![
        command_field.width(Length::Fill).into(),
        send_button.into(),
        queue_button.into(),
        cancel_button.into(),
        model_label.into(),
        model_pick,
        queue_badge,
        clear_button.into(),
    ])
    .spacing(ROW_SPACING)
    .padding(ROW_PADDING)
    .align_y(Alignment::Center)
    .into()
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `prompt_input_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "prompt_input_tests.rs"]
mod tests;
