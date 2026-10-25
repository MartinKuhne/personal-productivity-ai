//! The iced agent prompt row — prompt box, send / queue / cancel controls and
//! the chat-model selector (UI-001 / UI-064 / UI-065).
//!
//! This is the agent-panel-composed prompt surface. It shares its intent
//! vocabulary with the pane-level command band
//! ([`crate::ui_iced::panels::bottom`]) rather than re-inventing one: the
//! Send and Queue buttons both emit [`AgentMessage::Submit`], and the
//! reducer runs the shared [`submit_commands`] helper to decide
//! `RunAgent` (idle) vs `QueueAgentPrompt` (running), so the running-vs-idle
//! branch lives in one place instead of per widget. Cancel emits
//! [`UserCommand::CancelAgent`]; the model pick emits
//! [`UserCommand::SelectChatModel`]; Clear session emits
//! [`UserCommand::ClearAgentSession`].
//!
//! # Shared, not reimplemented ([RUST-051])
//!
//! * [`submit_commands`] and [`clear_command_input_command`] — the
//!   framework-free submit solver, re-exported from
//!   [`crate::ui_iced::panels::bottom`].
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

use iced::widget::{Column, Row, pick_list, text, text_input};
use iced::{Alignment, Background, Border, Element, Length};

use iced_pro::state::{BaseState, VisualState};
use iced_pro::token::shape;
use iced_pro::token::space;
use iced_pro::token::typography::{self, TypeRung};
use iced_pro::widget::button::ButtonVariant;
use iced_pro::widget::button_widget::Button;
use iced_pro::widget::text_input as pro_text_input;

use crate::bus::events::user_command::UserCommand;
use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::message::{AgentMessage, Message};
use crate::ui_iced::panels::top::ChatModelEntry;

use super::panel_state::AgentPanelViewModel;

/// The framework-free submit solver, shared rather than duplicated
/// ([RUST-051]). The reducer calls it on [`AgentMessage::Submit`].
pub use crate::ui_iced::panels::bottom::{clear_command_input_command, submit_commands};

/// The rung every control label in the prompt row wears — `type.body`
/// (`UIF-TOK-004`).
///
/// Held as a rung because the row needs the size *and* the 20 px line box:
/// the field, the four buttons and the queue badge all have to sit on the
/// same baseline, and a rung makes that one fact rather than five.
pub const CONTROL_RUNG: TypeRung = typography::BODY;

/// Logical point size of the prompt controls — `type.body` (`UIF-TOK-004`).
/// Value unchanged at `14`.
pub const CONTROL_TEXT_SIZE: f32 = CONTROL_RUNG.size;

/// Spacing between the controls in the row — `space.sm` (`UIF-TOK-001`),
/// the standard sibling-control gap.
///
/// **Movement: 6 → 8 (+2).** `6` is not on the base-4 grid and sits exactly
/// midway between `XS` (4) and `SM` (8); the tie is broken on *role* as in
/// every other INT-7 piece — these are sibling controls, so `SM`.
pub const ROW_SPACING: f32 = space::SM;

/// Padding inside the row — `space.xs` (`UIF-TOK-001`).
///
/// **Movement: 6 → 4 (−2).** The same grid violation, the opposite
/// tie-break: this is the band's *inner inset*, which is `XS`'s documented
/// role ("compact inner padding … the menu container inset"), and it puts
/// the panel-side prompt row on the same inset as the command band's
/// [`BAND_PADDING`](crate::ui_iced::panels::bottom::BAND_PADDING) that this
/// row duplicates in layout.
pub const ROW_PADDING: f32 = space::XS;

/// Stable id for the agent prompt field ([AGENTS §5]).
///
/// Distinct from the bottom band's
/// [`COMMAND_INPUT_ID`](crate::ui_iced::panels::bottom::COMMAND_INPUT_ID):
/// the two fields hold the same string but are different live widgets, and
/// one id may only belong to one.
pub const PROMPT_INPUT_ID: &str = "agent_prompt_input";

/// Stable id for the Send button ([AGENTS §5]).
pub const SEND_BUTTON_ID: &str = "agent_send_button";

/// Stable id for the Queue button ([AGENTS §5]).
pub const QUEUE_BUTTON_ID: &str = "agent_queue_button";

/// Stable id for the Stop button ([AGENTS §5]).
pub const STOP_BUTTON_ID: &str = "agent_stop_button";

/// Stable id for the Clear-session button ([AGENTS §5]).
pub const CLEAR_SESSION_BUTTON_ID: &str = "agent_clear_session_button";

