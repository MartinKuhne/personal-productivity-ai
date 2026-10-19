//! Iced agent-panel conversation/turn model ([TASK-018], UI-048).
//!
//! This module owns the **view-side conversation model** the iced agent panel
//! renders: the typed turn list, the UI-048 thinking-delimiter gate, and the
//! [`AgentPanelViewModel`] snapshot the widget tree reads. It deliberately
//! reuses — rather than reimplements — the framework-free pieces the legacy UI
//! build already had ([RUST-051]):
//!
//! * [`AgentPanelState`] (`src/app/ui/agent/panel_state.rs:13`) — the
//!   UI-owned widget toggles / command input, re-exported unchanged.
//! * [`AgentTranscript`] / [`TranscriptBlock`]
//!   (`src/app/ui/agent/transcript.rs`) — the `AgentEvent` accumulator,
//!   re-exported unchanged.
//!
//! # Why the turn model reads the flat `content` buffer
//!
//! The legacy UI agent view rendered `AgentTranscript::content` — the flat
//! markdown string — as its single render source
//! (`src/app/ui/panels/center.rs:153-177`), **not** the structured
//! `blocks`. That choice is load-bearing and this port keeps it: the content
//! buffer is where the two out-of-band mutations land —
//! `UserCommand::ApplyTaskToggles` rewrites task checkboxes in place
//! (`src/app/command_executor.rs:568-576`) and the bus-lag truncation marker
//! is appended on `Lagged(n)` (`src/app/orchestrator.rs:438-443`) — and
//! neither touches `blocks`. Rendering the live `content` buffer is therefore
//! the only way to keep task toggles and the truncation notice visible across
//! re-renders.
//!
//! The user turn is recovered from the `**User:** ` marker the orchestrator
//! injects ahead of a continuation prompt
//! (`src/app/orchestrator.rs:267-272`, [`strings::AGENT_USER_TURN_MARKER`]).
//! [`turns_from_content`] splits the buffer at those paragraph-start markers
//! so the view can theme the user turn distinctly while every other span
//! stays assistant markdown.
//!
//! # Known limitation
//!
//! The injected form is `\n\n**User:** {prompt}\n\n`; the user turn is read
//! up to the first blank line after the marker. A prompt that itself
//! contains a blank line would be split early (its tail rendering as
//! assistant markdown). Single newlines inside a prompt are preserved. The
//! the legacy UI build had the same ambiguity — it simply rendered the whole buffer
//! as one blob, so it never distinguished the turns at all.
//!
//! Unit tests live in the sibling `panel_state_tests.rs` sidecar.

use crate::config::AppConfig;
use crate::ui::app::FastMdApp;
use crate::ui::strings;
use crate::ui_iced::panels::top::{CHAT_USE_CASE, ChatModelEntry};

/// The legacy UI build's UI-owned panel state, reused unchanged ([RUST-051]) —
/// `src/app/ui/agent/panel_state.rs:13`.
pub use crate::ui::agent::panel_state::AgentPanelState;

/// The legacy UI build's `AgentEvent` accumulator and its block enum, reused
/// unchanged ([RUST-051]) — `src/app/ui/agent/transcript.rs`.
pub use crate::ui::agent::transcript::{AgentTranscript, TranscriptBlock};

/// Who authored a transcript turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnRole {
    /// The user's prompt.
    User,
    /// Assistant output (prose, tool-call blockquotes, truncation notices).
    Assistant,
}

