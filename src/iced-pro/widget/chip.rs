//! The chip paint resolver and geometry — the assist, filter and input
//! chip of §3.6 (`UIF-ATOM-006`).
//!
//! Like [`toggle`](crate::widget::toggle) and [`badge`](crate::widget::badge),
//! this module resolves paint and computes boxes; it does not build a
//! widget. [`resolve_style`] is a pure function of
//! `(variant, height, VisualState, palette)` and the geometry functions
//! are pure functions of the resolved style and the measured text width,
//! so every cell of §3.6's matrix is checkable without a window.
//!
//! # The matrix is keyed on polarity, not on variant
//!
//! §3.6 writes its state table as **Unselected** and **Selected**, not as
//! `Assist` / `Filter` / `Input`, and that is the right key: the assist
//! chip and the unselected filter chip paint *identically*, and no
//! non-filter chip can be selected at all. [`ChipPolarity`] is therefore
//! what the resolver takes, exactly as [`TogglePolarity`] keys §3.3's
//! two rows. The variant reaches the resolver only through
//! [`ChipStyle::radius`] and the two `shows_*` flags, which are shape and
//! slot decisions rather than paint.
//!
//! [`TogglePolarity`]: crate::widget::toggle::TogglePolarity
//!
//! # What §3.6 does not give, and what was chosen
//!
//! The section prints Rest and Hover for the unselected polarity and one
//! row for the selected one. Four cells had to be decided:
//!
//! | Cell | Choice | Why |
//! |---|---|---|
//! | Unselected / Pressed | `surface.active` + `text.strong` + `accent.border` | the same third rung the toggle's off-row takes; there is no third background §3.6 could have meant |
//! | Selected / Pressed | `accent.fill_active` + `text.strong` + `accent.border` | the accent's own pressed rung, mirroring `on_paint`; the perimeter stays `accent.border` because §3.6 fixes it for the selected chip without qualifying the state |
//! | Both / Disabled | `surface.raised` + `border.subtle` + `text_disabled` | the crate-wide disabled convention (§3.3, §3.1) |
//! | Both / Invalid | the Rest cell | §3.6 gives a chip nothing to validate |
//!
//! The **disabled selected chip** is the one place the palette forces a
//! real concession, and the numbers say so: `text_disabled` over
//! `accent.fill` is **`1.4280:1`**, so the selected background cannot
//! survive into the disabled state under a compliant label. The fill
//! drops to `surface.raised` (`text_disabled` there is `5.5643:1`, inside
//! the `4.0`–`6.0` disabled band). What is deliberately *not* dropped is
//! the leading `✓`: §3.6 makes it the selected chip's mark, and a chip
//! that silently un-checked itself would report a state that is not true.
//! It stays `text.strong` — `18.7108:1` on the disabled fill — so the
//! selection remains legible while the label goes quiet.
//!
//! # Every ratio below was recomputed
//!
//! The figures come from the raw sRGB in [`Palette::dark`] through
//! [`contrast::contrast_ratio`], not from §3.6's printed column. Against
//! the chip's `surface.raised` mount:
//!
//! | Pair | §3.6 prints | True |
//! |---|---|---|
//! | `text.primary` / `surface.raised` | `12.38:1` | `12.3755:1` ✓ |
//! | `border.strong` / `surface.raised` | `4.24:1` | `4.2379:1` ✓ |
//! | `text.strong` / `accent.fill` | `4.80:1` | `4.8020:1` ✓ |
//!
//! All three agree, so nothing here is conformed to the spec's printed
//! numbers and nothing has to be corrected against them. The supporting
//! figures the resolver leans on are `accent.border` on `surface.raised`
//! `6.2727:1`, `accent.fill` on `surface.raised` `3.8965:1`,
//! `accent_fill_active` on `surface.raised` `3.3082:1` and
//! `text.strong` on `accent_fill_active` `5.6559:1`.
//!
//! **One mount is not allowed.** §3.6 lists the mounts a selected chip
//! may wear — `surface.base`, `surface.raised`, `surface.overlay` or
//! `surface.hover` — and rules out `surface.active` by name, because
//! `accent.fill` against `surface.active` is **`2.9768:1`**, under the
//! `3.00:1` a fill that carries its own boundary owes. Every allowed
//! mount clears: `4.1432` on `base`, `3.8965` on `raised`, `3.6895` on
//! `overlay` and `3.3809` on `hover`. `surface.active` is the background
//! a pressed row underneath the chip would paint, and the sidecar pins
//! both sides of that line.
//!
//! # The reserved `×` slot is reserved whether or not it is wanted
//!
//! §3.6 says a removable chip *reserves* a `24 × 24` trailing target.
//! The reservation is charged into [`chip_width`] unconditionally for a
//! removable chip, and [`remove_slot`] returns the same box the width
//! function paid for, so the hit target and the painted `×` cannot drift.
//! On the `24px` compact rung the slot is exactly as tall as the chip:
//! the floor is met at the boundary rather than by overflowing the box.
//!
//! Unit tests live in the sibling `chip_tests.rs` sidecar.

