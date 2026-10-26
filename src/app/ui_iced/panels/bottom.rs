//! Bottom command band — UI-001's "Bottom Pane (command prompt)".
//!
//! Like [`top`](super::top) and [`left`](super::left), the band is a **pure
//! view**: it renders a [`BottomPanelViewModel`] snapshot into an
//! [`iced::Element`] of [`Message`] and never touches the `UserCommand` bus,
//! the agent session or `FastMdApp` itself — the reducer publishes what the
//! band emits, so every intent is self-contained and replayable.
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
//! The band's framework-free helpers — [`compute_prompt_prefix`],
//! [`compute_file_context`], [`format_models_list`],
//! [`parse_command_intent`] and [`clear_command_input_command`] — live in
//! this module. Every one is framework-free (pure `Path`/`String`/
//! `UserCommand` signatures, no framework type anywhere in the body), so
//! the view, the reducer and the sidecar tests all run the same rules.
//!
//! The submit behaviour — "running → queue the prompt, idle → parse the
//! intent, then clear the input" — is extracted into [`submit_commands`] so
//! it is testable without a runtime. The reducer calls it and publishes the
//! returned commands in order.
//!
//! # Not implemented, with reasons
//!
//! * **IME Enter workaround.** iced 0.14 does not surface IME commits to
//!   the application at all — `text_input` publishes its `on_submit`
//!   message from its own key handling
//!   (`iced_widget-0.14.2/src/text_input.rs:1050-1056`) — so there is no
//!   second delivery path to reconcile and no equivalent helper to write.
//! * **Shift+Enter = newline.** iced's `text_input` publishes `on_submit`
//!   on Enter with **no modifier check** (`text_input.rs:1050-1056`), and
//!   the message is a fixed `Message`, so the view cannot branch on Shift.
//!   A faithful multi-line field would need `text_editor`, whose
//!   `Action::Edit(Edit::Enter)` likewise carries no modifiers — the gap is
//!   framework-wide. If multi-line prompts matter, the reducer must own a
//!   `text_editor` buffer and treat Enter as submit unconditionally, or the
//!   hint text must change.
//! * **No test-only "send" click sink.** The band exposes no synthetic click
//!   target for harnesses; interaction tests drive the real `on_submit`
//!   message through the `iced_test::simulator` tier.
//! * **Hand-rolled right-alignment.** iced's `Row` lays the Stop button
//!   out, so neither a magic number nor `available_rect_before_wrap`
//!   bookkeeping is needed.
//! * **Controlled command field.** The band's command string remains owned by
//!   `agent_panel_state.command_input`; [`BottomPanelViewModel`] derives a
//!   non-clearable [`TextInputModel`]
//!   snapshot for [`TextInput`]. The
//!   field routes typing and Escape restoration as
//!   [`AgentMessage::InputChanged`], Enter's explicit
//!   [`Committed`](iced_pro::widget::text_input_controller::TextInputEmission::Committed)
//!   tier as [`AgentMessage::Submit`], and blur's
//!   [`FocusCommitted`](iced_pro::widget::text_input_controller::TextInputEmission::FocusCommitted)
//!   tier as `InputChanged` only. Its paint comes from the framework widget
//!   and [`Palette::pro`], not a hand-built iced style.
//!
//! Unit tests live in the sibling `bottom_tests.rs` sidecar.

#[cfg(test)]
#[path = "bottom_tests.rs"]
mod tests;

use iced::widget::{Column, Row, container, text};
use iced::{Alignment, Color, Element, Font, Length};

use iced_pro::token::space;
use iced_pro::token::typography;
use iced_pro::widget::button::ButtonVariant;
use iced_pro::widget::button_widget::Button;
use iced_pro::widget::text_input::{InputKind, TextInput};
use iced_pro::widget::text_input_controller::TextInputEmission;
use iced_pro::widget::text_input_model::TextInputModel;

use crate::bus::events::user_command::UserCommand;
use crate::ui::app::FastMdApp;
use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::message::{AgentMessage, Message};

// Framework-free helpers owned by this module. Each is framework-free: no
// framework type appears in any signature or body. See the individual items
// below.

/// Parses the user prompt to determine the intended command.
/// Computes the prompt prefix based on the selected directory and available content libraries.
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

    let base_size = FILE_CONTEXT_BASE_SIZE;
    let min_size = FILE_CONTEXT_MIN_SIZE;

    let font_size = if file_len > dir_len && dir_len > 0 {
        let ratio = dir_len as f32 / file_len as f32;
        (base_size * ratio).max(min_size)
    } else {
        base_size
    };

    Some((file_name, font_size))
}

/// Renders the chat-model catalogue the `/models` command reports.
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
#[must_use]
pub fn clear_command_input_command() -> UserCommand {
    UserCommand::ClearCommandInput
}

/// Logical point size of the prompt prefix and the command input —
/// `type.body` (`UIF-TOK-004`). Value unchanged at `14`.
pub const PROMPT_TEXT_SIZE: f32 = typography::BODY.size;

