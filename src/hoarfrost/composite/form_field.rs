//! The stacked form field — label, control, reserved caption (`UIF-COMP-001`).
//!
//! §3B.1 fixes this field's vertical rhythm to the pixel: a `20px` header
//! row, `space.xs`, a `32px` control slot, `space.xs`, and a `16px`
//! caption slot that is *reserved* whether or not anything is written in
//! it. The reason the spec reserves it is the requirement itself —
//! `UIF-PRIN-003` forbids a state change from moving content, and a
//! validation message that appears by pushing the next field down is the
//! canonical violation.
//!
//! # The zero-shift proof is structural, not numerical
//!
//! A test that asserts the field is `76px` tall in both the valid and the
//! invalid case proves the current numbers agree. It does not prove the
//! next change cannot break them, and it does not prove *why*.
//!
//! [`bands`] therefore takes the field's bounds and nothing else. There is
//! no parameter for the label, none for the helper text, none for the
//! validation error, and none for whether the field is valid. The caption
//! band is `CAPTION_SLOT_HEIGHT` because that is the only height the
//! function can produce, not because a branch was taken that happened to
//! keep it. An empty slot, a helper sentence and a validation error are
//! the same rectangle, and no future edit can make them differ without
//! deleting the parameter that is currently absent — which is a change a
//! reviewer can see.
//!
//! The same argument is how [`crate::widget::text_input`] proves its
//! trailing slot is reserved: [`text_area_bounds`][crate::widget::text_input::text_area_bounds]
//! is a function of the outer bounds and the input kind only.
//!
//! # The control slot holds any control, and cannot tell which
//!
//! [`bands`] returns the control slot as a rectangle. It does not know
//! whether a text input, a switch or a combo box will be placed in it, and
//! it does not care — which is the property §3B.1 needs and that a
//! field-wrapper written per-control would not have. The sidecar places
//! three different atom kinds through [`stacked`] and asserts the field's
//! geometry is byte-identical across them, so "generic" is a measured
//! claim rather than a description of intent.
//!
//! # What the spec leaves open, and what this module decided
//!
//! §3B.1 says the caption slot renders helper text when valid and a
//! validation error when invalid. It does not say what happens when the
//! text is longer than one line, and the two available answers are not
//! symmetric: wrapping is the only one that changes the slot's height,
//! which is the exact thing the same sentence forbids. This module
//! therefore pins the band to a single line and clips horizontally
//! ([`RESERVED_BAND_OVERFLOW`]), and records the silence rather than
//! inventing a wrap mode nothing asked for.
//!
//! It also does not say what an *invalid* field with **no** error message
//! shows. Rendering the helper there would be wrong — the field is not
//! valid, so the helper's calm colour would contradict the red border on
//! the control above — and inventing a generic "invalid" string would put
//! words in the framework the spec never wrote. The band stays empty and
//! reserved: the control carries the state, the slot carries nothing.
//!
//! Unit tests live in the sibling `form_field_tests.rs` sidecar.

use crate::token::color::Palette;
use crate::token::{shape, size, space, typography};
use iced::{Color, Rectangle};

// ---------------------------------------------------------------------------
// The fixed rhythm
// ---------------------------------------------------------------------------

/// The header row's height — §3B.1's `height: 20px`.
///
/// **A Surface Geometry Constant, not a token read.** No rung in §2.2 or
/// §2.4 carries `20.0` as a row height: `type.ui_compact`'s line height
/// is `18.0` and `type.body`'s is `20.0`, but the header is specified at
/// 13px, so the printed row is two pixels taller than the type inside it.
/// The surplus is real — it is what keeps the `*` required marker and a
/// right-aligned hint from touching the rule below — so the figure is
/// registered against §3B.1 rather than quietly composed from a rung that
/// would put the label on the wrong line.
pub const HEADER_HEIGHT: f32 = 20.0;

/// The gap between every band — `space.xs` (§3B.1, twice).
pub const BAND_GAP: f32 = space::XS;

/// The control slot's height — `size.control.md`.
///
/// The slot, not the control: a host that places a `28px` compact control
/// in a standard field still gets a `32px` band, with the control centred
/// in it. That is what keeps a field's height a property of the field
/// rather than of whichever rung its control happens to use.
pub const CONTROL_SLOT_HEIGHT: f32 = size::CONTROL_MD;

/// The reserved caption/validation slot's height — `type.caption`'s line.
///
/// §3B.1 prints `height: 16px` and names `type.caption` in the same
/// breath, and `type.caption`'s line height *is* `16.0`, so the band is
/// exactly one caption line. Reading the token rather than the literal is
/// what makes that identity checkable: a retune of the caption rung moves
/// the band with it.
pub const CAPTION_SLOT_HEIGHT: f32 = typography::CAPTION.line_height;

