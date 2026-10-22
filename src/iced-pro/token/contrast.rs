//! WCAG 2.1 contrast measurement for the token set (`UIF-TOK-006`,
//! measurement half of `UIF-VER-001`).
//!
//! This module answers one question — *can this foreground be read
//! against that background?* — and answers it as a number rather than an
//! opinion. It resolves no styles, owns no palette and touches no I/O:
//! [`relative_luminance`], [`contrast_ratio`] and [`composite_over`] are
//! pure functions over `iced::Color`, which is what makes the
//! `UIF-VER-001` gate checkable without a window.
//!
//! # The maths
//!
//! **Relative luminance** (WCAG 2.1) linearises each sRGB channel with
//! the standard piecewise transfer function and then combines the
//! linearised channels with the Rec. 709 luminance weights:
//!
//! ```text
//!                     | c / 12.92                        c <= 0.03928
//!   C(c) =            | ((c + 0.055) / 1.055) ^ 2.4      otherwise
//!
//!   L = 0.2126 * C(R) + 0.7152 * C(G) + 0.0722 * C(B)
//! ```
//!
//! The two branches meet at the `0.03928` knee, where the linear segment
//! approximates the toe of the power curve. The exponent is applied to
//! a *channel*, never to a colour, and the weights sum to `1.0`, so
//! [`relative_luminance`] lands in `[0.0, 1.0]` for any in-gamut
//! colour: `0.0` for black, `1.0` for white.
//!
//! **Contrast ratio** is the WCAG 2.1 formula
//!
//! ```text
//!   (L_lighter + 0.05) / (L_darker + 0.05)
//! ```
//!
//! which is symmetric in its arguments — [`contrast_ratio`] takes the
//! lighter luminance first regardless of parameter order, so
//! `contrast_ratio(fg, bg) == contrast_ratio(bg, fg)` always — and is
//! bounded to `[1.0, 21.0]`: `1.0` for identical colours, `21.0` for
//! black against white.
//!
//! # What the thresholds actually cover
//!
//! [`MIN_TEXT_CONTRAST`] is `UIF-TOK-006` rule 1: normal text (below
//! 18px) against its immediate background. Every rung of the type scale
//! except [`crate::token::typography::TITLE_MD`] and
//! [`crate::token::typography::TITLE_LG`] sits under 18px, so rule 1
//! governs essentially all text this framework draws — including
//! [`crate::token::typography::CAPTION`], which sits on the
//! [`crate::token::typography::MIN_SIZE`] floor and is therefore the
//! smallest, most contrast-hungry text there is.
//!
//! [`MIN_CHROME_CONTRAST`] is `UIF-TOK-006` rule 2, whose wording is
//! the scope: "UI chrome, active borders, and focus rings MUST maintain
//! at least 3.0:1 contrast against adjacent surfaces." Measured against
//! the token set, that reads as follows.
//!
//! * **In scope** — things that signal a *change* of state:
//!   [`accent_primary`](crate::token::color::Palette::accent_primary) as
//!   a keyboard focus ring, active tab top indicator, selected-row
//!   outline or splitter drag stroke;
//!   [`border_strong`](crate::token::color::Palette::border_strong) as
//!   the perimeter of a floating card, and as the outline a latched
//!   navbar trigger wears. Both clear 3.0 against every surface they are
//!   actually specified to sit on.
//! * **Out of scope** —
//!   [`border_subtle`](crate::token::color::Palette::border_subtle).
//!   It is a **resting** divider: pane split lines at rest, menu
//!   separators, resting input borders, inactive tab borders. It never
//!   signals a state change; the moment a divider becomes active the spec
//!   swaps it for `accent.primary` (`UIF-PANE-001`). Measured, it tops
//!   out at `1.3732:1` against `surface.sunken` and sits at `1.0203:1`
//!   against `surface.hover`, so no reading of rule 2 can rescue it.
//!   Applying the 3.0 floor to it would fail every divider in the
//!   interface for a token that was never meant to be an indicator, so
//!   rule 2 is **not** asserted against it. That exclusion is a reading
//!   of the word "active" in the rule, not an exemption granted to a
//!   failing token, and the tests name it explicitly rather than quietly
//!   dropping the token from the table.
//!
//! # Translucent tokens must be composited before they are measured
//!
//! [`contrast_ratio`] deliberately ignores alpha: luminance is a
//! property of a *painted* colour, and a translucent token has not been
//! painted yet. Measuring a translucent token directly is meaningless —
//! [`surface_scrim`](crate::token::color::Palette::surface_scrim) is
//! `surface.base` at 55% alpha, so a naive ratio against the canvas it
//! dims comes out at exactly `1.0000:1`, and against white it reports
//! `19.8954:1` for a layer that visibly darkens that white to mid
//! grey. Ask the question *after* [`composite_over`] has laid the layer
//! down: the scrim over `surface.base` composites back to base itself,
//! and over white it lands on `RGB(119.7, 119.7, 120.8)`, where
//! `text.strong` measures a sane `4.4289:1`.
//!
//! # The spec's printed contrast column is not authoritative
//!
//! Ledger gap **G-5** (`doc/planning/iced-pro-tasks.md`) establishes
//! that the `Luminance Contrast vs color.surface.base` column in
//! `doc/planning/ui-framework.md` §2.5 was hand-collected from mixed
//! sources and is materially wrong in several rows — `surface.sunken`
//! is printed as `0.98:1`, which no WCAG ratio can be, and
//! `status.warning` is off by `+1.36`. **The values this module
//! computes are authoritative.** Nothing here transcribes a printed
//! ratio; the reconciliation lives in the test sidecar, which recomputes
//! every row and asserts the *role* threshold rather than the printed
//! figure.
//!
//! # Retuned token values
//!
//! Five pairings the spec mandates could not clear the threshold the spec
//! itself sets at the spec's literal token values. They were fixed by
//! retuning three tokens rather than by weakening the rules — see the
//! sidecar's `RETUNE_HEADROOM` assertions for the measured value of each:
//!
//! * `text.strong` on `accent.primary` — the Selection Foreground Lock
//!   (`UIF-TOK-006` rule 3) — measured `4.4669:1`, `0.0331` short of
//!   the 4.5:1 normal-text rule. `accent.primary` is darkened to
//!   `RGB(98, 100, 240)`, giving `4.5648:1`.
//! * `status.danger` on `surface.overlay` (`4.1473:1`) and on
//!   `surface.raised` (`4.3800:1`) — the input-dialog validation slot
//!   and the Danger button's resting foreground. `status.danger` is
//!   lightened to `RGB(255, 28, 2)`, giving `4.5718:1` and `4.8283:1`.
//! * `accent.primary` on `surface.active` (`2.8441:1`) and
//!   `border.strong` on `surface.active` (`2.8775:1`) — both under the
//!   3.0:1 chrome floor. `surface.active` is darkened to
//!   `RGB(43, 43, 51)`, giving `3.0750:1` and `3.1793:1`.
//!
//! `accent.primary` could not absorb both the first and the last failure:
//! the lock caps its luminance at `0.183333` while the chrome floor
//! demands at least `0.197944` against an unretuned `surface.active`.
//! That contradiction is why the fix is spread over three tokens instead
//! of one.
//!
//! The spec has not yet been revised to match. Until it is, the retuned
//! values in [`crate::token::color`] are authoritative and the deviation
//! is tracked as `G-7` in `doc/planning/iced-pro-tasks.md`.
//!
//! Unit tests live in the sibling `contrast_tests.rs` sidecar.

