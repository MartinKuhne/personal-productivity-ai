//! The `ProgressBar` and `Spinner` paint resolvers and geometry
//! (`UIF-ATOM-009`, §3.9).
//!
//! Like [`badge`](crate::widget::badge) and
//! [`toggle`](crate::widget::toggle), this module resolves paint rather
//! than building a widget. Every function here is pure, and the two
//! values that could have smuggled nondeterminism into a progress
//! indicator — time and the OS motion setting — arrive as **arguments**.
//!
//! # No clock, and no OS setting
//!
//! A progress bar is the widget most tempting to make temporal: it
//! *moves*, so the obvious implementation asks the clock where it is.
//! That implementation cannot be snapshotted. So the indeterminate
//! segment's position enters through
//! [`indeterminate_segment`] as a `phase` the host supplies, and
//! §3.9's "static striped segment when OS `reduced_motion` is enabled"
//! enters through the same door as a `bool`. This crate never reads
//! `Instant::now()` and never queries the platform for its motion
//! preference; the host, which owns both, decides what the bar looks like
//! this frame.
//!
//! # The bar
//!
//! A `4px` ([`size::PROGRESS_BAR_HEIGHT`]) track on `surface.active` at
//! `radius.pill`, with a fill whose colour §3.9 ties to the status:
//!
//! | Status | Fill | Recomputed on `surface.active` | §3.9 prints |
//! |---|---|---|---|
//! | `Active` | `accent.border` | `4.7922:1` | `4.79:1` ✓ |
//! | `Paused` | `status.warning` | `4.9256:1` | `4.93:1` ✓ |
//! | `Error` | `status.danger.text` | `4.7223:1` | `4.72:1` ✓ |
//!
//! All three agree with the spec's printed column, which is worth saying
//! out loud: most of §3.8's figures did not survive recomputation, and
//! these did.
//!
//! The fill classifies as [`ChromeClass::Judged`] rather than
//! [`ChromeClass::CoincidentWithFill`]. It is not coincident with its own
//! fill — the fill *is* the thing being measured — and it is not a
//! resting divider either: a progress fill is the indicator, and the
//! separation it owes the track is the separation the whole widget exists
//! to make. `3.0:1` is the floor it is held to, and all three statuses
//! clear it by more than `1.7:1`.
//!
//! # The indeterminate segment
//!
//! §3.9 fixes the segment at `35%` of the track's width. Its position is
//! `phase` mapped across the track so that the segment enters from the
//! left edge and exits past the right, which is what makes the loop read
//! as travel rather than as a jump back to the start.
//!
//! Under injected reduced motion the segment stops travelling and becomes
//! a **static striped** block. The stripe period is not a spec number —
//! §3.9 says "striped" and stops there — so it is a private rendering
//! constant (`STRIPE_PERIOD`) rather than a token, and the gap is
//! recorded rather than papered over.
//!
//! # The spinner
//!
//! `14px` ([`size::SPINNER_SM`]) or `20px` ([`size::SPINNER_MD`]) at a
//! `2px` stroke, `border.subtle` track, `accent.text` arc — or
//! `text.strong` when the spinner sits inside a solid `Primary` or
//! `Danger` button, where the accent would disappear into the fill.
//!
//! The `2px` stroke has **no token of its own**. §2.3's closed stroke set
//! contains exactly two weights, `border.hairline` (`1`) and
//! `border.focus` (`2`), and neither is semantically a spinner stroke —
//! one is a divider, the other a keyboard ring. [`SPINNER_STROKE`] takes
//! the value from `border.focus` because the framework forbids a raw
//! literal and `2` is the only `2` the closed set offers. The mismatch is
//! a spec gap, not a licence to invent a token.
//!
//! The arc's sweep is likewise unspecified. [`SPINNER_ARC_FRACTION`] is
//! set to three-quarters of a turn, which is the shortest sweep that
//! still reads as rotation rather than as a wobbling tick.
//!
//! Unit tests live in the sibling `progress_tests.rs` sidecar.

use crate::token::color::Palette;
use crate::token::contrast::ChromeClass;
use crate::token::shape;
use crate::token::size;
use crate::widget::button_widget::Layer;
use crate::widget::progress_model::{ProgressModel, ProgressStatus};
use iced::{Border, Color, Point, Rectangle};

/// The bar's track thickness — §3.9's `size.progress.bar_height` (`4px`).
pub const TRACK_HEIGHT: f32 = size::PROGRESS_BAR_HEIGHT;

/// The bar's corner radius — `radius.pill`, so a `4px` track reads as a
/// capsule rather than as a rectangle with cut corners.
pub const TRACK_RADIUS: f32 = shape::RADIUS_PILL;

/// The indeterminate segment's width as a fraction of the track — §3.9's
/// `35%`.
///
/// A fraction rather than a size because the segment is defined *relative
/// to the track*, and a track has no fixed width. Naming it as a ratio
/// also keeps it out of the token layer, where a percentage would not
/// belong.
pub const INDETERMINATE_SEGMENT_FRACTION: f32 = 0.35;

