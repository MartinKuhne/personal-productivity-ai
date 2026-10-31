//! WCAG 2.1 contrast measurement for the token set (`UIF-TOK-006`,
//! measurement half of `UIF-VER-001`).
//!
//! This module answers one question — *can this foreground be read
//! against that background?* — and answers it as a number rather than an
//! opinion. It resolves no styles, owns no palette and touches no I/O:
//! [`relative_luminance`], [`contrast_ratio`] and [`composite_over`] are
//! pure functions over `iced::Color`, which is what makes the
//! `UIF-VER-001` gate checkable without a window. The one place a
//! [`Palette`] does arrive is [`fill_lock`], and it arrives as an
//! argument rather than a possession: rule 4 is a rule about which
//! *token* a colour is, so the palette is read for role identity and
//! never stored.
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
//! Rule 2 is not a floor at all. It takes
//! [`text_disabled`](crate::token::color::Palette::text_disabled) out
//! from under the `4.50:1` rule and puts it inside a *band*,
//! [`DISABLED_TEXT_MIN_CONTRAST`] to [`DISABLED_TEXT_MAX_CONTRAST`]:
//! dim enough not to compete with live text, bright enough still to be
//! read. A band needs both bounds asserted, and asserting only the floor
//! would let a disabled label drift up into the active range unnoticed.
//!
//! [`MIN_CHROME_CONTRAST`] is rule 3: active control borders and solid
//! control fills against their adjacent surface. The revised rule does
//! not treat every stroke as the same kind of thing. It classifies each
//! one into one of four categories, and they are not held to the same
//! number — see [`ChromeClass`], which makes that classification an
//! executable part of this module rather than prose:
//!
//! | Class | What it is | Threshold |
//! |---|---|---|
//! | [`Judged`](ChromeClass::Judged) | Active perimeter borders and focus rings | `>= 3.00:1` |
//! | [`CoincidentWithFill`](ChromeClass::CoincidentWithFill) | A `1px` stroke set to the *same* token as the solid fill it surrounds | `>= 3.00:1`, carried by the fill |
//! | [`UnpaintedBackground`](ChromeClass::UnpaintedBackground) | A `transparent` stroke at rest (Ghost) | judged against the parent surface, and only once focused or hovered |
//! | [`RestingDivider`](ChromeClass::RestingDivider) | Passive structural hairlines (`border.subtle`) | exempt |
//!
//! The `Judged` set is [`border_strong`](crate::token::color::Palette::border_strong),
//! [`accent_border`](crate::token::color::Palette::accent_border) and
//! [`status_danger_text`](crate::token::color::Palette::status_danger_text)
//! measured over `surface.base` through `surface.active`. The
//! `CoincidentWithFill` set is
//! [`accent_fill`](crate::token::color::Palette::accent_fill),
//! [`accent_fill_active`](crate::token::color::Palette::accent_fill_active)
//! and
//! [`status_danger_fill_active`](crate::token::color::Palette::status_danger_fill_active)
//! measured over `surface.base` through `surface.overlay`.
//!
//! `CoincidentWithFill` keeps a threshold of its own because the stroke
//! carries no information that the fill does not. When a button's border
//! *is* its fill, nothing at the perimeter separates the control from
//! the surface behind it, so the fill has to do the whole job on its
//! own — and that is a different question from "does this visible stroke
//! read against its neighbour", which is what [`MIN_CHROME_CONTRAST`]
//! answers. The two numbers happen to be equal at `3.00:1` since the
//! 2026-10-23 reconciliation; they stay separate constants because they
//! are asked of different things. See
//! [`MIN_COINCIDENT_FILL_CONTRAST`] for why merging them would be wrong.
//!
//! [`border_subtle`](crate::token::color::Palette::border_subtle) is a
//! `RestingDivider`: pane split lines at rest, menu separators,
//! resting input borders, inactive tab borders. It never signals a
//! *change* of state, and the moment a divider becomes active the spec
//! swaps it for a `Judged` token (`UIF-PANE-001`). Measured, it tops
//! out at `1.3732:1` against `surface.sunken` and falls to `1.0314:1`
//! against `surface.active`, so no reading of rule 3 can rescue it.
//! Holding a resting hairline to an active-indicator threshold would
//! fail every divider in the interface over a token that was never meant
//! to indicate anything. That exemption is a reading of the word
//! "active" in the rule, not a waiver granted to a failing token, and
//! the tests name the class explicitly rather than quietly dropping the
//! token from the table.
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
//! # Rule 4 — the solid-fill foreground lock, made checkable
//!
//! Rule 4 is the one rule in §2.5 that is not a threshold. It does not ask
//! a colour to reach a number; it asks that a *role* always be paired with
//! one specific colour:
//!
//! > Whenever `color.accent.fill`, `color.accent.fill_active`,
//! > `color.status.danger.fill`, or `color.status.danger.fill_active` is
//! > used as a background fill, the foreground text and icon color MUST
//! > lock to `color.text.strong`.
//!
//! [`fill_lock`] answers that as a verdict — [`FillLock`] — over a
//! resolved `(background, foreground)` pair, and
//! [`assert_fill_foreground_lock`] is the shield the resolvers call while
//! they are building a style. Three things about the reading matter:
//!
//! * **The rule is about roles, not hues.** Which colour is a "locked
//!   solid fill" is a fact about the palette, not about the RGB, so
//!   [`fill_lock_role`] reads the role off [`Palette`] by value identity
//!   the same way the gallery reads a colour back to its token name. A
//!   palette that retunes `accent.fill` moves the lock with it; nothing
//!   here carries a colour literal of its own.
//! * **A `transparent` background is not a fill.** Rule 4 governs
//!   *solid* fills, and a colour with zero alpha paints nothing, so it is
//!   [`Unlocked`](FillLockRole::Unlocked) and a Ghost button's resting
//!   cell — `transparent` under `text.primary` — is legal rather than
//!   merely tolerated. The same is true of a foreground with zero alpha:
//!   nothing is drawn in that slot, so there is no text or icon for the
//!   rule to protect ([`ForegroundNotPainted`](FillLock::ForegroundNotPainted)).
//! * **The scrim is excluded, and the reason is the same one.**
//!   [`surface_scrim`](crate::token::color::Palette::surface_scrim) is a
//!   *translucent* token: it is not a fill that carries a label, it is a
//!   dimming layer that composites over something else, and its own
//!   painted colour depends on what is underneath. Rule 4's four fills are
//!   opaque tokens that a label sits directly on. A layer with `a < 1.0`
//!   is therefore never a locked fill, which is why
//!   [`fill_lock_role`] short-circuits on alpha before it compares
//!   anything.
//!
//! The lock holds across the four fills at `4.8020:1` to `5.7543:1`
//! (the spec prints `4.80:1` to `5.75:1`). The prohibition half is
//! protective in ten of its twelve pairings — every foreground-only
//! token lands under the `3.00:1` chrome floor on the resting fills and
//! on both danger fills, best case `2.7460:1` — and *prohibitive rather
//! than protective* in the other two: `accent.text` measures `3.0293:1`
//! on `accent.fill_active` and `3.0820:1` on `status.danger.fill_active`
//! and clears that floor both times. Rule 4 forbids them anyway, which is
//! the point: the rule pins a role and cannot be tuned into compliance
//! the way a threshold can.
//!
//! # The spec's printed numeric columns are not authoritative
//!
//! Ledger gap **G-5** (`doc/planning/hoarfrost-tasks.md`) establishes
//! that the numeric columns of `spec.md` §2.5 were
//! hand-collected and are wrong in places. **The values this module
//! computes are authoritative.** Nothing here transcribes a printed
//! figure; the reconciliation lives in the test sidecar, which
//! recomputes every cell of the revised table and asserts the *role*
//! threshold rather than the printed number.
//!
//! Recomputing all `110` numeric cells of the revised §2.5 (22 rows ×
//! `L` + four contrast columns) leaves `4` cells that do not match, all
//! of them transcription rather than colour error:
//!
//! * `status.danger.fill_active` prints `L = 0.1309`; `RGB(202, 24, 16)`
//!   measures `0.132472`.
//! * That row's `vs surface.base` prints `3.43` against a measured
//!   `3.457478`, and `vs surface.active` prints `2.46` against
//!   `2.484163` — both inherit the bad `L`.
//! * `accent.fill_active`'s `vs surface.active` prints `2.52` where the
//!   colour measures `2.527391`, which rounds to `2.53`.
//!
//! The first three are one mistake propagating; the fourth is
//! independent. None changes a pass into a fail or the reverse.
//!
//! # The retunes the spec has now absorbed
//!
//! An earlier revision of this crate carried three crate-local value
//! changes — `accent.primary`, `status.danger` and `surface.active`
//! retuned to clear thresholds the spec's own numbers missed (ledger
//! **G-7**). The 2026-10-22 revision of §2.5 makes those unnecessary
//! rather than merely superseded: it splits each affected colour by
//! polarity role, and each role-scoped token is bright or dark enough to
//! clear the threshold its role demands. There is no crate-local value
//! left to reconcile, and [`crate::token::color`] is the spec's number
//! verbatim.
//!
//! What the revision left unsettled — two places where the rules and the
//! printed values contradicted each other — is now settled on the rule
//! side rather than the colour side. `UIF-TOK-006` rule 2's ceiling was
//! widened to [`DISABLED_TEXT_MAX_CONTRAST`] (`6.05:1`) to admit
//! `text.disabled` on `surface.sunken` at `6.020452:1`, and rule 3's
//! `CoincidentWithFill` floor was brought to
//! [`MIN_COINCIDENT_FILL_CONTRAST`] (`3.00:1`) to admit
//! `status.danger.fill_active` on `surface.overlay` at `3.078876:1`.
//! No colour moved in either case. The sidecar keeps both pairs as
//! positive, value-pinned pass assertions so the resolution itself is
//! under test rather than remembered.
//!
//! Unit tests live in the sibling `contrast_tests.rs` sidecar.

