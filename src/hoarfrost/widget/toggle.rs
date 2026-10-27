//! The toggle-family paint resolver and geometry — `Checkbox`, `Radio` and
//! `ToggleSwitch` (`UIF-ATOM-003`, §3.3).
//!
//! Like [`button`](crate::widget::button), this module resolves paint; it
//! does not build a widget. [`resolve_style`] is a pure function of
//! `(polarity, state, palette)` returning a [`ToggleStyle`], and the
//! geometry functions are pure functions of the control and the label
//! width, so every cell of §3.3 is checkable without a window. The iced
//! `Element` that turns a [`ToggleStyle`] into something a pointer can
//! click is not part of this increment.
//!
//! # Why the resolver takes a polarity rather than a control
//!
//! §3.3 is not three tables. It is **one** two-row matrix — *Unchecked /
//! Off* and *Checked / Indeterminate / On* — that all three controls
//! share, and the sharing is the point: a ticked checkbox, a selected
//! radio and a switched-on toggle are the same statement told in three
//! shapes. What differs between them is the **shape**, never the colour.
//!
//! So the resolver is written against [`TogglePolarity`] and does not
//! take a [`ToggleControl`] at all. The control appears only in
//! [`indicator_size`] and [`indicator_radius`], which answer geometry
//! questions the colour half has no business answering. Had the matrix
//! been written as three near-identical `match` arms, retuning the
//! checked fill would have meant three edits that could disagree; written
//! against the polarity it is one.
//!
//! The second row's name is worth pausing on. The spec groups
//! `Indeterminate` with `Checked` and `On`, so an indeterminate checkbox
//! is an **accent-filled** box carrying a `−` dash rather than the
//! quiet empty box an unchecked one is. [`TogglePolarity::On`] is that
//! row: *the control is not empty*, not *the control is ticked*.
//!
//! # The matrix
//!
//! | Polarity | Rest | Hovered | Pressed | Disabled |
//! |---|---|---|---|---|
//! | `Off` | `surface.code` / `border.strong` / `text.muted` / `text.primary` | `surface.hover` / `accent.border` / `text.strong` / `text.strong` | `surface.active` / `accent.border` / `text.strong` / `text.strong` | `surface.raised` / `border.subtle` / `text.disabled` / `text.disabled` |
//! | `On` | `accent.fill` / `accent.fill` / `text.strong` / `text.primary` | `accent.fill` / `text.strong` / `text.strong` / `text.strong` | `accent.fill_active` / `accent.fill_active` / `text.strong` / `text.strong` | `surface.raised` / `border.subtle` / `text.disabled` / `text.disabled` |
//!
//! Columns are *fill / border / mark / label*. The **mark** is whatever
//! the control draws inside its indicator: the `✓` of a checkbox, the
//! dot of a radio, the thumb of a switch. One channel covers all three
//! because §3.3 gives them the same colour in every cell.
//!
//! The `Off` row's label is `text.primary` at rest and `text.strong`
//! once hovered or pressed — the same step a Secondary button takes. It
//! is *not* `text.muted`: the `7.04:1` §3.3 prints beside `text.muted`
//! is the **switch thumb** on `surface.code`, not the label, and
//! reading that figure as the label's would have put a 7:1 grey where the
//! spec asks for an 12:1 one.
//!
//! # The focus ring is arithmetic, not a per-polarity constant
//!
//! §3.3 prints `accent.border` for the `Off` row's focused cell and
//! `text.strong` for the `On` row's, which reads as a two-way policy.
//! It is not. [`resolve_style`] asks
//! [`focus::resolve_ring_color_for_background`] for the ring against the
//! fill it just resolved, exactly as the button resolver does, and the
//! polarity split falls out of the numbers:
//!
//! ```text
//!   Off   fill surface.code    accent.border 6.1680 PASS -> accent.border
//!   Off   fill surface.hover   accent.border 5.4427 PASS -> accent.border
//!   Off   fill surface.active  accent.border 4.7922 PASS -> accent.border
//!   On    fill accent.fill     accent.border 1.6098 FAIL -> text.strong 4.8020
//!   On    fill accent.fill_active  accent.border 1.8961 FAIL -> text.strong 5.6559
//! ```
//!
//! No line of this file says which ring a polarity wears. The polarity
//! picks a *fill*, the shared rule picks the ring that fill allows, and
//! the two colours §3.3 prints are the two answers that arithmetic
//! returns. That matters most on the `On` row: `accent.border` on
//! `accent.fill` is `1.6098:1`, an invisible indicator at the exact
//! moment a keyboard user needs one — the same failure **G-9** was
//! about, arriving here through a different atom.
//!
//! # The perimeter stroke classifies three ways
//!
//! [`indicator_chrome_class`] sorts the resolved border under
//! `UIF-TOK-006` rule 3 rather than holding one threshold against it:
//!
//! * `CoincidentWithFill` — the `On` row's Rest and Pressed cells, where
//!   the border *is* the fill and draws no boundary of its own. The
//!   separation is then carried by the fill against whatever the control
//!   is mounted to, which reaches `3.6895:1` and `3.1325:1` on
//!   `surface.overlay` — both clear of the `3.00:1` that class owes.
//! * `RestingDivider` — the disabled border's `border.subtle`, which is
//!   not indicator material and is exempt.
//! * `Judged` — everything else, against the `3.0:1` chrome floor.
//!
//! And the `(fill, mark)` pair goes through
//! [`contrast::assert_fill_foreground_lock`] at construction. The `On`
//! row's two fills are exactly the tokens rule 4 pins to `text.strong`,
//! so the lock reports `Locked` there and `NotSolidFill` on the `Off`
//! row, where the background is a surface rather than a locked fill.
//!
//! # The hit target extends across the label
//!
//! §3.3's geometry line asks for a `24 × 24` minimum "extending across
//! their inline label", and §2.2 says the same of the box itself: the
//! `16` glyph is the *indicator*, not the click area. [`hit_size`]
//! builds the row — indicator, [`LABEL_GAP`], label — and floors both
//! dimensions at [`size::HIT_TARGET_MIN`], so a bare 16px checkbox with
//! no label still presents a 24×24 target and a long label widens it
//! rather than the height being allowed to shrink.
//!
//! # Two figures in §3.3 do not survive recomputation
//!
//! Every ratio above was computed from the raw sRGB in
//! [`Palette::dark`] through [`contrast::contrast_ratio`], not copied
//! from the spec's printed column. Two of the spec's numbers are wrong
//! (RUST-041):
//!
//! | Pair | §3.3 prints | True |
//! |---|---|---|
//! | `accent.border` / `surface.hover` | `5.35:1` | `5.4427:1` |
//! | `accent.border` / `surface.code` (the top of the `Off` focused-ring range) | `6.67:1` | `6.1680:1` |
//!
//! Both are transcription errors rather than design disagreements: the
//! tokens involved are the ones the spec names, and neither corrected
//! figure breaks a floor — `5.4427` and `6.1680` clear `3.0:1` as
//! comfortably as the printed numbers did. Nothing here changes to match
//! the printed values; the sidecar pins the true ones so a future
//! retune is measured rather than assumed.
//!
//! Unit tests live in the sibling `toggle_tests.rs` sidecar.