use crate::state::{BaseState, InteractionInput, VisualState, resolve};
use crate::token::color::Palette;
use crate::token::contrast::{self, ChromeClass};
use crate::token::shape;
use crate::token::size;
use crate::token::space;
use crate::widget::chip_model::{ChipHeight, ChipModel, ChipVariant};
use crate::widget::focus;
use iced::{Border, Color, Point, Rectangle};

/// The compact chip height — §3.6's `24px`, [`size::BAR_STATUS`].
pub const COMPACT_HEIGHT: f32 = size::BAR_STATUS;

/// The filter-bar chip height — §3.6's `28px`, [`size::CONTROL_SM`].
pub const BAR_HEIGHT: f32 = size::CONTROL_SM;

/// Horizontal padding inside the chip — §3.6's `space.sm` (`8px`).
pub const H_PADDING: f32 = space::SM;

/// Gap between a chip's leading glyph, its label and its `×` slot —
/// §3.6's `space.xs` (`4px`).
///
/// One gap for both sides because §3.6 gives the chip a single
/// inter-element rung. The gap is *inside* the hit target, which is what
/// makes the glyph, the word and the `×` read as one control rather than
/// three things parked near each other.
pub const LABEL_GAP: f32 = space::XS;

/// The trailing `×` hit target §3.6 reserves on a removable chip —
/// `24 × 24`, [`size::HIT_TARGET_MIN`].
///
/// The slot is square and is sized by the hit-target floor rather than by
/// the glyph painted inside it. A `14px` `×` in a `14px` box is a target
/// a pointer misses; the reservation is the whole reason the box is
/// `24`.
pub const REMOVE_TARGET: f32 = size::HIT_TARGET_MIN;

/// The leading glyph's box — §3.6's `14px` `✓`, [`size::ICON_SM`].
pub const GLYPH_SIZE: f32 = size::ICON_SM;

/// A chip's border width — `shape::BORDER_HAIRLINE`, constant across
/// every state.
///
/// A chip that thickened its border when selected would change its own
/// box with its meaning, the same layout-instability objection
/// `UIF-PRIN-003` raises against hover growth.
pub const BORDER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// The focus ring width on a chip — `shape::BORDER_FOCUS`.
///
/// The same attention weight a button or a toggle uses, so a focused chip
/// in a filter bar is the same signal as a focused field next to it.
pub const FOCUS_RING_WIDTH: f32 = shape::BORDER_FOCUS;

/// The pill radius a chip takes when §3.6's sentence calls it a pill —
/// [`shape::RADIUS_PILL`].
pub const RADIUS_PILL: f32 = shape::RADIUS_PILL;

/// The squarer radius a chip takes when §3.6's sentence calls it a
/// filter or input chip — [`shape::RADIUS_WIDGET`].
pub const RADIUS_WIDGET: f32 = shape::RADIUS_WIDGET;

/// The width of a focus ring that is not being drawn.
const NO_RING_WIDTH: f32 = 0.0;

/// The colour of a focus ring that is not being drawn.
const NO_RING_COLOR: Color = Color::TRANSPARENT;

/// The trailing slot of a chip that reserves none.
const NO_SLOT: f32 = 0.0;

/// Which half of §3.6's two-row state table a cell belongs to.
///
/// The section's own key, not an invention: it writes **Unselected** for
/// `Assist` / `Filter` / `Input` together and **Selected** for `Filter`
/// alone, so the resolver is keyed on the row rather than on the variant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ChipPolarity {
    /// §3.6's unselected row — `surface.raised` behind `text.primary`.
    #[default]
    Unselected,
    /// §3.6's selected row — `accent.fill` behind `text.strong` with a
    /// leading `✓`.
    Selected,
}

impl ChipPolarity {
    /// The polarity a model resolves to.
    #[must_use]
    pub fn of<Id>(model: &ChipModel<Id>) -> Self {
        if model.variant().is_selected() {
            Self::Selected
        } else {
            Self::Unselected
        }
    }
}