use iced::Color;

use crate::token::color::Palette;

/// Minimum contrast normal text (`< 18px`) must hold against its
/// immediate background (`UIF-TOK-006` rule 1).
///
/// This is the WCAG 2.1 AA threshold for normal-size text, adopted
/// verbatim. Because [`crate::token::typography::MIN_SIZE`] is `12px` and only the two
/// largest rungs reach 18px, this is the threshold essentially every
/// label in the interface is measured against.
pub const MIN_TEXT_CONTRAST: f32 = 4.5;

/// Minimum contrast UI chrome — active borders and solid control fills —
/// must hold against adjacent surfaces (`UIF-TOK-006` rule 3).
///
/// Read the rule as covering *active* indicators only. See
/// [`ChromeClass`] for which strokes are held to this number and which
/// are not.
pub const MIN_CHROME_CONTRAST: f32 = 3.0;

/// Minimum contrast a solid fill must reach against the surface it sits
/// on when its perimeter stroke is the *same token* as the fill
/// (`UIF-TOK-006` rule 3, `CoincidentWithFill`).
///
/// A stroke coincident with its own fill draws no boundary of its own, so
/// the fill alone has to separate the control from what is behind it.
///
/// **This is deliberately equal to [`MIN_CHROME_CONTRAST`], and the two
/// constants must not be merged.** They are equal in *value* today and
/// different in *meaning*: [`MIN_CHROME_CONTRAST`] is what a visible
/// stroke must reach against its neighbour, while this is what a *fill*
/// must reach when it is standing in for a stroke that paints nothing.
/// `ChromeClass::minimum_contrast` dispatches on the class, so the two
/// numbers are read at different call sites for different reasons, and a
/// future revision that re-separates them — or exempts one class entirely
/// — has to be able to move one without moving the other. An earlier
/// reading of the rule put this at `3.08:1`, above the chrome floor; the
/// 2026-10-23 reconciliation brought it down to `3.00:1` so the floor
/// matches the solver that produced it rather than the two-decimal
/// rounding of its worst cell.
pub const MIN_COINCIDENT_FILL_CONTRAST: f32 = 3.0;

