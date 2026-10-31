//! The adaptive keyboard focus indicator — one colour rule and two
//! placements, shared by every widget that shows focus (`UIF-TOK-006`
//! rule 2, ledger decision **D-6** resolving **G-9**; `UIF-ATOM-000`
//! §3.0.2 rule 2).
//!
//! A focus indicator has exactly one job: to tell a keyboard user which
//! control is theirs, right now. An indicator nobody can see is not an
//! accessibility feature wearing a slightly different colour, so this
//! module resolves it against the colour it actually has to stand out
//! from rather than honouring one token into invisibility.
//!
//! # Why the rule is not a per-variant constant
//!
//! §3.1 spells each variant's focused cell as a fixed colour — Primary a
//! `text.strong` ring, the other three an `accent.border` one. That
//! works for the cell the spec snapshots, which is always `Rest + ring`.
//! [`crate::state`] makes focus *additive*, so a focused control can
//! also be hovered or pressed, and the ring then lands on a fill the
//! spec never drew it against. On a Danger button that is `accent.border`
//! indigo over a red fill at `1.7188:1` — an invisible ring at the
//! moment the user most needs one (gap **G-9**).
//!
//! Hardcoding the answer per variant cannot fix that, because the right
//! answer depends on the *resolved fill*, not on the variant: the same
//! variant needs a different ring in different states. So the rule lives
//! here, once, and every widget calls it instead of re-deriving it.
//!
//! # The rule
//!
//! Two candidates, tried in a fixed order against the fill:
//!
//! 1. [`accent_border`](crate::token::color::Palette::accent_border) —
//!    the spec's ring colour (`UIF-ATOM-000` §2). Used whenever it
//!    clears [`MIN_CHROME_CONTRAST`] (`3.0:1`) against the fill.
//! 2. [`text_strong`](crate::token::color::Palette::text_strong) — the
//!    fallback, tried when the accent cannot clear the floor.
//!
//! The order matters and is not arbitrary: the accent is the interface's
//! designated focus colour, so it wins whenever it is visible, and
//! `text.strong` is what a ring degrades to. That keeps every ring this
//! framework draws on a dark surface identical to the one the spec asks
//! for. Only a fill that swallows the accent changes colour.
//!
//! # What the polarity split did to this rule
//!
//! Before the 2026-10-22 revision of §2.5 there was no
//! `accent.border` — the ring candidate was `accent.primary`, the same
//! token that painted the CTA fill, and the rule had to fall back from a
//! *fill* token to white. The revision split the accent into
//! `accent.fill` / `accent.fill_active` / `accent.border`, which gives
//! the rule a stroke token purpose-built for the job: `accent.border` is
//! `L = 0.3020` against the fill's `0.1687`, so it clears `3.0:1`
//! against every surface in the elevation ladder (worst case
//! `4.7922:1` on `surface.active`) and the fallback only fires where it
//! genuinely must.
//!
//! The fallback now fires on **all four** saturated fills, and never on
//! a surface:
//!
//! ```text
//!   on accent.fill                 border 1.6098  strong 4.8020  -> text.strong
//!   on accent.fill_active          border 1.8961  strong 5.6559  -> text.strong
//!   on status.danger.fill          border 1.7188  strong 5.1270  -> text.strong
//!   on status.danger.fill_active   border 1.9291  strong 5.7543  -> text.strong
//! ```
//!
//! That is the D-6 rule working exactly as intended: a ring on a
//! saturated fill is white, because no indigo stroke can be made out
//! against a saturated fill, and the fill's own foreground is already
//! white so the ring reads as a continuation of the label.
//!
//! # When neither candidate clears the floor
//!
//! Both candidates can fail at once — a mid-grey fill sits close enough
//! to the accent's luminance to absorb it *and* close enough to white to
//! absorb that. That case is **not** left undefined. The resolver returns
//! whichever candidate has the **higher** contrast against the fill,
//! falling back to `text.strong` on an exact tie.
//!
//! The reasoning is that the resolver's duty is to pick the best
//! available indicator, not to refuse to draw one. Returning the loser
//! would be worse than returning a weak winner, and returning a sentinel
//! would push the same decision onto every caller — the exact thing this
//! module exists to prevent. The result may still be under `3.0:1`, and
//! that is a real accessibility shortfall rather than something this
//! function can invent contrast to fix: it means the *fill* is the
//! problem, and the fix belongs in the palette. The audit in the widget
//! sidecars keeps that visible; nothing here silently waives it.
//!
//! # A transparent background is not a fill
//!
//! [`resolve_ring_color`] answers "what ring shows against this fill?"
//! and a fully transparent colour is not a fill — nothing is painted, so
//! the ring belongs to whatever the widget is mounted on. Resolving
//! against `Color::TRANSPARENT` would be measuring a colour that never
//! appears, the same mistake [`crate::token::contrast`] warns about for the
//! scrim.
//!
//! [`resolve_ring_color_for_background`] is therefore the entry point a
//! widget should call with its resolved background: a transparent
//! background short-circuits to the spec's own ring colour,
//! [`accent_border`](crate::token::color::Palette::accent_border),
//! which clears the chrome floor against every surface in the ladder. A
//! Ghost button's ring sits on the container's surface, and that is what
//! it is measured against — not on `transparent`.
//!
//! It is worth stating plainly that the ring token does **not** clear
//! `3.0:1` against white: `accent.border` against `text.strong` is
//! `2.9829:1`, which §2.5 itself prints. A ring drawn on the inverted
//! editor surface therefore has no candidate that satisfies the chrome
//! floor, and the resolver returns `accent.border` as the higher of the
//! two rather than the white that would be invisible. That is a limit of
//! the token set on a light background, not of the rule, and the sidecar
//! pins it rather than letting the short-circuit imply the ring always
//! clears.
//!
//! # Two placements, one colour rule
//!
//! §3.0.2 asks for two focus indicators and the difference between them
//! is *placement*, not colour:
//!
//! * an **outer ring, `2px`** — buttons and inputs, drawn outside the
//!   control's border so it never clips content or overwrites an
//!   `Invalid` border;
//! * an **inset outline, `1px`** — menu and tree rows, drawn inside the
//!   row perimeter so adjacent rows in a dense list do not clip it.
//!
//! Both take their colour from the rule above, unchanged. That is not
//! convenience, it is the reason the rule lives in one file at all: a
//! second resolver for the inset case would drift away from the first
//! the moment either was retuned, and the two indicators would stop
//! meaning the same thing to the same user. So placement is modelled as
//! data — [`FocusPlacement`], which carries its own stroke weight out of
//! [`shape`] — and [`resolve_focus_indicator_for_background`] resolves
//! the *colour* through [`resolve_ring_color_for_background`] whichever
//! placement it is handed.
//!
//! What comes back is a [`FocusIndicator`]: colour, stroke and placement
//! travelling together, plus [`FocusIndicator::bounds`] and
//! [`FocusIndicator::radius`] to turn whatever box the caller has into
//! the box the stroke belongs on. A renderer never learns which rule
//! made the colour or which edge the stroke sits on; it asks for the
//! bounds and strokes them.
//!
//! # Tree rows are app-owned; the outline is not
//!
//! §3.0.2 names "Menu & Tree Rows" together, but decision **D-2** makes
//! the tree view the application's own widget, so this crate cannot paint
//! a tree row. What it can do — and what [`FocusPlacement::Inset`] exists
//! for — is hand the app the very value its own menu rows are drawn with:
//!
//! ```text
//! let outline = resolve_inset_outline_for_background(row_background, &palette);
//!
//! renderer.fill_quad(Quad {
//!     bounds: outline.bounds(row_bounds),          // inside the row, not outside
//!     border: Border::new(outline.radius(ROW_RADIUS))
//!         .color(outline.color)                   // the shared adaptive colour
//!         .width(outline.width),                  // border.hairline
//!     ..Default::default()
//! });
//! ```
//!
//! A tree row that goes through that call resolves its outline against
//! the fill it actually painted, exactly as a menu row does: a hovered
//! row gets `accent.border`, a destructive one gets `text.strong`, and
//! the application cannot re-derive — and so cannot contradict — the
//! framework's rule. Handing the app a bare `Color` instead would push
//! the width and the side back onto the caller, which is precisely where
//! an inset outline starts drifting back to `2px` and outward.
//!
//! Unit tests live in the sibling `focus_tests.rs` sidecar.

