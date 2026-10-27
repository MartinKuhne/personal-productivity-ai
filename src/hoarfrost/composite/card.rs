//! The card container — a raised grouping surface (`UIF-COMP-005`).
//!
//! §3B.5 gives the card four numbers and one conditional: `surface.raised`,
//! `radius.card`, `space.lg` of inner padding, and a border that is
//! `border.subtle` at rest but `border.strong` when the card is
//! interactive or elevated. The container paints nothing else, which is
//! the point: a card is a *surface*, and everything with a state is
//! something else sitting on it.
//!
//! # The border is the only state, and it is a chrome decision
//!
//! The resting border is `border.subtle` on `surface.raised` — a delta of
//! `1.27:1`, far below the `3.0:1` rule 2 of `UIF-TOK-006` sets for
//! active indicators. That is not a failure. A resting divider is meant to
//! be quiet, and the card's edge is doing separation, not signalling; the
//! same exemption the menu separator carries. What the exemption does *not*
//! cover is the elevated border, and the sidecar measures that one against
//! the floor rather than assuming it.
//!
//! # Why the padding is inset from the border, not inside it
//!
//! [`content_bounds`] insets by the padding *and* the stroke weight. A
//! card whose content ignored the border width would let a child's own
//! focus ring land on the stroke, and at `1px` against a `2px` ring the
//! two are indistinguishable on screen. Subtracting both is what makes the
//! card's interior a region a child can trust.

use crate::token::color::Palette;
use crate::token::{shape, space};
use crate::widget::button_widget::Layer;
use iced::{Color, Rectangle};

/// The card's corner radius — `radius.card` (§3B.5).
pub const RADIUS: f32 = shape::RADIUS_CARD;

/// The card's inner padding — `space.lg` (§3B.5).
pub const PADDING: f32 = space::LG;

/// The card's border weight — the `1px` §3B.5 prints for both elevations.
pub const BORDER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// The card's fill — `surface.raised` at every elevation.
///
/// Recorded as a name because the two elevations differ *only* in the
/// stroke: §3B.5 does not lift a card onto a brighter surface when it
/// becomes interactive, and a resolver that brightened the fill as well
/// would be inventing a third elevation the token ladder does not have.
pub const BACKGROUND_TOKEN: &str = "surface.raised";

/// Which edge treatment a card wears.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardElevation {
    /// A grouping surface at rest — `border.subtle`.
    Rest,
    /// A card the user can act on, or one lifted above its neighbours —
    /// `border.strong`.
    Elevated,
}

impl CardElevation {
    /// Whether this elevation's border is an active indicator.
    ///
    /// The resting border is separation and is exempt from rule 2 of
    /// `UIF-TOK-006`; the elevated border is chrome and is not.
    #[must_use]
    pub const fn is_active_indicator(self) -> bool {
        matches!(self, Self::Elevated)
    }
}

/// The card's resolved paint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardStyle {
    /// The fill — `surface.raised`.
    pub background: Color,
    /// The perimeter stroke.
    pub border_color: Color,
    /// The stroke weight.
    pub border_width: f32,
    /// The corner radius.
    pub radius: f32,
    /// The inner padding.
    pub padding: f32,
}

/// Resolve a card's paint for one elevation.
#[must_use]
pub fn resolve_style(elevation: CardElevation, palette: &Palette) -> CardStyle {
    CardStyle {
        background: palette.surface_raised,
        border_color: match elevation {
            CardElevation::Rest => palette.border_subtle,
            CardElevation::Elevated => palette.border_strong,
        },
        border_width: BORDER_WIDTH,
        radius: RADIUS,
        padding: PADDING,
    }
}

/// The region inside a card that its content may occupy.
///
/// Inset by the padding and by the stroke, for the reason given in the
/// module docs.
#[must_use]
pub fn content_bounds(bounds: Rectangle, style: &CardStyle) -> Rectangle {
    let inset = style.padding + style.border_width;

    Rectangle {
        x: bounds.x + inset,
        y: bounds.y + inset,
        width: (bounds.width - inset - inset).max(0.0),
        height: (bounds.height - inset - inset).max(0.0),
    }
}

/// The card's painted layer.
#[must_use]
pub fn card_layer(bounds: Rectangle, style: &CardStyle) -> Layer {
    Layer {
        bounds,
        fill: style.background,
        border: iced::Border {
            color: style.border_color,
            width: style.border_width,
            radius: style.radius.into(),
        },
    }
}

#[cfg(test)]
#[path = "card_tests.rs"]
mod card_tests;