/// Lower bound of the disabled-text legibility band (`UIF-TOK-006`
/// rule 2).
///
/// Below this a disabled control stops being readable, which the rule
/// forbids: disabled means *present but inert* (`UIF-PRIN-004`), not
/// gone.
pub const DISABLED_TEXT_MIN_CONTRAST: f32 = 4.0;

/// Upper bound of the disabled-text legibility band (`UIF-TOK-006`
/// rule 2).
///
/// Above this a disabled label reads as live text and the rule's purpose
/// — keeping inert controls from competing with active ones — is lost.
///
/// The ceiling is `6.05:1` rather than a round `6.00:1` because
/// `text.disabled` on `surface.sunken` measures `6.020452:1` and the
/// token is not moving: `surface.sunken` is the darkest place a disabled
/// placeholder can appear, and lightening it enough to bring that pair
/// back under `6.00:1` would collapse the `sunken → base` step the
/// elevation ladder depends on. `6.05:1` is the smallest ceiling that
/// admits the measured worst case with margin, so the band still means
/// something while the palette stays as `UIF-TOK-005` specifies it.
pub const DISABLED_TEXT_MAX_CONTRAST: f32 = 6.05;

/// How a perimeter stroke or fill boundary is evaluated under
/// `UIF-TOK-006` rule 3.
///
/// Rule 3 does not apply one threshold to every stroke. A focus ring, a
/// border that *is* the button's fill, a transparent resting stroke and
/// a decorative hairline are four different things, and the rule says so.
/// Modelling them as a class rather than as a comment keeps the exemption
/// honest: a token cannot quietly stop being a `RestingDivider` to dodge
/// a measurement, and a test can assert the whole classification at once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChromeClass {
    /// An active perimeter border or focus ring — the ordinary case,
    /// measured directly against the adjacent surface at
    /// [`MIN_CHROME_CONTRAST`].
    Judged,

    /// A `1px` perimeter stroke set to the exact same token as the
    /// solid fill it surrounds (`accent.fill`, `accent.fill_active`,
    /// `status.danger.fill_active`). Boundary contrast is carried by the
    /// solid fill against the surrounding surface, which must therefore
    /// reach [`MIN_COINCIDENT_FILL_CONTRAST`].
    CoincidentWithFill,

    /// A `transparent` stroke on a rest-state Ghost button. There is
    /// nothing at the perimeter to measure at rest; the class is
    /// evaluated against the parent container's surface only once the
    /// control is focused or hovered and something is actually painted.
    UnpaintedBackground,

    /// A passive structural hairline (`border.subtle`). Decorative at
    /// rest, never an indicator, and exempt from the active-chrome floor.
    RestingDivider,
}

