//! Where a tooltip lands — an anchor, a card size, and the existing anchor solver.
//!
//! Placement is **not** reinvented here. [`anchor::solve`] already answers "given a
//! trigger, a card size and a viewport, where does a floating rect go", including the
//! per-axis flip and the viewport clamp, and [`crate::surface::menu`] already leans on it.
//! A tooltip is the same question with a different default alignment, so this module
//! contributes the two things the solver cannot know:
//!
//! 1. **[`TooltipAnchor`]** — what the tooltip describes. The interesting case is
//!    [`TooltipAnchor::Band`], whose horizontal and vertical anchors come from
//!    **different** rects on purpose.
//! 2. **[`TOOLTIP_ALIGNMENT`]** — below, left-aligned, which is what every tooltip the
//!    spec describes does.
//!
//! ## Why a navbar tooltip anchors to the *bar*, not the *button*
//!
//! A navbar entry is a control **inside** a bar: the bar is [`size::CONTROL_LG`] (36px)
//! tall and the button inside it is [`size::CONTROL_SM`] (28px), inset by a few pixels.
//! Anchoring to the button
//! would place the tooltip at `button.bottom + gap`, which lands **inside the bar** — the
//! tooltip would sit against the bar's own bottom edge and read as part of it rather than
//! as something floating beneath it.
//!
//! [`TooltipAnchor::Band`] therefore takes the vertical extent from the **bar** and the
//! horizontal extent from the **entry**:
//!
//! ```text
//!      ┌───────────────────────────────┐  band.y
//!      │   ┌───────┐                   │
//!      │   │ entry │  ← x, width from here
//!      │   └───────┘                   │
//!      └───────────────────────────────┘  band.bottom
//!                    │
//!                    ├── TOOLTIP_GAP (space::XS)
//!                    ▼
//!      ┌───────────────┐   left edge under the entry,
//!      │ full label    │   top edge clear of the bar
//!      └───────────────┘
//! ```
//!
//! That is "positioned below the navbar" (`UIF-NAV-003`) read the only way that survives
//! the inset: below the **bar**, under the **entry**.
//!
//! ## The gap is the solver's, not ours
//!
//! There is deliberately **no new gap number**. [`anchor::solve`] already keeps a
//! control-anchored card [`space::XS`] away from its trigger and mirrors that same gap on
//! the `Above` fallback, so a tooltip gets the breathing room a menu gets for free.
//! [`TOOLTIP_GAP`] is an alias for that value, not a second source of truth — tests assert
//! the resolved position against it so the coupling stays visible.
//!
//! [`size::CONTROL_LG`]: crate::token::size::CONTROL_LG
//! [`size::CONTROL_SM`]: crate::token::size::CONTROL_SM
//! [`space::XS`]: crate::token::space::XS

use crate::geometry::anchor::{self, Anchor, ControlAlignment, Placement};
use crate::token::space;
use iced::{Point, Rectangle, Size};

/// How a tooltip lines up against its anchor.
///
/// **Below, left-aligned** — the tooltip's left edge is level with the anchor's left edge
/// and it hangs beneath it. Chosen because every tooltip the spec describes (a truncated
/// navbar label, a truncated tab label) reads left-to-right from the thing it describes,
/// and a centred tooltip under a wide bar would float in the middle of nothing.
///
/// The solver may still move it: [`Placement::clamped_horizontal`] means the viewport
/// pushed it back inside, and [`Placement::flipped_vertical`] means there was no room
/// below so it went above.
pub const TOOLTIP_ALIGNMENT: ControlAlignment = ControlAlignment::below_start();

/// The gap a tooltip keeps from its anchor.
///
/// An alias for the solver's own trigger gap, not a new measurement. Exposed so a caller
/// or a test can say "the tooltip sits `space::XS` below the bar" without reaching into
/// a private constant in [`anchor`].
pub const TOOLTIP_GAP: f32 = space::XS;

/// What a tooltip describes, and therefore what it anchors to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TooltipAnchor {
    /// An ordinary control: the tooltip hangs below the control's own rect.
    ///
    /// Right for anything not inside a taller container — a dialog button, a menu row, a
    /// field.
    Control(Rectangle),
    /// A control **inside a bar** — a navbar entry, a tab-strip tab.
    ///
    /// Vertical extent from `band`, horizontal extent from `control`. See the [module
    /// docs](self) for why mixing them is the only reading of "below the navbar" that
    /// survives the entry's inset inside the bar.
    Band {
        /// The bar the entry sits in — supplies `y` and `height`.
        band: Rectangle,
        /// The entry itself — supplies `x` and `width`.
        control: Rectangle,
    },
    /// The pointer itself, for a tooltip that follows the cursor rather than a rect.
    ///
    /// No current requirement asks for this; it is here because the solver already accepts a
    /// point anchor, and a cursor-following tooltip is the one case where an anchor is not a
    /// rect. The card lands below-right of the cursor at the solver's pointer offset.
    Pointer(Point),
}

impl TooltipAnchor {
    /// A navbar-entry or tab anchor: below the bar, under the entry.
    #[must_use]
    pub const fn band(band: Rectangle, control: Rectangle) -> Self {
        Self::Band { band, control }
    }

    /// The rect actually handed to the solver.
    ///
    /// For [`Self::Band`] this is the bar's `y`/`height` with the entry's `x`/`width`; for
    /// [`Self::Pointer`] it is a zero-size rect at the cursor. Exposed so a host or a test
    /// can see what was anchored without re-deriving it.
    ///
    /// Negative extents are left alone — [`anchor::solve`] sanitises them to `0.0`, and
    /// duplicating that rule here would give it two definitions.
    #[must_use]
    pub fn rect(self) -> Rectangle {
        match self {
            Self::Control(rect) => rect,
            Self::Band { band, control } => Rectangle::new(
                Point::new(control.x, band.y),
                Size::new(control.width, band.height),
            ),
            Self::Pointer(point) => Rectangle::new(point, Size::ZERO),
        }
    }

    /// The [`Anchor`] this variant maps onto for the solver.
    #[must_use]
    pub fn to_anchor(self) -> Anchor {
        match self {
            Self::Control(rect) => Anchor::Control {
                rect,
                alignment: TOOLTIP_ALIGNMENT,
            },
            Self::Band { .. } => Anchor::Control {
                rect: self.rect(),
                alignment: TOOLTIP_ALIGNMENT,
            },
            Self::Pointer(point) => Anchor::Pointer { point },
        }
    }

    /// Resolve where a tooltip of `card_size` sits inside `viewport`.
    ///
    /// Runs [`anchor::solve`] with [`TOOLTIP_ALIGNMENT`] (a pointer anchor keeps the
    /// solver's own cursor offset). The returned [`Placement`] carries the flip and clamp
    /// decisions, so a caller that wants to know *why* the tooltip moved can read them
    /// instead of guessing from the rect.
    ///
    /// Degenerate inputs are the solver's, and they are defined rather than panicked: a
    /// non-finite rect sanitises to the origin, a card larger than the viewport pins its
    /// leading edge to the near margin and reports `scrollable`.
    #[must_use]
    pub fn resolve(self, card_size: Size, viewport: Rectangle) -> Placement {
        anchor::solve(self.to_anchor(), card_size, viewport)
    }
}

#[cfg(test)]
#[path = "place_tests.rs"]
mod place_tests;
