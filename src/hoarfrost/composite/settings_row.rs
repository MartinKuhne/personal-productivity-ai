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
//! §3B.1 prints `Min height 48px` together with `padding 12px 16px`,
//! and the two do not pin the row to `48` — they bound it from below.
//! The section composes the floor itself: `12px` top padding + `20px`
//! `type.body` title + `12px` bottom padding + `4px` border/inset
//! reserve = `48px`, and [`MIN_HEIGHT`] is that sum rather than the
//! literal.
//!
//! The reserve is why the floor and the row's own content differ. A
//! title alone is `20 + 24 = 44`, four pixels short of the floor, and
//! the floor is what closes them; a title with a description is
//! `20 + 16 + 24 = 60`, which is the two-line figure §3B.1 prints with
//! `0px` slack, so the content wins outright and the reserve has
//! already been absorbed into the taller column. [`row_height`]
//! therefore computes the maximum of the floor and the content rather
//! than returning the printed figure, because a row that clamped back
//! down to `48` would clip the description it was told to show.
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
//! # The builder reads the same numbers as the geometry
//!
//! [`bands`] is the pure answer and [`horizontal`] is the widget, and the
//! two are not permitted to be written down twice. Every dimension the
//! builder sets is one the geometry already reads: the card's padding is
//! [`row_padding`], made of [`PADDING_VERTICAL`] over
//! [`PADDING_HORIZONTAL`]; the split is [`COLUMN_GAP`]; and the floor is
//! [`floor_slack`], which is [`MIN_HEIGHT`] minus the padding it cannot
//! reach with words alone. The text column is a [`Column`], so a
//! description lands *under* the title at [`text_column_height`] rather
//! than beside it on one baseline, and the built row measures what
//! [`row_height`] returns instead of `max(title, control)`.
//!
//! Unit tests live in the sibling `settings_row_tests.rs` sidecar.

use crate::token::color::Palette;
use crate::token::{shape, size, space, typography};
use iced::widget::{Column, Container, Row, Space, text};
use iced::{Color, Length, Padding, Rectangle};

/// The `4px` border/inset reserve §3.1's settings-row floor carries on
/// top of its padding and its title line.
///
/// §3B.1 composes the single-line floor as `12 + 20 + 12 + 4`, and the
/// fourth term is this: room for the row's own perimeter stroke and the
/// inset a control sits at inside the card, which the padding alone does
/// not pay for. It is `space.xs`, the smallest padding rung, because
/// that is what the reserve actually has to cover — a `1px` border plus
/// a hair of breathing room, not a gap you read as spacing.
pub const BORDER_INSET_RESERVE: f32 = space::XS;

/// The row's minimum height — §3B.1's `Min height 48px`.
///
/// **A floor, not a height.** §3B.1 prints it as `Min height`, and
/// [`row_height`] treats it as the lower bound of a maximum rather than
/// the row's answer: a title-only row lands exactly on it, and anything
/// taller — a description, a `32px` control — pushes past it. A row
/// that clamped back down to `48` would clip the description it was
/// told to show.
///
/// **Derived from the spec's own composition**, not printed: `12px` top
/// padding + `20px` `type.body` title + `12px` bottom padding + `4px`
/// [`BORDER_INSET_RESERVE`] = `48px`. Adding the parts is what makes
/// the total checkable; a bare `48.0` could drift away from the row it
/// claims to bound and nothing would notice.
pub const MIN_HEIGHT: f32 =
    PADDING_VERTICAL + TITLE_RUNG.line_height + PADDING_VERTICAL + BORDER_INSET_RESERVE;

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

/// The height the row's interior must carry so the padded row never falls
/// below [`MIN_HEIGHT`].
///
/// The floor is paid for by the text column, because the text column is
/// the filling column: whatever it cannot fill with words it fills with
/// the [`BORDER_INSET_RESERVE`] slack the floor already counts. It is
/// derived from [`MIN_HEIGHT`] rather than restated as `24.0` so the
/// builder and [`bands`] answer to one number.
#[must_use]
pub fn interior_floor() -> f32 {
    (MIN_HEIGHT - PADDING_VERTICAL - PADDING_VERTICAL).max(0.0)
}

/// The slack the text column carries so a short row still reaches the floor.
///
/// A title alone is `20` against a `24` interior floor, so it carries
/// `4`; a title with a description is already `36`, so it carries
/// nothing. A reserve, never a cap — a description that wraps simply
/// grows past it, and the row grows with it.
#[must_use]
pub fn floor_slack(has_description: bool) -> f32 {
    (interior_floor() - text_column_height(has_description)).max(0.0)
}

/// The row's declared padding pair, built from the two constants so the
/// builder cannot invent a third figure.
#[must_use]
pub fn row_padding() -> Padding {
    Padding::ZERO
        .vertical(PADDING_VERTICAL)
        .horizontal(PADDING_HORIZONTAL)
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

/// The filling text side of the row: the title with its description
/// stacked *beneath* it.
///
/// A [`Column`], not a [`Row`] — the whole point of the pair is that the
/// explanation sits under the name. The two lines carry no gap between
/// them because [`text_column_height`] models them as `20 + 16`; spacing
/// here would put the built widget out of step with the figure the row is
/// sized from.
#[must_use]
pub fn text_column<'a, Message>(
    title: &'a str,
    description: Option<&'a str>,
    style: &SettingsRowStyle,
) -> Column<'a, Message>
where
    Message: 'a,
{
    let column = Column::new()
        .width(Length::Fill)
        .push(title_line(title, style));

    match present(description) {
        Some(description) => column.push(description_line(description, style)),
        None => column,
    }
}

/// The title's line — `type.body` at `Medium`, in the strong voice.
fn title_line<'a, Message>(title: &'a str, style: &SettingsRowStyle) -> iced::Element<'a, Message>
where
    Message: 'a,
{
    text(title.to_string())
        .size(TITLE_RUNG.size)
        .line_height(TITLE_RUNG.iced_line_height())
        .font(iced::Font {
            weight: iced::font::Weight::Medium,
            ..iced::Font::with_name("Fira Sans")
        })
        .color(style.title_colour)
        .into()
}

/// The description's line — `type.caption` at `Regular`, in the muted voice.
fn description_line<'a, Message>(
    description: &'a str,
    style: &SettingsRowStyle,
) -> iced::Element<'a, Message>
where
    Message: 'a,
{
    text(description.to_string())
        .size(DESCRIPTION_RUNG.size)
        .line_height(DESCRIPTION_RUNG.iced_line_height())
        .color(style.description_colour)
        .into()
}

/// The settings row: a filling title/description column and a shrinking
/// control column, laid out to [`bands`] and padded to [`row_padding`].
///
/// The text column fills and the control column shrinks, and the text
/// column carries [`floor_slack`] so a row with nothing tall in it still
/// stands at [`MIN_HEIGHT`]. The height that comes back is [`row_height`]
/// for the shape on show — the floor when nothing else reaches it,
/// otherwise the taller of the text column and the hosted control, plus
/// the padding — and the widths are the ones [`bands`] would reserve for
/// the same control.
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
    let mut text = text_column(title, description, style);

    let slack = floor_slack(description_present(description));
    if slack > 0.0 {
        text = text.push(Space::new().height(Length::Fixed(slack)));
    }

    Row::new()
        .width(Length::Fill)
        .padding(row_padding())
        .spacing(COLUMN_GAP)
        .align_y(iced::alignment::Vertical::Center)
        .push(text)
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
