//! The horizontal settings row — a filling text column beside a shrinking
//! control (`UIF-COMP-001`).
//!
//! The second half of §3B.1. Where [`super::form_field`] answers "how does
//! a label sit above a control", this answers "how does a setting explain
//! itself next to the thing that changes it": a raised card row, the name
//! and the explanation filling the space available, and the control taking
//! exactly the width it needs at the right end.
//!
//! # The floor is a floor, not a height
//!
//! §3B.1 prints `Min height 48px` together with `padding 12px 16px`, and
//! the two do not pin the row to `48` — they bound it from below. A title
//! alone is `20 + 24 = 44`, so the floor holds; a title with a
//! description is `20 + 16 + 24 = 60`, so the content wins. [`row_height`]
//! computes the maximum rather than returning the printed figure, because
//! a row that clamped to `48` would clip the description it was told to
//! show.
//!
//! The control participates the same way: it is a competing *content*
//! height, not a decoration. A `32px` switch beside a title-only row makes
//! the row `56` tall; the same switch beside a title-and-description row
//! changes nothing, because the text column is already taller. That is the
//! whole rule, and it is why the control's height is an argument rather
//! than a constant.
//!
//! # Which column shrinks, and why it cannot be the other way round
//!
//! The text column is `Fill` and the control column is `Shrink` (§3B.1).
//! Reversing them would let a long setting name push a `ToggleSwitch` off
//! the edge of the card, and would make the control's position a function
//! of the longest string in the list — the same class of defect
//! `UIF-PRIN-003` exists to prevent, arriving from content instead of
//! from state. [`bands`] therefore reserves the trailing column from the
//! control's own width and gives the remainder to the text, and the text
//! column's width is never allowed to go negative when a host passes a
//! control wider than the row.
//!
//! Unit tests live in the sibling `settings_row_tests.rs` sidecar.

use crate::token::color::Palette;
use crate::token::{shape, size, space, typography};
use iced::widget::{Container, Row, text};
use iced::{Color, Length, Rectangle};

/// The row's minimum height — §3B.1's `Min height 48px`.
///
/// **A Surface Geometry Constant, not a token read.** §2.2 carries
/// `size.control.md = 32` and `size.control.lg = 36`; nothing on the
/// ladder is `48`, and no combination of a rung with the printed `12px`
/// vertical padding produces it from a single line of text. The figure is
/// the row's own bound and is registered against §3B.1.
pub const MIN_HEIGHT: f32 = 48.0;

/// The row's vertical padding — the `12px` of §3B.1's `12px 16px`.
pub const PADDING_VERTICAL: f32 = space::MD;

/// The row's horizontal padding — the `16px` of §3B.1's `12px 16px`.
pub const PADDING_HORIZONTAL: f32 = space::LG;

/// The minimum gap between the text column and the control column —
/// §3B.1's `min gap 16px`.
pub const COLUMN_GAP: f32 = space::LG;

/// The row's resting fill — `surface.raised`.
pub const BACKGROUND_TOKEN: &str = "surface.raised";

/// The row's resting border — `1px border.subtle`.
pub const BORDER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// The row's corner radius — `radius.widget`.
pub const RADIUS: f32 = shape::RADIUS_WIDGET;

/// The title's rung — §3B.1's `14px Medium`, which is `type.body` at its
/// emphasis weight.
pub const TITLE_RUNG: &typography::TypeRung = &typography::BODY;

/// The description's rung — §3B.1's `12px Regular`, `type.caption`.
pub const DESCRIPTION_RUNG: &typography::TypeRung = &typography::CAPTION;

/// The control height a row is sampled at when the host has no opinion.
pub const DEFAULT_CONTROL_HEIGHT: f32 = size::CONTROL_MD;

/// Whether a description is content.
///
/// A `Some("  ")` is the same fact as `None`, and the row's height is a
/// function of whether a description exists — so a blank string left in
/// place would reserve a line of card for nothing. Hosts compute the flag
/// through here rather than passing `option.is_some()`.
#[must_use]
pub fn description_present(description: Option<&str>) -> bool {
    present(description).is_some()
}

/// The text column's height for one or two lines.
#[must_use]
pub fn text_column_height(has_description: bool) -> f32 {
    let title = TITLE_RUNG.line_height;
    if has_description {
        title + DESCRIPTION_RUNG.line_height
    } else {
        title
    }
}

/// The row's height for a given content shape.
///
/// The greater of the printed floor and the content's own height, where
/// the content is the taller of the text column and the hosted control.
/// See the module docs for why the floor does not pin the row.
#[must_use]
pub fn row_height(has_description: bool, control_height: f32) -> f32 {
    let content = text_column_height(has_description).max(control_height);
    (content + PADDING_VERTICAL + PADDING_VERTICAL).max(MIN_HEIGHT)
}

/// The two columns a settings row divides its bounds into.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SettingsBands {
    /// The filling text column.
    pub text: Rectangle,
    /// The shrinking control column.
    pub control: Rectangle,
}

