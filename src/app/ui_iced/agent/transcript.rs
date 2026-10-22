//! The iced agent transcript view — the streaming conversation surface
//! (UI-048).
//!
//! Renders an [`AgentPanelViewModel`] snapshot into a scrollable column:
//! the gated "Thinking Process" disclosure, the ordered turn list (user
//! turns as themed text, assistant turns through iced's **built-in**
//! Markdown widget), and the "Agent is thinking…" placeholder while the
//! first assistant chunk is still in flight.
//!
//! # Built-in Markdown widget (verified iced 0.14.2 API)
//!
//! The transcript renders through `iced::widget::markdown` rather than the
//! custom renderer the document viewer uses, which exists for FTWA tables
//! plus theme-token control. Verified against the local registry source
//! `iced_widget-0.14.2/src/markdown.rs`:
//!
//! | API | Citation |
//! | --- | --- |
//! | `markdown::parse(&str) -> impl Iterator<Item = Item> + '_` | `markdown.rs:426-429` |
//! | `markdown::view(items, settings) -> Element<'a, Uri, Theme, Renderer>` | `markdown.rs:1197-1206` |
//! | `markdown::view_with(items, settings, viewer)` (custom image viewer hook) | `markdown.rs:1213-1231` |
//! | `Item::{ Heading, Paragraph, CodeBlock{language,code,lines}, List, Image{url,title,alt}, Quote, Rule, Table }` | `markdown.rs:192-235` |
//! | `Content::{ parse, push_str, items }` (incremental streaming buffer) | `markdown.rs:95-182` |
//! | `Settings::with_text_size(size, style)` | `markdown.rs:1067-1085` |
//! | `Style { font, inline_code_highlight, inline_code_padding, inline_code_color, inline_code_font, code_block_font, link_color }` | `markdown.rs:1102-1117` |
//! | `Viewer::image(settings, url, title, alt)` (customisable image rendering) | `markdown.rs:1576-1593` |
//! | `Catalog` impl for `iced::Theme` (code-block container) | `markdown.rs:1706-1722` |
//!
//! `markdown::view` emits its link clicks as the `Uri` (= `String`) message
//! type, so the view `.map`s them onto
//! [`RenderMessage::LinkActivated`] with `source_file: None` — the variant
//! the message channel already reserves for "text not backed by a file
//! (e.g. an agent transcript)"
//! (`src/app/ui_iced/message.rs:538-544`). The reducer resolves the link.
//!
//! # Feature gate
//!
//! The widget needs iced's `markdown` cargo feature
//! (`iced-0.14.0/Cargo.toml:93` `markdown = ["iced_widget/markdown"]`,
//! `iced_widget-0.14.2/Cargo.toml:51` `markdown = ["dep:pulldown-cmark"]`).
//! It is enabled in the workspace `Cargo.toml`.
//!
//! # Auto-scroll
//!
//! Follow-the-stream reuses [`crate::ui_iced::logs::should_follow`] exactly
//! as [`crate::ui_iced::agent_debug`] does — one rule, three windows. The
//! view only reports its viewport geometry through
//! [`AgentMessage::ScrollReported`]; the reducer decides and dispatches the
//! scroll operation against [`AGENT_TRANSCRIPT_SCROLLABLE_ID`].
//!
//! Unit tests live in the sibling `transcript_tests.rs` sidecar.

use iced::widget::{Column, Row, button, container, scrollable, text};
use iced::{Alignment, Element, Length};

use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::logs;
use crate::ui_iced::message::{AgentMessage, Message, RenderMessage};

use super::panel_state::{AgentPanelViewModel, TranscriptTurn};

/// Stable iced widget id of the transcript `scrollable`.
///
/// The reducer needs a stable id to aim
/// `operation::scrollable::scroll_to` at
/// (`iced_runtime-0.14.0/src/widget/operation.rs:31-39`); a widget without
/// an explicit id cannot be scrolled programmatically. Named once so the
/// view and the reducer cannot drift ([RUST-021]).
pub const AGENT_TRANSCRIPT_SCROLLABLE_ID: &str = "agent_transcript_scroll";

