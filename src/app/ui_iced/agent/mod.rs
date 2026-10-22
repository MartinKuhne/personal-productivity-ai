//! The agent panel — the streaming conversation view, prompt row and
//! chat-model selector (UI-048).
//!
//! The module is split by concern ([RUST-051]): the framework-free pieces are
//! reused as they are, and only what the render layer needs is re-typed.
//!
//! * [`panel_state`] — the conversation/turn model, the UI-048
//!   thinking-delimiter gate and the [`AgentPanelViewModel`] snapshot.
//!   Re-exports [`AgentPanelState`] and [`AgentTranscript`] unchanged.
//! * [`transcript`] — the scrolling conversation surface: gated thinking
//!   disclosure, user turns as themed text, assistant turns through iced's
//!   built-in Markdown widget, the "Agent is thinking…" placeholder, and
//!   the reused follow-the-stream report.
//! * [`prompt_input`] — the prompt box with send / queue / cancel controls
//!   and the chat-model `pick_list`, riding the shared submit solver and the
//!   command channel.
//!
//! # What is NOT here
//!
//! The **conversation logger** (`src/app/ui/agent/conversation_logger.rs`)
//! is deliberately kept out of the view. It is an `AgentEventObserver` that
//! writes prompt/response pairs and mutating-tool records to timestamped
//! markdown files under the system library's `Conversations` folder
//! (VFS-110..114) — pure persistence/background work with no render
//! surface. It stays reducer/background plumbing (the `update` path or a
//! background observer), not a view concern. Its pure formatting helpers
//! (`format_turn_entry`, `format_prompt_heading`, `format_write_tool_calls`,
//! …) are not needed by the view, so nothing is re-exported from them here.
//!
//! # Composition
//!
//! [`agent_panel`] assembles the header, status line, the scrolling
//! [`transcript`] body and the [`prompt_input`] row into one
//! `Element<Message>`. The panel is a pure view: it reads only the
//! [`AgentPanelViewModel`] snapshot and emits messages; the reducer owns
//! the bus, the agent session and the follow-the-stream scroll operation.
//!
//! Unit tests live in the sibling `mod_tests.rs` sidecar.

pub mod panel_state;
pub mod prompt_input;
pub mod transcript;

use iced::widget::{Column, Row, button, container, text};
use iced::{Alignment, Element, Length};

use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::message::Message;

pub use panel_state::{
    AgentPanelState, AgentPanelViewModel, AgentTranscript, TranscriptBlock, TranscriptTurn,
    TurnRole, chat_model_options, split_user_turn, thinking_section_visible, transcript_is_empty,
    turns_from_content, waiting_for_first_response,
};
pub use prompt_input::{
    agent_prompt_row, cancel_enabled, clear_session_message, current_model_option, queue_display,
    queue_enabled, select_model_message, send_enabled,
};
pub use transcript::{
    AGENT_TRANSCRIPT_SCROLLABLE_ID, agent_transcript_view, link_activated_message,
    markdown_settings, markdown_style, render_markdown_items, render_turn, scroll_reported_message,
    thinking_block, transcript_body,
};

/// Logical point size of the agent-session header.
pub const HEADER_TEXT_SIZE: f32 = 18.0;

/// Logical point size of the status line.
pub const STATUS_TEXT_SIZE: f32 = 14.0;

/// Spacing between the panel's bands.
pub const PANEL_SPACING: f32 = 6.0;

/// Padding around the panel.
pub const PANEL_PADDING: f32 = 8.0;

/// The message the panel header's Close button produces: the
/// `ClearAgentSession` intent.
///
/// Purity: pure.
#[must_use]
pub fn close_message() -> Message {
    prompt_input::clear_session_message()
}

/// Renders the full agent panel from a snapshot.
///
/// Layout:
///
/// ```text
/// 🤖 FastMD Agent Session                         [Close]
/// Status: <status>
/// ┌───────────────────────────────────────────────┐
/// │ [▾ Thinking Process]                          │  (gated, disclosable)
/// │ <user turn>  <assistant markdown>  …          │  (scrollable, follows)
/// └───────────────────────────────────────────────┘
/// [ prompt ______________ ] [Send][Queue][Stop]  Model:[▾] [Queued: n]
/// ```
///
/// The header, status band and prompt row are always allocated; the
/// transcript body fills the remaining height and scrolls.
#[must_use]
pub fn agent_panel(view_model: &AgentPanelViewModel) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();

    let header = Row::with_children(vec![
        text(strings::AGENT_SESSION_HEADER)
            .size(HEADER_TEXT_SIZE)
            .color(palette.accent_text)
            .into(),
        container(
            button(text(strings::AGENT_SESSION_CLOSE_BUTTON).size(STATUS_TEXT_SIZE))
                .on_press(close_message()),
        )
        .width(Length::Fill)
        .align_right(Length::Shrink)
        .into(),
    ])
    .align_y(Alignment::Center);

    let status = text(format!(
        "{}{}",
        strings::AGENT_STATUS_PREFIX,
        view_model.status
    ))
    .size(STATUS_TEXT_SIZE)
    .color(palette.text_strong);

    let body = agent_transcript_view(view_model);
    let prompt = agent_prompt_row(view_model);

    Column::with_children(vec![header.into(), status.into(), body, prompt])
        .spacing(PANEL_SPACING)
        .padding(PANEL_PADDING)
        .into()
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `mod_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