/// The spinner's stroke width — §3.9's `2px`.
///
/// Bound to [`shape::BORDER_FOCUS`] because §2.3's closed stroke set has
/// no spinner weight; see the [module docs](self) for why that is a gap
/// rather than a solution.
pub const SPINNER_STROKE: f32 = shape::BORDER_FOCUS;

/// The small spinner's diameter — `size.spinner.sm`.
pub const SPINNER_SMALL: f32 = size::SPINNER_SM;

/// The medium spinner's diameter — `size.spinner.md`.
pub const SPINNER_MEDIUM: f32 = size::SPINNER_MD;

/// The active arc's sweep as a fraction of a full turn.
///
/// §3.9 says "active arc" without fixing how much of the circle it
/// covers. Three-quarters is the choice: less than that and the arc reads
/// as a tick wobbling rather than a ring turning.
pub const SPINNER_ARC_FRACTION: f32 = 0.75;

/// The trailing stop button's edge length — §3.9's `24 × 24px` `[×]`.
///
/// Equal to [`size::HIT_TARGET_MIN`] because it *is* a hit target: the
/// one part of §3.9 that is a real control. The bar above it is not.
pub const STOP_BUTTON_SIZE: f32 = size::HIT_TARGET_MIN;

/// The stripe period of the reduced-motion static segment, in logical px.
///
/// **Not a token.** §3.9 asks for a "static striped segment" and gives
/// no period, so there is no spec number to bind to and inventing a
/// `size.progress.stripe` would put a figure in the token layer that
/// `UIF-TOK-002` does not define. Private, and named so the gap is
/// visible at the call site.
const STRIPE_PERIOD: f32 = 8.0;

/// A resolved progress bar: the track and the fill it carries.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProgressStyle {
    /// The track colour — `surface.active`.
    pub track: Color,
    /// The fill colour for the model's status.
    pub fill: Color,
    /// The track thickness.
    pub height: f32,
    /// The corner radius.
    pub radius: f32,
}

/// A resolved indeterminate segment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SegmentStyle {
    /// The segment's fill — the status colour.
    pub fill: Color,
    /// The segment's width as a fraction of the track.
    pub width_fraction: f32,
    /// The segment's left edge as a fraction of the track, in
    /// `-INDETERMINATE_SEGMENT_FRACTION..=1.0`.
    ///
    /// Negative at the start of the travel so the segment slides *in*
    /// rather than appearing at the left edge already whole.
    pub offset_fraction: f32,
    /// Whether the segment is drawn as static stripes rather than as a
    /// travelling block.
    pub striped: bool,
}

/// Resolve the track-and-fill paint for one model (`UIF-ATOM-009`).
///
/// A pure function of `(model, palette)`. The mode does not appear in
/// the result: whether the bar is determinate decides *how much* fill is
/// drawn, not what the fill is.
#[must_use]
pub fn resolve_style(model: &ProgressModel, palette: &Palette) -> ProgressStyle {
    ProgressStyle {
        track: palette.surface_active,
        fill: fill_colour(model.status(), palette),
        height: TRACK_HEIGHT,
        radius: TRACK_RADIUS,
    }
}

/// The fill colour §3.9 assigns to a status.
///
/// Three statuses, three colours, and the mapping is the whole of
/// §3.9's status semantics for the bar. Note that `Paused` takes
/// `status.warning` — the spec's choice, and worth reading carefully:
/// paused work is not *wrong*, it is work the user should notice has
/// stopped.
#[must_use]
pub const fn fill_colour(status: ProgressStatus, palette: &Palette) -> Color {
    match status {
        ProgressStatus::Active => palette.accent_border,
        ProgressStatus::Paused => palette.status_warning,
        ProgressStatus::Error => palette.status_danger_text,
    }
}

/// Resolve the indeterminate segment for an injected `phase`.
///
/// `phase` is any `f32`; it is wrapped into `0.0..=1.0` so a host can
/// advance it without tracking the loop. The segment's left edge runs
/// from `-0.35` to `1.0`, which is the travel that makes the block
/// enter and leave the track instead of snapping back.
///
/// `reduced_motion` is the host's answer to the OS setting, not a read
/// of it. When `true` the segment is static and striped, and `phase`
/// stops mattering — that is the point of the mode.
#[must_use]
pub fn indeterminate_segment(
    status: ProgressStatus,
    phase: f32,
    reduced_motion: bool,
    palette: &Palette,
) -> SegmentStyle {
    let wrapped = wrap_unit(phase);
    let span = 1.0 + INDETERMINATE_SEGMENT_FRACTION;
    SegmentStyle {
        fill: fill_colour(status, palette),
        width_fraction: INDETERMINATE_SEGMENT_FRACTION,
        offset_fraction: if reduced_motion {
            0.0
        } else {
            wrapped * span - INDETERMINATE_SEGMENT_FRACTION
        },
        striped: reduced_motion,
    }
}