use crate::token::color::Palette;
use crate::token::contrast::{MIN_CHROME_CONTRAST, contrast_ratio};
use crate::token::shape;
use iced::{Color, Rectangle};

/// Resolve a focus ring against the fill it has to be visible on
/// (`UIF-TOK-006` rule 2, decision **D-6**).
///
/// Returns [`accent_border`](Palette::accent_border) when
/// `contrast_ratio(accent_border, fill)` reaches [`MIN_CHROME_CONTRAST`]
/// (`3.0:1`), otherwise [`text_strong`](Palette::text_strong).
///
/// When **neither** candidate clears `3.0:1` the resolver returns the
/// higher-contrast of the two rather than giving up — see the [module
/// docs](self) for why a weak indicator still beats a random one, and why
/// a fill that defeats both is a palette defect the audit must catch
/// rather than something silently waived here.
///
/// `fill` must be a colour that will actually be painted. A fully
/// transparent `fill` describes nothing being painted; call
/// [`resolve_ring_color_for_background`] instead of passing one here.
#[must_use]
pub fn resolve_ring_color(fill: Color, palette: &Palette) -> Color {
    let accent_contrast = contrast_ratio(palette.accent_border, fill);
    if accent_contrast >= MIN_CHROME_CONTRAST {
        return palette.accent_border;
    }

    let strong_contrast = contrast_ratio(palette.text_strong, fill);
    if strong_contrast >= MIN_CHROME_CONTRAST {
        return palette.text_strong;
    }

    // Neither candidate clears the floor. Pick the one that is less
    // invisible; `text.strong` wins an exact tie because it is the
    // higher-contrast token against every dark surface the framework
    // paints, so the tie-break agrees with the rule's own ordering.
    if strong_contrast >= accent_contrast {
        palette.text_strong
    } else {
        palette.accent_border
    }
}