/// Logical point size of the Stop button label — `type.body`. Value
/// unchanged at `14`.
pub const STOP_BUTTON_TEXT_SIZE: f32 = typography::BODY.size;

/// Spacing between the band's items — `space.sm` (`UIF-TOK-001`), the
/// standard sibling-control gap. Value unchanged at `8`.
pub const BAND_SPACING: f32 = space::SM;

/// Vertical spacing between the prompt prefix and the file-context label —
/// `space.2xs` (`UIF-TOK-001`), the dense inline micro-gap rung. Value
/// unchanged at `2`.
pub const CONTEXT_SPACING: f32 = space::XXS;

/// Inner padding of the band — `space.xs` (`UIF-TOK-001`).
///
/// **Movement: 6 → 4.** `6` is not on the base-4 spacing grid, and
/// `UIF-TOK-001` states that a spacing value which is not one of the
/// rungs must not appear in layout code. The old value sat exactly
/// midway between `XS` (4) and `SM` (8), so the tie was broken on
/// *role* rather than distance: `SM` is documented as a sibling-control
/// **gap**, while `XS` is the compact **inner padding** rung — which is
/// what a band's inset is. The band tightens by 2 pt per side.
pub const BAND_PADDING: f32 = space::XS;

/// Starting size of the file-context label — `type.caption`
/// (`UIF-TOK-004`).
///
/// The label sits one rung below the prompt's `BODY` on purpose: it is
/// the de-emphasised half of the prefix/file pair. Value unchanged at
/// `12`.
pub const FILE_CONTEXT_BASE_SIZE: f32 = typography::CAPTION.size;

/// Floor the file-context label may shrink to — `MIN_SIZE`
/// (`UIF-TOK-004`), the interface-wide 12px legibility rule.
///
/// **Movement: 8 → 12, and the de-emphasis gradient dies with it.**
/// The helper used `min_size = 8`, a size the type scale forbids. Raising
/// the floor to the rule's own value means it now equals
/// [`FILE_CONTEXT_BASE_SIZE`], so `base * ratio` can never land below it
/// and the ratio branch resolves to the base on every input. The label
/// still reads as de-emphasised — `CAPTION` (12) against the prompt's
/// `BODY` (14) — but it no longer *modulates* with the directory/file
/// length ratio.
///
/// Restoring a gradient is a behaviour decision, not a token alignment:
/// it needs the base rung raised above the floor (e.g. base `BODY`, floor
/// `CAPTION`), which would make the label as large as the prompt in the
/// common case and undo the de-emphasis from the other direction. Left
/// as a stated follow-on rather than decided here.
pub const FILE_CONTEXT_MIN_SIZE: f32 = typography::MIN_SIZE;

/// Fallback size for the file-context label when the reused
/// [`compute_file_context`] helper produced none — `MIN_SIZE`
/// (`UIF-TOK-004`). Value unchanged at `12`.
pub const FILE_CONTEXT_FALLBACK_SIZE: f32 = typography::MIN_SIZE;

/// Stable id for the command input ([AGENTS §5]).
pub const COMMAND_INPUT_ID: &str = "bottom_command_input";

/// Stable id for the Stop button ([AGENTS §5]).
pub const STOP_BUTTON_ID: &str = "bottom_stop_button";

