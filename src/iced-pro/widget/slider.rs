//! The `Slider` paint resolver and geometry (§3.5, `UIF-ATOM-005`).
//!
//! Like [`toggle`](crate::widget::toggle) and
//! [`progress`](crate::widget::progress), this module resolves paint and
//! answers geometry questions; it does not build a widget and it does not
//! own a gesture. The drag lives in
//! [`slider_host`](crate::widget::slider_host). Everything here is a pure
//! function of `(state, mount, palette)` or of rectangles, so every cell
//! of §3.5 is checkable without a window, and the value↔pixel mapping a
//! drag depends on is testable on its own.
//!
//! # The anatomy
//!
//! A `4px` ([`size::SLIDER_TRACK_HEIGHT]) rail at `radius.pill`, whose
//! unfilled portion is [`surface.active`](Palette::surface_active) inside
//! a `1px` [`border.subtle`](Palette::border_subtle) outline, and whose
//! filled portion is [`accent.fill`](Palette::accent.fill). A `16px`
//! ([`size::SLIDER_THUMB]) thumb sits on the rail inside a `24px`
//! ([`size::HIT_TARGET_MIN]) hit target, painted in
//! [`text.strong`](Palette::text_strong) with a `2px`
//! [`accent.border`](Palette::accent_border) ring. Optional stepped tick
//! marks are `2 × 4px` in [`border.strong`](Palette::border_strong).
//!
//! The rail and the thumb are two different kinds of thing and the audit
//! treats them that way. The rail is the groove: its own perimeter is
//! `border.subtle`, a resting divider, exempt. The thumb is the
//! indicator, and its silhouette is the **union** of its face and its
//! ring — so [`thumb_is_distinguishable`] asks whether *either* channel
//! clears the floor against whatever the thumb is currently sitting on.
//!
//! That union is not a loosening; it is what the eye is actually given. A
//! resting thumb parked on the filled rail has a ring measuring
//! `1.6098:1` against it (`accent.border` on `accent.fill`), and is
//! nevertheless unmistakable, because its white face separates at
//! `4.8020:1`. Measuring the ring alone would call that invisible. The
//! union is also not a free pass: in every state there is exactly one
//! channel doing the work in each place, and the sidecar pins which one,
//! so a palette retune that took *both* channels down fails loudly.
//!
//! # There is no foreground-on-fill pair here
//!
//! [`contrast::assert_fill_foreground_lock`] guards the rule that a solid
//! `*.fill` carries `color.text.strong` and nothing else. The slider
//! never paints text or a glyph on a fill: the rail is a groove, the
//! thumb is a handle, and the only thing drawn over the accent is
//! another stroke. Calling the lock here would assert a relationship
//! between two fills — [`SliderStyle::rail_filled`] and
//! [`SliderStyle::thumb_fill`] — that the rule does not govern, and on
//! `Hovered`/`Pressed` both are `accent.fill`, which the lock correctly
//! reports as "not `text.strong`". The assertion is therefore absent by
//! design, not overlooked; the pairs this control does owe are the ones
//! measured above and below.
//!
//! # The thumb inverts on hover, and that is the spec's move
//!
//! §3.5 gives the resting thumb a white face with a blue ring, and the
//! hovered/pressed thumb the reverse: an `accent.fill` face with a
//! `text.strong` ring. It groups `Hovered` and `Pressed` into one cell
//! and does not spend `accent.fill_active` on the press, so this resolver
//! paints them identically rather than inventing a third step the section
//! does not price.
//!
//! The inversion is a half-swap rather than a whole one. The resting
//! face's white moves to the ring, but the resting ring's
//! `accent.border` is replaced by `accent.fill` on the face — the same
//! hue one step deeper, two distinct tokens. What that costs is that the
//! hovered face is then the same colour as the filled rail it sits on,
//! which is precisely why the ring has to invert: over the fill the ring
//! is the only channel left carrying the handle, and it is the one
//! §3.5 spends `text.strong` on.
//!
//! The inversion is the same move the `On` row of
//! [`toggle`](crate::widget::toggle) makes for the same reason: there is
//! nowhere darker to take an accent fill without spending the active
//! variant early, so the affordance is a ring changing rather than the
//! body changing.
//!
//! # Disabled drops the accent entirely
//!
//! §3.5 stops at the enabled states. The disabled skin here follows the
//! crate's established disabled vocabulary — the row every
//! [`TogglePolarity`](crate::widget::toggle::TogglePolarity) shares —
//! with one addition the toggle does not need: the **filled rail loses
//! its accent** and becomes [`border.strong`](Palette::border_strong).
//!
//! That is not decoration. A disabled thumb's ring is
//! [`text.disabled`](Palette::text_disabled), and `text.disabled` against
//! a still-accented `accent.fill` rail is `1.4280:1` — the indicator
//! would vanish into the groove at exactly the moment the control needs
//! to remain readable-but-inert (`UIF-PRIN-004`). Taking the accent out
//! of the rail moves the ring to `5.2686:1` against the mount and
//! `4.2510:1` against the unfilled rail, and has the side effect of
//! being correct on its own terms: a disabled control should not
//! advertise liveness in the loudest colour the palette has.
//!
//! The de-accented rail is a new background for the thumb to travel over,
//! and it is the one place in this control where the ring cannot carry the
//! silhouette: `text.disabled` on `border.strong` is `1.3130:1`. The
//! face does it instead — the disabled thumb's `surface.raised` face
//! against `border.strong` measures `4.2379:1`. This is the case the
//! [union rule](#the-anatomy) exists for, and it is why the union is
//! pinned per-surface in the sidecar rather than left as an argument:
//! the design is only safe while at least one of the two channels clears
//! everywhere, and a flat `4.24` on the face is a thin margin to lose
//! silently to a palette edit.
//!
//! # The filled rail's third measurement, recorded and not judged
//!
//! Every ratio below was computed from the raw sRGB in [`Palette::dark`]
//! through [`contrast::contrast_ratio`], not copied from §3.5's printed
//! column. The two figures the spec does print survive recomputation
//! exactly:
//!
//! | Pair | §3.5 prints | Recomputed |
//! |---|---|---|
//! | `accent.fill` / `surface.overlay` | `3.69:1` | `3.6895:1` ✓ |
//! | `accent.fill` / `surface.base` | `4.14:1` | `4.1432:1` ✓ |
//!
//! There is a third pair the spec never mentions, and it has to be
//! decided rather than skipped: the filled rail against the **unfilled**
//! rail it is laid beside. That measurement is
//! [`FILLED_ON_UNFILLED_RATIO`] = `2.9768:1`, which is *below* the
//! `3.0:1` chrome floor [`contrast::MIN_CHROME_CONTRAST`] sets.
//!
//! It is deliberately **not** emitted as an audited pair, and the reason
//! is the rule itself. `UIF-TOK-006` rule 3 governs a control's
//! perimeter against the surface outside it — the boundary at which the
//! control separates from the world. The filled and unfilled rails are
//! both *interior* to one control: neither is a boundary, and the
//! contrast between them is not a legibility obligation the widget owes
//! anything. What the slider owes is that the whole control separates
//! from its mount, which is the pair §3.5 itself measures — `3.6895` on
//! `overlay`, `4.1432` on `base` — and that pair clears.
//!
//! The value position is not carried by the fill boundary either. It is
//! carried by the thumb, whose ring clears `4.79:1` against the unfilled
//! rail in its resting state. So the sub-floor interior measurement is
//! not load-bearing, and [`filled_on_unfilled`] exists so the number is
//! in the artifact rather than missing from it.
//!
//! Unit tests live in the sibling `slider_tests.rs` sidecar.