/// Resolve a focus ring against a widget's **resolved background**
/// (`UIF-TOK-006` rule 2, decision **D-6**).
///
/// This is the entry point a widget calls. A background with zero alpha
/// paints nothing behind the ring, so the ring sits on the container's
/// surface rather than on a fill and there is nothing in the style to
/// resolve against. That case short-circuits to
/// [`accent_border`](Palette::accent_border) — the spec's own ring
/// colour, which clears `3.0:1` against every surface in the elevation
/// ladder — instead of measuring the ring against
/// `transparent`, which would report the contrast of a colour that is
/// never painted.
///
/// Any background with a non-zero alpha is resolved by
/// [`resolve_ring_color`], including a translucent one. A translucent
/// fill is measured on its own channels, the same alpha-blind reading
/// [`contrast_ratio`] makes everywhere: the ring is drawn *around* the
/// control rather than inside the fill, so the fill's own colour is the
/// adjacent one it must stand out from.
#[must_use]
pub fn resolve_ring_color_for_background(background: Color, palette: &Palette) -> Color {
    if background.a == 0.0 {
        return palette.accent_border;
    }
    resolve_ring_color(background, palette)
}

// ---------------------------------------------------------------------------
// Placement (§3.0.2 rule 2)
// ---------------------------------------------------------------------------