use iced::Color;

/// Minimum contrast normal text (`< 18px`) must hold against its
/// immediate background (`UIF-TOK-006` rule 1).
///
/// This is the WCAG 2.1 AA threshold for normal-size text, adopted
/// verbatim. Because [`crate::token::typography::MIN_SIZE`] is `12px` and only the two
/// largest rungs reach 18px, this is the threshold essentially every
/// label in the interface is measured against.
pub const MIN_TEXT_CONTRAST: f32 = 4.5;

/// Minimum contrast UI chrome — active borders and focus rings — must
/// hold against adjacent surfaces (`UIF-TOK-006` rule 2).
///
/// Read the rule as covering *active* indicators only. See the module
/// doc for why [`crate::token::color::Palette::border_subtle`] is not one of them.
pub const MIN_CHROME_CONTRAST: f32 = 3.0;

/// sRGB transfer-function knee: below this a channel is linear, above it
/// the power curve takes over.
const KNEE: f32 = 0.03928;

/// Divisor for the linear segment of the sRGB transfer function.
const LINEAR_DIVISOR: f32 = 12.92;

/// Additive offset of the power segment of the sRGB transfer function.
const POWER_OFFSET: f32 = 0.055;

/// Divisor of the power segment of the sRGB transfer function.
const POWER_SCALE: f32 = 1.055;