/// One rendered turn of the agent transcript.
///
/// The view renders a [`Self::User`] turn as themed text and a
/// [`Self::Assistant`] turn through iced's built-in Markdown widget
/// ([`crate::ui_iced::agent::transcript`]).
///
/// The assistant variant carries the **pre-parsed** `markdown::Item` list
/// alongside the raw `markdown` string. iced's `markdown::view` borrows the
/// items for the returned `Element`'s lifetime
/// (`iced_widget-0.14.2/src/markdown.rs:1197-1206`), so a pure view that
/// returns an escaping `Element` must have the items owned by something that
/// outlives it — here, the turn itself, which lives in the
/// [`AgentPanelViewModel`]. The raw `markdown` string is kept for
/// inspection and tests.
#[derive(Clone, Debug)]
pub enum TranscriptTurn {
    /// The user's prompt, rendered as themed text (not Markdown-parsed).
    User {
        /// The prompt text, marker stripped and trimmed.
        text: String,
    },
    /// Assistant output, rendered as Markdown.
    Assistant {
        /// The Markdown source for this span.
        markdown: String,
        /// The parsed Markdown items the view renders (see the type docs).
        items: Vec<iced::widget::markdown::Item>,
    },
}

impl TranscriptTurn {
    /// Builds an assistant turn, parsing `markdown` into its items once.
    #[must_use]
    pub fn assistant(markdown: &str) -> Self {
        Self::Assistant {
            markdown: markdown.to_string(),
            items: iced::widget::markdown::parse(markdown).collect(),
        }
    }

    /// The author of this turn.
    #[must_use]
    pub fn role(&self) -> TurnRole {
        match self {
            Self::User { .. } => TurnRole::User,
            Self::Assistant { .. } => TurnRole::Assistant,
        }
    }

    /// Whether this is a user turn.
    #[must_use]
    pub fn is_user(&self) -> bool {
        matches!(self, Self::User { .. })
    }

    /// The turn's text payload (the prompt or the Markdown source).
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Self::User { text } | Self::Assistant { markdown: text, .. } => text,
        }
    }
}

/// Finds the next user-turn marker that sits at a paragraph start.
///
/// A marker counts only at the very start of `text` or when the preceding
/// byte is a newline, matching the injected `\n\n**User:** ` form
/// ([`strings::AGENT_USER_TURN_MARKER`]) and rejecting mid-line occurrences
/// such as `the **User:** field` inside a paragraph.
///
/// Purity: pure function of `text`.
#[must_use]
fn find_user_marker(text: &str) -> Option<usize> {
    let marker = strings::AGENT_USER_TURN_MARKER;
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(rel) = text[from..].find(marker) {
        let idx = from + rel;
        if idx == 0 || bytes[idx - 1] == b'\n' {
            return Some(idx);
        }
        from = idx + 1;
    }
    None
}

/// Splits `text` at the first blank line (`\n\n`), returning the part before
/// it and the remainder after it. With no blank line the whole string is the
/// head and the tail is empty.
///
/// Purity: pure function of `text`.
#[must_use]
fn split_at_blank_line(text: &str) -> (&str, &str) {
    match text.find("\n\n") {
        Some(idx) => (&text[..idx], &text[idx + 2..]),
        None => (text, ""),
    }
}

/// Splits a single content span into an optional user turn plus the
/// remaining assistant markdown.
///
/// Returns `None` when `text` does not begin (at a paragraph start) with the
/// user-turn marker — the whole span is then plain assistant content. When it
/// does, the tuple is `(user_text, assistant_rest)` where `user_text` runs
/// from after the marker to the first blank line and `assistant_rest` is
/// everything after that blank line.
///
/// Purity: pure function of `text`.
#[must_use]
pub fn split_user_turn(text: &str) -> Option<(String, String)> {
    let idx = find_user_marker(text)?;
    let after = &text[idx + strings::AGENT_USER_TURN_MARKER.len()..];
    let (user_text, rest) = split_at_blank_line(after);
    Some((user_text.trim().to_string(), rest.to_string()))
}

