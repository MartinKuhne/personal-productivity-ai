//! Optical box versus em box (`UIF-PRIN-006`, §1.1 pillar 6).
//!
//! §1.1: *"directional glyphs (`▸`, `▾`, `◀`, `▶`) and circular/pill
//! badges center on their optical bounding weight inside `16×16px` /
//! `24×24px` slots."*
//!
//! # The honest boundary
//!
//! A glyph's optical centre is a property of the *font's outlines*. This
//! crate has no font loaded at the point these rules are evaluated, and
//! inventing a table of per-glyph insets — `▸ is 1.3px right of centre`
//! — would be transcribing numbers nobody measured, against a typeface
//! that may not be the one eventually rendered. That is worse than not
//! building the mechanism, because it looks like data.
//!
//! So the split is drawn here rather than faked:
//!
//! * **Built** — the mechanism that separates a glyph's optical box
//!   from its em box, and the arithmetic that centres one inside the
//!   other. This is where the *concept* lives, and it is fully tested.
//! * **Host-supplied** — the actual insets. A host that has measured
//!   `▾` against the typeface it ships passes the numbers in; a host
//!   that has not passes [`OpticalInset::ZERO`] and gets geometric
//!   centring, which is the correct answer for a symmetric glyph and a
//!   visible, attributable compromise for an asymmetric one.
//!
//! This is a genuine framework boundary, recorded in the
//! [`UIF-PRIN-006`](crate::spec_coverage) ledger entry: the framework
//! supplies the *shape* of optical centring, never the measurements.
//!
//! # The model
//!
//! A glyph is drawn into an **em box** — the slot. Its visible ink is
//! smaller than that box and usually not evenly surrounded by the
//! leftover space: a `▸` leaves more room on its left than its right.
//! [`OpticalInset`] records those four gaps. From it:
//!
//! * [`optical_box`] answers *where the ink sits* inside the em box.
//! * [`optical_shift`] answers *how far to move the em box* so the ink
//!   lands on the slot's geometric centre.
//!
//! Geometric centring is the symmetric special case: a zero or symmetric
//! inset produces a zero shift, which pins that the mechanism degrades
//! to today's behaviour rather than changing it.
//!
//! Unit tests live in the sibling `optical_tests.rs` sidecar.

use crate::token::size;
use iced::Rectangle;

/// The small glyph slot — `16×16px` (`UIF-PRIN-006`).
///
/// Read from [`size::ICON_MD`] rather than restated.
pub const GLYPH_SLOT_SM: f32 = size::ICON_MD;

/// The large glyph slot — `24×24px` (`UIF-PRIN-006`).
///
/// Read from [`size::HIT_TARGET_MIN`] rather than restated. It is the
/// same figure the hit-target floor uses, which is not a coincidence:
/// §1.1's larger optical slot and the minimum pointer target are the
/// same square for different reasons.
pub const GLYPH_SLOT_MD: f32 = size::HIT_TARGET_MIN;

/// The gap between a glyph's em box and its optical (ink) box
/// (`UIF-PRIN-006`).
///
/// Positive values mean the ink is inset from that edge of the em box.
/// An asymmetric inset is the whole point: it is what makes a `▸` sit
/// right-of-centre in a box that is geometrically centred, and therefore
/// what [`optical_shift`] exists to correct.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OpticalInset {
    /// Gap from the em box's left edge to the ink.
    pub left: f32,
    /// Gap from the em box's top edge to the ink.
    pub top: f32,
    /// Gap from the ink to the em box's right edge.
    pub right: f32,
    /// Gap from the ink to the em box's bottom edge.
    pub bottom: f32,
}

impl OpticalInset {
    /// No optical difference — the ink fills the em box.
    ///
    /// The identity element: [`optical_box`] with this inset returns the
    /// full slot, and [`optical_shift`] returns no movement. A host with
    /// no measurements uses this and gets exactly geometric centring.
    pub const ZERO: Self = Self {
        left: 0.0,
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
    };

    /// An explicit four-edge inset.
    ///
    /// The host-supplied form: the numbers come from measuring the glyph
    /// against the typeface the host actually ships.
    #[must_use]
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    /// A symmetric inset — equal gaps horizontally and vertically.
    ///
    /// The shape that produces a zero shift while still shrinking the
    /// optical box, i.e. a glyph that is optically centred but smaller
    /// than its slot.
    #[must_use]
    pub const fn symmetric(horizontal: f32, vertical: f32) -> Self {
        Self {
            left: horizontal,
            top: vertical,
            right: horizontal,
            bottom: vertical,
        }
    }

