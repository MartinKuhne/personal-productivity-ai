//! Geometry and paint resolution for the `UIF-ATOM-004` ComboBox / Select
//! trigger — the closed box the user clicks, independent of the flyout it
//! opens (which is the menu engine's, not this module's).
//!
//! Everything here is a value in, a value out. No widget is built, no
//! colour is looked up lazily, no clock is read. The two things the
//! increment needs that the text-input atom does *not* already give us are
//! the trigger's own trailing-slot geometry — a select owns a chevron that
//! is a hit target, not merely a decoration — and the chevron's colour,
//! which §3.4 states directly and which does *not* match the input atom's
//! icon channel on hover.
//!
//! # What §3.4 actually says about the trigger
//!
//! §3.4 prices the control as an input (`min_width = 120px`, the two
//! control rungs, `space.md` padding) and then adds one sentence of its
//! own about the glyph: *chevron `text.muted` at rest, `text.strong` on
//! hover/open*. That sentence is the only colour rule in §3.4 the trigger
//! cannot inherit, because [`text_input`]'s icon channel goes
//! [`text_primary`](Palette::text_primary) on hover, not
//! [`text_strong`](Palette::text_strong).
//!
//! So [`chevron_color`] is written as an override layered on the atom's
//! own channel rather than as a fresh palette reading: the two states §3.4
//! names are answered by §3.4, and every state §3.4 leaves alone —
//! disabled, invalid, focused-but-closed — falls through to whatever
//! [`InputKind::DropdownTrigger`] already resolves. That keeps this
//! module from inventing a third opinion about a state the spec is silent
//! on, and it means a future change to the dropdown-trigger row of §3.2
//! reaches the combobox without a second edit here.
//!
//! # The trailing slot is a hit target, not a glyph
//!
//! On a plain text input the trailing slot holds a glyph and the slot's
//! own bounds carry no interaction meaning. On a select they do: the
//! chevron *is* the open affordance, so [`trailing_slot_bounds`] is sized
//! to [`size::HIT_TARGET_MIN`] rather than to the glyph, and the glyph is
//! centred inside it. Clicking anywhere in that square opens the flyout;
//! the 14px mark the user sees is deliberately smaller than the area
//! that responds to them.
//!
//! Unit tests live in the sibling `combo_box_tests.rs` sidecar.

use crate::state::{BaseState, VisualState};
use crate::token::color::Palette;
use crate::token::{shape, size, space};
use crate::widget::text_input::{self, InputKind, InputStyle};
use iced::{Color, Point, Rectangle, Size};

/// The standard trigger height — `size.control.md` (§3.4, `UIF-TOK-002`).
pub const HEIGHT_STANDARD: f32 = size::CONTROL_MD;

/// The compact trigger height — `size.control.sm` (§3.4, `UIF-TOK-002`).
pub const HEIGHT_COMPACT: f32 = size::CONTROL_SM;

/// Horizontal padding inside the trigger — `space.md` (§3.4,
/// `UIF-TOK-001`).
pub const HORIZONTAL_PADDING: f32 = space::MD;

/// The gap charged between the value text and the trailing slot —
/// `space.sm`, the same charge a text input makes so a value reads the
/// same length in either control.
pub const SLOT_GAP: f32 = space::SM;

/// The trailing slot's square side — `size.hit_target.min` (24px).
///
/// The slot is square and vertically centred, and it is sized to the hit
/// target rather than to the glyph because on a select the glyph is the
/// open affordance. See the module docs.
pub const TRAILING_SLOT: f32 = size::HIT_TARGET_MIN;

/// The chevron's drawn size — `size.icon.sm` (14px).
///
/// Smaller than a text input's [`SLOT_GLYPH_SIZE`](text_input::SLOT_GLYPH_SIZE)
/// because a select's glyph sits inside a 24px target it does not fill,
/// and §3.4's trigger reads as a quieter control than a search field.
pub const CHEVRON_SIZE: f32 = size::ICON_SM;

/// Corner radius of the trigger — `radius.widget`, the same rounding the
/// input atom takes (§3.2, `UIF-TOK-003`).
pub const RADIUS: f32 = shape::RADIUS_WIDGET;

/// The smallest width a trigger may take — `min_width = 120px` (§3.4).
///
/// A registered composite bound rather than a token because §3.4 states
/// the figure inline; the token lint's carve-out registry carries the
/// §-reference.
pub const MIN_WIDTH: f32 = 120.0;

/// The two height rungs §3.4 prices for a select, in the order the
/// gallery samples them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ComboRung {
    /// The standard rung — `size.control.md`.
    Standard,
    /// The compact rung — `size.control.sm`.
    Compact,
}