/// The whole stacked field's height: header + gap + control + gap + caption.
///
/// Derived rather than printed so the total cannot disagree with its parts.
/// Every term is fixed and none of them is content, which is the whole
/// argument for `0px` layout shift in one expression.
pub const STACKED_HEIGHT: f32 =
    HEADER_HEIGHT + BAND_GAP + CONTROL_SLOT_HEIGHT + BAND_GAP + CAPTION_SLOT_HEIGHT;

/// The field's total height.
#[must_use]
pub const fn stacked_height() -> f32 {
    STACKED_HEIGHT
}

/// The three bands a stacked field divides its bounds into.
///
/// The bands are contiguous and gap-separated, and their combined height is
/// [`STACKED_HEIGHT`] regardless of what any of them contains.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldBands {
    /// The label / required marker / hint row.
    pub header: Rectangle,
    /// The slot the hosted control sits in.
    pub control: Rectangle,
    /// The reserved caption / validation band.
    pub caption: Rectangle,
}

/// Divide a field's bounds into its three bands.
///
/// **A function of the bounds alone.** No label, no helper text, no
/// validation error and no validity flag appears in this signature, and
/// that absence — not any assertion about the numbers — is the proof that
/// a validation transition cannot move the control. See the module docs.
#[must_use]
pub fn bands(bounds: Rectangle) -> FieldBands {
    let header = Rectangle {
        x: bounds.x,
        y: bounds.y,
        width: bounds.width,
        height: HEADER_HEIGHT,
    };
    let control_y = bounds.y + HEADER_HEIGHT + BAND_GAP;
    let control = Rectangle {
        x: bounds.x,
        y: control_y,
        width: bounds.width,
        height: CONTROL_SLOT_HEIGHT,
    };
    let caption = Rectangle {
        x: bounds.x,
        y: control_y + CONTROL_SLOT_HEIGHT + BAND_GAP,
        width: bounds.width,
        height: CAPTION_SLOT_HEIGHT,
    };

    FieldBands {
        header,
        control,
        caption,
    }
}

// ---------------------------------------------------------------------------
// The header
// ---------------------------------------------------------------------------

/// The required-field marker §3B.1 puts after the label.
pub const REQUIRED_MARKER: char = '*';

/// The tooltip the required marker owes (§3B.1: `tooltip "Required"`).
pub const REQUIRED_TOOLTIP: &str = "Required";

/// The leading glyph a validation caption carries (§3B.1: `⚠`).
pub const WARNING_GLYPH: char = '\u{26A0}';

/// The header row's resolved parts.
///
/// The label and the hint are two different voices on one row: the label
/// is the field's name and the hint is a comment on it, so they resolve to
/// different tokens and never to a single colour the caller picks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeaderParts<'a> {
    /// The field's own name.
    pub label: &'a str,
    /// The label's colour — `text.primary`.
    pub label_colour: Color,
    /// The required marker, absent on an optional field.
    pub marker: Option<char>,
    /// The marker's colour — `status.danger.text`.
    pub marker_colour: Color,
    /// The right-aligned hint, absent when there is nothing to hint.
    pub hint: Option<&'a str>,
    /// The hint's colour — `text.muted`.
    pub hint_colour: Color,
}

/// Resolve the header row's parts.
///
/// The marker is `status.danger.text` because §3B.1 says so, and that is
/// one of the few places in the spec where the danger colour is load-bearing
/// on a *non-error* element: an asterisk is a legal notice, not a fault.
/// The sidecar measures it against every surface a field is mounted on so
/// the pairing is vouched for rather than inherited.
#[must_use]
pub fn resolve_header<'a>(
    label: &'a str,
    required: bool,
    hint: Option<&'a str>,
    palette: &Palette,
) -> HeaderParts<'a> {
    HeaderParts {
        label,
        label_colour: palette.text_primary,
        marker: required.then_some(REQUIRED_MARKER),
        marker_colour: palette.status_danger_text,
        hint: present(hint),
        hint_colour: palette.text_muted,
    }
}

// ---------------------------------------------------------------------------
// The caption band
// ---------------------------------------------------------------------------

/// Which voice the reserved band is carrying, if any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptionKind {
    /// Nothing to say. The band is still there and still `16px`.
    Empty,
    /// `helper_text` in `text.muted` — the field is valid.
    Helper,
    /// `validation_error` in `status.danger.text` with a leading `⚠`.
    Error,
}