/// A chip's fully resolved paint and box for one `(polarity, height, state)`.
///
/// Every field is a value a renderer consumes as-is: no token lookup, no
/// conditional, no further resolution. The style deliberately does not
/// carry the variant or the polarity — those selected the values and are
/// not themselves painted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChipStyle {
    /// The chip's fill.
    pub fill: Color,
    /// The perimeter stroke colour.
    pub border: Color,
    /// The perimeter stroke width — always [`BORDER_WIDTH`].
    pub border_width: f32,
    /// The label colour.
    pub text: Color,
    /// The leading glyph colour — the host's leading icon, or the
    /// selected chip's mandatory `✓`.
    pub glyph: Color,
    /// The keyboard focus ring colour — transparent when no ring is drawn.
    pub focus_ring_color: Color,
    /// The keyboard focus ring width — `0.0` when no ring is drawn.
    pub focus_ring_width: f32,
    /// The corner radius — [`RADIUS_PILL`] or [`RADIUS_WIDGET`].
    pub radius: f32,
    /// The chip's height — [`COMPACT_HEIGHT`] or [`BAR_HEIGHT`].
    pub height: f32,
    /// Horizontal padding — [`H_PADDING`].
    pub h_padding: f32,
    /// Gap between chip elements — [`LABEL_GAP`].
    pub label_gap: f32,
    /// The trailing `×` target's edge length — [`REMOVE_TARGET`] when
    /// [`shows_remove`](Self::shows_remove) is set, `0.0` otherwise.
    pub remove_target: f32,
    /// Whether the leading `✓` is painted.
    ///
    /// True only in the selected polarity. A separate flag from the
    /// glyph colour so a renderer knows whether to reserve the glyph box
    /// without re-deriving the polarity.
    pub shows_check: bool,
    /// Whether the trailing `×` slot is reserved and painted.
    pub shows_remove: bool,
}

/// Resolve the paint and box for one chip (`UIF-ATOM-006`).
///
/// A pure function: the same four inputs always produce the same
/// [`ChipStyle`]. The polarity selects the row of §3.6's table, the
/// height selects the box, the base half of `state` selects the cell and
/// the `focused` half adds the ring on top without touching anything
/// else — the same additive-focus shape as
/// [`toggle::resolve_style`](crate::widget::toggle::resolve_style).
///
/// The ring is not chosen by polarity. It is resolved against the fill
/// this function just produced, through
/// [`focus::resolve_ring_color_for_background`], so the selected chip's
/// saturated `accent.fill` displaces the accent ring the same arithmetic
/// a switched-on toggle's does (`UIF-TOK-006` rule 2, decision **D-6**).
///
/// The `(fill, text)` pair goes through
/// [`contrast::assert_fill_foreground_lock`] before the style is handed
/// back — the `UIF-TOK-006` rule 4 solid-fill foreground lock, checked
/// at the moment the style is built rather than trusted.
#[must_use]
pub fn resolve_style(
    polarity: ChipPolarity,
    height: ChipHeight,
    state: VisualState,
    palette: &Palette,
) -> ChipStyle {
    let base = match polarity {
        ChipPolarity::Unselected => unselected_paint(state.base, palette),
        ChipPolarity::Selected => selected_paint(state.base, palette),
    };

    contrast::assert_fill_foreground_lock(base.fill, base.text, palette);

    let (focus_ring_color, focus_ring_width) = if state.ring_visible() {
        (
            focus::resolve_ring_color_for_background(base.fill, palette),
            FOCUS_RING_WIDTH,
        )
    } else {
        (NO_RING_COLOR, NO_RING_WIDTH)
    };

    ChipStyle {
        fill: base.fill,
        border: base.border,
        border_width: BORDER_WIDTH,
        text: base.text,
        glyph: base.glyph,
        focus_ring_color,
        focus_ring_width,
        radius: RADIUS_WIDGET,
        height: height.px(),
        h_padding: H_PADDING,
        label_gap: LABEL_GAP,
        remove_target: NO_SLOT,
        shows_check: matches!(polarity, ChipPolarity::Selected),
        shows_remove: false,
    }
}