/// Resolve a spinner's paint.
///
/// `large` selects `size.spinner.md` over `size.spinner.sm`.
/// `inside_solid` switches the arc from `accent.text` to `text.strong`,
/// which is what §3.9 requires inside a solid `Primary` or `Danger`
/// button: `accent.text` on `accent.fill` is `2.5719:1` and on
/// `status.danger.fill` `2.7460:1`, an arc that is there and not there
/// at the same time. `text.strong` clears both at `4.8020:1` and
/// `5.1270:1`.
#[must_use]
pub fn spinner_style(large: bool, inside_solid: bool, palette: &Palette) -> SpinnerStyle {
    SpinnerStyle {
        diameter: if large { SPINNER_MEDIUM } else { SPINNER_SMALL },
        stroke: SPINNER_STROKE,
        track: palette.border_subtle,
        arc: if inside_solid {
            palette.text_strong
        } else {
            palette.accent_text
        },
    }
}

/// A resolved spinner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpinnerStyle {
    /// The ring's diameter.
    pub diameter: f32,
    /// The stroke width.
    pub stroke: f32,
    /// The full-circle track colour.
    pub track: Color,
    /// The travelling arc colour.
    pub arc: Color,
}

/// The bar's track layer.
#[must_use]
pub fn track_layer(bounds: Rectangle, style: &ProgressStyle) -> Layer {
    Layer {
        bounds,
        fill: style.track,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: shape::RADIUS_PILL.into(),
        },
    }
}

/// The determinate fill layer, or `None` when nothing is complete.
///
/// A zero-fraction fill paints nothing rather than painting a zero-width
/// quad: the headless snapshot should record the absence rather than a
/// rectangle the renderer has to clip away.
#[must_use]
pub fn fill_layer(bounds: Rectangle, style: &ProgressStyle, fraction: f32) -> Option<Layer> {
    if fraction <= 0.0 {
        return None;
    }
    let clamped = if fraction > 1.0 { 1.0 } else { fraction };
    Some(Layer {
        bounds: Rectangle::new(
            bounds.position(),
            iced::Size::new(bounds.width * clamped, bounds.height),
        ),
        fill: style.fill,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: shape::RADIUS_PILL.into(),
        },
    })
}

/// The indeterminate segment layer.
#[must_use]
pub fn segment_layer(bounds: Rectangle, segment: &SegmentStyle) -> Layer {
    let width = bounds.width * segment.width_fraction;
    Layer {
        bounds: Rectangle::new(
            Point::new(bounds.x + bounds.width * segment.offset_fraction, bounds.y),
            iced::Size::new(width, bounds.height),
        ),
        fill: segment.fill,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: shape::RADIUS_PILL.into(),
        },
    }
}

/// Classify the bar's fill under `UIF-TOK-006` rule 3 (`UIF-ATOM-009`).
///
/// The fill is [`ChromeClass::Judged`] for every status. It is not
/// [`ChromeClass::CoincidentWithFill`], because that class describes a
/// *stroke* set to the same token as the fill it surrounds — a border
/// that draws no boundary. Here the fill is the measured thing, against
/// the track behind it, and it owes the ordinary `3.0:1`.
///
/// The spinner's two channels classify differently and
/// [`spinner_arc_chrome_class`] is the honest answer for the arc:
/// `border.subtle`'s track is a [`ChromeClass::RestingDivider`] and
/// exempt, while the arc is `Judged`.
#[must_use]
pub fn progress_chrome_class(_fill: Color, _palette: &Palette) -> ChromeClass {
    ChromeClass::Judged
}

/// Classify a spinner's arc under `UIF-TOK-006` rule 3.
#[must_use]
pub fn spinner_arc_chrome_class(arc: Color, palette: &Palette) -> ChromeClass {
    if same_channels(arc, palette.border_subtle) {
        ChromeClass::RestingDivider
    } else {
        ChromeClass::Judged
    }
}

/// The stripe period the reduced-motion segment repeats at, exposed for
/// the gallery and the sidecar so the value is asserted rather than
/// assumed.
#[must_use]
pub const fn stripe_period() -> f32 {
    STRIPE_PERIOD
}

/// Wrap `value` into `0.0..=1.0`.
fn wrap_unit(value: f32) -> f32 {
    let mut v = value % 1.0;
    if v < 0.0 {
        v += 1.0;
    }
    v
}

/// Whether two colours are the same channel-for-channel.
fn same_channels(a: Color, b: Color) -> bool {
    (a.r - b.r).abs() < f32::EPSILON
        && (a.g - b.g).abs() < f32::EPSILON
        && (a.b - b.b).abs() < f32::EPSILON
        && (a.a - b.a).abs() < f32::EPSILON
}

#[cfg(test)]
#[path = "progress_tests.rs"]
mod progress_tests;