/// Which side of a control's own perimeter its focus indicator is drawn
/// on (`UIF-ATOM-000` §3.0.2 rule 2).
///
/// The two placements are not stylistic variants of one another; they are
/// answers to two different failure modes, which is why the placement is
/// data rather than a caller-supplied number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FocusPlacement {
    /// `2px` *outside* the control's border — buttons and inputs.
    ///
    /// Growing the box rather than shrinking it is what keeps the ring
    /// from clipping a control's content or overwriting the `Invalid`
    /// border, which shares that same perimeter (§3.2 gives an invalid
    /// input a `2px status.danger.text` border and still wants its focus
    /// ring).
    Outer,
    /// `1px` *inside* the row's perimeter — menu and tree rows.
    ///
    /// A dense list clips at the row boundary, so a ring grown outward
    /// is cut by the rows above and below it — worst at the top and
    /// bottom of the card, where there is nothing beyond the row to draw
    /// into. Keeping the stroke inside the perimeter makes the whole
    /// indicator visible at any row pitch.
    Inset,
}

impl FocusPlacement {
    /// The stroke weight §3.0.2 fixes for this placement.
    ///
    /// Both come from [`shape`] rather than a literal: the outer ring is
    /// the attention weight (`border.focus`) and the inset outline is
    /// the structural weight (`border.hairline`). The two are different
    /// rungs on purpose, so a row's outline never reads as a button's
    /// ring that lost a pixel.
    #[must_use]
    pub const fn stroke(self) -> f32 {
        match self {
            Self::Outer => shape::BORDER_FOCUS,
            Self::Inset => shape::BORDER_HAIRLINE,
        }
    }

    /// The rectangle the stroke of `width` is drawn on, given the
    /// control's own box.
    ///
    /// iced strokes a quad **inward from its bounds** — `border_width`
    /// pixels of stroke, inside the edge — so a placement is entirely a
    /// statement about whether that rectangle was grown out of the
    /// control's box or left on it:
    ///
    /// * [`Outer`](Self::Outer) grows it by `width`, which puts the
    ///   stroke's *inner* edge exactly on the control's border and the
    ///   whole ring outside it;
    /// * [`Inset`](Self::Inset) leaves it where it is, which puts the
    ///   stroke's *outer* edge exactly on the row's perimeter and the
    ///   whole outline inside it.
    ///
    /// Shrinking the rectangle for an inset stroke would not move the
    /// stroke inside the row — it would move it inside the row *and*
    /// `width` further in, leaving a gap of fill between the perimeter
    /// and the indicator. That is a third placement §3.0.2 does not
    /// describe.
    #[must_use]
    pub fn bounds(self, control: Rectangle, width: f32) -> Rectangle {
        match self {
            Self::Outer => control.expand(width),
            Self::Inset => control,
        }
    }

    /// The corner radius to stroke with, for a control of `radius`.
    ///
    /// The outer ring needs its own, larger radius to stay concentric
    /// with the control it rings: its outer edge is `width` further out
    /// than the control's, so the corner has to be `width` rounder too
    /// or the ring cuts the corner it is meant to follow.
    ///
    /// The inset outline needs no adjustment at all. Its outer edge *is*
    /// the control's edge, so it rides the control's own radius and the
    /// corner that gets flattened by the stroke is the corner the fill
    /// already drew.
    #[must_use]
    pub const fn radius(self, radius: f32, width: f32) -> f32 {
        match self {
            Self::Outer => radius + width,
            Self::Inset => radius,
        }
    }
}

// ---------------------------------------------------------------------------
// The resolved indicator
// ---------------------------------------------------------------------------