/// Apply a variant's shape and slot decisions to a resolved style.
///
/// Split out of [`resolve_style`] because the radius and the `×` slot are
/// the only two things the *variant* decides, and folding them into the
/// paint match would have made a colour decision depend on a shape
/// question (`RUST-023`). [`resolve_style`] returns the filter/input
/// shape; an assist chip asks for the pill here.
///
/// **The `Assist` radius is named by the spec.** §3.6 assigns
/// `radius.pill` to "`Assist` chips and status/count pills" and
/// `radius.widget` to "`Filter` and `Input` chips", so the branch below
/// is the prescribed one rather than a reading of an ambiguous sentence.
/// The pill is also the branch §3.8's status chip lands in, which keeps
/// the two chip-shaped things in the framework consistent.
#[must_use]
pub fn with_variant(style: ChipStyle, is_assist: bool, is_removable: bool) -> ChipStyle {
    ChipStyle {
        radius: if is_assist {
            RADIUS_PILL
        } else {
            RADIUS_WIDGET
        },
        remove_target: if is_removable { REMOVE_TARGET } else { 0.0 },
        shows_remove: is_removable,
        ..style
    }
}

/// Resolve a whole model's style in one call.
///
/// The convenience a host actually wants: polarity, height, variant shape
/// and the removable slot all come out of the model, so no call site has
/// to remember to apply [`with_variant`] and cannot get a pill-less assist
/// chip by omission.
///
/// It takes [`InteractionInput`] rather than a resolved
/// [`VisualState`] for one reason: the model's
/// [`disabled`](ChipModel::is_disabled) flag is **authoritative**. The
/// input's own `enabled` field is overwritten from the model before the
/// state is resolved, so a caller cannot paint a disabled chip as live by
/// handing over a live-looking observation. That is the same fail-safe
/// [`crate::state::resolve`] applies — a control that has not been told it is
/// enabled shows the disabled skin — reached here through the model that
/// actually owns the flag.
#[must_use]
pub fn resolve_model<Id>(
    model: &ChipModel<Id>,
    input: InteractionInput,
    palette: &Palette,
) -> ChipStyle {
    let state = resolve(InteractionInput {
        enabled: !model.is_disabled(),
        ..input
    });
    let style = resolve_style(ChipPolarity::of(model), model.height(), state, palette);
    with_variant(
        style,
        matches!(model.variant(), ChipVariant::Assist),
        model.is_removable(),
    )
}

/// The four paint channels a base state fixes, before the ring is added.
struct BasePaint {
    fill: Color,
    border: Color,
    text: Color,
    glyph: Color,
}

/// §3.6 — Unselected (`Assist` / `Filter` / `Input`).
///
/// The rest and hover cells are the section's own words: `surface.raised`
/// behind `text.primary`, moving to `surface.hover` behind `text.strong`.
/// The pressed rung is the crate's third step, and the disabled trio is
/// the crate-wide convention §3.1 and §3.3 already use — §3.6 gives this
/// chip no disabled skin of its own.
const fn unselected_paint(base: BaseState, palette: &Palette) -> BasePaint {
    match base {
        // `Invalid` shares the `Rest` cell: §3.6 gives a chip nothing to
        // validate, and inventing an error skin for a control that cannot
        // be wrong would be modelling a state the spec does not have.
        BaseState::Rest | BaseState::Invalid => BasePaint {
            fill: palette.surface_raised,
            border: palette.border_strong,
            text: palette.text_primary,
            glyph: palette.text_primary,
        },
        BaseState::Hovered => BasePaint {
            fill: palette.surface_hover,
            border: palette.accent_border,
            text: palette.text_strong,
            glyph: palette.text_strong,
        },
        BaseState::Pressed => BasePaint {
            fill: palette.surface_active,
            border: palette.accent_border,
            text: palette.text_strong,
            glyph: palette.text_strong,
        },
        BaseState::Disabled => BasePaint {
            fill: palette.surface_raised,
            border: palette.border_subtle,
            text: palette.text_disabled,
            glyph: palette.text_disabled,
        },
    }
}

/// §3.6 — Selected (`Filter`).
///
/// `accent.fill` behind `text.strong` with a `1px accent.border`
/// perimeter and the mandatory leading `✓` in `text.strong`. The
/// perimeter is *not* coincident with the fill, which is what keeps the
/// selected chip's boundary at `6.2727:1` where a coincident
/// `accent.fill` edge would be judged on the fill itself.
///
/// The disabled cell is the concession the [module docs](self) describe:
/// the fill cannot stay `accent.fill` under a disabled foreground
/// (`1.4280:1`), so it drops to `surface.raised` — but the `✓` stays
/// `text.strong`, because the glyph is the state and a chip may not
/// report a state it does not have.
const fn selected_paint(base: BaseState, palette: &Palette) -> BasePaint {
    match base {
        BaseState::Rest | BaseState::Invalid => BasePaint {
            fill: palette.accent_fill,
            border: palette.accent_border,
            text: palette.text_strong,
            glyph: palette.text_strong,
        },
        BaseState::Hovered => BasePaint {
            fill: palette.accent_fill,
            border: palette.text_strong,
            text: palette.text_strong,
            glyph: palette.text_strong,
        },
        BaseState::Pressed => BasePaint {
            fill: palette.accent_fill_active,
            border: palette.accent_border,
            text: palette.text_strong,
            glyph: palette.text_strong,
        },
        BaseState::Disabled => BasePaint {
            fill: palette.surface_raised,
            border: palette.border_subtle,
            text: palette.text_disabled,
            glyph: palette.text_strong,
        },
    }
}