use crate::state::{BaseState, VisualState};
use crate::token::color::Palette;
use crate::token::contrast::{self, ChromeClass};
use crate::token::shape;
use crate::token::size;
use crate::widget::button_widget::Layer;
use crate::widget::focus;
use iced::{Border, Color, Point, Rectangle};

/// The rail's thickness — §3.5's `size.slider.track_height` (`4px`).
pub const TRACK_HEIGHT: f32 = size::SLIDER_TRACK_HEIGHT;

/// The rail's corner radius — `radius.pill`, so a `4px` rail reads as a
/// capsule rather than as a rectangle with cut corners.
pub const TRACK_RADIUS: f32 = shape::RADIUS_PILL;

/// The thumb's edge — §3.5's `size.slider.thumb` (`16 × 16px`).
pub const THUMB_SIZE: f32 = size::SLIDER_THUMB;

/// The thumb's pointer target — `size.hit_target_min` (`24 × 24px`).
///
/// The same indicator-versus-target split [`hit_size`](crate::widget::toggle::hit_size) carries for
/// a checkbox: the `16` is what is drawn, the `24` is what is aimed at,
/// and the gap between them is the only reason a slider is usable with a
/// trackpad.
pub const THUMB_HIT_SIZE: f32 = size::HIT_TARGET_MIN;

