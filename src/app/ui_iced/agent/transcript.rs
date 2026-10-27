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

use hoarfrost::token::typography::{self, TypeRung};
use hoarfrost::token::{shape, space};
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

/// The rung the transcript body renders on — `type.body` (`UIF-TOK-004`).
///
/// §2.4 puts document prose and text-input values on this rung; the
/// transcript is prose the user reads, so it takes the same one. Held as a
/// rung because the body needs the size *and* the 20 px line box.
pub const TRANSCRIPT_RUNG: TypeRung = typography::BODY;

/// Base logical point size of the transcript body text — `type.body`
/// (`UIF-TOK-004`). Value unchanged at `14`.
pub const TRANSCRIPT_TEXT_SIZE: f32 = TRANSCRIPT_RUNG.size;

/// Spacing between transcript blocks — `space.sm` (`UIF-TOK-001`), the
/// standard sibling-gap rung. Value unchanged at `8`.
pub const TRANSCRIPT_SPACING: f32 = space::SM;

/// Spacing inside the thinking disclosure — `space.xs` (`UIF-TOK-001`).
/// Value unchanged at `4`.
pub const THINKING_SPACING: f32 = space::XS;

/// Padding around the whole transcript body — `space.sm` (`UIF-TOK-001`).
/// Value unchanged at `8`.
pub const TRANSCRIPT_PADDING: f32 = space::SM;

/// Padding inside a user-turn bubble — `space.xs` (`UIF-TOK-001`).
///
/// **Replaces a bare `.padding(4)` literal** ([RUST-021]).
pub const TURN_PADDING: f32 = space::XS;

/// Perimeter stroke of a user-turn bubble — `border.hairline`
/// (`UIF-TOK-003`). **Replaces a bare `1.0` literal.**
pub const TURN_BORDER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// Corner radius of a user-turn bubble — `shape.radius.widget`
/// (`UIF-TOK-003`). **Replaces a bare `4.0` literal.**
pub const TURN_BORDER_RADIUS: f32 = shape::RADIUS_WIDGET;

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

/// The link colour a transcript hyperlink paints — `color.accent.text`
/// (`UIF-TOK-005`).
///
/// **A real contrast fix, not a rename.** This used to be
/// [`Palette::accent`], which *is* `color.accent.fill` — one of the four
/// tokens `UIF-TOK-005` rule 4 reserves for **backgrounds**. Painted as a
/// foreground it measures:
///
/// | over | ratio |
/// | --- | --- |
/// | `surface.base` | `4.14:1` |
/// | `surface.raised` | `3.90:1` |
/// | `surface.code` | `3.83:1` |
///
/// — every one of them **below** the `4.50:1` active-text floor §2.5 rule 1
/// sets, on every surface the transcript can sit on. `color.accent.text` is
/// the token the semantic system actually provides for accent-coloured
/// *foreground*, and it measures `10.66:1` / `10.02:1` / `9.85:1` over the
/// same three surfaces.
///
/// The fix is the role change, **not** a retune of the fill: `accent.fill`
/// stays exactly where it is for the fills that use it, and nothing that
/// paints *on* it moves.
///
/// Ratios recomputed from raw sRGB through
/// [`contrast::contrast_ratio`](hoarfrost::token::contrast::contrast_ratio),
/// not transcribed.
///
/// Purity: pure.
#[must_use]
pub fn transcript_link_color() -> iced::Color {
    Palette::fastmd_dark().accent_text
}

/// The Markdown text [`iced::widget::markdown::Style`] mapped from the FastMD dark palette tokens
/// — no numeric colour literal is introduced.
///
/// The inline-code chip reuses
/// [`CODE_HIGHLIGHT_PADDING`](crate::ui_iced::render::inline::CODE_HIGHLIGHT_PADDING)
/// rather than restating a padding of its own, so an `` `inline code` `` chip
/// is the same shape in the transcript and in the document viewer — one
/// definition, one look ([RUST-021]).
///
/// Purity: pure.
#[must_use]
pub fn markdown_style() -> iced::widget::markdown::Style {
    let palette = Palette::fastmd_dark();
    iced::widget::markdown::Style {
        font: iced::Font::DEFAULT,
        inline_code_highlight: iced::widget::markdown::Highlight {
            background: iced::Background::Color(palette.code_surface),
            border: iced::Border::default().rounded(shape::RADIUS_WIDGET),
        },
        inline_code_padding: crate::ui_iced::render::inline::CODE_HIGHLIGHT_PADDING,
        inline_code_color: palette.text_strong,
        inline_code_font: iced::Font::MONOSPACE,
        code_block_font: iced::Font::MONOSPACE,
        link_color: transcript_link_color(),
    }
}