use crate::state::{BaseState, VisualState};
use crate::token::color::Palette;
use crate::token::contrast::{self, ChromeClass};
use crate::token::shape;
use crate::token::size;
use crate::token::space;
use crate::widget::button_widget::Layer;
use crate::widget::focus;
use iced::{Border, Color, Point, Rectangle};

/// Resting border width of a toggle indicator — `border.hairline`
/// (§3.3, `UIF-TOK-003`).
///
/// Constant across the base states: an indicator that thickened its
/// border on hover would move its own mark, which `UIF-PRIN-003`
/// forbids.
pub const BORDER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// Focus ring width on a toggle indicator — `border.focus` (§3.3,
/// `UIF-TOK-003`).
///
/// The same attention weight a button's ring uses, so a focused checkbox
/// and a focused button read as the same signal rather than as two
/// related-but-different ones.
pub const FOCUS_RING_WIDTH: f32 = shape::BORDER_FOCUS;

/// Gap between the indicator and its inline label — `space.sm` (§3.3,
/// `UIF-TOK-001`).
///
/// This is the only thing separating the two halves of the hit target
/// visually, and it is inside the target rather than outside it: the gap
/// is clickable, which is what makes the row read as one control
/// instead of a glyph next to a caption.
pub const LABEL_GAP: f32 = space::SM;

/// The width of a focus ring that is not being drawn.
///
/// Not a visual magnitude but the *absence* of one, which is why it is
/// allowed outside the token modules — and named, so `resolve_style`
/// reads as "no ring" rather than "zero point zero".
const NO_RING_WIDTH: f32 = 0.0;