/// The thumb's ring weight — §3.5's `2px`, taken from `border.focus`.
pub const THUMB_RING_WIDTH: f32 = shape::BORDER_FOCUS;

/// The rail's outline weight — §3.5's `1px`, taken from
/// `border.hairline`.
pub const RAIL_BORDER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// A tick mark's thickness — §3.5's `2px`.
///
/// **No token of its own exists for this.** §2.3's closed stroke set has
/// two weights, `border.hairline` (`1`) and `border.focus` (`2`), and
/// neither is semantically a tick mark — one is a divider, the other a
/// keyboard ring. [`TICK_WIDTH`] takes the `2` from `border.focus`
/// because [`UIF-PRIN-001`](crate::token) forbids a raw literal and `2`
/// is the only `2` the closed set offers. The mismatch is a spec gap, not
/// a licence to invent a token; the same compromise
/// [`progress::SPINNER_STROKE`](crate::widget::progress::SPINNER_STROKE)
/// records.
pub const TICK_WIDTH: f32 = shape::BORDER_FOCUS;

/// A tick mark's height — §3.5's `4px`.
///
/// Equal to [`TRACK_HEIGHT`] because it *is* the same measurement: a
/// tick is exactly as tall as the rail it marks, which is what makes the
/// row of ticks read as a scale cut into the groove rather than as a
/// separate strip drawn under it.
pub const TICK_HEIGHT: f32 = size::SLIDER_TRACK_HEIGHT;

/// The keyboard focus ring's weight — `border.focus`, the same attention
/// weight every other control's ring carries.
pub const FOCUS_RING_WIDTH: f32 = shape::BORDER_FOCUS;

/// The filled rail against the unfilled rail, measured from the raw
/// palette: `2.9768:1`.
///
/// Below the `3.0:1` chrome floor, and deliberately not audited. See the
/// [module docs](self#the-filled-rails-third-measurement-recorded-and-not-judged)
/// for why the pair is interior rather than a perimeter.
pub const FILLED_ON_UNFILLED_RATIO: f32 = 2.9768;

/// A resolved slider: the groove, the fill in it, and the thumb over both.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderStyle {
    /// The groove behind the filled portion — `surface.active`.
    pub rail_unfilled: Color,
    /// The groove's outline — `border.subtle`, a resting divider.
    pub rail_border: Color,
    /// The filled portion of the rail.
    pub rail_filled: Color,
    /// The thumb's face.
    pub thumb_fill: Color,
    /// The thumb's ring — the channel that makes the thumb visible.
    pub thumb_border: Color,
    /// The thumb's ring weight.
    pub thumb_border_width: f32,
    /// The tick marks' colour.
    pub tick: Color,
    /// The keyboard focus ring colour — transparent when no ring is due.
    pub focus_ring_color: Color,
    /// The keyboard focus ring width — `0.0` when no ring is due.
    pub focus_ring_width: f32,
}

/// Resolve the paint for one `(state, mount)` pair.
///
/// The mount is a parameter rather than an assumption because the ring has
/// to be resolved against what the slider is actually sitting on, and a
/// slider is mounted on more than one surface in the framework. It is
/// otherwise unused: the rail and the thumb paint the same tokens wherever
/// they landed, and the audit is what asks about the mount.
///
/// The ring is not a constant. It goes through
/// [`focus::resolve_ring_color_for_background`] against the mount, the
/// same rule [`resolve_style`](crate::widget::toggle::resolve_style) applies, so a slider on a surface
/// the accent cannot survive on displaces its ring by arithmetic rather
/// than by a special case (`UIF-TOK-006` rule 2, decision **D-6**).
#[must_use]
pub fn resolve_style(state: VisualState, mount: Color, palette: &Palette) -> SliderStyle {
    let base = base_paint(state.base, palette);

    let (focus_ring_color, focus_ring_width) = if state.ring_visible() {
        (
            focus::resolve_ring_color_for_background(mount, palette),
            FOCUS_RING_WIDTH,
        )
    } else {
        (NO_RING_COLOR, NO_RING_WIDTH)
    };

    SliderStyle {
        rail_unfilled: base.rail_unfilled,
        rail_border: base.rail_border,
        rail_filled: base.rail_filled,
        thumb_fill: base.thumb_fill,
        thumb_border: base.thumb_border,
        thumb_border_width: THUMB_RING_WIDTH,
        tick: base.tick,
        focus_ring_color,
        focus_ring_width,
    }
}