/// The [`iced::widget::markdown::Settings`] handed to `markdown::view`.
///
/// # Why the fields are set by hand
///
/// `Settings::with_text_size` derives its whole ladder arithmetically off
/// the base — `h1 = base × 2`, then 25% smaller per level, and
/// `code = base × 0.75` (`iced_widget-0.14.2/src/markdown.rs:1073-1083`).
/// At the transcript's `type.body` base of 14 that produces:
///
/// | field | derived | verdict |
/// | --- | --- | --- |
/// | `h1_size` | 28.00 | off-scale (scale tops out at 24) |
/// | `h2_size` | 24.50 | off-scale |
/// | `h3_size` | 21.00 | off-scale |
/// | `h4_size` | 17.50 | off-scale |
/// | `h5_size` | 14.00 | legal |
/// | `h6_size` | 14.00 | legal |
/// | `code_size` | **10.50** | **below `type.min` (12)** |
/// | `spacing` | 12.25 | off the base-4 grid |
///
/// i.e. one size under the legibility floor and five off the scale, none of
/// them chosen by anyone — they are a side effect of a multiplication table
/// that knows nothing about `UIF-TOK-004`. So every field is bound to a rung
/// here instead, reusing the **same** ladder the document viewer's
/// [heading module](crate::ui_iced::render::heading) produces. The
/// transcript and the document now render
/// one Markdown at one set of sizes, and the floor test has something to
/// hold.
///
/// Purity: pure.
#[must_use]
pub fn markdown_settings() -> iced::widget::markdown::Settings {
    use crate::ui_iced::render::heading;

    iced::widget::markdown::Settings {
        text_size: iced::Pixels(TRANSCRIPT_TEXT_SIZE),
        h1_size: iced::Pixels(heading::H1_SIZE),
        h2_size: iced::Pixels(heading::H2_SIZE),
        h3_size: iced::Pixels(heading::H3_SIZE),
        h4_size: iced::Pixels(heading::H4_SIZE),
        h5_size: iced::Pixels(heading::H5_SIZE),
        // §2.4's ladder has no rung below CAPTION, and the document renderer
        // floors H5 and H6 together — same rule here.
        h6_size: iced::Pixels(heading::H5_SIZE),
        code_size: iced::Pixels(crate::ui_iced::render::code::CODE_TEXT_SIZE),
        spacing: iced::Pixels(TRANSCRIPT_SPACING),
        style: markdown_style(),
    }
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
                .line_height(TRANSCRIPT_RUNG.iced_line_height())
                .color(palette.text_strong),
        )
        .padding(TURN_PADDING)
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(palette.surface_variant)),
            border: iced::Border {
                color: palette.border,
                width: TURN_BORDER_WIDTH,
                radius: TURN_BORDER_RADIUS.into(),
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
/// # Why the header is iced's `button`, not the framework's
///
/// [`Button`](hoarfrost::widget::button_widget::Button) has no `.width()`
/// builder — its width comes from the content `size_hint` inside a fixed
/// control band ([`button::HEIGHT_COMPACT`](hoarfrost::widget::button::HEIGHT_COMPACT)
/// / `HEIGHT_STANDARD`), and the only width-shaped constructor is
/// [`icon_only`](hoarfrost::widget::button_widget::Button::icon_only). A
/// disclosure header has to stretch the full pane
/// width so the whole row is the click target, which the framework control
/// cannot express. That is a framework gap, not a preference — recorded here
/// so the next pass does not read the iced widget as an oversight.
///
/// Purity: pure over `view_model`.
#[must_use]
pub fn thinking_block(view_model: &AgentPanelViewModel) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();

    let chevron = if view_model.thinking_expanded {
        strings::ICON_DISCLOSE_EXPANDED
    } else {
        strings::ICON_DISCLOSE_COLLAPSED
    };

    let header = button(
        Row::with_children(vec![
            text(strings::AGENT_THINKING_PROCESS)
                .size(TRANSCRIPT_TEXT_SIZE)
                .line_height(TRANSCRIPT_RUNG.iced_line_height())
                .color(palette.text_strong)
                .into(),
            container(text(chevron).size(TRANSCRIPT_TEXT_SIZE))
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
            .line_height(TRANSCRIPT_RUNG.iced_line_height())
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
                .line_height(TRANSCRIPT_RUNG.iced_line_height())
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