/// The colour of a focus ring that is not being drawn.
const NO_RING_COLOR: Color = Color::TRANSPARENT;

/// The three toggle atoms of §3.3 (`UIF-ATOM-003`).
///
/// The three share one colour matrix and differ only in the geometry
/// this enum selects — see the [module docs](self) for why the resolver
/// is keyed on polarity rather than on this type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToggleControl {
    /// The `16 × 16` square box with `radius.widget`, binary or
    /// tri-state.
    Checkbox,
    /// The `16 × 16` circular indicator with a centred inner dot when
    /// selected.
    Radio,
    /// The `36 × 20` pill track with a sliding thumb.
    Switch,
}

/// Which half of §3.3's two-row matrix a cell belongs to.
///
/// `On` is the spec's *Checked / Indeterminate / On* row: the control
/// is **not empty**, which covers a ticked box, a dashed indeterminate
/// box, a selected radio and a switched-on toggle alike. `Off` is the
/// empty half.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TogglePolarity {
    /// Unchecked, unselected, off.
    Off,
    /// Checked, indeterminate, selected, or on.
    On,
}

/// A toggle indicator's fully resolved paint for one `(polarity, state)`
/// pair.
///
/// Every field is a value a renderer consumes as-is: no token lookup, no
/// conditional, no further resolution. The style deliberately does not
/// carry the control or the polarity — those selected the values and are
/// not themselves painted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToggleStyle {
    /// The indicator's fill: the box background, or the switch track.
    pub indicator_fill: Color,
    /// The indicator's perimeter stroke colour.
    pub indicator_border: Color,
    /// The indicator's perimeter stroke width, in logical pixels.
    pub indicator_border_width: f32,
    /// The mark drawn inside the indicator — the `✓`, the radio dot or
    /// the switch thumb.
    pub mark: Color,
    /// The inline label colour.
    pub label: Color,
    /// The keyboard focus ring colour — fully transparent when no ring
    /// is drawn.
    pub focus_ring_color: Color,
    /// The keyboard focus ring width — `0.0` when no ring is drawn.
    pub focus_ring_width: f32,
}

/// Resolve the paint for one `(polarity, state)` pair against a palette
/// (`UIF-ATOM-003`).
///
/// A pure function: the same three inputs always produce the same
/// [`ToggleStyle`]. The base half of `state` selects fill, border, mark
/// and label from §3.3; the `focused` half adds the ring on top without
/// touching them, following the same additive-focus shape as
/// [`button::resolve_style`](crate::widget::button::resolve_style).
///
/// The ring is not chosen by polarity. It is resolved against the
/// indicator fill this function just produced, through
/// [`focus::resolve_ring_color_for_background`], so the `On` row's
/// saturated fill displaces the accent to `text.strong` by arithmetic
/// rather than by a special case (`UIF-TOK-006` rule 2, decision
/// **D-6**).
///
/// The `(fill, mark)` pair goes through
/// [`contrast::assert_fill_foreground_lock`] before the style is handed
/// back — the `UIF-TOK-006` rule 4 solid-fill foreground lock, checked
/// at the moment the style is built.
#[must_use]
pub fn resolve_style(
    polarity: TogglePolarity,
    state: VisualState,
    palette: &Palette,
) -> ToggleStyle {
    let base = base_paint(polarity, state.base, palette);

    contrast::assert_fill_foreground_lock(base.indicator_fill, base.mark, palette);

    let (focus_ring_color, focus_ring_width) = if state.ring_visible() {
        (
            focus::resolve_ring_color_for_background(base.indicator_fill, palette),
            FOCUS_RING_WIDTH,
        )
    } else {
        (NO_RING_COLOR, NO_RING_WIDTH)
    };

    ToggleStyle {
        indicator_fill: base.indicator_fill,
        indicator_border: base.indicator_border,
        indicator_border_width: BORDER_WIDTH,
        mark: base.mark,
        label: base.label,
        focus_ring_color,
        focus_ring_width,
    }
}

/// The four paint channels a base state fixes, before the ring is added.
struct BasePaint {
    indicator_fill: Color,
    indicator_border: Color,
    mark: Color,
    label: Color,
}