/// The paint channels a base state fixes, before the ring is added.
struct BasePaint {
    rail_unfilled: Color,
    rail_border: Color,
    rail_filled: Color,
    thumb_fill: Color,
    thumb_border: Color,
    tick: Color,
}

/// Select the base-state cell.
///
/// Written as one `match` over [`BaseState`] rather than a
/// `(state, polarity)` cross product because a slider has no polarity:
/// there is no second row to share, and the filled portion is a position
/// rather than a mode.
const fn base_paint(base: BaseState, palette: &Palette) -> BasePaint {
    match base {
        // `Invalid` shares the `Rest` cell. §3.5 gives the slider no
        // invalid skin: `RangeModel::validation` carries words, and
        // §3B.1 renders them in the field's reserved caption slot rather
        // than on the control, so there is nothing for the slider to
        // repaint.
        BaseState::Rest | BaseState::Invalid => BasePaint {
            rail_unfilled: palette.surface_active,
            rail_border: palette.border_subtle,
            rail_filled: palette.accent_fill,
            thumb_fill: palette.text_strong,
            thumb_border: palette.accent_border,
            tick: palette.border_strong,
        },
        // The face and the ring swap. §3.5 states this as one rule for
        // both `Hovered` and `Pressed`, so both get the same cell.
        BaseState::Hovered | BaseState::Pressed => BasePaint {
            rail_unfilled: palette.surface_active,
            rail_border: palette.border_subtle,
            rail_filled: palette.accent_fill,
            thumb_fill: palette.accent_fill,
            thumb_border: palette.text_strong,
            tick: palette.border_strong,
        },
        BaseState::Disabled => BasePaint {
            rail_unfilled: palette.surface_active,
            rail_border: palette.border_subtle,
            rail_filled: palette.border_strong,
            thumb_fill: palette.surface_raised,
            thumb_border: palette.text_disabled,
            tick: palette.border_strong,
        },
    }
}

/// The width the thumb's centre travels — the rail's width less the
/// thumb's own.
///
/// Contained rather than overhanging: the thumb's face stays inside the
/// control's bounds at both ends, so a slider flush against a panel edge
/// does not paint outside itself. A rail narrower than the thumb collapses
/// to zero travel rather than to a negative span.
#[must_use]
pub fn thumb_travel(track: Rectangle) -> f32 {
    (track.width - THUMB_SIZE).max(0.0)
}

/// The x coordinate of the thumb's centre at a fraction of the travel.
#[must_use]
pub fn thumb_center_x(track: Rectangle, fraction: f32) -> f32 {
    let t = if fraction.is_finite() {
        fraction.clamp(0.0, 1.0)
    } else {
        0.0
    };
    track.x + THUMB_SIZE / 2.0 + t * thumb_travel(track)
}

/// The fraction of the travel a given x coordinate falls at.
///
/// Clamped at both ends rather than reported out-of-range, because the
/// caller is a pointer that has left the track and still needs an answer:
/// the value the drag has reached is the nearest end, not nothing.
///
/// A `NaN` x is not a position at all and rests at the start, the same
/// way [`thumb_center_x`] rests a non-finite fraction there. The clamp
/// cannot do this on its own: `f32::clamp` passes `NaN` straight through,
/// which would carry a value no pointer was ever at into
/// [`value_at_fraction`](crate::widget::range_model::RangeModel::value_at_fraction) and out of the drag as a commit.
/// The finiteness check therefore happens *before* the clamp, the same
/// ordering [`clamp`](crate::widget::range_model::RangeModel::clamp) uses for a non-finite value.
#[must_use]
pub fn fraction_at_x(track: Rectangle, x: f32) -> f32 {
    if x.is_nan() {
        return 0.0;
    }
    let span = thumb_travel(track);
    if span <= 0.0 {
        return 0.0;
    }
    ((x - track.x - THUMB_SIZE / 2.0) / span).clamp(0.0, 1.0)
}

/// The rail's rectangle, vertically centred in `bounds`.
#[must_use]
pub fn track_rect(bounds: Rectangle) -> Rectangle {
    Rectangle::new(
        Point::new(bounds.x, bounds.y + (bounds.height - TRACK_HEIGHT) / 2.0),
        iced::Size::new(bounds.width, TRACK_HEIGHT),
    )
}

