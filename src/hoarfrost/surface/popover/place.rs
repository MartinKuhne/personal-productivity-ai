//! Where a teaching popover lands — a preferred side, a card size, and the
//! existing anchor solver.
//!
//! Placement is **not** reinvented here. [`anchor::solve`] already answers
//! "given a trigger, a card size and a viewport, where does a floating
//! rect go", including the per-axis flip and the viewport clamp, and
//! [`crate::surface::menu`] and [`crate::surface::tooltip`] both lean on
//! it. A teaching popover is the same question with one thing the solver
//! cannot know: which side the *host* wants.
//!
//! # Why a preferred side and not a fixed one
//!
//! §3.7 puts the popover's `placement` in the **data model** rather than
//! in the geometry paragraph. That is the spec saying the side is a
//! per-callout decision — a callout pointing at a toolbar icon wants
//! `Below`, one pointing at a status-bar field wants `Above`, and one
//! explaining a side panel wants a side. So [`PreferredSide`] is carried
//! by the model and translated here into the solver's
//! [`ControlAlignment`], which is the same pair of axes the menu already
//! speaks.
//!
//! The preference is only a preference. [`Placement::flipped_vertical`]
//! and [`Placement::flipped_horizontal`] say what the viewport overrode,
//! which matters more here than it does for a tooltip: a teaching
//! popover is *interactive*, so a host that pointed it `Below` and got
//! `Above` may want to move the trigger's highlight too. The flags are
//! why the return type is the solver's `Placement` rather than a bare
//! `Rectangle`.
//!
//! # The gap is the solver's, not ours
//!
//! There is deliberately **no new gap number**. [`anchor::solve`] already
//! keeps a control-anchored card [`space::XS`] away from its trigger and
//! mirrors that same gap on the flipped side, so a popover gets the
//! breathing room a menu gets for free. [`POPOVER_GAP`] is an alias for
//! that value, not a second source of truth.
//!
//! The gap is also load-bearing for a reason it is not for a menu: the
//! user has to drive the pointer from the trigger, **across the gap**,
//! onto a footer button. A gap that belonged to the popover rather than
//! to the solver would be a second knob that could be turned until the
//! crossing became a precision task.
//!
//! [`Placement::flipped_vertical`]: crate::geometry::anchor::Placement::flipped_vertical
//! [`Placement::flipped_horizontal`]: crate::geometry::anchor::Placement::flipped_horizontal
//! [`space::XS`]: crate::token::space::XS

use crate::geometry::anchor::{self, Anchor, ControlAlignment, HorizontalAlign, VerticalAlign};
use crate::token::space;
use iced::{Rectangle, Size};

/// The gap a teaching popover keeps from its trigger.
///
/// An alias for the solver's own trigger gap, not a new measurement.
/// Exposed so a caller or a test can say "the popover sits `space::XS`
/// below the trigger" without reaching into a private constant in
/// [`anchor`].
pub const POPOVER_GAP: f32 = space::XS;

/// Which side of its trigger a popover prefers to open on (§3.7).
///
/// Four sides, not eight: §3.7's model carries a single `placement`
/// value and a teaching callout points at *something*, which fixes one
/// axis. The cross-axis alignment each side picks is the solver's most
/// conservative choice — start-aligned vertically, top-aligned
/// horizontally — so a flipped popover never also drifts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PreferredSide {
    /// Below the trigger, left edges level — the same default a fixed menu
    /// takes (`UIF-FMENU-001`).
    #[default]
    Below,
    /// Above the trigger, left edges level.
    Above,
    /// To the left of the trigger, top edges level.
    Leading,
    /// To the right of the trigger, top edges level.
    Trailing,
}

impl PreferredSide {
    /// The [`ControlAlignment`] this side maps onto for the solver.
    ///
    /// The mapping is a translation, not a choice made twice: `Below`
    /// and `Above` keep the menu's start alignment, and the two side
    /// placements reuse the *submenu* axes (`Top` + `Trailing` /
    /// `Leading`), which are already defined as "hung beside the anchor
    /// across the micro gap" and already flip to the other side near a
    /// viewport edge.
    #[must_use]
    pub const fn alignment(self) -> ControlAlignment {
        match self {
            Self::Below => ControlAlignment::new(VerticalAlign::Below, HorizontalAlign::Start),
            Self::Above => ControlAlignment::new(VerticalAlign::Above, HorizontalAlign::Start),
            Self::Leading => ControlAlignment::new(VerticalAlign::Top, HorizontalAlign::Leading),
            Self::Trailing => ControlAlignment::new(VerticalAlign::Top, HorizontalAlign::Trailing),
        }
    }

    /// The side the solver falls back to when this one cannot hold the
    /// card, as this enum names it.
    ///
    /// The mirror pair, so a host can ask "did I get what I asked for?"
    /// without reading the solver's own flip flags. `Leading` and
    /// `Trailing` mirror each other; the vertical pair does too.
    #[must_use]
    pub const fn mirror(self) -> Self {
        match self {
            Self::Below => Self::Above,
            Self::Above => Self::Below,
            Self::Leading => Self::Trailing,
            Self::Trailing => Self::Leading,
        }
    }

    /// Whether this side is one of the two horizontal placements.
    #[must_use]
    pub const fn is_horizontal(self) -> bool {
        matches!(self, Self::Leading | Self::Trailing)
    }
}

/// Resolve where a popover of `card_size` sits inside `viewport`.
///
/// Runs [`anchor::solve`] with [`PreferredSide::alignment`]. The returned
/// [`Placement`] carries the flip and clamp decisions, so a caller that
/// wants to know *why* the card moved can read them instead of guessing
/// from the rect.
///
/// Degenerate inputs are the solver's, and they are defined rather than
/// panicked: a non-finite rect sanitises to the origin, and a card larger
/// than the viewport pins its leading edge to the near margin and reports
/// itself scrollable.
#[must_use]
pub fn resolve(
    trigger: Rectangle,
    side: PreferredSide,
    card_size: Size,
    viewport: Rectangle,
) -> Placement {
    anchor::solve(
        Anchor::Control {
            rect: trigger,
            alignment: side.alignment(),
        },
        card_size,
        viewport,
    )
}

/// The solver's resolved placement, re-exported so a caller does not
/// have to reach past the popover module to read the flip and clamp
/// flags.
pub type Placement = anchor::Placement;

#[cfg(test)]
#[path = "place_tests.rs"]
mod place_tests;
