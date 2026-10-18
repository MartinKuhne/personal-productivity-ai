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
//! Every framework-free helper the egui band owned stays where it is and is
//! re-exported rather than copied: [`compute_prompt_prefix`],
//! [`compute_file_context`], [`format_models_list`] and
//! [`parse_command_intent`] have no egui types in their signatures, so both
//! UIs — and the reducer — run the same code. Only what egui bound to
//! `egui::Ui`/`egui::Context` is re-typed here.
//!
//! The one piece of *behaviour* that egui kept inline inside the widget
//! closure — "running → queue the prompt, idle → parse the intent, then clear
//! the input" — is extracted into [`submit_commands`] so it is testable
//! without a runtime. The reducer calls it and publishes the returned
//! commands in order.
//!
//! # Not ported, with reasons
//!
//! * **The IME Enter workaround** (`ui/panels/bottom.rs:13-40`). egui needed
//!   `is_enter_pressed` because `egui-winit` swallows the raw `Key::Enter`
//!   when an IME claims it (`VK_PROCESSKEY`) and only an
//!   `Event::Ime(Commit("\n"))` arrives. iced 0.14 does not surface IME
//!   commits to the application at all — `text_input` publishes its
//!   `on_submit` message from its own key handling
//!   (`iced_widget-0.14.2/src/text_input.rs:1050-1056`), so there is no
//!   second delivery path to reconcile and no equivalent helper to write.
//! * **Shift+Enter = newline.** The egui band submitted on Enter *unless*
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
//!   is `egui_kittest` plumbing. CON-003 rules it out; the iced equivalent is
//!   the `iced_test::simulator` tier of [TASK-025]/[TASK-026].
//! * **Hand-rolled right-alignment.** egui computed a 100 pt rect by hand to
//!   park the Stop button at the far right (`ui/panels/bottom.rs:194-209`).
//!   iced's `Row` lays it out, so neither the magic number nor the
//!   `available_rect_before_wrap` bookkeeping is needed.
//!
//! Unit tests live in the sibling `bottom_tests.rs` sidecar.

#[cfg(test)]
#[path = "bottom_tests.rs"]
mod tests;

use iced::widget::{Column, Row, button, container, text, text_input};
use iced::{Alignment, Element, Font, Length};

use crate::bus::events::user_command::UserCommand;
use crate::ui::app::FastMdApp;
use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::message::{AgentMessage, Message};
use crate::ui_iced::theme::to_iced_color;

/// The egui band's framework-free helpers, reused rather than duplicated
/// ([RUST-051]) — `src/app/ui/panels/bottom.rs:44-134`.
pub use crate::ui::panels::bottom::{
    clear_command_input_command, compute_file_context, compute_prompt_prefix, format_models_list,
    parse_command_intent,
};

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
    /// the egui band used: `selection().agent_context(..)`,
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
/// Extracted from the egui band's inline submit branch
/// (`src/app/ui/panels/bottom.rs:259-294`), which ran the same two paths
/// inside the widget closure:
///
/// * **Agent running** — the prompt is queued behind the current turn
///   ([`UserCommand::QueueAgentPrompt`]) and the input is cleared. A
///   blank/whitespace prompt queues nothing, so no stray clear is published
///   either (egui's `if !prompt.is_empty()` guard).
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
        .color(to_iced_color(palette.text_strong));

    let (file_name, file_size) = view_model
        .file_context
        .clone()
        .unwrap_or_else(|| (String::new(), FILE_CONTEXT_FALLBACK_SIZE));

    let file_label = text(file_name)
        .font(Font::MONOSPACE)
        .size(file_size)
        .color(to_iced_color(palette.text_muted));

    // egui stacked the prefix above the file name
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
            .color(to_iced_color(palette.error)),
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