/// The thumb's face rectangle at a fraction of the travel.
#[must_use]
pub fn thumb_rect(track: Rectangle, fraction: f32) -> Rectangle {
    let centre = thumb_center_x(track, fraction);
    Rectangle::new(
        Point::new(
            centre - THUMB_SIZE / 2.0,
            track.y + (TRACK_HEIGHT - THUMB_SIZE) / 2.0,
        ),
        iced::Size::new(THUMB_SIZE, THUMB_SIZE),
    )
}

/// The thumb's pointer target — the `24 × 24` box centred on the face.
#[must_use]
pub fn hit_rect(track: Rectangle, fraction: f32) -> Rectangle {
    let centre = thumb_center_x(track, fraction);
    Rectangle::new(
        Point::new(
            centre - THUMB_HIT_SIZE / 2.0,
            track.y + (TRACK_HEIGHT - THUMB_HIT_SIZE) / 2.0,
        ),
        iced::Size::new(THUMB_HIT_SIZE, THUMB_HIT_SIZE),
    )
}

/// The fractions the `count` tick marks sit at, first at `0.0` and last
/// at `1.0`.
///
/// Fewer than two marks produce nothing: a single tick at one end would
/// be a scale with one graduation, which says less than no scale at all.
#[must_use]
pub fn tick_fractions(count: usize) -> Vec<f32> {
    if count < 2 {
        return Vec::new();
    }
    let last = (count - 1) as f32;
    (0..count).map(|index| index as f32 / last).collect()
}

/// The whole groove: the unfilled rail with its outline.
#[must_use]
pub fn rail_layer(track: Rectangle, style: &SliderStyle) -> Layer {
    Layer {
        bounds: track,
        fill: style.rail_unfilled,
        border: Border {
            color: style.rail_border,
            width: RAIL_BORDER_WIDTH,
            radius: TRACK_RADIUS.into(),
        },
    }
}

/// The filled portion of the rail, or `None` at the very start.
///
/// The fill runs to the thumb's centre, so at `0.0` the whole of it is
/// hidden under the thumb's left half. Returning `None` rather than
/// painting it keeps the recorded frame honest about there being nothing
/// filled.
#[must_use]
pub fn filled_layer(track: Rectangle, style: &SliderStyle, fraction: f32) -> Option<Layer> {
    let t = if fraction.is_finite() {
        fraction.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if t <= 0.0 {
        return None;
    }
    let width = thumb_center_x(track, t) - track.x;
    if width <= 0.0 {
        return None;
    }
    Some(Layer {
        bounds: Rectangle::new(track.position(), iced::Size::new(width, track.height)),
        fill: style.rail_filled,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: TRACK_RADIUS.into(),
        },
    })
}

/// The thumb's face and ring.
#[must_use]
pub fn thumb_layer(track: Rectangle, style: &SliderStyle, fraction: f32) -> Layer {
    Layer {
        bounds: thumb_rect(track, fraction),
        fill: style.thumb_fill,
        border: Border {
            color: style.thumb_border,
            width: style.thumb_border_width,
            radius: TRACK_RADIUS.into(),
        },
    }
}

/// The tick-mark layers, one per graduation.
#[must_use]
pub fn tick_layers(track: Rectangle, style: &SliderStyle, count: usize) -> Vec<Layer> {
    tick_fractions(count)
        .into_iter()
        .map(|fraction| {
            let centre = thumb_center_x(track, fraction);
            Layer {
                bounds: Rectangle::new(
                    Point::new(
                        centre - TICK_WIDTH / 2.0,
                        track.y + (TRACK_HEIGHT - TICK_HEIGHT) / 2.0,
                    ),
                    iced::Size::new(TICK_WIDTH, TICK_HEIGHT),
                ),
                fill: style.tick,
                border: Border::default().width(0.0),
            }
        })
        .collect()
}

/// The keyboard focus ring, drawn on the thumb's `24 × 24` hit box.
///
/// The ring belongs to the **thumb**, not to the track. A ring around the
/// whole groove would advertise the range rather than the control the
/// user is moving, and it would be `4px` tall — a ring nobody can see
/// the point of. Drawing it on the hit box rather than on the face leaves
/// a `4px` gap between the ring and the thumb, which is what keeps the
/// ring from reading as a second border on a thumb that already has one.
#[must_use]
pub fn focus_ring_layer(track: Rectangle, style: &SliderStyle, fraction: f32) -> Option<Layer> {
    if style.focus_ring_width == NO_RING_WIDTH {
        return None;
    }
    Some(Layer {
        bounds: hit_rect(track, fraction),
        fill: Color::TRANSPARENT,
        border: Border {
            color: style.focus_ring_color,
            width: style.focus_ring_width,
            radius: TRACK_RADIUS.into(),
        },
    })
}