/// Maps the flat transcript `content` buffer into an ordered turn list.
///
/// Every user-turn marker becomes a [`TranscriptTurn::User`]; the text
/// between markers (and any leading/trailing span) becomes
/// [`TranscriptTurn::Assistant`]. Blank spans are dropped so the view never
/// renders an empty turn. The bus-lag truncation marker and tool-call
/// blockquotes are ordinary text in the buffer and ride along inside the
/// assistant spans unchanged.
///
/// Purity: pure function of `content`.
#[must_use]
pub fn turns_from_content(content: &str) -> Vec<TranscriptTurn> {
    let mut turns = Vec::new();
    let mut rest = content;

    while let Some(idx) = find_user_marker(rest) {
        let before = &rest[..idx];
        if !before.trim().is_empty() {
            turns.push(TranscriptTurn::assistant(before));
        }
        let after = &rest[idx + strings::AGENT_USER_TURN_MARKER.len()..];
        let (user_text, remainder) = split_at_blank_line(after);
        turns.push(TranscriptTurn::User {
            text: user_text.trim().to_string(),
        });
        rest = remainder;
    }

    if !rest.trim().is_empty() {
        turns.push(TranscriptTurn::assistant(rest));
    }

    turns
}

/// UI-048 gate: whether the "Thinking Process" block has anything to show.
///
/// The `🤔…🤔` delimiter is resolved upstream by
/// `AgentTranscript::apply_event` / `split_thinking_and_content`
/// (`src/app/ui/agent/transcript.rs`) and the agent's response formatter
/// (AGENT-022); the panel only decides whether the extracted buffer is
/// non-empty, exactly as the legacy UI's `if !thinking.is_empty()` did
/// (`src/app/ui/panels/center.rs:142`). Kept identical to
/// [`crate::ui_iced::panels::center::thinking_section_visible`] — a test
/// asserts the two agree so the gate cannot drift while both render paths
/// exist ([RUST-041]).
///
/// Purity: pure function of `thinking`.
#[must_use]
pub fn thinking_section_visible(thinking: &str) -> bool {
    !thinking.is_empty()
}

/// Whether the transcript has any visible content (turns or thinking).
///
/// Purity: pure.
#[must_use]
pub fn transcript_is_empty(turns: &[TranscriptTurn], thinking: &str) -> bool {
    turns.is_empty() && !thinking_section_visible(thinking)
}

/// Whether the panel should show the "Agent is thinking…" placeholder: the
/// agent is running but no assistant output has arrived since the last user
/// turn (the waiting-for-first-chunk state).
///
/// `last_is_user` is `true` when the final turn is a user turn (or there are
/// no turns yet). While that holds and the agent is running, the assistant
/// message is still partial/absent and the placeholder stands in.
///
/// Purity: pure.
#[must_use]
pub fn waiting_for_first_response(turns: &[TranscriptTurn], running: bool) -> bool {
    if !running {
        return false;
    }
    match turns.last() {
        None => true,
        Some(turn) => turn.is_user(),
    }
}

/// Everything the iced agent panel needs to render, snapshotted off
/// `FastMdApp`.
///
/// Same contract as [`crate::ui_iced::logs::BackgroundLogsViewModel`] and
/// the pane view models: the widget tree reads only this struct, which keeps
/// the panel unit-testable without a live iced runtime, a bus or a running
/// agent. The [TASK-022]/[TASK-023] reducer snapshots the agent session and
/// hands the snapshot down ([FMD-065], [FMD-069]).
///
/// `Clone + Debug` only: the assistant turns own `iced::widget::markdown::Item`
/// values, which derive neither `PartialEq` nor `Eq`, so the snapshot cannot
/// either — the sidecar tests assert field by field instead (the same shape
/// [`crate::ui_iced::logs::BackgroundLogsViewModel`] takes for its bus-typed
/// entries).
#[derive(Clone, Debug)]
pub struct AgentPanelViewModel {
    /// Whether the agent session owns the pane (`AgentPanelState::show_results`).
    pub visible: bool,
    /// The ordered conversation turns, derived from the transcript's flat
    /// `content` buffer by [`turns_from_content`].
    pub turns: Vec<TranscriptTurn>,
    /// The extracted thinking buffer (UI-048), gated by
    /// [`thinking_section_visible`].
    pub thinking: String,
    /// The "Thinking Process" disclosure state (the legacy UI kept this in
    /// `collapsing` widget memory; here it is explicit, driven by
    /// [`crate::ui_iced::message::AgentMessage::ThinkingToggled`]).
    pub thinking_expanded: bool,
    /// The agent status line text (`AgentState::status`).
    pub status: String,
    /// Whether the agent is currently running (`AgentState::running`).
    pub running: bool,
    /// Prompts parked behind the current turn (`AgentState::pending_prompts`).
    pub queued_prompts: Vec<String>,
    /// The chat-model selector options (UI-064), filtered to the chat use
    /// case exactly as the top bar does.
    pub chat_models: Vec<ChatModelEntry>,
    /// The currently active chat-model key (`AppConfig::current_chat_model_key`).
    pub current_chat_model: Option<String>,
    /// The command input's current text (`AgentPanelState::command_input`).
    pub command_input: String,
    /// Follow-the-stream preference for the transcript scroll. the legacy UI hardcoded
    /// `stick_to_bottom(true)` on the agent scroll area
    /// (`src/app/ui/panels/center.rs:140`); the field keeps the shared
    /// [`crate::ui_iced::logs::should_follow`] rule usable and lets a future
    /// user toggle exist without touching the view.
    pub auto_scroll: bool,
}