/// Base logical point size of the transcript body text — the default body
/// size, stated explicitly because iced needs it.
pub const TRANSCRIPT_TEXT_SIZE: f32 = 14.0;

/// Spacing between transcript blocks.
pub const TRANSCRIPT_SPACING: f32 = 8.0;

/// Spacing inside the thinking disclosure.
pub const THINKING_SPACING: f32 = 4.0;

/// Padding around the whole transcript body.
pub const TRANSCRIPT_PADDING: f32 = 8.0;

/// The [`Message`] the transcript scrollable emits for a viewport report,
/// built from a reused [`logs::ScrollSnapshot`].
///
/// Purity: pure function of `snapshot`.
#[must_use]
pub fn scroll_reported_message(snapshot: &logs::ScrollSnapshot) -> Message {
    Message::Agent(AgentMessage::ScrollReported {
        offset_y: snapshot.offset_y,
        viewport_height: snapshot.viewport_height,
        content_height: snapshot.content_height,
    })
}

/// The [`Message`] a clicked Markdown link in the transcript produces.
///
/// `source_file` is `None` because the transcript is not backed by a file —
/// the same reading the [`RenderMessage::LinkActivated`] docs give for an
/// agent transcript (`src/app/ui_iced/message.rs:541-543`).
///
/// Purity: pure function of `url`.
#[must_use]
pub fn link_activated_message(url: String) -> Message {
    Message::Render(RenderMessage::LinkActivated {
        url,
        source_file: None,
    })
}

/// The Markdown text [`iced::widget::markdown::Style`] mapped from the FastMD dark palette tokens
/// — no numeric colour literal is introduced.
///
/// Purity: pure.
#[must_use]
pub fn markdown_style() -> iced::widget::markdown::Style {
    let palette = Palette::fastmd_dark();
    iced::widget::markdown::Style {
        font: iced::Font::DEFAULT,
        inline_code_highlight: iced::widget::markdown::Highlight {
            background: iced::Background::Color(palette.code_surface),
            border: iced::Border::default().rounded(4),
        },
        inline_code_padding: iced::Padding {
            top: 0.0,
            right: 1.0,
            bottom: 0.0,
            left: 1.0,
        },
        inline_code_color: palette.text_strong,
        inline_code_font: iced::Font::MONOSPACE,
        code_block_font: iced::Font::MONOSPACE,
        link_color: palette.accent,
    }
}

/// The [`iced::widget::markdown::Settings`] handed to `markdown::view`, sized from
/// [`TRANSCRIPT_TEXT_SIZE`].
///
/// Purity: pure.
#[must_use]
pub fn markdown_settings() -> iced::widget::markdown::Settings {
    iced::widget::markdown::Settings::with_text_size(TRANSCRIPT_TEXT_SIZE, markdown_style())
}

/// Renders pre-parsed Markdown items through the built-in Markdown widget.
///
/// The items are owned by the caller (the [`TranscriptTurn`]) because
/// `markdown::view` borrows them for the returned `Element`'s lifetime
/// (`iced_widget-0.14.2/src/markdown.rs:1197-1206`). Link clicks map to
/// [`link_activated_message`].
///
/// Purity: pure over `items` (the only effect is the message the widget
/// emits on a link click).
#[must_use]
pub fn render_markdown_items(items: &[iced::widget::markdown::Item]) -> Element<'_, Message> {
    iced::widget::markdown::view(items, markdown_settings()).map(link_activated_message)
}

/// Renders a single turn: a user turn as themed text, an assistant turn as
/// Markdown (from its pre-parsed items).
///
/// Purity: pure over `turn`.
#[must_use]
pub fn render_turn(turn: &TranscriptTurn) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();
    match turn {
        TranscriptTurn::User { text: user_text } => container(
            text(user_text)
                .size(TRANSCRIPT_TEXT_SIZE)
                .color(palette.text_strong),
        )
        .padding(4)
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(palette.surface_variant)),
            border: iced::Border {
                color: palette.border,
                width: 1.0,
                radius: 4.0.into(),
            },
            ..container::Style::default()
        })
        .width(Length::Fill)
        .into(),
        TranscriptTurn::Assistant { items, .. } => render_markdown_items(items),
    }
}