/// Select the base-state quad for a polarity.
///
/// Split per polarity rather than matched as a `(polarity, state)` cross
/// product so each row of §3.3 reads top to bottom in one place
/// (`RUST-023`).
const fn base_paint(polarity: TogglePolarity, base: BaseState, palette: &Palette) -> BasePaint {
    match polarity {
        TogglePolarity::Off => off_paint(base, palette),
        TogglePolarity::On => on_paint(base, palette),
    }
}

/// §3.3 — Unchecked / Off.
///
/// The indicator is a quiet, empty box: `surface.code` is the code-well
/// surface, which is what makes an unchecked box read as *an empty
/// field* rather than as a disabled one. The label steps
/// `text.primary` → `text.strong` on hover exactly as a Secondary
/// button's does; the mark — the switch thumb, the only one of the three
/// that is always painted — is `text.muted` at rest and brightens with
/// the label.
const fn off_paint(base: BaseState, palette: &Palette) -> BasePaint {
    match base {
        // `Invalid` shares the `Rest` cell: §3.3 gives a toggle no
        // invalid skin. Validation is a §3.2 input concern and no
        // boolean control in the spec validates anything.
        BaseState::Rest | BaseState::Invalid => BasePaint {
            indicator_fill: palette.surface_code,
            indicator_border: palette.border_strong,
            mark: palette.text_muted,
            label: palette.text_primary,
        },
        BaseState::Hovered => BasePaint {
            indicator_fill: palette.surface_hover,
            indicator_border: palette.accent_border,
            mark: palette.text_strong,
            label: palette.text_strong,
        },
        BaseState::Pressed => BasePaint {
            indicator_fill: palette.surface_active,
            indicator_border: palette.accent_border,
            mark: palette.text_strong,
            label: palette.text_strong,
        },
        BaseState::Disabled => BasePaint {
            indicator_fill: palette.surface_raised,
            indicator_border: palette.border_subtle,
            mark: palette.text_disabled,
            label: palette.text_disabled,
        },
    }
}

/// §3.3 — Checked / Indeterminate / On.
///
/// The indicator is filled with the accent, and the mark is
/// `text.strong` — the `UIF-TOK-006` rule 4 lock, which pins the
/// foreground of a locked solid fill to white and is asserted at
/// construction rather than trusted.
///
/// The hover cell is the interesting one: the fill stays `accent.fill`
/// and the *border* goes to `text.strong`. There is nowhere darker to
/// take the accent without spending `accent.fill_active` early, so the
/// affordance is a white ring around the box rather than a change to the
/// box — the same move the Primary button makes for the same reason.
const fn on_paint(base: BaseState, palette: &Palette) -> BasePaint {
    match base {
        BaseState::Rest | BaseState::Invalid => BasePaint {
            indicator_fill: palette.accent_fill,
            indicator_border: palette.accent_fill,
            mark: palette.text_strong,
            label: palette.text_primary,
        },
        BaseState::Hovered => BasePaint {
            indicator_fill: palette.accent_fill,
            indicator_border: palette.text_strong,
            mark: palette.text_strong,
            label: palette.text_strong,
        },
        BaseState::Pressed => BasePaint {
            indicator_fill: palette.accent_fill_active,
            indicator_border: palette.accent_fill_active,
            mark: palette.text_strong,
            label: palette.text_strong,
        },
        BaseState::Disabled => BasePaint {
            indicator_fill: palette.surface_raised,
            indicator_border: palette.border_subtle,
            mark: palette.text_disabled,
            label: palette.text_disabled,
        },
    }
}

/// Classify a resolved indicator border under `UIF-TOK-006` rule 3
/// (`UIF-ATOM-003`).
///
/// Three of the four [`ChromeClass`] arms are reachable here and each is
/// decided by the *token identity* of the border, not by the channel
/// name:
///
/// * a border set to the exact same colour as the fill it surrounds is
///   [`ChromeClass::CoincidentWithFill`] — it draws no boundary, so the
///   fill carries the separation and owes `3.00:1` against the surface
///   behind it;
/// * `border.subtle` is [`ChromeClass::RestingDivider`], a passive
///   hairline that is not indicator material and is exempt;
/// * everything else is [`ChromeClass::Judged`] against `3.0:1`.
///
/// [`ChromeClass::UnpaintedBackground`] is deliberately **not**
/// produced: no toggle cell paints a transparent indicator, so a
/// resolver that could return it would be modelling a state §3.3 does
/// not have.
#[must_use]
pub fn indicator_chrome_class(border: Color, fill: Color, palette: &Palette) -> ChromeClass {
    if same_channels(border, fill) {
        ChromeClass::CoincidentWithFill
    } else if same_channels(border, palette.border_subtle) {
        ChromeClass::RestingDivider
    } else {
        ChromeClass::Judged
    }
}

