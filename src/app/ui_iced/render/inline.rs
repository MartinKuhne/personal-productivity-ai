//! Inline styled-run mapping and the iced `Rich` text builder ([TASK-015]).
//!
//! The egui build painted the same `InlineElem` runs through three
//! different APIs and kept their shared mapping in
//! `src/app/ui/render/style.rs` (`styled_text` for the `RichText` path,
//! `text_format` for the `LayoutJob` path). iced collapses both into one:
//! a [`Span`] carries size, font, colour, background highlight, underline
//! and strikethrough, and a [`Rich`] text widget lays a slice of them out
//! as one wrapped run with clickable links.
//!
//! The mapping is split in two so it is testable without a renderer:
//!
//! * [`SpanModel`] — a **pure descriptor**: every palette token already
//!   resolved to an `iced::Color`, the font already chosen, the link
//!   target already carried. Tests assert on these values.
//! * [`to_iced_span`] / [`rich_text_block`] — the thin widget builders that
//!   turn descriptors into widgets and wire the link click to
//!   [`RenderMessage::LinkActivated`].
//!
//! Verified iced APIs (local sources): `Span` fields —
//! `iced_core-0.14.0/src/text.rs:379-402`; `Span::size`/`font`/`color`/
//! `link`/`background`/`underline`/`strikethrough` builders —
//! `iced_core-0.14.0/src/text.rs:423-547`; `Rich::with_spans` —
//! `iced_widget-0.14.2/src/text/rich.rs:64-72`; `Rich::on_link_click` —
//! `rich.rs:140-146`; `rich_text` helper —
//! `iced_widget-0.14.2/src/helpers.rs:1194-1204`.
//!
//! Unit tests live in the sibling `inline_tests.rs` sidecar.

use crate::markdown::{InlineElem, TextStyle, image_placeholder};
use crate::ui::theme::Palette;
use crate::ui_iced::message::{Message, RenderMessage};
use crate::ui_iced::theme::to_iced_color;
use iced::widget::rich_text;
use iced::widget::text::{Rich, Span};
use iced::{Background, Color, Element, Font, Length};
use std::path::Path;

/// Point size of body/paragraph inline text.
///
/// The egui renderer passed `None` for the size on paragraph runs, which
/// meant `TextStyle::Body`; the FastMD body size is 14 pt
/// (`src/app/ui/render/table/cell.rs:194` uses the same fallback).
pub const BODY_SIZE: f32 = 14.0;

/// Point size of the list bullet / ordinal marker.
///
/// Ported from `src/app/ui/render/inline.rs:128-131` (`RichText::new(…).size(14.0)`).
pub const LIST_MARKER_SIZE: f32 = 14.0;

/// Horizontal padding around an inline-code highlight.
///
/// egui's `RichText::background_color` painted the background flush to
/// the glyphs; iced's span highlight takes explicit padding
/// (`Span::padding`, `iced_core-0.14.0/src/text.rs:532-535`), so a small
/// inset keeps the code chip readable.
pub const CODE_HIGHLIGHT_PADDING: f32 = 1.0;

/// A single styled inline run with every theme token already resolved.
///
/// `PartialEq` holds because `iced::Color` is `PartialEq`
/// (`iced_core-0.14.0/src/color.rs:14`), which is what lets the render
/// mapping be asserted directly in tests.
#[derive(Clone, Debug, PartialEq)]
pub struct SpanModel {
    /// The text of this run.
    pub text: String,
    /// Point size the run renders at.
    pub size: f32,
    /// Resolved text colour.
    pub color: Color,
    /// Resolved font handle (monospace for code runs, italic where asked).
    pub font: Font,
    /// Background highlight colour, present only for inline code.
    pub highlight: Option<Color>,
    /// Whether the run is underlined (links).
    pub underline: bool,
    /// Whether the run is struck through.
    pub strikethrough: bool,
    /// Raw link target, present only on link runs.
    pub link: Option<String>,
}

impl SpanModel {
    /// A plain run of `text` at `size` in the default body colour.
    #[must_use]
    pub fn plain(text: impl Into<String>, size: f32, palette: Palette) -> Self {
        Self {
            text: text.into(),
            size,
            color: to_iced_color(palette.text),
            font: Font::DEFAULT,
            highlight: None,
            underline: false,
            strikethrough: false,
            link: None,
        }
    }
}

/// The iced [`Font`] a [`TextStyle`] maps onto at a base font.
///
/// Code switches to [`Font::MONOSPACE`] (`iced_core-0.14.0/src/font.rs:27`);
/// italics ride on `Font::style` because iced's text widgets have no
/// `.italic()` helper (the same trick `panels::top::indexing_status_font`
/// uses).
#[must_use]
pub fn font_for_style(style: &TextStyle) -> Font {
    let mut font = if style.code {
        Font::MONOSPACE
    } else {
        Font::DEFAULT
    };
    if style.italic {
        font.style = iced::font::Style::Italic;
    }
    font
}