/// The gated, disclosable "Thinking Process" block (UI-048).
///
/// The block is allocated only when [`AgentPanelViewModel::show_thinking`]
/// holds; the header is a button that flips the disclosure via
/// [`AgentMessage::ThinkingToggled`], and the body renders the extracted
/// thinking text in muted italics.
///
/// Purity: pure over `view_model`.
#[must_use]
pub fn thinking_block(view_model: &AgentPanelViewModel) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();

    let header = button(
        Row::with_children(vec![
            text(strings::AGENT_THINKING_PROCESS)
                .size(TRANSCRIPT_TEXT_SIZE)
                .color(palette.text_strong)
                .into(),
            container(text(if view_model.thinking_expanded {
                "▾"
            } else {
                "▸"
            }))
            .width(Length::Fill)
            .align_right(Length::Shrink)
            .into(),
        ])
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .on_press(Message::Agent(AgentMessage::ThinkingToggled {
        expanded: !view_model.thinking_expanded,
    }));

    let body: Element<'_, Message> = if view_model.thinking_expanded {
        text(&view_model.thinking)
            .size(TRANSCRIPT_TEXT_SIZE)
            .font(italic_font())
            .color(palette.text_muted)
            .into()
    } else {
        Column::<Message>::new().into()
    };

    Column::with_children(vec![header.into(), body])
        .spacing(THINKING_SPACING)
        .into()
}

/// The transcript body: thinking block, turn list, and the waiting
/// placeholder — assembled in order.
///
/// Purity: pure over `view_model`.
#[must_use]
pub fn transcript_body(view_model: &AgentPanelViewModel) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();

    let mut children: Vec<Element<'_, Message>> = Vec::new();

    if view_model.show_thinking() {
        children.push(thinking_block(view_model));
    }

    for turn in &view_model.turns {
        children.push(render_turn(turn));
    }

    if view_model.waiting_for_response() {
        children.push(
            text(strings::AGENT_THINKING_LABEL)
                .size(TRANSCRIPT_TEXT_SIZE)
                .font(italic_font())
                .color(palette.text_muted)
                .into(),
        );
    }

    Column::with_children(children)
        .spacing(TRANSCRIPT_SPACING)
        .padding(TRANSCRIPT_PADDING)
        .into()
}

/// Renders the scrolling agent transcript from a snapshot.
///
/// Emitted messages:
/// * thinking header → [`AgentMessage::ThinkingToggled`]
/// * Markdown link → [`RenderMessage::LinkActivated`] (`source_file: None`)
/// * any scroll of the list → [`AgentMessage::ScrollReported`], the input
///   the reducer feeds to [`crate::ui_iced::logs::should_follow`].
///
/// The scrollable is always allocated; an empty transcript renders an empty
/// column inside it, so the shape never changes across the empty/non-empty
/// transition.
#[must_use]
pub fn agent_transcript_view(view_model: &AgentPanelViewModel) -> Element<'_, Message> {
    scrollable(transcript_body(view_model))
        .id(AGENT_TRANSCRIPT_SCROLLABLE_ID)
        .height(Length::Fill)
        .on_scroll(|viewport| scroll_reported_message(&logs::scroll_snapshot(&viewport)))
        .into()
}

/// Italic variant of the default UI font — iced's `Text` has no `.italic()`
/// helper, so the style rides on the [`iced::Font`] struct (the same trick
/// as `ui_iced::panels::center::italic_font`).
fn italic_font() -> iced::Font {
    iced::Font {
        style: iced::font::Style::Italic,
        ..iced::Font::DEFAULT
    }
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `transcript_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "transcript_tests.rs"]
mod tests;