/// Divide a row's bounds into the text column and the control column.
///
/// The control column is reserved first, from the control's own width, and
/// the text column takes what is left. A control wider than the row's
/// interior leaves the text column at zero width rather than a negative
/// one, so a pathological host gets an unreadable row instead of a
/// wrapping-overflow panic.
#[must_use]
pub fn bands(bounds: Rectangle, control_width: f32) -> SettingsBands {
    let interior_x = bounds.x + PADDING_HORIZONTAL;
    let interior_width = (bounds.width - PADDING_HORIZONTAL - PADDING_HORIZONTAL).max(0.0);
    let interior_y = bounds.y + PADDING_VERTICAL;
    let interior_height = (bounds.height - PADDING_VERTICAL - PADDING_VERTICAL).max(0.0);

    let reserved = control_width.min(interior_width);
    let text_width = (interior_width - reserved - COLUMN_GAP).max(0.0);

    SettingsBands {
        text: Rectangle {
            x: interior_x,
            y: interior_y,
            width: text_width,
            height: interior_height,
        },
        control: Rectangle {
            x: interior_x + interior_width - reserved,
            y: interior_y,
            width: reserved,
            height: interior_height,
        },
    }
}

/// The row's resolved paint and text colours.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SettingsRowStyle {
    /// The card fill — `surface.raised`.
    pub background: Color,
    /// The perimeter stroke — `border.subtle`.
    pub border_color: Color,
    /// The stroke weight — `1px`.
    pub border_width: f32,
    /// The corner radius — `radius.widget`.
    pub radius: f32,
    /// The title's colour — `text.strong`, or `text.disabled` when the
    /// setting cannot be changed.
    pub title_colour: Color,
    /// The description's colour — `text.muted`, or `text.disabled` when
    /// the setting cannot be changed.
    pub description_colour: Color,
}

/// Resolve the row's paint.
///
/// The row's own chrome never changes with state: §3B.1 gives it one fill
/// and one border, and the *control* in the right column is what reacts to
/// the pointer. Only the words dim, and they dim together — a row whose
/// title stayed bright while its description greyed out reads as a
/// different setting rather than an unavailable one.
#[must_use]
pub fn resolve_style(disabled: bool, palette: &Palette) -> SettingsRowStyle {
    if disabled {
        SettingsRowStyle {
            background: palette.surface_raised,
            border_color: palette.border_subtle,
            border_width: BORDER_WIDTH,
            radius: RADIUS,
            title_colour: palette.text_disabled,
            description_colour: palette.text_disabled,
        }
    } else {
        SettingsRowStyle {
            background: palette.surface_raised,
            border_color: palette.border_subtle,
            border_width: BORDER_WIDTH,
            radius: RADIUS,
            title_colour: palette.text_strong,
            description_colour: palette.text_muted,
        }
    }
}

/// The row's painted card layer.
#[must_use]
pub fn card_layer(
    bounds: Rectangle,
    style: &SettingsRowStyle,
) -> crate::widget::button_widget::Layer {
    crate::widget::button_widget::Layer {
        bounds,
        fill: style.background,
        border: iced::Border {
            color: style.border_color,
            width: style.border_width,
            radius: style.radius.into(),
        },
    }
}

/// The settings row: a filling title/description column and a shrinking
/// control column, laid out to [`bands`].
#[must_use]
pub fn horizontal<'a, Message>(
    title: &'a str,
    description: Option<&'a str>,
    control: iced::Element<'a, Message>,
    style: &SettingsRowStyle,
) -> iced::Element<'a, Message>
where
    Message: 'a,
{
    let mut column = Row::new()
        .width(Length::Fill)
        .align_y(iced::alignment::Vertical::Center);

    column = column.push(
        text(title.to_string())
            .size(TITLE_RUNG.size)
            .line_height(TITLE_RUNG.line_height)
            .font(TITLE_RUNG.font(typography::FAMILY_SANS, typography::Weight::Medium))
            .color(style.title_colour),
    );

    if let Some(description) = present(description) {
        column = column.push(
            text(description.to_string())
                .size(DESCRIPTION_RUNG.size)
                .line_height(DESCRIPTION_RUNG.line_height)
                .font(DESCRIPTION_RUNG.font(typography::FAMILY_SANS, typography::Weight::Regular))
                .color(style.description_colour),
        );
    }

    Row::new()
        .width(Length::Fill)
        .spacing(COLUMN_GAP)
        .align_y(iced::alignment::Vertical::Center)
        .push(Container::new(column).width(Length::Fill))
        .push(Container::new(control).width(Length::Shrink))
        .into()
}

/// Whether an optional string carries anything.
///
/// A `Some("")` description is the same fact as no description, and the
/// row's height is a function of whether a description exists — so an
/// empty string left in place would reserve a line for nothing.
fn present(value: Option<&str>) -> Option<&str> {
    value.filter(|inner| !inner.trim().is_empty())
}

#[cfg(test)]
#[path = "settings_row_tests.rs"]
mod settings_row_tests;