impl ChromeClass {
    /// The contrast this class must reach, or `None` when the class is
    /// exempt from the measurement entirely.
    ///
    /// `UnpaintedBackground` returns the chrome floor rather than `None`
    /// because it is not exempt — it is merely unevaluable at rest. Once
    /// the control is focused or hovered there is a painted stroke and a
    /// parent surface, and the ordinary `3.00:1` applies to them.
    #[must_use]
    pub const fn minimum_contrast(self) -> Option<f32> {
        match self {
            Self::Judged | Self::UnpaintedBackground => Some(MIN_CHROME_CONTRAST),
            Self::CoincidentWithFill => Some(MIN_COINCIDENT_FILL_CONTRAST),
            Self::RestingDivider => None,
        }
    }

    /// Is this class exempt from rule 3's active-chrome threshold?
    #[must_use]
    pub const fn is_exempt(self) -> bool {
        self.minimum_contrast().is_none()
    }
}

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
/// (`UIF-TOK-006` rule 3, [`ChromeClass::Judged`])?
///
/// True when [`contrast_ratio`] reaches [`MIN_CHROME_CONTRAST`]
/// (`3.0:1`). The comparison is inclusive.
///
/// Applies to *active* chrome — focus rings, active borders, selected
/// outlines. For a stroke coincident with its own fill use
/// [`meets_coincident_fill_contrast`], which answers the same question
/// of a *fill* standing in for a stroke; for a resting divider use
/// [`meets_chrome_contrast_as`] so the class is stated.
#[must_use]
pub fn meets_chrome_contrast(a: Color, b: Color) -> bool {
    contrast_ratio(a, b) >= MIN_CHROME_CONTRAST
}

