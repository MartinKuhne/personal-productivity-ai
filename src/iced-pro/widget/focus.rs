//! The adaptive keyboard focus ring — one rule every widget shares
//! (`UIF-TOK-006` rule 2, ledger decision **D-6** resolving **G-9**).
//!
//! A focus ring has exactly one job: to tell a keyboard user which
//! control is theirs, right now. A ring nobody can see is not an
//! accessibility feature wearing a slightly different colour, so this
//! module resolves the ring against the colour it actually has to stand
//! out from rather than honouring one token into invisibility.
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
//! Unit tests live in the sibling `focus_tests.rs` sidecar.

use crate::token::color::Palette;
use crate::token::contrast::{MIN_CHROME_CONTRAST, contrast_ratio};
use iced::Color;

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

#[cfg(test)]
#[path = "focus_tests.rs"]
mod focus_tests;