/// Maps one styled text run onto a [`SpanModel`].
///
/// Colour precedence mirrors the egui mapping
/// (`src/app/ui/render/style.rs:99-133`): `muted` wins over `bold`,
/// `bold` wins over the default body colour. Code runs get the `border`
/// token as their background, exactly as `styled_text` chose it
/// (`style.rs:82`).
#[must_use]
pub fn span_for_text(text: &str, style: &TextStyle, size: f32, palette: Palette) -> SpanModel {
    let color = if style.muted {
        palette.text_muted
    } else if style.bold {
        palette.text_strong
    } else {
        palette.text
    };
    SpanModel {
        text: text.to_owned(),
        size,
        color: to_iced_color(color),
        font: font_for_style(style),
        highlight: style.code.then(|| to_iced_color(palette.border)),
        underline: false,
        strikethrough: style.strikethrough,
        link: None,
    }
}

/// Maps one link onto a [`SpanModel`].
///
/// Links take the `accent_text` token and are underlined. The egui build
/// used `ui.hyperlink_to`, whose colour came from egui's own
/// `Visuals::hyperlink_color` default — not a FastMD token
/// (`src/app/ui/theme.rs:104-106` says so explicitly) — so the token
/// choice here is the honest themed equivalent rather than a ported value.
#[must_use]
pub fn span_for_link(url: &str, label: &str, size: f32, palette: Palette) -> SpanModel {
    SpanModel {
        text: label.to_owned(),
        size,
        color: to_iced_color(palette.accent_text),
        font: Font::DEFAULT,
        highlight: None,
        underline: true,
        strikethrough: false,
        link: Some(url.to_owned()),
    }
}

/// Maps a slice of parser inline elements onto span models.
///
/// `SoftBreak` becomes a single space (egui rendered `ui.label(" ")`),
/// `Image` becomes the shared `[Image: url]` placeholder
/// ([`image_placeholder`]) in the muted colour, and `Html` renders muted
/// — the same presentation the egui inline renderer gave them
/// (`src/app/ui/render/inline.rs:146-177`).
#[must_use]
pub fn spans_from_inline(elems: &[InlineElem], size: f32, palette: Palette) -> Vec<SpanModel> {
    elems
        .iter()
        .map(|elem| match elem {
            InlineElem::Text(text, style) => span_for_text(text, style, size, palette),
            InlineElem::Link(url, label) => span_for_link(url, label, size, palette),
            InlineElem::Image(url) => SpanModel {
                highlight: None,
                color: to_iced_color(palette.text_muted),
                ..SpanModel::plain(image_placeholder(url), size, palette)
            },
            InlineElem::Html(html) => SpanModel {
                color: to_iced_color(palette.text_muted),
                ..SpanModel::plain(html.clone(), size, palette)
            },
            InlineElem::SoftBreak => SpanModel::plain(" ", size, palette),
        })
        .collect()
}

/// Builds the iced [`Span`] for one model.
#[must_use]
pub fn to_iced_span(model: &SpanModel) -> Span<'static, String> {
    let mut span = Span::<String>::new(model.text.clone())
        .size(model.size)
        .color(model.color)
        .font(model.font)
        .underline(model.underline)
        .strikethrough(model.strikethrough);
    if let Some(background) = model.highlight {
        span = span
            .background(Background::Color(background))
            .padding(CODE_HIGHLIGHT_PADDING);
    }
    if let Some(link) = model.link.clone() {
        span = span.link(link);
    }
    span
}

/// Renders span models as one wrapped iced `Rich` text block.
///
/// `source_file` is carried into every emitted
/// [`RenderMessage::LinkActivated`] so the reducer can resolve the link
/// relative to the document it came from; `None` means the rendered text
/// is not file-backed. Width is `Fill` and wrapping is word-level, which
/// is the iced equivalent of the `.wrap()` pinning the egui renderer
/// applied to every label (`src/app/ui/render/inline.rs:150-158`).
#[must_use]
pub fn rich_text_block(
    models: &[SpanModel],
    source_file: Option<&Path>,
) -> Rich<'static, String, Message, iced::Theme, iced::Renderer> {
    let spans: Vec<Span<'static, String>> = models.iter().map(to_iced_span).collect();
    let source = source_file.map(Path::to_path_buf);
    rich_text(spans)
        .width(Length::Fill)
        .wrapping(iced::widget::text::Wrapping::Word)
        .on_link_click(move |url| {
            Message::Render(RenderMessage::LinkActivated {
                url,
                source_file: source.clone(),
            })
        })
}

/// [`rich_text_block`] as an [`Element`], for call sites that only need
/// the erased widget type.
///
/// `'static` because every span owns its text and its link target, so the
/// element borrows nothing from the descriptor slice.
#[must_use]
pub fn rich_text_element(
    models: &[SpanModel],
    source_file: Option<&Path>,
) -> Element<'static, Message> {
    Element::new(rich_text_block(models, source_file))
}

#[cfg(test)]
#[path = "inline_tests.rs"]
mod tests;