impl AgentPanelViewModel {
    /// Snapshots the agent panel's inputs off `app`, reading the same
    /// accessors the legacy UI panel used: `agent_panel_state.show_results` /
    /// `command_input`, `agent_transcript.content` / `.thinking`,
    /// `agent().state().status` / `.running` / `.pending_prompts`, and the
    /// chat-model list off `config()`.
    pub fn from_app(app: &FastMdApp) -> Self {
        let panel = &app.orchestrator.agent_panel_state;
        let transcript = &app.orchestrator.agent_transcript;
        let state = app.agent().state();

        Self {
            visible: panel.show_results,
            turns: turns_from_content(&transcript.content),
            thinking: transcript.thinking.clone(),
            thinking_expanded: true,
            status: state.status.clone(),
            running: state.running,
            queued_prompts: state.pending_prompts.clone(),
            chat_models: chat_model_options(app.config()),
            current_chat_model: app.config().current_chat_model_key(),
            command_input: panel.command_input.clone(),
            auto_scroll: true,
        }
    }

    /// Whether the UI-048 thinking block renders for this snapshot.
    #[must_use]
    pub fn show_thinking(&self) -> bool {
        thinking_section_visible(&self.thinking)
    }

    /// Whether the transcript has nothing to show.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        transcript_is_empty(&self.turns, &self.thinking)
    }

    /// Whether the "Agent is thinking…" placeholder should stand in for the
    /// not-yet-arrived assistant message.
    #[must_use]
    pub fn waiting_for_response(&self) -> bool {
        waiting_for_first_response(&self.turns, self.running)
    }

    /// Returns a copy carrying `thinking_expanded` — how the reducer
    /// combines its own disclosure state with a freshly snapshotted
    /// transcript.
    #[must_use]
    pub fn with_thinking_expanded(mut self, expanded: bool) -> Self {
        self.thinking_expanded = expanded;
        self
    }
}

/// Builds the chat-model selector options off `config`, mirroring the top
/// bar's filter ([`crate::ui_iced::panels::top`]): chat-use-case models
/// first, falling back to every configured model when none declare the chat
/// use case, sorted by name.
///
/// Purity: pure function of `config`.
#[must_use]
pub fn chat_model_options(config: &AppConfig) -> Vec<ChatModelEntry> {
    let to_entry = |name: &str, cfg: &crate::config::LlmConfig| ChatModelEntry {
        name: name.to_string(),
        cost: cfg.get_cost(),
    };

    let mut models: Vec<ChatModelEntry> = config
        .models
        .iter()
        .filter(|(_, cfg)| cfg.has_use_case(CHAT_USE_CASE))
        .map(|(name, cfg)| to_entry(name, cfg))
        .collect();
    if models.is_empty() {
        models = config
            .models
            .iter()
            .map(|(name, cfg)| to_entry(name, cfg))
            .collect();
    }
    models.sort_by(|a, b| a.name.cmp(&b.name));
    models
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `panel_state_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "panel_state_tests.rs"]
mod tests;
