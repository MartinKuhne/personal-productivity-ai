//! Fenced code-block mapping and widget.
//!
//! A fence renders as a bordered `code_surface` panel holding a monospace
//! label, a muted language caption and a copy-to-clipboard button:
//! [`Font::MONOSPACE`] (`iced_core-0.14.0/src/font.rs:27-31`) on a
//! [`Palette::code_surface`] background with a [`Palette::border`] frame.
//!
//! The copy button cannot copy from the view: iced exposes the clipboard to
//! `update` only (`iced_core-0.14.0/src/clipboard.rs:7-38` has `read`/
//! `write` on the `Clipboard` trait, reached from the update signature).
//! The content therefore rides on the button's
//! [`RenderMessage::CopyCodeRequested`] message and the reducer writes it
//! — which also makes the message part of the pure model and directly
//! assertable in tests.
//!
//! Unit tests live in the sibling `code_tests.rs` sidecar.

use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::message::{Message, RenderMessage};
use crate::ui_iced::panels::top::phosphor_font;
use iced::widget::{Column, Row, button, container, text};
use iced::{Alignment, Background, Element, Font, Length, Padding};

/// Point size of the monospace code text — the body size, 14 pt.
pub const CODE_TEXT_SIZE: f32 = 14.0;

/// Point size of the language caption.
pub const CODE_LANGUAGE_SIZE: f32 = 11.0;

/// Inner padding of the code panel.
pub const CODE_PADDING: f32 = 8.0;

/// Gap between the caption, the code and the copy button.
pub const CODE_SPACING: f32 = 4.0;

/// Width of the copy button's column, so the wrapping code label always
/// leaves room for the button.
pub const CODE_COPY_COLUMN_WIDTH: f32 = 30.0;

/// Border width drawn around the code panel, in logical pixels.
pub const CODE_BORDER_WIDTH: f32 = 1.0;

/// Corner radius of the code panel's border, matching the 4 px radius the
/// FastMD dark visuals put on interactive containers.
pub const CODE_BORDER_RADIUS: f32 = 4.0;

/// A fenced code block, resolved against the palette.
///
/// `copy_message` is part of the model rather than built inside the widget
/// so the exact clipboard payload the button carries is testable.
#[derive(Clone, Debug, PartialEq)]
pub struct CodeBlockModel {
    /// The fence's language tag, `None` when the fence had none.
    pub language: Option<String>,
    /// The verbatim code content.
    pub content: String,
    /// The message the copy button emits — always
    /// [`RenderMessage::CopyCodeRequested`] carrying [`Self::content`].
    pub copy_message: Message,
}

/// Builds the code-block model for one fence.
#[must_use]
pub fn code_block_model(language: Option<&str>, content: &str) -> CodeBlockModel {
    CodeBlockModel {
        language: language.map(str::to_owned),
        content: content.to_owned(),
        copy_message: Message::Render(RenderMessage::CopyCodeRequested {
            content: content.to_owned(),
        }),
    }
}

/// Whether the language caption renders.
///
/// An absent *or* empty tag shows no caption.
#[must_use]
pub fn has_language_caption(model: &CodeBlockModel) -> bool {
    model
        .language
        .as_deref()
        .is_some_and(|lang| !lang.is_empty())
}

/// Renders a fenced code block.
///
/// iced rebuilds the widget tree structurally each frame, so the caption's
/// presence does not destabilise sibling identity; no placeholder
/// allocation is needed.
#[must_use]
pub fn code_block(model: &CodeBlockModel) -> Element<'static, Message> {
    let palette = Palette::fastmd_dark();

    let caption = text(
        model
            .language
            .as_deref()
            .filter(|lang| !lang.is_empty())
            .map(str::to_owned)
            .unwrap_or_default(),
    )
    .size(CODE_LANGUAGE_SIZE)
    .color(palette.text_muted);

    let body = text(model.content.clone())
        .font(Font::MONOSPACE)
        .size(CODE_TEXT_SIZE)
        .color(palette.text)
        .width(Length::Fill)
        .wrapping(iced::widget::text::Wrapping::WordOrGlyph);

    let copy = button(
        text(strings::ICON_COPY)
            .font(phosphor_font())
            .size(CODE_TEXT_SIZE),
    )
    .on_press(model.copy_message.clone());

    let row = Row::with_children(vec![body.into(), copy.into()])
        .spacing(CODE_SPACING)
        .align_y(Alignment::Start);

    let panel = Column::with_children(vec![caption.into(), row.into()]).spacing(CODE_SPACING);

    container(panel)
        .width(Length::Fill)
        .padding(Padding::from(CODE_PADDING))
        .style(move |_theme| container::Style {
            background: Some(Background::Color(palette.code_surface)),
            border: iced::Border {
                color: palette.border,
                width: CODE_BORDER_WIDTH,
                radius: CODE_BORDER_RADIUS.into(),
            },
            ..container::Style::default()
        })
        .into()
}

#[cfg(test)]
#[path = "code_tests.rs"]
mod tests;