/// A resolved focus indicator: the colour the adaptive rule picked, the
/// stroke the placement calls for, and the edge it is drawn on.
///
/// The three travel together deliberately. A bare colour cannot be drawn
/// without a width and a side, and letting a caller supply either by
/// hand is exactly how an inset outline ends up `2px`, or a button's
/// ring ends up inside its own border.
/// [`resolve_focus_indicator_for_background`] is the one place all
/// three are chosen together, and [`bounds`](Self::bounds) /
/// [`radius`](Self::radius) turn whatever box the caller holds into the
/// box the stroke belongs on — so no call site can put the two
/// placements' geometry in the wrong order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FocusIndicator {
    /// The stroke colour, resolved by the shared adaptive rule
    /// ([`resolve_ring_color_for_background`]).
    pub color: Color,
    /// The stroke weight, taken from [`FocusPlacement::stroke`].
    pub width: f32,
    /// Which side of the control's perimeter the stroke is drawn on.
    pub placement: FocusPlacement,
}

impl FocusIndicator {
    /// The `2px` outer ring §3.0.2 gives buttons and inputs, in `color`.
    #[must_use]
    pub const fn outer(color: Color) -> Self {
        Self {
            color,
            width: shape::BORDER_FOCUS,
            placement: FocusPlacement::Outer,
        }
    }

    /// The `1px` inset outline §3.0.2 gives menu and tree rows, in
    /// `color`.
    #[must_use]
    pub const fn inset(color: Color) -> Self {
        Self {
            color,
            width: shape::BORDER_HAIRLINE,
            placement: FocusPlacement::Inset,
        }
    }

    /// The rectangle this indicator's stroke is drawn on, given the
    /// control's own box.
    #[must_use]
    pub fn bounds(self, control: Rectangle) -> Rectangle {
        self.placement.bounds(control, self.width)
    }

    /// The corner radius that keeps this indicator concentric with a
    /// control of `radius`.
    #[must_use]
    pub fn radius(self, radius: f32) -> f32 {
        self.placement.radius(radius, self.width)
    }
}

// ---------------------------------------------------------------------------
// The shared resolution
// ---------------------------------------------------------------------------

/// Resolve a focus indicator for a widget's **resolved background**, for
/// either placement (`UIF-ATOM-000` §3.0.2 rule 2, decision **D-6**).
///
/// The colour is [`resolve_ring_color_for_background`] — the same call,
/// with the same candidates, the same ordering and the same transparent-
/// background short-circuit, that produces the outer ring on a button.
/// `placement` changes the geometry and nothing else. There is
/// deliberately no second colour rule here: two rules tuned separately
/// stop meaning the same thing, and §3.0.2 says so in one sentence that
/// covers both placements.
///
/// So the two focused cells of §4.9.2 are arithmetic rather than
/// policy. A normal focused row sits on `surface.hover`, where
/// `accent.border` measures `5.4427:1` and wins; a destructive focused
/// row sits on `status.danger_fill`, where the accent measures
/// `1.7188:1` and loses to `text.strong` at `5.1270:1`. Neither cell
/// names its outline colour.
#[must_use]
pub fn resolve_focus_indicator_for_background(
    background: Color,
    palette: &Palette,
    placement: FocusPlacement,
) -> FocusIndicator {
    FocusIndicator {
        color: resolve_ring_color_for_background(background, palette),
        width: placement.stroke(),
        placement,
    }
}

/// The `1px` inset focus outline §3.0.2 gives a menu or tree row,
/// resolved against the row's own background.
///
/// [`resolve_focus_indicator_for_background`] with
/// [`FocusPlacement::Inset`], so a row-oriented caller never has to
/// name a placement it has already committed to by asking for an inset
/// outline. This is also the entry point an application-owned tree row
/// uses — see the [module docs](self) for the call shape.
#[must_use]
pub fn resolve_inset_outline_for_background(
    background: Color,
    palette: &Palette,
) -> FocusIndicator {
    resolve_focus_indicator_for_background(background, palette, FocusPlacement::Inset)
}

#[cfg(test)]
#[path = "focus_tests.rs"]
mod focus_tests;
