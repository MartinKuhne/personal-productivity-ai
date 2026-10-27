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

use iced::widget::{Column, Row, container, text};
use iced::{Alignment, Element, Length};

use hoarfrost::token::space;
use hoarfrost::token::typography::{self, TypeRung};
use hoarfrost::widget::button::ButtonVariant;
use hoarfrost::widget::button_widget::Button;

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

/// The rung the agent panel's header line sits on — `type.title_sm`
/// (`UIF-TOK-004`).
///
/// Held as a rung rather than a bare size because the header needs the point
/// size *and* the authoritative 24 px line box, and binding both to one
/// source is what stops a size retune leaving a mismatched line height behind.
pub const HEADER_RUNG: TypeRung = typography::TITLE_SM;

/// Logical point size of the agent-session header — `type.title_sm`
/// (`UIF-TOK-004`).
///
/// **Movement: 18 → 16 (−2).** `18` is not a rung on the type scale —
/// `TITLE_SM` is 16 and `TITLE_MD` is 20, so nothing legal produced that
/// number. §2.4 assigns `type.title_sm` to panel and overlay headers, which
/// is what this is; `TITLE_MD` is the *document-H2* rung and this band is
/// chrome, not document prose. The same off-scale `18` INT-7c fixed on
/// [`AGENT_HEADER_SIZE`](super::panels::center::AGENT_HEADER_SIZE).
pub const HEADER_TEXT_SIZE: f32 = HEADER_RUNG.size;

/// The rung the status line and prompt controls share — `type.body`
/// (`UIF-TOK-004`). Value unchanged at `14`.
pub const BODY_RUNG: TypeRung = typography::BODY;

/// Logical point size of the status line — `type.body` (`UIF-TOK-004`).
/// Value unchanged at `14`.
pub const STATUS_TEXT_SIZE: f32 = BODY_RUNG.size;

/// Spacing between the panel's bands — `space.sm` (`UIF-TOK-001`).
///
/// **Movement: 6 → 8 (+2).** `6` is not on the base-4 grid and sits exactly
/// midway between `XS` (4) and `SM` (8). The tie is broken on *role*, as
/// INT-7a broke it for `BAND_PADDING` and INT-7c for
/// [`VIEWER_SPACING`](super::panels::center::VIEWER_SPACING): `SM` is the
/// standard **sibling gap**
/// and `XS` the compact **inner padding**, and these are sibling bands.
pub const PANEL_SPACING: f32 = space::SM;

/// Padding around the panel — `space.sm` (`UIF-TOK-001`). Value unchanged
/// at `8`.
pub const PANEL_PADDING: f32 = space::SM;

/// Stable id for the panel header's Close button ([AGENTS §5]).
///
/// Distinct from the center panel's own agent-band close control and from the
/// prompt row's Clear-session control: one id, one live widget.
pub const CLOSE_BUTTON_ID: &str = "agent_panel_close_button";

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

    let close_button = Button::new(
        ButtonVariant::Ghost,
        text(strings::AGENT_SESSION_CLOSE_BUTTON)
            .size(STATUS_TEXT_SIZE)
            .line_height(BODY_RUNG.iced_line_height()),
        close_message(),
        CLOSE_BUTTON_ID,
    )
    .compact();

    let header = Row::with_children(vec![
        text(strings::AGENT_SESSION_HEADER)
            .size(HEADER_TEXT_SIZE)
            .line_height(HEADER_RUNG.iced_line_height())
            .font(HEADER_RUNG.font(typography::Family::Sans, HEADER_RUNG.weight))
            .color(palette.accent_text)
            .into(),
        container(close_button)
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
    .line_height(BODY_RUNG.iced_line_height())
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
