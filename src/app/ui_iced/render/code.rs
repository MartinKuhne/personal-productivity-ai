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
use iced::widget::{Column, Row, container, text};
use iced::{Alignment, Background, Element, Font, Length, Padding};
use iced_pro::token::typography;
use iced_pro::token::{shape, size, space};
use iced_pro::widget::button::ButtonVariant;
use iced_pro::widget::button_widget::Button;

/// The rung the code body renders on — `type.body` (`UIF-TOK-004`).
///
/// §2.4 leaves the rung for *code blocks* **unstated**: of the six monospace
/// usages it names, only the editor buffer (`type.body`), log entries
/// (`type.ui_compact`) and timestamps (`type.caption`) get one. `BODY` is
/// chosen because the code block is the same reading task as the editor
/// buffer, which the spec does put on this rung — and because the surrounding
/// document prose is `BODY`, so a fenced block does not shrink mid-paragraph.
/// The gap is recorded rather than papered over: this is a decision made in
/// the app, not a value read off the spec.
pub const CODE_RUNG: typography::TypeRung = typography::BODY;

/// Point size of the monospace code text — `type.body` (`UIF-TOK-004`).
/// Value unchanged at `14`.
pub const CODE_TEXT_SIZE: f32 = CODE_RUNG.size;

/// Point size of the language caption — `type.caption` (`UIF-TOK-004`).
///
/// **Movement: 11 → 12 (+1). This is a legibility-floor fix, not a rename.**
/// `11` sits **below** [`typography::MIN_SIZE`] (12), the rule that "no text
/// anywhere in the interface may render smaller than 12px" (§2.4,
/// `UIF-TOK-004`). The caption was rendering illegal text on every fenced
/// block. `CAPTION` is also the rung §2.4 assigns to labels of this kind
/// (shortcut hints, status chips), so the fix lands on a real rung rather
/// than nudging the illegal number up by hand.
pub const CODE_LANGUAGE_SIZE: f32 = typography::CAPTION.size;

/// Inner padding of the code panel — `space.sm` (`UIF-TOK-001`). Value
/// unchanged at `8`.
pub const CODE_PADDING: f32 = space::SM;

/// Gap between the caption, the code and the copy button — `space.xs`
/// (`UIF-TOK-001`). Value unchanged at `4`.
pub const CODE_SPACING: f32 = space::XS;

/// Width of the copy button's column, so the wrapping code label always
/// leaves room for the button.
///
/// **Value unchanged at 30, now derived:** `size.control.sm + space.2xs`
/// (`28 + 2`). The reservation has to hold the button's hit area plus a
/// hair of slack so a code line wrapping against the column edge never
/// slides under the glyph. `CONTROL_SM` is the compact control rung the
/// button is built on; `XXS` is the dense micro-gap rung. No framework token
/// names a *wrap reservation* directly, so the composition is stated here
/// rather than left as a bare `30`.
pub const CODE_COPY_COLUMN_WIDTH: f32 = size::CONTROL_SM + space::XXS;

/// Border width drawn around the code panel — `border.hairline`
/// (`UIF-TOK-003`). Value unchanged at `1`.
pub const CODE_BORDER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// Corner radius of the code panel's border — `shape.radius.widget`
/// (`UIF-TOK-003`).
///
/// Value unchanged at `4`. The old doc justified `4` as "the radius the
/// FastMD dark visuals put on interactive containers"; that is exactly what
/// `RADIUS_WIDGET` is, so the number now comes from the token that says so.
pub const CODE_BORDER_RADIUS: f32 = shape::RADIUS_WIDGET;

/// Point size of the copy button's glyph — `size.icon.sm` (`UIF-TOK-002`).
///
/// Value unchanged at `14`. The old code reused [`CODE_TEXT_SIZE`] for the
/// glyph, which happened to be the same number but tied a glyph's size to a
/// type rung: retuning the code font would have resized the icon with it.
pub const CODE_COPY_GLYPH_SIZE: f32 = size::ICON_SM;

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
    .line_height(typography::CAPTION.iced_line_height())
    .color(palette.text_muted);

    let body = text(model.content.clone())
        .font(Font::MONOSPACE)
        .size(CODE_TEXT_SIZE)
        .line_height(CODE_RUNG.iced_line_height())
        .color(palette.text)
        .width(Length::Fill)
        .wrapping(iced::widget::text::Wrapping::WordOrGlyph);

    let copy = Button::icon_only(
        ButtonVariant::Ghost,
        text(strings::ICON_COPY)
            .font(phosphor_font())
            .size(CODE_COPY_GLYPH_SIZE),
        model.copy_message.clone(),
        "code_copy",
        strings::CODE_COPY_BUTTON_TOOLTIP,
    )
    .compact()
    .palette(Palette::pro());

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