/// Classify a resolved chip perimeter under `UIF-TOK-006` rule 3
/// (`UIF-ATOM-006`).
///
/// Three of the four [`ChromeClass`] arms are reachable and each is
/// decided by the *token identity* of the perimeter, not by which
/// polarity it belongs to:
///
/// * `border.subtle` — the disabled chip's edge — is
///   [`ChromeClass::RestingDivider`] and exempt. A disabled chip is not
///   advertising an affordance, so its hairline is not indicator
///   material.
/// * a perimeter set to the exact same colour as the fill it surrounds
///   would be [`ChromeClass::CoincidentWithFill`]. **No chip resolves to
///   it today** — §3.6 gives every chip a perimeter of its own — but the
///   arm is here because the classifier is shared and a caller handed a
///   coincident edge must get the honest class and the `3.00:1` it owes.
/// * everything else is [`ChromeClass::Judged`] against `3.0:1`:
///   `border.strong` on `surface.raised` at `4.2379:1`, `accent.border`
///   on `surface.raised` at `6.2727:1`, `accent.border` on
///   `surface.hover` at `5.4427:1`.
#[must_use]
pub fn chip_chrome_class(style: &ChipStyle, palette: &Palette) -> ChromeClass {
    if same_channels(style.border, style.fill) {
        ChromeClass::CoincidentWithFill
    } else if same_channels(style.border, palette.border_subtle) {
        ChromeClass::RestingDivider
    } else {
        ChromeClass::Judged
    }
}

/// The width a chip needs for its leading glyph and label.
///
/// The reserved `×` slot is charged in whenever
/// [`ChipStyle::shows_remove`] is set, whether or not the pointer is
/// anywhere near it. That is what "reserves" means in §3.6: the space is
/// paid for up front so a chip never reflows when the `×` becomes
/// visible, and so [`remove_slot`] returns the exact box this function
/// already paid for.
#[must_use]
pub fn chip_width(style: &ChipStyle, leading_width: f32, text_width: f32) -> f32 {
    let mut content = text_width;
    if leading_width > 0.0 {
        content += leading_width + style.label_gap;
    }
    if style.shows_remove {
        content += style.remove_target + style.label_gap;
    }
    content + style.h_padding * 2.0
}

/// The trailing `×` hit target for a chip occupying `bounds`.
///
/// `None` when the chip is not removable. The returned box is
/// [`REMOVE_TARGET`] square, flush to the chip's trailing edge inside the
/// horizontal padding, and vertically centred — so on the `24px` compact
/// rung it is exactly as tall as the chip and on the `28px` rung it sits
/// `2px` clear of each edge.
///
/// The box is derived from the same two constants [`chip_width`] charged
/// to the total, which is what makes it impossible for the painted `×`
/// and the clickable `×` to be different sizes.
#[must_use]
pub fn remove_slot(style: &ChipStyle, bounds: Rectangle) -> Option<Rectangle> {
    if !style.shows_remove {
        return None;
    }
    let side = style.remove_target;
    Some(Rectangle::new(
        Point::new(
            (bounds.x + bounds.width - style.h_padding - side).max(bounds.x),
            bounds.y + (bounds.height - side) / 2.0,
        ),
        iced::Size::new(side, side),
    ))
}

/// The perimeter a renderer draws, in the form iced's `Border` wants.
///
/// A convenience over assembling the three channels by hand at every call
/// site, so a chip's border cannot be drawn with the width of one state
/// and the colour of another.
#[must_use]
pub fn border_of(style: &ChipStyle) -> Border {
    Border {
        color: style.border,
        width: style.border_width,
        radius: style.radius.into(),
    }
}

/// Whether two colours carry identical channels.
fn same_channels(a: Color, b: Color) -> bool {
    (a.r - b.r).abs() < f32::EPSILON
        && (a.g - b.g).abs() < f32::EPSILON
        && (a.b - b.b).abs() < f32::EPSILON
        && (a.a - b.a).abs() < f32::EPSILON
}

#[cfg(test)]
#[path = "chip_tests.rs"]
mod chip_tests;