/// Whether two colours carry identical channels.
///
/// Exact equality rather than a tolerance: these are token values that
/// either came from the same field of the same palette or they did not,
/// and a near-match would be a different pair of tokens that happened to
/// land close.
fn same_channels(a: Color, b: Color) -> bool {
    a.r == b.r && a.g == b.g && a.b == b.b && a.a == b.a
}

// ---------------------------------------------------------------------------
// Geometry (§3.3 geometry line, §2.2)
// ---------------------------------------------------------------------------

/// The painted indicator box for a control, as `(width, height)`.
///
/// `Checkbox` and `Radio` share `size.checkbox.box` — the radio is the
/// same square drawn round, which is what lets one function and one hit
/// test cover both. The switch's track is the only shape with an aspect
/// ratio of its own.
#[must_use]
pub const fn indicator_size(control: ToggleControl) -> (f32, f32) {
    match control {
        ToggleControl::Checkbox | ToggleControl::Radio => (size::CHECKBOX_BOX, size::CHECKBOX_BOX),
        ToggleControl::Switch => (size::SWITCH_TRACK_WIDTH, size::SWITCH_TRACK_HEIGHT),
    }
}

/// The indicator's corner radius (§3.3, `UIF-TOK-003`).
///
/// The checkbox takes `radius.widget`, the same corner every other
/// interactive control in the framework uses. The radio and the switch
/// take `radius.pill`: for the radio that turns the `16` square into a
/// circle, for the switch it turns the `36 × 20` box into a capsule.
#[must_use]
pub const fn indicator_radius(control: ToggleControl) -> f32 {
    match control {
        ToggleControl::Checkbox => shape::RADIUS_WIDGET,
        ToggleControl::Radio | ToggleControl::Switch => shape::RADIUS_PILL,
    }
}

/// The full pointer hit box for a control carrying a label of
/// `label_width`, as `(width, height)`.
///
/// §3.3 asks for a `24 × 24` minimum "extending across their inline
/// label", and this is that rule made arithmetic: the row is the
/// indicator, [`LABEL_GAP`] and the label, and **both** dimensions are
/// floored at [`size::HIT_TARGET_MIN`].
///
/// The two floors do different jobs. The height floor is what saves a
/// bare `16` checkbox — with or without a label, a `16`-tall row is
/// below the minimum and gets grown. The width floor is what saves a
/// control with no label at all, so an icon-only checkbox still presents
/// a square target rather than a `16`-wide strip. A long label widens
/// the row past the floor, which is the intended direction: more label
/// means more target, never less.
///
/// A negative `label_width` is treated as no label. The value comes from
/// a text measurement upstream, and a failed measurement should collapse
/// to the smallest legal target rather than produce a negative box.
#[must_use]
pub fn hit_size(control: ToggleControl, label_width: f32) -> (f32, f32) {
    let (indicator_w, indicator_h) = indicator_size(control);
    let label = label_width.max(0.0);
    let width = if label > 0.0 {
        indicator_w + LABEL_GAP + label
    } else {
        indicator_w
    };

    (
        width.max(size::HIT_TARGET_MIN),
        indicator_h.max(size::HIT_TARGET_MIN),
    )
}

/// The horizontal travel of a `ToggleSwitch` thumb — the distance its
/// left edge moves between off and on.
///
/// Derived from the three track tokens rather than written: the track is
/// `36`, the thumb `14`, and [`size::SWITCH_THUMB_INSET`] sits on each
/// side, leaving `16` of travel. Writing `16.0` here would have been a
/// fourth number that a retune of the other three could leave behind.
#[must_use]
pub const fn thumb_travel() -> f32 {
    size::SWITCH_TRACK_WIDTH - size::SWITCH_THUMB - 2.0 * size::SWITCH_THUMB_INSET
}

/// The x-offset of the thumb's left edge inside the track, measured from
/// the track's left edge.
///
/// `0.0` off, [`thumb_travel`] on. The thumb is vertically centred by
/// the inset, so only the horizontal position changes — which is what
/// makes the switch read as a binary slide rather than a re-layout.
#[must_use]
pub const fn thumb_offset(on: bool) -> f32 {
    if on { thumb_travel() } else { 0.0 }
}

