//! The window's chrome: the navbar row, the Close button and the status bar.
//!
//! Three bands, none of them invented. The navbar is the framework's own
//! [`NavBar`] composite over a [`NavBarModel`] (`UIF-NAV-001`); the Close
//! button is a hoarfrost [`Button`] in the `Danger` variant
//! (`UIF-ATOM-001`); the status bar is a `container` painted from the
//! same token palette so it sits on the same surface rung as the navbar.
//!
//! The Close button is the only control in this program that does anything
//! real: it maps to [`Message::Close`], which returns [`iced::exit`].
//!
//! Unit tests live in the sibling `chrome_tests.rs` sidecar.

use iced::widget::{Row, container};
use iced::{Alignment, Background, Border, Element, Length, Shadow};

use hoarfrost::composite::navbar::{NavBar, NavBarModel};
use hoarfrost::token::color::Palette;
use hoarfrost::token::{shape, size, space, typography};
use hoarfrost::widget::button::ButtonVariant;
use hoarfrost::widget::button_widget::Button;

use crate::app::Message;
use crate::data;
use crate::section::rung;

/// Label of the Close button.
pub const CLOSE_LABEL: &str = "Close";

/// Stable focus id of the Close button.
pub const CLOSE_FOCUS_ID: &str = "demo.chrome.close";

/// Stable id of the status bar container.
pub const STATUS_BAR_ID: &str = "demo.chrome.status_bar";

/// Height of the status bar band.
pub const STATUS_BAR_HEIGHT: f32 = size::CONTROL_SM;

/// How many interactive widgets the chrome itself lays down.
///
/// The navbar's menu triggers plus the Close button. The status bar is a
/// read-out, not a control, so it is not counted.
pub const WIDGET_COUNT: usize = data::NAVBAR_TRIGGERS.len() + 1;

/// The top band: the framework navbar with the Close button pinned right.
///
/// The navbar occupies the filling width so its triggers keep the
/// framework's own spacing; the Close button is a shrink-width `Danger`
/// button at the far end.
#[must_use]
pub fn navbar_row(model: &NavBarModel) -> Element<'_, Message> {
    let navbar: Element<'_, Message> = NavBar::new(model)
        .with_palette(Palette::dark())
        .on_toggle_menu(Message::NavbarToggled)
        .into();

    Row::new()
        .width(Length::Fill)
        .align_y(Alignment::Center)
        .spacing(space::XS)
        .push(navbar)
        .push(close_button())
        .into()
}

/// The `Danger` button that ends the program.
///
/// Wired straight to [`iced::exit`] through [`Message::Close`] — the one
/// real behaviour the demo is allowed (PLAN C4).
#[must_use]
pub fn close_button<'a>() -> Element<'a, Message> {
    Button::new(
        ButtonVariant::Danger,
        rung(CLOSE_LABEL, typography::BODY, Palette::dark().text_strong),
        Message::Close,
        CLOSE_FOCUS_ID,
    )
    .palette(Palette::dark())
    .with_tooltip("Close the demo window")
    .into()
}

/// The bottom band: the last demo event and the rendered-widget count.
///
/// `event` is whatever the most recent control reported; `widgets` is the
/// number of demo widgets the panes laid out on this frame.
#[must_use]
pub fn status_bar(event: &str, widgets: usize) -> Element<'_, Message> {
    let palette = Palette::dark();

    let right = container(rung(
        format!("{widgets} widgets rendered"),
        typography::CAPTION,
        palette.text_muted,
    ))
    .width(Length::Fill)
    .align_x(Alignment::End);

    container(
        Row::new()
            .width(Length::Fill)
            .align_y(Alignment::Center)
            .spacing(space::SM)
            .push(rung(event, typography::CAPTION, palette.text_primary))
            .push(right),
    )
    .id(STATUS_BAR_ID)
    .width(Length::Fill)
    .height(Length::Fixed(STATUS_BAR_HEIGHT))
    .padding(iced::Padding::from([0.0, space::SM]))
    .style(move |_theme: &iced::Theme| container::Style {
        background: Some(Background::Color(palette.surface_raised)),
        border: Border {
            color: palette.border_subtle,
            width: shape::BORDER_HAIRLINE,
            radius: 0.0.into(),
        },
        shadow: Shadow::default(),
        text_color: Some(palette.text_primary),
        snap: true,
    })
    .into()
}

#[cfg(test)]
#[path = "chrome_tests.rs"]
mod tests;