/// The prompt field's paint, resolved through the framework's text-input
/// atom (`UIF-ATOM-002`).
///
/// The same crossing point as the command band's
/// [`command_field_style`](crate::ui_iced::panels::bottom::command_field_style):
/// iced styles a `text_input` from `(Theme, Status)` while the framework
/// resolves one from `(InputKind, VisualState, Palette)`, so this translates
/// iced's four statuses into the framework's `(base, focused)` pair. Fill,
/// perimeter, value, placeholder, icon and selection all come from
/// [`pro_text_input::resolve_input_style`] at
/// [`InputKind::TextEntry`](pro_text_input::InputKind::TextEntry) rather
/// than from iced's own theme.
///
/// Purity: pure function of `status`.
#[must_use]
pub fn prompt_field_style(
    status: iced::widget::text_input::Status,
) -> iced::widget::text_input::Style {
    use iced::widget::text_input::{Status, Style};

    let palette = Palette::pro();
    let base = match status {
        Status::Active => BaseState::Rest,
        Status::Hovered => BaseState::Hovered,
        Status::Focused { .. } => BaseState::Rest,
        Status::Disabled => BaseState::Disabled,
    };
    let focused = matches!(status, Status::Focused { .. });
    let resolved = pro_text_input::resolve_input_style(
        pro_text_input::InputKind::TextEntry,
        VisualState { base, focused },
        &palette,
    );

    Style {
        background: Background::Color(resolved.background),
        border: Border {
            color: resolved.border_color,
            width: resolved.border_width,
            radius: shape::RADIUS_WIDGET.into(),
        },
        icon: resolved.icon,
        placeholder: resolved.placeholder,
        value: resolved.text,
        selection: resolved.selection_background,
    }
}

/// A framework button that stays **allocated** whether or not it can fire.
///
/// [`Button::new`] takes an unconditional press, so the AGENTS §6
/// "always allocate, gate the action" rule is applied here by adding
/// `.disabled()` when the gate is closed rather than by dropping the widget.
/// The row's shape therefore never changes across the idle ⇄ running or
/// empty ⇄ filled transitions, and a closed gate gets the framework's real
/// disabled skin instead of a control that looks live and does nothing.
#[must_use]
fn gated_button(
    variant: ButtonVariant,
    label: &'static str,
    message: Message,
    id: &'static str,
    enabled: bool,
) -> Button<'static, Message> {
    let button = Button::new(
        variant,
        text(label)
            .size(CONTROL_TEXT_SIZE)
            .line_height(CONTROL_RUNG.iced_line_height()),
        message,
        id,
    )
    .compact();
    if enabled { button } else { button.disabled() }
}

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
/// `ClearAgentSession` intent the agent header's Close button carries.
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

/// Renders the agent prompt row from a snapshot.
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
        .id(PROMPT_INPUT_ID)
        .size(CONTROL_TEXT_SIZE)
        .line_height(CONTROL_RUNG.iced_line_height())
        .padding(space::XS)
        .style(|_theme, status| prompt_field_style(status))
        .on_input(|text| Message::Agent(AgentMessage::InputChanged { text }))
        .on_submit(Message::Agent(AgentMessage::Submit {
            text: view_model.command_input.clone(),
        }));

    let send_button = gated_button(
        ButtonVariant::Primary,
        strings::AGENT_SEND_BUTTON,
        Message::Agent(AgentMessage::Submit {
            text: view_model.command_input.clone(),
        }),
        SEND_BUTTON_ID,
        send_enabled(&view_model.command_input, view_model.running),
    );

    let queue_button = gated_button(
        ButtonVariant::Secondary,
        strings::AGENT_QUEUE_BUTTON,
        Message::Agent(AgentMessage::Submit {
            text: view_model.command_input.clone(),
        }),
        QUEUE_BUTTON_ID,
        queue_enabled(&view_model.command_input, view_model.running),
    );

    let cancel_button = gated_button(
        crate::ui_iced::panels::bottom::stop_button_variant(view_model.running),
        strings::STOP_AGENT_BUTTON,
        Message::Command(UserCommand::CancelAgent),
        STOP_BUTTON_ID,
        cancel_enabled(view_model.running),
    );

    let model_label = text(strings::AGENT_MODEL_LABEL)
        .size(CONTROL_TEXT_SIZE)
        .line_height(CONTROL_RUNG.iced_line_height())
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

    let clear_button = gated_button(
        ButtonVariant::Ghost,
        strings::AGENT_SESSION_CLOSE_BUTTON,
        clear_session_message(),
        CLEAR_SESSION_BUTTON_ID,
        true,
    );

    let queue_badge: Element<'_, Message> = match queue_display(&view_model.queued_prompts) {
        Some(label) => text(label)
            .size(CONTROL_TEXT_SIZE)
            .line_height(CONTROL_RUNG.iced_line_height())
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