/// The `8 × 8` centred inner dot a selected `Radio` draws (§3.3).
///
/// Expressed as half of `size.checkbox.box` rather than as a written
/// `8`: §3.3 gives the dot no token of its own, and halving the box both
/// produces the spec's figure and keeps the two locked together — a box
/// that later changed size would take the dot with it instead of leaving
/// an `8` stranded in a smaller ring.
#[must_use]
pub const fn radio_dot_size() -> f32 {
    size::CHECKBOX_BOX / 2.0
}

// ---------------------------------------------------------------------------
// Paint layers
// ---------------------------------------------------------------------------

/// The indicator's own rectangle, top-left at `origin`.
///
/// The row's hit box is wider than this — [`hit_size`] carries the label
/// — but every painted layer belongs to the indicator alone.
#[must_use]
pub fn indicator_rect(control: ToggleControl, origin: Point) -> Rectangle {
    let (width, height) = indicator_size(control);
    Rectangle::new(origin, iced::Size::new(width, height))
}

/// The indicator's fill-and-border layer.
#[must_use]
pub fn indicator_layer(indicator: Rectangle, control: ToggleControl, style: &ToggleStyle) -> Layer {
    Layer {
        bounds: indicator,
        fill: style.indicator_fill,
        border: Border::default()
            .color(style.indicator_border)
            .width(style.indicator_border_width)
            .rounded(indicator_radius(control)),
    }
}

/// The mark layer — the switch thumb or the radio's inner dot — if the
/// mark for this control is geometry rather than a font glyph.
///
/// `None` in three cases, each a different reason:
///
/// * `Checkbox` always, because its mark is a `✓` or a `−` drawn from the
///   icon font. Recording it as a quad would be painting a rectangle and
///   calling it a tick.
/// * `Radio` when unselected, because there is nothing inside the ring.
/// * Never for `Switch`: the thumb is painted in both positions, which
///   is what makes the control read as a slide rather than a flash.
#[must_use]
pub fn mark_layer(
    indicator: Rectangle,
    control: ToggleControl,
    polarity: TogglePolarity,
    style: &ToggleStyle,
) -> Option<Layer> {
    match control {
        ToggleControl::Checkbox => None,
        ToggleControl::Radio => {
            if polarity != TogglePolarity::On {
                return None;
            }
            let dot = radio_dot_size();
            let offset = (indicator.width - dot) / 2.0;
            Some(Layer {
                bounds: Rectangle::new(
                    Point::new(indicator.x + offset, indicator.y + offset),
                    iced::Size::new(dot, dot),
                ),
                fill: style.mark,
                border: Border::default().width(0.0),
            })
        }
        ToggleControl::Switch => {
            let inset = size::SWITCH_THUMB_INSET;
            let x = indicator.x + inset + thumb_offset(polarity == TogglePolarity::On);
            Some(Layer {
                bounds: Rectangle::new(
                    Point::new(x, indicator.y + inset),
                    iced::Size::new(size::SWITCH_THUMB, size::SWITCH_THUMB),
                ),
                fill: style.mark,
                border: Border::default().width(0.0),
            })
        }
    }
}

/// The focus ring drawn outside the indicator, if one is due.
///
/// Grown outward by the ring width and stroked on the grown quad, for
/// the same reason the button's ring is: iced strokes inward, so a ring
/// drawn as the indicator's own border would eat into the fill the state
/// just resolved. The radius grows by the same amount to keep the ring
/// concentric with the box it surrounds.
///
/// Note that the ring belongs to the **indicator**, not to the row. §3.3
/// calls it a ring on the control's base, and a ring around a 200px
/// labelled row would advertise the whole line rather than the box the
/// user is toggling.
#[must_use]
pub fn focus_ring_layer(
    indicator: Rectangle,
    control: ToggleControl,
    style: &ToggleStyle,
) -> Option<Layer> {
    if style.focus_ring_width == NO_RING_WIDTH {
        return None;
    }

    Some(Layer {
        bounds: indicator.expand(style.focus_ring_width),
        fill: Color::TRANSPARENT,
        border: Border::default()
            .color(style.focus_ring_color)
            .width(style.focus_ring_width)
            .rounded(indicator_radius(control) + style.focus_ring_width),
    })
}

#[cfg(test)]
#[path = "toggle_tests.rs"]
mod toggle_tests;