impl CaptionKind {
    /// Whether this kind is the one that reserves the band for a fault.
    #[must_use]
    pub const fn is_error(self) -> bool {
        matches!(self, Self::Error)
    }
}

/// The reserved band's resolved contents.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaptionParts<'a> {
    /// Which voice is speaking.
    pub kind: CaptionKind,
    /// The leading glyph — `⚠` on an error, nothing otherwise.
    pub glyph: Option<char>,
    /// The words, absent when the band is empty.
    pub text: Option<&'a str>,
    /// The colour the words and glyph are painted in.
    pub colour: Color,
}

/// Resolve what the reserved caption band shows.
///
/// The rule is §3B.1's, read as a decision tree with the two holes the
/// spec left filled in explicitly:
///
/// 1. Invalid **with** an error message → the message, in danger, with
///    the `⚠`.
/// 2. Invalid **without** one → empty. The control above already carries
///    the fault; the band invents nothing.
/// 3. Valid with a helper → the helper, muted.
/// 4. Valid without one → empty.
///
/// In all four the band is `CAPTION_SLOT_HEIGHT` tall, because
/// [`bands`] never saw this value.
#[must_use]
pub fn resolve_caption<'a>(
    valid: bool,
    helper: Option<&'a str>,
    error: Option<&'a str>,
    palette: &Palette,
) -> CaptionParts<'a> {
    if !valid {
        return match present(error) {
            Some(message) => CaptionParts {
                kind: CaptionKind::Error,
                glyph: Some(WARNING_GLYPH),
                text: Some(message),
                colour: palette.status_danger_text,
            },
            None => CaptionParts {
                kind: CaptionKind::Empty,
                glyph: None,
                text: None,
                colour: palette.text_muted,
            },
        };
    }

    match present(helper) {
        Some(message) => CaptionParts {
            kind: CaptionKind::Helper,
            glyph: None,
            text: Some(message),
            colour: palette.text_muted,
        },
        None => CaptionParts {
            kind: CaptionKind::Empty,
            glyph: None,
            text: None,
            colour: palette.text_muted,
        },
    }
}

/// How text that exceeds a reserved band's width is handled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverflowPolicy {
    /// One line, clipped at the band's width.
    Clip,
    /// More lines, growing the band.
    Wrap,
}

/// The policy every reserved band here uses — and the reason it cannot be
/// the other one.
///
/// §3B.1 fixes the caption at `16px` *and* requires `0px` layout shift.
/// Wrapping is the only overflow mode that grows a band, so choosing it
/// would break the second requirement to satisfy the first. Clipping keeps
/// both. The spec never states this, which is a gap in §3B.1 rather than a
/// licence to pick either answer: the alternative is recorded here so the
/// decision is attributable.
pub const RESERVED_BAND_OVERFLOW: OverflowPolicy = OverflowPolicy::Clip;

/// A band's height under the overflow policy.
///
/// Total by construction: the policy cannot produce a height that differs
/// from the reservation, which is the invariant stated as a function rather
/// than asserted at a call site.
#[must_use]
pub const fn reserved_band_height(reserved: f32, policy: OverflowPolicy) -> f32 {
    match policy {
        OverflowPolicy::Clip | OverflowPolicy::Wrap => reserved,
    }
}

// ---------------------------------------------------------------------------
// The widget
// ---------------------------------------------------------------------------

use crate::widget::button_widget::Layer;
use iced::widget::{Column, Container, Row, text};
use iced::{Length, Padding};

/// A control slot's painted reservation marker.
///
/// The field itself paints nothing behind its control — the hosted control
/// brings its own chrome — so this exists for the gallery and for any host
/// that wants to *show* the reserved band rather than only honour it. It
/// is deliberately not used by [`stacked`].
#[must_use]
pub fn slot_marker(bounds: Rectangle, colour: Color) -> Layer {
    Layer {
        bounds,
        fill: Color::TRANSPARENT,
        border: iced::Border {
            color: colour,
            width: shape::BORDER_HAIRLINE,
            radius: shape::RADIUS_WIDGET.into(),
        },
    }
}