/// Classify the thumb's ring under `UIF-TOK-006` rule 3.
///
/// The ring is the indicator's perimeter and is held to the ordinary
/// `3.0:1` chrome floor in every state, including disabled — which is
/// the state the [module docs](self#disabled-drops-the-accent-entirely)
/// explain had to be designed around it rather than into it.
#[must_use]
pub fn thumb_chrome_class(border: Color, palette: &Palette) -> ChromeClass {
    if same_channels(border, palette.border_subtle) {
        ChromeClass::RestingDivider
    } else {
        ChromeClass::Judged
    }
}

/// Classify the groove's own outline under `UIF-TOK-006` rule 3.
///
/// `border.subtle` is a resting divider: the groove is not an indicator
/// and does not owe the floor. A rail outline drawn in anything else would
/// be a different token and would be judged as one.
#[must_use]
pub fn rail_border_chrome_class(border: Color, palette: &Palette) -> ChromeClass {
    if same_channels(border, palette.border_subtle) {
        ChromeClass::RestingDivider
    } else {
        ChromeClass::Judged
    }
}

/// Measure the filled rail against the unfilled rail.
///
/// Exposed so the gallery and the sidecar can carry the number. It is
/// **not** wired into any audit pair; see the
/// [module docs](self#the-filled-rails-third-measurement-recorded-and-not-judged)
/// for the decision and its reason.
#[must_use]
pub fn filled_on_unfilled(palette: &Palette) -> f32 {
    contrast::contrast_ratio(palette.accent_fill, palette.surface_active)
}

/// The ratio that makes the thumb visible where it currently sits.
///
/// The thumb's silhouette is the union of its face and its ring, so the
/// stronger of the two channels is the one the eye is given. See the
/// [module docs](self#the-anatomy) for why measuring the ring alone
/// would call a resting thumb on the filled rail invisible when it is
/// plainly not.
///
/// This is a *maximum*, not a waiver. It answers one question — does the
/// handle read against this background — and the sidecar pins which
/// channel answers it in every state, so losing both is still a failure.
#[must_use]
pub fn thumb_distinguishable_ratio(style: &SliderStyle, behind: Color) -> f32 {
    contrast::contrast_ratio(style.thumb_fill, behind)
        .max(contrast::contrast_ratio(style.thumb_border, behind))
}

/// Whether the thumb reads against `behind` at the `3.0:1` chrome floor
/// [`contrast::MIN_CHROME_CONTRAST`] sets.
#[must_use]
pub fn thumb_is_distinguishable(style: &SliderStyle, behind: Color) -> bool {
    thumb_distinguishable_ratio(style, behind) >= contrast::MIN_CHROME_CONTRAST
}

/// Which channel of the thumb carries its silhouette against `behind`.
///
/// Returned as data so a test can pin *where* the visibility comes from
/// rather than only that it exists. A state that quietly moved every
/// thumb from face-carried to ring-carried would keep the union passing
/// while changing what the control looks like; this makes that a
/// detectable change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThumbCarrier {
    /// The face separates from the background; the ring does not.
    Face,
    /// The ring separates from the background; the face does not.
    Ring,
    /// Both channels clear the floor.
    Both,
    /// Neither channel clears the floor — the thumb is invisible.
    Neither,
}

/// Classify which thumb channel clears [`contrast::MIN_CHROME_CONTRAST`] against
/// `behind`.
#[must_use]
pub fn thumb_carrier(style: &SliderStyle, behind: Color) -> ThumbCarrier {
    let face = contrast::meets_chrome_contrast(style.thumb_fill, behind);
    let ring = contrast::meets_chrome_contrast(style.thumb_border, behind);
    match (face, ring) {
        (true, true) => ThumbCarrier::Both,
        (true, false) => ThumbCarrier::Face,
        (false, true) => ThumbCarrier::Ring,
        (false, false) => ThumbCarrier::Neither,
    }
}

/// The width of a focus ring that is not being drawn.
const NO_RING_WIDTH: f32 = 0.0;

/// The colour of a focus ring that is not being drawn.
const NO_RING_COLOR: Color = Color::TRANSPARENT;

/// Whether two colours are the same channel-for-channel.
fn same_channels(a: Color, b: Color) -> bool {
    a.r == b.r && a.g == b.g && a.b == b.b && a.a == b.a
}

#[cfg(test)]
#[path = "slider_tests.rs"]
mod slider_tests;