/// Exponent of the sRGB power segment.
const POWER_EXPONENT: f32 = 2.4;

/// Rec. 709 luminance weight of the red channel.
const WEIGHT_RED: f32 = 0.2126;

/// Rec. 709 luminance weight of the green channel.
const WEIGHT_GREEN: f32 = 0.7152;

/// Rec. 709 luminance weight of the blue channel.
const WEIGHT_BLUE: f32 = 0.0722;

/// The `+0.05` both sides of the contrast ratio carry, which models the
/// minimum reflectance of a real display and keeps the ratio finite when
/// a black-on-black pairing is measured.
const CONTRAST_OFFSET: f32 = 0.05;

/// Linearise one sRGB channel (`C(c)` in the module doc).
///
/// The channel is clamped to `[0.0, 1.0]` first. `iced::Color` does not
/// enforce that range, and the transfer function is undefined outside it:
/// a negative channel raised to the fractional exponent `2.4` yields
/// `NaN`, which a layout engine propagates silently into collapsed or
/// unbounded contrast. Clamping keeps this a total function whose worst
/// case is a saturated-but-finite luminance.
fn linearize(channel: f32) -> f32 {
    let c = channel.clamp(0.0, 1.0);
    if c <= KNEE {
        c / LINEAR_DIVISOR
    } else {
        ((c + POWER_OFFSET) / POWER_SCALE).powf(POWER_EXPONENT)
    }
}

/// WCAG 2.1 relative luminance of a colour, in `[0.0, 1.0]`.
///
/// Each channel is linearised with the sRGB piecewise transfer function
/// and combined with the Rec. 709 weights:
///
/// ```text
///   C(c) = c / 12.92                      if c <= 0.03928
///        = ((c + 0.055) / 1.055) ^ 2.4    otherwise
///   L    = 0.2126 * C(R) + 0.7152 * C(G) + 0.0722 * C(B)
/// ```
///
/// Alpha is deliberately ignored. Luminance describes a colour that has
/// been painted; a translucent token has not been painted yet, so its
/// luminance is a number about nothing. Composite it first with
/// [`composite_over`] and measure the result.
#[must_use]
pub fn relative_luminance(color: Color) -> f32 {
    WEIGHT_RED * linearize(color.r)
        + WEIGHT_GREEN * linearize(color.g)
        + WEIGHT_BLUE * linearize(color.b)
}