/// The stacked field: a header row, the hosted control, and the reserved
/// caption band, laid out to [`bands`] and nothing else.
///
/// The three bands are fixed-height containers separated by
/// [`BAND_GAP`], so the control cannot be moved by a caption appearing
/// and the caption cannot be moved by the control's own height — a `28px`
/// control in a `32px` slot is centred, not top-aligned, and the band
/// below it does not notice.
#[must_use]
pub fn stacked<'a, Message>(
    header: HeaderParts<'a>,
    control: iced::Element<'a, Message>,
    caption: CaptionParts<'a>,
) -> iced::Element<'a, Message>
where
    Message: 'a,
{
    Column::with_children(vec![
        header_row(header)
            .height(Length::Fixed(HEADER_HEIGHT))
            .into(),
        Container::new(control)
            .height(Length::Fixed(CONTROL_SLOT_HEIGHT))
            .align_y(iced::alignment::Vertical::Center)
            .into(),
        caption_row(caption)
            .height(Length::Fixed(CAPTION_SLOT_HEIGHT))
            .align_y(iced::alignment::Vertical::Center)
            .into(),
    ])
    .spacing(BAND_GAP)
    .into()
}

/// The header row: label, marker, and a right-aligned hint.
fn header_row<'a, Message>(parts: HeaderParts<'a>) -> Container<'a, Message>
where
    Message: 'a,
{
    let mut label = Row::with_children(vec![text_part(
        parts.label_colour,
        typography::UI_COMPACT.size,
        typography::UI_COMPACT.iced_line_height(),
        iced::Font {
            weight: iced::font::Weight::Medium,
            ..iced::Font::with_name("Fira Sans")
        },
        parts.label.to_string(),
    )]);

    if let Some(marker) = parts.marker {
        label = label.push(text_part(
            parts.marker_colour,
            typography::UI_COMPACT.size,
            typography::UI_COMPACT.iced_line_height(),
            iced::Font {
                weight: iced::font::Weight::Medium,
                ..iced::Font::with_name("Fira Sans")
            },
            marker.to_string(),
        ));
    }

    let mut row = Row::new()
        .width(Length::Fill)
        .align_y(iced::alignment::Vertical::Center);
    row = row.push(label);
    if let Some(hint) = parts.hint {
        row = row.push(
            Container::new(text_part(
                parts.hint_colour,
                typography::CAPTION.size,
                typography::CAPTION.iced_line_height(),
                iced::Font::with_name("Fira Sans"),
                hint.to_string(),
            ))
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Right),
        );
    }

    Container::new(row)
}

/// The caption band's row: the optional glyph and the optional words.
fn caption_row<'a, Message>(parts: CaptionParts<'a>) -> Container<'a, Message>
where
    Message: 'a,
{
    let mut row = Row::new()
        .width(Length::Fill)
        .spacing(space::XXS)
        .align_y(iced::alignment::Vertical::Center);

    if let Some(text_value) = parts.rendered() {
        row = row.push(text_part(
            parts.colour,
            typography::CAPTION.size,
            typography::CAPTION.iced_line_height(),
            iced::Font::with_name("Fira Sans"),
            text_value,
        ));
    }

    Container::new(row).padding(Padding::new(0.0))
}

/// One piece of text on a rung, painted in a resolved colour.
///
/// `line_height` is an `iced::LineHeight`, deliberately not a bare `f32`.
/// A bare `f32` at this boundary would reach iced through
/// `From<f32> for LineHeight`, which means *multiple of the font size* —
/// so a caller handing over a rung's pixel line height of `16.0` would
/// get 16 × size pixels per line and the glyph would be clipped out of
/// its band. See
/// [`TypeRung::iced_line_height`](typography::TypeRung::iced_line_height),
/// which is what every caller here passes.
fn text_part<'a, Message>(
    colour: Color,
    size: f32,
    resolved_line_height: iced::advanced::text::LineHeight,
    font: iced::Font,
    content: String,
) -> iced::Element<'a, Message>
where
    Message: 'a,
{
    text(content)
        .size(size)
        .line_height(resolved_line_height)
        .font(font)
        .color(colour)
        .into()
}

/// Whether an optional string carries anything.
///
/// A `Some("")` helper is the same fact as no helper at all, and treating
/// it as present would give a field a caption band that renders an empty
/// element and reports `CaptionKind::Helper` — a state no host means and no
/// test would catch from the outside.
fn present(text_value: Option<&str>) -> Option<&str> {
    text_value.filter(|value| !value.trim().is_empty())
}

impl<'a> CaptionParts<'a> {
    /// The band's rendered string: the glyph and the words, when there are
    /// any.
    #[must_use]
    pub fn rendered(&self) -> Option<String> {
        match (self.glyph, self.text) {
            (Some(glyph), Some(words)) => Some(format!("{glyph} {words}")),
            (None, Some(words)) => Some(words.to_string()),
            (_, None) => None,
        }
    }
}

#[cfg(test)]
#[path = "form_field_tests.rs"]
mod form_field_tests;