/// Does a solid fill separate a coincident-perimeter control from the
/// surface it sits on (`UIF-TOK-006` rule 3, [`ChromeClass::CoincidentWithFill`])?
///
/// True when [`contrast_ratio`] reaches [`MIN_COINCIDENT_FILL_CONTRAST`]
/// (`3.00:1`). The comparison is inclusive.
///
/// The number is the same as [`MIN_CHROME_CONTRAST`]; what differs is
/// what is being measured. Use this rather than [`meets_chrome_contrast`]
/// wherever the stroke is coincident with its fill, so the class the
/// measurement answers for is visible at the call site.
#[must_use]
pub fn meets_coincident_fill_contrast(fill: Color, surface: Color) -> bool {
    contrast_ratio(fill, surface) >= MIN_COINCIDENT_FILL_CONTRAST
}

/// Does `a` clear the threshold that `class` demands of it against `b`?
///
/// An exempt class returns `true` unconditionally: exempt means the
/// measurement is not made, not that it failed. Callers that need to
/// distinguish "passed" from "not judged" should read
/// [`ChromeClass::minimum_contrast`] directly.
#[must_use]
pub fn meets_chrome_contrast_as(class: ChromeClass, a: Color, b: Color) -> bool {
    match class.minimum_contrast() {
        Some(minimum) => contrast_ratio(a, b) >= minimum,
        None => true,
    }
}

/// Does `fg` against `bg` fall inside the disabled-text legibility band
/// (`UIF-TOK-006` rule 2)?
///
/// True when [`contrast_ratio`] is at least [`DISABLED_TEXT_MIN_CONTRAST`]
/// and at most [`DISABLED_TEXT_MAX_CONTRAST`]. Both bounds are inclusive.
#[must_use]
pub fn meets_disabled_text_band(fg: Color, bg: Color) -> bool {
    let ratio = contrast_ratio(fg, bg);
    (DISABLED_TEXT_MIN_CONTRAST..=DISABLED_TEXT_MAX_CONTRAST).contains(&ratio)
}

// ---------------------------------------------------------------------------
// UIF-TOK-006 rule 4 — the solid-fill foreground lock
// ---------------------------------------------------------------------------

/// Which side of the `UIF-TOK-006` rule 4 lock a colour sits on.
///
/// Rule 4 is a rule about roles rather than hues: the same RGB is a locked
/// fill under one token name and an ordinary surface under another. The
/// role is therefore read off the [`Palette`] by value identity, the same
/// way [`crate::gallery`] reads a painted colour back to the token it
/// came from — so a palette that retunes `accent.fill` moves the lock
/// with it, and this module carries no colour of its own.
///
/// Alpha decides the question before any comparison is made. A colour with
/// `a < 1.0` has not finished being painted: a `transparent` slot paints
/// nothing, and [`surface_scrim`](crate::token::color::Palette::surface_scrim)
/// is a dimming layer whose visible colour belongs to whatever is
/// composited beneath it. Neither is a *solid* fill, so neither is ever
/// [`LockedFill`](Self::LockedFill).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillLockRole {
    /// One of the four saturated solid fills whose foreground rule 4
    /// pins to `text.strong`: `accent.fill`, `accent.fill_active`,
    /// `status.danger.fill`, `status.danger.fill_active`.
    LockedFill,

    /// A foreground-only token rule 4 forbids on a locked fill:
    /// `status.danger.text`, `status.warning`, `accent.text`. These are
    /// the colours a *label* is written in and can never be painted on.
    ForegroundOnly,

    /// Neither role — a surface, a border, or a text role rule 4 does not
    /// name. The lock places no demand through this value.
    Unlocked,
}

impl FillLockRole {
    /// Is this colour one of the four solid fills the lock governs?
    #[must_use]
    pub const fn is_locked_fill(self) -> bool {
        matches!(self, Self::LockedFill)
    }

    /// Is this a foreground-only token that may not sit on a locked fill?
    #[must_use]
    pub const fn is_foreground_only(self) -> bool {
        matches!(self, Self::ForegroundOnly)
    }

    /// The column label for this role.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::LockedFill => "locked-solid-fill",
            Self::ForegroundOnly => "foreground-only",
            Self::Unlocked => "unlocked",
        }
    }
}