/// What the bottom band needs to render, snapshotted off `FastMdApp`.
///
/// Mirrors [`super::top::TopBarViewModel`]: the widget tree reads only this
/// struct, so the band is unit-testable without a live iced runtime and stays
/// decoupled from where each field comes from.
#[derive(Clone, Debug, PartialEq)]
pub struct BottomPanelViewModel {
    /// The command input's current text and control settings, snapshotted from
    /// `AgentPanelState::command_input`.
    pub command_input: TextInputModel,
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
    /// Snapshots the band's inputs off `app`:
    /// `selection().agent_context(..)`, `selection().prompt_dir(..)`,
    /// `content_libraries()`, `agent().state().running` and
    /// `orchestrator.agent_panel_state.command_input`.
    pub fn from_app(app: &FastMdApp) -> Self {
        let tabs = &app.orchestrator.tabs.tabs;
        let (active_file, _, _) = app.selection().agent_context(tabs);
        let command_input = app.orchestrator.agent_panel_state.command_input.clone();

        let prompt_prefix = compute_prompt_prefix(
            app.selection().prompt_dir(tabs).as_deref(),
            app.content_libraries(),
        );

        Self {
            command_input: TextInputModel::new(command_input)
                .with_placeholder(strings::COMMAND_INPUT_HINT)
                .with_clearable(false),
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
        submit_commands(self.agent_running, &self.command_input.value)
    }
}

/// The commands a submit produces, in publish order.
///
/// * **Agent running** — the prompt is queued behind the current turn
///   ([`UserCommand::QueueAgentPrompt`]) and the input is cleared. A
///   blank/whitespace prompt queues nothing, so no stray clear is published
///   either.
/// * **Agent idle** — [`parse_command_intent`] decides the command
///   (`/models` → `ShowModels`, `/model …` → the deprecation notice,
///   anything else → `RunAgent`), followed by
///   [`clear_command_input_command`]. A blank prompt parses to `None`, so
///   nothing is published at all.
///
/// The clear command travels with the intent so the reducer's result is
/// replayable from the returned vector alone.
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
/// A disabled control that still paints in its enabled look would present a
/// red "Stop" over nothing running (the button-visibility defect UI-001
/// reports). The label therefore drops to the `text_muted` token while idle
/// and returns to `error` the moment a turn is cancellable — same palette
/// rule as
/// [`crate::ui_iced::panels::top::indexing_status_color`].
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

/// Framework button variant the Stop control wears.
///
/// `Danger` while a turn is actually cancellable; `Secondary` when idle,
/// so the band never paints a red CTA over nothing to stop. Paired with
/// [`stop_button_color`], whose running value
/// ([`Palette::error`](Palette::error) = `status_danger_text`) is exactly
/// the `Danger` rest cell's resolved foreground — the label obeys the
/// framework's contrast lock while running, and drops below `Secondary`'s
/// lock to `text_muted` when idle so an inert control reads inert.
///
/// Purity: pure function of `running`.
#[must_use]
pub fn stop_button_variant(running: bool) -> ButtonVariant {
    if running {
        ButtonVariant::Danger
    } else {
        ButtonVariant::Secondary
    }
}

/// Renders the bottom command band from a view-model snapshot.
///
/// Emitted messages:
/// * typing → [`Message::Agent`] [`AgentMessage::InputChanged`]
/// * Enter's controlled `Committed` tier → [`Message::Agent`]
///   [`AgentMessage::Submit`] with the submitted value (the reducer runs
///   [`submit_commands`] on it)
/// * blur's `FocusCommitted` tier → [`Message::Agent`]
///   [`AgentMessage::InputChanged`] with no submit
/// * Escape's cancelled edit → [`Message::Agent`]
///   [`AgentMessage::InputChanged`] with the restored pre-edit value
/// * Stop while running → [`Message::Command`] [`UserCommand::CancelAgent`]
///
/// Every item is **always allocated**: with no file context the label is an
/// empty string rather than a missing widget, and the Stop button is always
/// present but inert unless the agent is running
/// (`Button::disabled`). The widget-tree shape therefore does not change
/// across the idle ⇄ running or no-file ⇄ file transitions (AGENTS §6).
///
/// Both interactive items sit on the same control rung: the field is padded
/// to `CONTROL_SM` (`BODY` line height 20 + 2 × `space.xs`) and the Stop
/// button takes `.compact()`, so the row satisfies §1's Row Density
/// Homogeneity rule rather than mixing 28 and 32 in one band.
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

    // The prompt prefix sits above the file name; the Column keeps that shape.
    let context_group = Column::with_children(vec![prompt_prefix.into(), file_label.into()])
        .spacing(CONTEXT_SPACING);

    let command_field = TextInput::controlled(
        InputKind::TextEntry,
        view_model.command_input.controlled(),
        COMMAND_INPUT_ID,
        command_field_emit,
    )
    .width(Length::Fill)
    .compact()
    .palette(Palette::pro());

    let stop_label = text(strings::STOP_AGENT_BUTTON)
        .size(STOP_BUTTON_TEXT_SIZE)
        .line_height(typography::BODY.iced_line_height())
        .color(stop_button_color(view_model.agent_running));

    let stop_button = Button::new(
        stop_button_variant(view_model.agent_running),
        stop_label,
        Message::Command(UserCommand::CancelAgent),
        STOP_BUTTON_ID,
    )
    .palette(Palette::pro())
    .compact();

    let stop_button: Element<'_, Message> = if stop_enabled(view_model.agent_running) {
        stop_button.into()
    } else {
        stop_button.disabled().into()
    };

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

/// Routes the command field's framework tiers onto the existing agent
/// message channel: Enter submits, while blur and Escape only synchronize the
/// command text.
fn command_field_emit(emission: TextInputEmission) -> Option<Message> {
    match emission {
        TextInputEmission::Changed(live) => Some(Message::Agent(AgentMessage::InputChanged {
            text: live.value().to_string(),
        })),
        TextInputEmission::Committed(commit) => Some(Message::Agent(AgentMessage::Submit {
            text: commit.value().to_string(),
        })),
        TextInputEmission::FocusCommitted(commit) => {
            Some(Message::Agent(AgentMessage::InputChanged {
                text: commit.value().to_string(),
            }))
        }
        TextInputEmission::Cancelled(cancel) => Some(Message::Agent(AgentMessage::InputChanged {
            text: cancel.restored().to_string(),
        })),
        TextInputEmission::FocusChanged(_) | TextInputEmission::Cleared(_) => None,
    }
}