/// WCAG 2.1 contrast ratio between two colours, in `[1.0, 21.0]`.
///
/// ```text
///   (L_lighter + 0.05) / (L_darker + 0.05)
/// ```
///
/// **Symmetric in its arguments:** the lighter luminance is always put
/// in the numerator, so `contrast_ratio(a, b) == contrast_ratio(b, a)`
/// for every pair. Callers therefore need not care which way round they
/// hold the foreground and background, and a ratio can never come back
/// below `1.0` by being inverted — which is exactly the mistake that
/// produced the spec's impossible `0.98:1` for `surface.sunken`
/// (gap **G-5**).
///
/// Identical colours give `1.0`; black against white gives `21.0`.
/// Alpha is ignored for the reason given on [`relative_luminance`]:
/// composite a translucent layer with [`composite_over`] before asking.
#[must_use]
pub fn contrast_ratio(a: Color, b: Color) -> f32 {
    let luminance_a = relative_luminance(a);
    let luminance_b = relative_luminance(b);
    let (lighter, darker) = if luminance_a >= luminance_b {
        (luminance_a, luminance_b)
    } else {
        (luminance_b, luminance_a)
    };
    (lighter + CONTRAST_OFFSET) / (darker + CONTRAST_OFFSET)
}

/// Source-over (`Porter-Duff "over"`) compositing of a translucent
/// foreground onto a background.
///
/// ```text
///   a_out = a_fg + a_bg * (1 - a_fg)
///   rgb_out = (rgb_fg * a_fg + rgb_bg * a_bg * (1 - a_fg)) / a_out
/// ```
///
/// Both inputs and the result are **un-premultiplied**, the form
/// `iced::Color` stores, which is why the blend is divided back out by
/// `a_out`. Consequences worth having:
///
/// * an opaque foreground returns the foreground unchanged;
/// * a fully transparent foreground returns the background unchanged;
/// * compositing the scrim over `surface.base` returns `surface.base`
///   exactly, because the scrim *is* base at 55% alpha.
///
/// This exists because [`surface_scrim`](crate::token::color::Palette::surface_scrim)
/// is translucent and any contrast question about it can only be asked
/// *after* it has been laid over something. **Contrast must never be
/// measured on a translucent token directly** — the alpha is the rule,
/// the resulting colour is not a token, and the surface underneath is
/// part of the answer.
#[must_use]
pub fn composite_over(fg: Color, bg: Color) -> Color {
    let out_alpha = fg.a + bg.a * (1.0 - fg.a);
    if out_alpha <= 0.0 {
        return Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        };
    }
    let weighted_bg = bg.a * (1.0 - fg.a);
    let blend = |f: f32, b: f32| (f * fg.a + b * weighted_bg) / out_alpha;
    Color {
        r: blend(fg.r, bg.r),
        g: blend(fg.g, bg.g),
        b: blend(fg.b, bg.b),
        a: out_alpha,
    }
}

/// Does `fg` read against `bg` as normal text (`UIF-TOK-006` rule 1)?
///
/// True when [`contrast_ratio`] reaches [`MIN_TEXT_CONTRAST`] (`4.5:1`).
/// The comparison is inclusive: exactly `4.5:1` passes.
///
/// A translucent `fg` must be composited over `bg` with
/// [`composite_over`] before being passed here, or the answer describes
/// a colour that will never appear on screen.
#[must_use]
pub fn meets_text_contrast(fg: Color, bg: Color) -> bool {
    contrast_ratio(fg, bg) >= MIN_TEXT_CONTRAST
}

/// Do these two chrome colours read against each other
/// (`UIF-TOK-006` rule 2)?
///
/// True when [`contrast_ratio`] reaches [`MIN_CHROME_CONTRAST`]
/// (`3.0:1`). The comparison is inclusive.
///
/// Applies to *active* chrome — focus rings, active borders, selected
/// outlines. It does not apply to resting dividers such as
/// [`border_subtle`](crate::token::color::Palette::border_subtle); see the module doc
/// for that scope call.
#[must_use]
pub fn meets_chrome_contrast(a: Color, b: Color) -> bool {
    contrast_ratio(a, b) >= MIN_CHROME_CONTRAST
}

#[cfg(test)]
#[path = "contrast_tests.rs"]
mod contrast_tests;