    /// The inset that centres an ink box of `ink` inside a square `slot`.
    #[must_use]
    pub const fn centring_square(slot: f32, ink: f32) -> Self {
        Self::symmetric((slot - ink) / 2.0, (slot - ink) / 2.0)
    }

    /// The inset that centres an ink box of `ink_width` × `ink_height`
    /// inside a `slot_width` × `slot_height` em box.
    ///
    /// The pill-badge form: a pill's ink is wide and short, so the
    /// vertical inset is much larger than the horizontal one and the
    /// optical box still centres in the square slot.
    #[must_use]
    pub const fn centring(
        slot_width: f32,
        slot_height: f32,
        ink_width: f32,
        ink_height: f32,
    ) -> Self {
        Self {
            left: (slot_width - ink_width) / 2.0,
            top: (slot_height - ink_height) / 2.0,
            right: (slot_width - ink_width) / 2.0,
            bottom: (slot_height - ink_height) / 2.0,
        }
    }

    /// Total horizontal gap the inset accounts for.
    #[must_use]
    pub const fn total_horizontal(self) -> f32 {
        self.left + self.right
    }

    /// Total vertical gap the inset accounts for.
    #[must_use]
    pub const fn total_vertical(self) -> f32 {
        self.top + self.bottom
    }

    /// This inset with every negative edge folded to zero.
    ///
    /// A negative gap would mean the ink extends *past* the em box,
    /// which this model has no meaning for — the slot is the boundary.
    /// Folding is applied inside [`optical_box`] so the returned
    /// geometry is always drawable; [`Self::exceeds`] is how a caller
    /// learns the inset was impossible rather than discovering it was
    /// silently swallowed.
    #[must_use]
    pub const fn clamped(self) -> Self {
        Self {
            left: if self.left > 0.0 { self.left } else { 0.0 },
            top: if self.top > 0.0 { self.top } else { 0.0 },
            right: if self.right > 0.0 { self.right } else { 0.0 },
            bottom: if self.bottom > 0.0 { self.bottom } else { 0.0 },
        }
    }

    /// Whether the inset asks for more space than the em box has.
    #[must_use]
    pub fn exceeds(&self, slot_width: f32, slot_height: f32) -> bool {
        self.total_horizontal() > slot_width || self.total_vertical() > slot_height
    }

    /// Whether the horizontal and vertical gaps are each equal on both
    /// sides — the condition under which no shift is needed.
    #[must_use]
    pub fn is_symmetric(&self) -> bool {
        self.left == self.right && self.top == self.bottom
    }
}

/// How far to move an em box so its ink lands on the slot's geometric
/// centre (`UIF-PRIN-006`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OpticalShift {
    /// Horizontal movement to apply to the em box.
    pub x: f32,
    /// Vertical movement to apply to the em box.
    pub y: f32,
}

/// Where a glyph's ink sits inside its em box (`UIF-PRIN-006`).
///
/// The returned rectangle is expressed relative to the em box's own
/// origin: `(0, 0)` is the em box's top-left. Negative insets are
/// folded to zero before the box is computed, and the result is clamped
/// so neither dimension goes below zero — the optical box can never be
/// larger than the slot it lives in, and never negative.
///
/// * `slot_width` / `slot_height` — the em box.
/// * `inset` — the host-supplied optical gaps.
#[must_use]
pub fn optical_box(slot_width: f32, slot_height: f32, inset: OpticalInset) -> Rectangle {
    let inset = inset.clamped();
    Rectangle {
        x: inset.left,
        y: inset.top,
        width: (slot_width - inset.total_horizontal()).max(0.0),
        height: (slot_height - inset.total_vertical()).max(0.0),
    }
}

/// The movement that centres a glyph's optical box on its slot's
/// geometric centre (`UIF-PRIN-006`).
///
/// Derived from the inset alone: if the ink leaves `left` on one side and
/// `right` on the other, its centre sits `(left - right) / 2` off the
/// em box's centre, so the em box moves by `(right - left) / 2` to put
/// it back. A symmetric inset therefore produces no movement, which is
/// what makes this a strict refinement of geometric centring rather than
/// a change to it.
#[must_use]
pub fn optical_shift(inset: OpticalInset) -> OpticalShift {
    let inset = inset.clamped();
    OpticalShift {
        x: (inset.right - inset.left) / 2.0,
        y: (inset.bottom - inset.top) / 2.0,
    }
}

#[cfg(test)]
#[path = "optical_tests.rs"]
mod optical_tests;