/// How one resolved `(background, foreground)` pairing stands under
/// `UIF-TOK-006` rule 4.
///
/// Five outcomes rather than a boolean, because a lint that can only say
/// "no" cannot tell a caller *which* of its two sentences it breached —
/// and the difference between "this background is not a fill, so the rule
/// never applied" and "this fill is carrying the wrong colour" is the
/// difference between a pass and a bug.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillLock {
    /// The background is not a locked solid fill, so rule 4 makes no
    /// demand on the foreground. This covers every surface, every border,
    /// and every `transparent` cell — a Ghost button at rest is legal
    /// here, not excused.
    NotSolidFill,

    /// The background is a locked solid fill and the foreground is
    /// `text.strong` — the pairing the rule requires.
    Locked,

    /// The background is a locked solid fill and nothing is painted in the
    /// foreground slot. There is no text or icon to read, so there is
    /// nothing for the rule to protect.
    ForegroundNotPainted,

    /// A foreground-only token painted on a locked solid fill — the
    /// pairing rule 4's second sentence forbids outright, and the one
    /// this lock exists to catch. Ten of the twelve such pairings are
    /// unreadable as well as forbidden — `accent.text` on `accent.fill`
    /// is `2.5719:1`, `status.warning` on `status.danger.fill` is
    /// `1.7667:1`, and the best of the ten reaches only `2.7460:1`.
    /// The other two are `accent.text` on the pressed fills: `3.0293:1`
    /// on `accent.fill_active` and `3.0820:1` on `status.danger.fill_active`,
    /// both of which clear the chrome floor outright. Rule 4 forbids them
    /// anyway, which is the clearest evidence that it pins a role rather
    /// than a threshold that could be tuned into compliance.
    ForegroundOnlyOnFill,

    /// A locked solid fill carrying a foreground that is neither
    /// `text.strong` nor unpainted.
    WrongForegroundOnFill,
}

impl FillLock {
    /// Does this verdict satisfy rule 4 rather than breach it?
    #[must_use]
    pub const fn is_compliant(self) -> bool {
        matches!(
            self,
            Self::NotSolidFill | Self::Locked | Self::ForegroundNotPainted
        )
    }

    /// Did the lock actually apply — i.e. was the background a locked
    /// solid fill?
    ///
    /// An audit that only counted compliant pairs could reach that number
    /// by walking nothing but surfaces. This is the counter that says the
    /// rule was exercised.
    #[must_use]
    pub const fn governed(self) -> bool {
        !matches!(self, Self::NotSolidFill)
    }

    /// The column label for this verdict.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::NotSolidFill => "not-a-solid-fill",
            Self::Locked => "locked",
            Self::ForegroundNotPainted => "foreground-not-painted",
            Self::ForegroundOnlyOnFill => "foreground-only-on-fill",
            Self::WrongForegroundOnFill => "wrong-foreground-on-fill",
        }
    }
}

/// The role a colour plays in the `UIF-TOK-006` rule 4 lock, read off
/// `palette` by value identity.
///
/// Alpha is decided first: anything below fully opaque is
/// [`Unlocked`](FillLockRole::Unlocked), because rule 4 governs *solid*
/// fills and a translucent colour has not finished being painted. That
/// is what keeps `surface.scrim` — base at 55%, a dimming layer rather
/// than a surface a label sits on — out of the locked set for the same
/// reason [`contrast_ratio`] refuses to measure it directly.
///
/// The fills are matched before the foreground-only tokens, so a palette
/// that aliased a fill and a foreground token to the same RGB would be
/// read as the fill: the stricter of the two readings wins.
#[must_use]
pub fn fill_lock_role(color: Color, palette: &Palette) -> FillLockRole {
    if color.a < 1.0 {
        return FillLockRole::Unlocked;
    }

    if color == palette.accent_fill
        || color == palette.accent_fill_active
        || color == palette.status_danger_fill
        || color == palette.status_danger_fill_active
    {
        return FillLockRole::LockedFill;
    }

    if color == palette.status_danger_text
        || color == palette.status_warning
        || color == palette.accent_text
    {
        return FillLockRole::ForegroundOnly;
    }

    FillLockRole::Unlocked
}