impl ComboRung {
    /// The rung's height.
    #[must_use]
    pub const fn height(self) -> f32 {
        match self {
            Self::Standard => HEIGHT_STANDARD,
            Self::Compact => HEIGHT_COMPACT,
        }
    }

    /// The rung's cell-id slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Standard => "std",
            Self::Compact => "cmp",
        }
    }
}

/// Every rung the matrix samples, standard first.
pub const RUNGS: [ComboRung; 2] = [ComboRung::Standard, ComboRung::Compact];

/// The trigger's outer bounds for one `(width, rung)` pair.
#[must_use]
pub fn trigger_bounds(origin: Point, width: f32, rung: ComboRung) -> Rectangle {
    Rectangle::new(origin, Size::new(width.max(0.0), rung.height()))
}

/// The trailing slot inside `trigger`: a square of
/// [`TRAILING_SLOT`] flush to the right edge, vertically centred.
///
/// Clamped to the trigger's own height so a caller who hands in a box
/// shorter than the hit target still gets a slot that fits inside it
/// rather than one that overflows the control.
#[must_use]
pub fn trailing_slot_bounds(trigger: Rectangle) -> Rectangle {
    let side = TRAILING_SLOT.min(trigger.height.max(0.0));
    Rectangle::new(
        Point::new(
            trigger.x + trigger.width - side,
            trigger.y + (trigger.height - side) / 2.0,
        ),
        Size::new(side, side),
    )
}

/// The chevron's drawn box, centred inside the trigger's trailing slot.
#[must_use]
pub fn chevron_bounds(trigger: Rectangle) -> Rectangle {
    let slot = trailing_slot_bounds(trigger);
    let side = CHEVRON_SIZE.min(slot.width).min(slot.height).max(0.0);
    Rectangle::new(
        Point::new(
            slot.x + (slot.width - side) / 2.0,
            slot.y + (slot.height - side) / 2.0,
        ),
        Size::new(side, side),
    )
}

/// The area the selected value (or the placeholder) is drawn into.
///
/// Runs from the leading padding to the slot's left edge minus
/// [`SLOT_GAP`], so the value never touches the glyph and its length does
/// not change when the slot changes size.
#[must_use]
pub fn value_bounds(trigger: Rectangle) -> Rectangle {
    let slot = trailing_slot_bounds(trigger);
    let left = trigger.x + HORIZONTAL_PADDING;
    let right = (slot.x - SLOT_GAP).max(left);
    Rectangle::new(
        Point::new(left, trigger.y),
        Size::new(right - left, trigger.height),
    )
}

/// The trigger's fully resolved fill/border/text style.
///
/// A thin delegation to [`text_input::resolve_input_style`] at
/// [`InputKind::DropdownTrigger`]: §3.4 prices the trigger as the input
/// atom's dropdown row rather than as a new cell, so the combobox takes
/// that row's fill, border and value colour wholesale. The one channel
/// this module does *not* take from it is the chevron — see
/// [`chevron_color`].
#[must_use]
pub fn resolve_trigger(state: VisualState, palette: &Palette) -> InputStyle {
    text_input::resolve_input_style(InputKind::DropdownTrigger, state, palette)
}

/// The chevron colour for one `(state, open)` pair.
///
/// Priority order, highest first:
///
/// 1. **Disabled** → [`text_disabled`](Palette::text_disabled). A dead
///    control's glyph is dead; §3.4's hover rule cannot fire on a control
///    that cannot be hovered into anything.
/// 2. **Invalid** → [`status_danger_text`](Palette::status_danger_text),
///    the dropdown-trigger row's own icon colour. §3.4 gives the chevron
///    only two named states and neither is the error state, so the error
///    signal comes from the atom the trigger resolves through rather than
///    being invented here.
/// 3. **Hovered, or the flyout is open** → [`text_strong`](Palette::text_strong).
///    This is §3.4's sentence, and it is the one place this module
///    overrides the input atom.
/// 4. **Focused but closed** → [`text_primary`](Palette::text_primary),
///    again the atom's own focused-icon colour. A tabbed-to select with
///    its flyout shut is not "open", and §3.4 does not call it strong.
/// 5. **Rest** → [`text_muted`](Palette::text_muted), where both §3.4
///    and the atom's icon channel agree.
#[must_use]
pub fn chevron_color(state: VisualState, open: bool, palette: &Palette) -> Color {
    match state.base {
        BaseState::Disabled => palette.text_disabled,
        BaseState::Invalid => palette.status_danger_text,
        BaseState::Hovered | BaseState::Pressed => palette.text_strong,
        BaseState::Rest => {
            if open {
                palette.text_strong
            } else if state.focused {
                palette.text_primary
            } else {
                palette.text_muted
            }
        }
    }
}