/// Does `(background, foreground)` satisfy the `UIF-TOK-006` rule 4
/// solid-fill foreground lock?
///
/// The whole rule, as one verdict:
///
/// * a background outside the four locked fills is not the rule's
///   business ([`NotSolidFill`](FillLock::NotSolidFill));
/// * a locked fill under `text.strong` is what the rule asks for
///   ([`Locked`](FillLock::Locked));
/// * a locked fill under an unpainted slot has nothing to read
///   ([`ForegroundNotPainted`](FillLock::ForegroundNotPainted));
/// * a locked fill under anything else breaches the rule, split by whether
///   the intruder was a foreground-only token.
///
/// Both inputs must be colours that will actually be painted. A
/// translucent layer should be composited with [`composite_over`] first —
/// but note that a composited scrim is still not a *fill*, so it resolves
/// to [`NotSolidFill`](FillLock::NotSolidFill) by alpha rather than by
/// any comparison.
#[must_use]
pub fn fill_lock(background: Color, foreground: Color, palette: &Palette) -> FillLock {
    if !fill_lock_role(background, palette).is_locked_fill() {
        return FillLock::NotSolidFill;
    }
    if foreground.a == 0.0 {
        return FillLock::ForegroundNotPainted;
    }
    if foreground == palette.text_strong {
        return FillLock::Locked;
    }
    if fill_lock_role(foreground, palette).is_foreground_only() {
        return FillLock::ForegroundOnlyOnFill;
    }
    FillLock::WrongForegroundOnFill
}

/// Does `(background, foreground)` satisfy the `UIF-TOK-006` rule 4
/// solid-fill foreground lock?
///
/// [`fill_lock`], flattened to a boolean for callers that only need the
/// pass/fail. Prefer [`fill_lock`] when a failure has to say why.
#[must_use]
pub fn meets_fill_foreground_lock(background: Color, foreground: Color, palette: &Palette) -> bool {
    fill_lock(background, foreground, palette).is_compliant()
}

/// The construction-time shield `UIF-TOK-006` rule 4 gives the resolvers.
///
/// A resolver calls this with the `(background, foreground)` pair it is
/// about to paint, while it is still building the style rather than after
/// the fact. In a debug build a breach aborts right there, at the
/// resolver and the pairing that caused it, so an illegal pairing cannot
/// be carried out of the function that made it.
///
/// It is a `debug_assert` rather than a hard panic for one reason: the
/// lock is read from the *palette*, and a host can hand a widget a palette
/// this crate never audited. A hard panic would turn a third-party token
/// alias into a crashed frame; the shield turns it into a failed test.
/// The gate that actually holds the rule in every build is the crate-wide
/// audit in the sidecar, which walks every pair every resolver can emit
/// across every reachable state and fails the build on any breach — so
/// the pairing that would trip this shield cannot reach a tested build in
/// the first place.
///
/// # Panics
/// In a `debug_assertions` build, when [`fill_lock`] returns a
/// non-compliant verdict for the pair.
#[track_caller]
pub fn assert_fill_foreground_lock(background: Color, foreground: Color, palette: &Palette) {
    debug_assert!(
        fill_lock(background, foreground, palette).is_compliant(),
        "UIF-TOK-006 rule 4 (Selection & Solid-Fill Foreground Lock) breached: {} — a solid \
         `*.fill` carries `color.text.strong` and nothing else (background {background:?}, \
         foreground {foreground:?})",
        fill_lock(background, foreground, palette).label(),
    );
}

/// Assert that the contrast ratio between `fg` and `bg` meets or exceeds `min_ratio`.
///
/// A construction-time shield called by style resolvers. In a debug build, a breach aborts
/// right at the resolver that constructed the pair.
///
/// # Panics
/// In a `debug_assertions` build, when `contrast_ratio(fg, bg) < min_ratio`.
#[track_caller]
pub fn assert_contrast(fg: Color, bg: Color, min_ratio: f32, label: &str) {
    let ratio = contrast_ratio(fg, bg);
    debug_assert!(
        ratio >= min_ratio,
        "contrast check failed for '{label}': ratio {ratio:.4}:1 < required {min_ratio:.4}:1 (fg {fg:?}, bg {bg:?})"
    );
}

#[cfg(test)]
#[path = "contrast_tests.rs"]
mod contrast_tests;
