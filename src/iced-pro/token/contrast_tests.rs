//! Tests for [`contrast`] — the WCAG 2.1 solver that carries
//! `UIF-TOK-006` and the measurement half of `UIF-VER-001`.
//!
//! Three groups, in order:
//!
//! 1. **Solver correctness.** Reference luminances, the piecewise
//!    transfer function on both sides of its knee, ratio symmetry and
//!    bounds, and Porter-Duff source-over compositing — including the
//!    demonstration that a translucent token measured before it is
//!    composited yields a number describing nothing.
//! 2. **The mandated-pairing gate.** Pairings derived from the spec's
//!    *usage contracts* across §2.5, §3.1, §3.2, §4.4.2 and §4.9.2 —
//!    which foreground is actually specified to sit on which background
//!    — each asserted against the threshold its role carries.
//! 3. **Spec reconciliation.** Every §2.5 row recomputed against
//!    `surface.base` and compared with the spec's printed figure.
//!
//! # The spec's printed contrast column is wrong, and that is pinned here
//!
//! Ledger gap **G-5** says the `Luminance Contrast vs color.surface.base`
//! column of `doc/planning/ui-framework.md` §2.5 was hand-collected and
//! cannot be trusted. Nothing below asserts equality with a printed
//! figure. `test_spec_rows_recompute_to_the_intended_role_threshold`
//! asserts each row clears the threshold its *role* needs, and
//! `test_spec_printed_figures_split_into_rounded_and_materially_wrong`
//! classifies every row as exact-or-rounded versus materially wrong, so
//! the wrongness is recorded rather than inherited.
//!
//! Materially wrong rows (recomputed − printed), after the `G-7`
//! retune of `accent.primary`, `surface.active` and `status.danger`:
//!
//! | Token | Printed | Recomputed | Delta |
//! |---|---|---|---|
//! | `surface.sunken` | `0.98` | `1.0176` | `+0.0376` — impossible, no WCAG ratio is `< 1` |
//! | `surface.active` | `1.56` | `1.4174` | `-0.1426` — was `1.5660` before the retune darkened it |
//! | `border.subtle` | `1.36` | `1.3495` | `-0.0105` — rounds to `1.35` |
//! | `border.strong` | `3.92` | `4.5062` | `+0.5862` — `3.92` is grey ≈110, not 120 |
//! | `accent.primary` | `4.45` | `4.3585` | `-0.0915` — was `4.4540` before the retune darkened it |
//! | `accent.text` | `10.12` | `10.6559` | `+0.5359` |
//! | `text.primary` | `12.63` | `13.1590` | `+0.5290` |
//! | `text.strong` | `19.86` | `19.8954` | `+0.0354` — truncated; rounds to `19.90` |
//! | `text.disabled` | `5.86` | `5.9165` | `+0.0565` |
//! | `status.success` | `15.40` | `15.1867` | `-0.2133` |
//! | `status.warning` | `5.50` | `6.8555` | `+1.3555` — largest error in the table |
//! | `status.danger` | `4.66` | `5.1339` | `+0.4739` — was `4.6570` before the retune lightened it |
//!
//! `text.strong` and `surface.active` are additions to G-5's list: both
//! are truncations rather than roundings, and both are still inside the
//! rounding band a two-decimal figure should occupy, but neither matches
//! the printed digits. `accent.primary` and `status.danger` moved into
//! this table only when the `G-7` retune moved the tokens; against the
//! spec's literal values they had been exact-or-rounded.
//!
//! # The five pairings that used to fail, and now clear
//!
//! Under the spec's literal token values five contract-mandated pairings
//! could not clear the threshold their own role carries. Ledger gap
//! **G-7** was resolved as decision **A** — retune the three offending
//! tokens minimally — so those five now pass, and
//! `test_the_five_pairings_that_used_to_fail_now_clear_their_rule`
//! pins each at its new measured value:
//!
//! | Pairing | Was | Now | Rule | Headroom |
//! |---|---|---|---|---|
//! | `text.strong` on `accent.primary` (Selection Foreground Lock) | `4.4669` | `4.5648` | `4.5` | `+0.0648` |
//! | `status.danger` on `surface.overlay` (validation slot, error banner) | `4.1473` | `4.5718` | `4.5` | `+0.0718` |
//! | `status.danger` on `surface.raised` (Danger button rest) | `4.3800` | `4.8283` | `4.5` | `+0.3283` |
//! | `accent.primary` on `surface.active` (pressed border) | `2.8441` | `3.0750` | `3.0` | `+0.0750` |
//! | `border.strong` on `surface.active` (Ghost pressed / latched trigger) | `2.8775` | `3.1793` | `3.0` | `+0.1793` |

use super::*;
use crate::token::color::{EditorPalette, Palette};
use crate::token::typography;

/// Tolerance on relative-luminance comparisons.
///
/// The largest error in the path is `powf`'s last-rounding error, which
/// for these channels is well under `1e-7` absolute. `1e-5` is two
/// orders of magnitude above that — loose enough that no legitimate
/// reassociation of the weighted sum can trip it, tight enough that a
/// changed weight or exponent moves the result far outside it.
const LUM_EPS: f32 = 1e-5;

/// Tolerance on a pinned contrast ratio.
///
/// Ratios are quotients of linearised channels, so the float error stays
/// near `1e-6` absolute. `1e-3` pins a printed figure to three decimals
/// — enough to tell `4.4669` from `4.5000`, which is the whole point of
/// recording it.
const RATIO_EPS: f32 = 1e-3;

/// Band separating a spec figure that merely rounds to the printed two
/// decimals from one that is materially wrong.
///
/// Every exact-or-rounded row lands within `0.0040` of its printed
/// figure; every materially wrong row is at least `0.0060` away. `5e-3`
/// sits between the two clusters with margin on both sides, which is
/// what makes the classifier in
/// `test_spec_printed_figures_split_into_rounded_and_materially_wrong`
/// a statement about the data rather than a tautology.
const PRINT_EPS: f32 = 5e-3;

/// The WCAG transfer-function knee, mirrored from production so the
/// branch tests can probe each side of it explicitly.
const KNEE: f32 = 0.03928;

/// The 18px boundary of `UIF-TOK-006` rule 1: text below this size is
/// *normal* text and owes `4.5:1`.
const NORMAL_TEXT_MAX_PX: f32 = 18.0;

/// Rule 1 must be the stricter of the two thresholds: smaller text is
/// harder to read, so it owes more contrast than a border does. Checked
/// at compile time because both sides are constants.
const _: () = assert!(
    MIN_TEXT_CONTRAST > MIN_CHROME_CONTRAST,
    "the text rule must be stricter than the chrome rule"
);

/// The floor rung that carries [`typography::MIN_SIZE`] is 12px, which
/// sits inside rule 1's normal-text band — the smallest text in the
/// interface is measured against the strictest threshold, not the
/// laxest.
const _: () = assert!(
    typography::CAPTION.size < NORMAL_TEXT_MAX_PX,
    "the 12px floor rung must be normal text under rule 1"
);

/// A foreground/background pairing drawn from a spec usage contract.
struct Pairing {
    /// The contract, naming the spec tokens and the section that places
    /// them together — the string a failing assertion prints.
    contract: String,
    /// The foreground token.
    fg: Color,
    /// The background token.
    bg: Color,
    /// The threshold the row's *role* carries: [`MIN_TEXT_CONTRAST`] for
    /// a text role, [`MIN_CHROME_CONTRAST`] for active chrome.
    min: f32,
}

/// Build a pairing.
fn pairing(contract: impl Into<String>, fg: Color, bg: Color, min: f32) -> Pairing {
    Pairing {
        contract: contract.into(),
        fg,
        bg,
        min,
    }
}

/// A colour as the spec writes it, for assertion messages.
fn rgb8(color: Color) -> String {
    format!(
        "RGB({}, {}, {})",
        (color.r * 255.0).round(),
        (color.g * 255.0).round(),
        (color.b * 255.0).round()
    )
}

/// Assert a pairing clears its threshold, naming the contract, the two
/// colours and the measured ratio when it does not.
fn assert_clears(p: &Pairing) {
    let ratio = contrast_ratio(p.fg, p.bg);
    assert!(
        ratio >= p.min,
        "{}: {} on {} measures {ratio:.4}:1, below the {:.1}:1 its role requires",
        p.contract,
        rgb8(p.fg),
        rgb8(p.bg),
        p.min,
    );
}

/// The dark palette, taken once so every table reads as token names.
fn palette() -> Palette {
    Palette::dark()
}

/// Every surface token in the elevation ladder, darkest to lightest.
fn all_surfaces(p: &Palette) -> [(&'static str, Color); 7] {
    [
        ("surface.sunken", p.surface_sunken),
        ("surface.base", p.surface_base),
        ("surface.raised", p.surface_raised),
        ("surface.code", p.surface_code),
        ("surface.overlay", p.surface_overlay),
        ("surface.hover", p.surface_hover),
        ("surface.active", p.surface_active),
    ]
}
// ---------------------------------------------------------------------------
// Solver correctness — relative luminance
// ---------------------------------------------------------------------------

/// Grey made of one repeated channel value.
///
/// The Rec. 709 weights sum to exactly `1.0`, so a neutral grey's
/// luminance *is* its linearised channel value — which turns every
/// transfer-function assertion into a single-channel assertion with no
/// weighting arithmetic in the way.
fn grey(channel: f32) -> Color {
    Color {
        r: channel,
        g: channel,
        b: channel,
        a: 1.0,
    }
}

/// The power branch of the sRGB transfer function, written out here so
/// the production branch selection is tested against an independent
/// expression rather than against itself.
fn power_branch(c: f32) -> f32 {
    ((c + 0.055) / 1.055).powf(2.4)
}

/// The linear branch of the sRGB transfer function.
fn linear_branch(c: f32) -> f32 {
    c / 12.92
}

#[test]
fn test_relative_luminance_matches_the_reference_values() {
    let cases: [(&str, Color, f32); 6] = [
        ("black", Color::from_rgb8(0, 0, 0), 0.0),
        ("white", Color::from_rgb8(255, 255, 255), 1.0),
        ("pure red", Color::from_rgb8(255, 0, 0), 0.2126),
        ("pure green", Color::from_rgb8(0, 255, 0), 0.7152),
        ("pure blue", Color::from_rgb8(0, 0, 255), 0.0722),
        // #808080: 0.501961 linearises to 0.2158605, and the weights
        // summing to 1.0 pass it straight through. Rounds to 0.2159.
        (
            "mid grey #808080",
            Color::from_rgb8(128, 128, 128),
            0.215_860,
        ),
    ];

    for (name, color, expected) in cases {
        let actual = relative_luminance(color);
        assert!(
            (actual - expected).abs() < LUM_EPS,
            "{name}: relative_luminance({}) = {actual:.7}, expected {expected:.7}",
            rgb8(color),
        );
    }
}

#[test]
fn test_relative_luminance_ignores_alpha() {
    let opaque = Color {
        r: 0.4,
        g: 0.3,
        b: 0.6,
        a: 1.0,
    };
    let translucent = Color {
        r: 0.4,
        g: 0.3,
        b: 0.6,
        a: 0.25,
    };
    assert!(
        (relative_luminance(opaque) - relative_luminance(translucent)).abs() < LUM_EPS,
        "luminance describes a painted colour; alpha must not enter it until compositing does"
    );
}

#[test]
fn test_transfer_function_below_the_knee_uses_the_linear_branch() {
    // 0.02 is well under the 0.03928 knee. The two branches disagree by
    // 2.1e-4 here, far outside LUM_EPS, so this pins the branch choice
    // rather than merely being consistent with it.
    let channel = 0.02;
    let expected = linear_branch(channel);
    let wrong = power_branch(channel);

    let actual = relative_luminance(grey(channel));
    assert!(
        (actual - expected).abs() < LUM_EPS,
        "below the knee the linear branch must apply: got {actual:.9}, expected {expected:.9}"
    );
    assert!(
        (actual - wrong).abs() > LUM_EPS,
        "the power branch would give {wrong:.9}; this case cannot distinguish the branches"
    );
}

#[test]
fn test_transfer_function_above_the_knee_uses_the_power_branch() {
    let channel = 0.5;
    let expected = power_branch(channel);
    let wrong = linear_branch(channel);

    let actual = relative_luminance(grey(channel));
    assert!(
        (actual - expected).abs() < LUM_EPS,
        "above the knee the power branch must apply: got {actual:.9}, expected {expected:.9}"
    );
    assert!(
        (actual - wrong).abs() > LUM_EPS,
        "the linear branch would give {wrong:.9}; this case cannot distinguish the branches"
    );
}

#[test]
fn test_transfer_function_branches_meet_continuously_at_the_knee() {
    // The linear segment is the toe of the power curve's tangent, so the
    // two branches agree at 0.03928 to ~7.6e-7. A discontinuity there
    // would show up as a visible banding step in near-black gradients,
    // which is what this guards against.
    let linear = linear_branch(KNEE);
    let power = power_branch(KNEE);
    let actual = relative_luminance(grey(KNEE));

    assert!(
        (actual - linear).abs() < LUM_EPS,
        "at exactly the knee the `<=` branch is the linear one: got {actual:.9}, linear {linear:.9}"
    );
    assert!(
        (linear - power).abs() < LUM_EPS,
        "the branches must meet: linear {linear:.9} vs power {power:.9}"
    );
}

#[test]
fn test_relative_luminance_clamps_channels_outside_the_srgb_gamut() {
    // `iced::Color` does not range-check its fields, and `powf(2.4)` of a
    // negative channel is NaN — a value a layout engine propagates
    // silently. Clamping must make this finite and equal to the clamped
    // colour's luminance instead.
    let wild = Color {
        r: 2.0,
        g: -1.0,
        b: 0.5,
        a: 1.0,
    };
    let clamped = Color {
        r: 1.0,
        g: 0.0,
        b: 0.5,
        a: 1.0,
    };

    let actual = relative_luminance(wild);
    assert!(
        actual.is_finite(),
        "an out-of-range channel must not produce NaN, got {actual}"
    );
    assert!(
        (actual - relative_luminance(clamped)).abs() < LUM_EPS,
        "out-of-range channels must clamp, not saturate into a different answer"
    );
    assert!(
        (0.0..=1.0).contains(&actual),
        "luminance must stay inside [0, 1], got {actual}"
    );
}

// ---------------------------------------------------------------------------
// Solver correctness — contrast ratio
// ---------------------------------------------------------------------------

#[test]
fn test_contrast_ratio_of_identical_colors_is_one() {
    let p = palette();
    for (name, color) in all_surfaces(&p) {
        assert!(
            (contrast_ratio(color, color) - 1.0).abs() < RATIO_EPS,
            "{name} against itself must be exactly 1.0, got {:.4}",
            contrast_ratio(color, color)
        );
    }
    assert!(
        (contrast_ratio(Color::BLACK, Color::BLACK) - 1.0).abs() < RATIO_EPS,
        "black on black is 1.0, not 0 or NaN — the +0.05 offset keeps it finite"
    );
}

#[test]
fn test_contrast_ratio_of_black_against_white_is_twenty_one() {
    assert!(
        (contrast_ratio(Color::BLACK, Color::WHITE) - 21.0).abs() < RATIO_EPS,
        "black on white is the WCAG maximum of 21.0, got {:.4}",
        contrast_ratio(Color::BLACK, Color::WHITE)
    );
}

#[test]
fn test_contrast_ratio_is_symmetric_in_its_arguments() {
    let p = palette();
    let e = EditorPalette::inverted();
    let mut colors: Vec<(&str, Color)> = all_surfaces(&p).to_vec();
    colors.extend([
        ("border.subtle", p.border_subtle),
        ("border.strong", p.border_strong),
        ("accent.primary", p.accent_primary),
        ("accent.text", p.accent_text),
        ("text.primary", p.text_primary),
        ("text.strong", p.text_strong),
        ("text.muted", p.text_muted),
        ("text.disabled", p.text_disabled),
        ("status.danger", p.status_danger),
        ("status.warning", p.status_warning),
        ("status.success", p.status_success),
        ("editor.bg", e.background),
        ("editor.fg", e.text),
        ("editor.err", e.error),
    ]);

    for (name_a, a) in &colors {
        for (name_b, b) in &colors {
            let forward = contrast_ratio(*a, *b);
            let backward = contrast_ratio(*b, *a);
            assert!(
                (forward - backward).abs() < 1e-4,
                "asymmetry: {name_a} vs {name_b} = {forward:.6} but {name_b} vs {name_a} = {backward:.6}"
            );
        }
    }
}

#[test]
fn test_contrast_ratio_stays_inside_the_wcag_bounds() {
    let p = palette();
    let colors: Vec<(&str, Color)> = all_surfaces(&p)
        .into_iter()
        .chain([
            ("border.subtle", p.border_subtle),
            ("border.strong", p.border_strong),
            ("accent.primary", p.accent_primary),
            ("accent.text", p.accent_text),
            ("text.primary", p.text_primary),
            ("text.strong", p.text_strong),
            ("text.muted", p.text_muted),
            ("text.disabled", p.text_disabled),
            ("status.danger", p.status_danger),
            ("status.warning", p.status_warning),
            ("status.success", p.status_success),
        ])
        .collect();

    for (name_a, a) in &colors {
        for (name_b, b) in &colors {
            let ratio = contrast_ratio(*a, *b);
            assert!(
                (1.0..=21.0).contains(&ratio),
                "{name_a} vs {name_b} = {ratio:.4}, outside the [1.0, 21.0] a WCAG ratio can occupy"
            );
        }
    }
}
// ---------------------------------------------------------------------------
// Solver correctness — source-over compositing
// ---------------------------------------------------------------------------

#[test]
fn test_composite_over_an_opaque_foreground_returns_the_foreground() {
    let p = palette();
    let fg = Color {
        r: 0.2,
        g: 0.6,
        b: 0.4,
        a: 1.0,
    };
    for (name, bg) in all_surfaces(&p) {
        let out = composite_over(fg, bg);
        assert!(
            (out.r - fg.r).abs() < LUM_EPS
                && (out.g - fg.g).abs() < LUM_EPS
                && (out.b - fg.b).abs() < LUM_EPS
                && (out.a - 1.0).abs() < LUM_EPS,
            "opaque fg over {name} must pass through untouched, got {out:?}"
        );
    }
}

#[test]
fn test_composite_over_a_fully_transparent_foreground_returns_the_background() {
    let p = palette();
    let fg = Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };
    for (name, bg) in all_surfaces(&p) {
        let out = composite_over(fg, bg);
        assert!(
            (out.r - bg.r).abs() < LUM_EPS
                && (out.g - bg.g).abs() < LUM_EPS
                && (out.b - bg.b).abs() < LUM_EPS
                && (out.a - bg.a).abs() < LUM_EPS,
            "a fully transparent fg over {name} must leave it exactly as it was, got {out:?}"
        );
    }
}

#[test]
fn test_composite_over_half_alpha_blends_the_two_evenly() {
    let out = composite_over(
        Color {
            r: 1.0,
            g: 0.0,
            b: 0.5,
            a: 0.5,
        },
        Color {
            r: 0.0,
            g: 1.0,
            b: 0.0,
            a: 1.0,
        },
    );
    assert!(
        (out.r - 0.5).abs() < LUM_EPS
            && (out.g - 0.5).abs() < LUM_EPS
            && (out.b - 0.25).abs() < LUM_EPS
            && (out.a - 1.0).abs() < LUM_EPS,
        "0.5 over an opaque background must be the exact midpoint, got {out:?}"
    );
}

#[test]
fn test_composite_over_alpha_is_the_source_over_sum_and_both_alphas_matter() {
    // Both inputs translucent: a_out = 0.5 + 0.5*(1-0.5) = 0.75, and the
    // un-premultiplied result divides the blend back out by a_out, so a
    // 1.0-over-0.0 pairing lands on 2/3 rather than 1/2.
    let out = composite_over(
        Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 0.5,
        },
        Color {
            r: 0.0,
            g: 0.0,
            b: 1.0,
            a: 0.5,
        },
    );
    assert!(
        (out.a - 0.75).abs() < LUM_EPS,
        "a_out must be a_fg + a_bg*(1-a_fg) = 0.75, got {}",
        out.a
    );
    assert!(
        (out.r - 2.0 / 3.0).abs() < LUM_EPS,
        "un-premultiplied blend must divide by a_out: expected 0.6667, got {}",
        out.r
    );
    assert!(
        (out.b - 1.0 / 3.0).abs() < LUM_EPS,
        "the background's own alpha must still contribute: expected 0.3333, got {}",
        out.b
    );
}

#[test]
fn test_composite_over_a_transparent_background_leaves_the_foreground_alone() {
    let fg = Color {
        r: 0.3,
        g: 0.7,
        b: 0.1,
        a: 0.4,
    };
    let out = composite_over(
        fg,
        Color {
            r: 0.9,
            g: 0.9,
            b: 0.9,
            a: 0.0,
        },
    );
    assert!(
        (out.r - fg.r).abs() < LUM_EPS
            && (out.g - fg.g).abs() < LUM_EPS
            && (out.b - fg.b).abs() < LUM_EPS
            && (out.a - 0.4).abs() < LUM_EPS,
        "nothing underneath means the foreground is all there is, got {out:?}"
    );
}

#[test]
fn test_composite_over_two_fully_transparent_layers_is_transparent() {
    let out = composite_over(
        Color {
            r: 1.0,
            g: 0.5,
            b: 0.0,
            a: 0.0,
        },
        Color {
            r: 0.0,
            g: 0.5,
            b: 1.0,
            a: 0.0,
        },
    );
    assert!(
        out.a == 0.0 && out.r == 0.0 && out.g == 0.0 && out.b == 0.0,
        "0 alpha over 0 alpha must be the transparent colour, not a 0/0 NaN, got {out:?}"
    );
}

#[test]
fn test_composite_over_the_scrim_on_base_reproduces_base() {
    let p = palette();
    let out = composite_over(p.surface_scrim, p.surface_base);
    assert!(
        (out.r - p.surface_base.r).abs() < LUM_EPS
            && (out.g - p.surface_base.g).abs() < LUM_EPS
            && (out.b - p.surface_base.b).abs() < LUM_EPS
            && (out.a - 1.0).abs() < LUM_EPS,
        "the scrim IS surface.base at 55% alpha, so over base it must land back on base exactly"
    );
}

#[test]
fn test_composite_over_the_scrim_on_white_lands_on_mid_grey() {
    let p = palette();
    let out = composite_over(p.surface_scrim, Color::WHITE);
    // 0.55*9 + 0.45*255 = 119.7 per neutral channel, 120.8 in blue.
    assert!(
        (out.r * 255.0 - 119.7).abs() < 0.1
            && (out.g * 255.0 - 119.7).abs() < 0.1
            && (out.b * 255.0 - 120.8).abs() < 0.1
            && (out.a - 1.0).abs() < LUM_EPS,
        "the scrim must visibly drag white down to ~RGB(119.7, 119.7, 120.8), got {}",
        rgb8(out)
    );
    assert!(
        (p.surface_scrim.a - 0.55).abs() < LUM_EPS,
        "the scrim's alpha is the rule the spec states: 0.55"
    );
}
#[test]
fn test_a_translucent_token_is_meaningless_until_it_is_composited() {
    let p = palette();
    let scrim = p.surface_scrim;

    // Measured directly, the scrim looks like *nothing at all* against
    // the canvas it exists to dim, and like near-perfect contrast against
    // white — a layer that visibly turns white into mid grey.
    let naive_on_base = contrast_ratio(scrim, p.surface_base);
    let naive_on_white = contrast_ratio(scrim, Color::WHITE);
    assert!(
        (naive_on_base - 1.0).abs() < RATIO_EPS,
        "the naive ratio is exactly 1.0 because the scrim's RGB *is* base: {naive_on_base:.4}"
    );
    assert!(
        (naive_on_white - 19.8954).abs() < RATIO_EPS,
        "the naive ratio reports near-maximum contrast for a dimming layer: {naive_on_white:.4}"
    );

    // Composited, the same questions get answers that describe what is
    // actually on screen.
    let over_base = composite_over(scrim, p.surface_base);
    let over_white = composite_over(scrim, Color::WHITE);
    let real_on_base = contrast_ratio(p.text_strong, over_base);
    let real_on_white = contrast_ratio(p.text_strong, over_white);

    assert!(
        (real_on_base - 19.8954).abs() < RATIO_EPS,
        "text.strong over a scrim laid on base is text.strong on base: {real_on_base:.4}"
    );
    assert!(
        (real_on_white - 4.4289).abs() < RATIO_EPS,
        "text.strong over a scrim laid on white is a marginal 4.4289:1, not 19.9:1: {real_on_white:.4}"
    );
    assert!(
        real_on_white + 1.0 < naive_on_white,
        "compositing must change the answer by more than a rounding step"
    );
    assert!(
        meets_text_contrast(p.text_strong, over_base),
        "the composited pairing clears the text rule"
    );
    assert!(
        !meets_text_contrast(p.text_primary, over_white),
        "text.primary over a scrim on white is 2.9293:1 — only the composited number can show that"
    );
}

// ---------------------------------------------------------------------------
// UIF-VER-001 — the mandated-pairing gate
// ---------------------------------------------------------------------------

/// Every foreground/background pairing the spec's usage contracts place
/// together, and which *does* clear the threshold its role carries.
///
/// The contracts are read from `doc/planning/ui-framework.md`:
///
/// * §2.5 — each token's `Role & Usage Contract` column
/// * §3.1 — the button variant × state matrix (BG/FG/Border triples)
/// * §3.2 — the text-input state matrix
/// * §4.4.2 — the tab visual state matrix
/// * §4.9.2 — the shared menu row state matrix
///
/// A surface is treated as a legitimate background for a text role
/// wherever any of those matrices puts that role on it. The five
/// pairings that used to fail under the spec's literal token values are
/// now in this table too, and are additionally pinned to their measured
/// values by
/// [`test_the_five_pairings_that_used_to_fail_now_clear_their_rule`],
/// so a regression in either direction is visible.
fn mandated_pairings() -> Vec<Pairing> {
    let p = palette();
    let e = EditorPalette::inverted();
    let t = MIN_TEXT_CONTRAST;
    let c = MIN_CHROME_CONTRAST;

    let mut v = Vec::new();

    // text.primary — "default body and resting control label text"
    // (§2.5). Placed on every surface by §3.1 (Ghost/Secondary rest FG),
    // §3.2 (Rest/Hovered value), §4.4.2 (inactive-hovered tab label)
    // and §4.7.1 (log viewport payload).
    for (name, bg) in all_surfaces(&p) {
        v.push(pairing(
            format!("text.primary on {name} — body/resting label (§2.5, §3.1, §3.2, §4.4.2)"),
            p.text_primary,
            bg,
            t,
        ));
    }

    // text.strong — "emphasized text, hovered/focused control labels"
    // (§2.5), and the FG of every Hovered/Pressed cell in §3.1, §4.9.2.
    for (name, bg) in all_surfaces(&p) {
        v.push(pairing(
            format!(
                "text.strong on {name} — emphasised/hovered/focused label (§2.5, §3.1, §4.9.2)"
            ),
            p.text_strong,
            bg,
            t,
        ));
    }

    // text.muted — "secondary descriptions, inactive tab labels,
    // keyboard shortcut hints, section headings" (§2.5, §4.8.1, §4.4.2,
    // §4.7.1 timestamps, §4.5.1 context label).
    for (name, bg) in all_surfaces(&p) {
        v.push(pairing(
            format!("text.muted on {name} — secondary/hint/heading text (§2.5, §4.8.1, §4.4.2)"),
            p.text_muted,
            bg,
            t,
        ));
    }

    // text.disabled — "disabled button/menu/input text and empty
    // placeholders" (§2.5). §3.1 puts it on surface.raised (Primary
    // disabled) and surface.base (Secondary/Danger disabled); §3.2 on
    // surface.code (rest placeholder) and surface.raised (disabled);
    // §4.9.2 on the transparent menu card, i.e. surface.overlay;
    // §4.4.2 on the inactive tab's close glyph over surface.raised.
    // It is never placed on surface.hover or surface.active — see
    // [`test_pairings_the_spec_never_places_on_hot_fills_are_recorded_not_assumed`].
    for (name, bg) in [
        ("surface.sunken", p.surface_sunken),
        ("surface.base", p.surface_base),
        ("surface.raised", p.surface_raised),
        ("surface.code", p.surface_code),
        ("surface.overlay", p.surface_overlay),
    ] {
        v.push(pairing(
            format!("text.disabled on {name} — disabled/placeholder text (§3.1, §3.2, §4.9.2)"),
            p.text_disabled,
            bg,
            t,
        ));
    }

    // accent.text — "headings, active links, menu checkmarks, submenu
    // indicators" (§2.5), plus the §4.7.1 `[Category]` tag over the
    // sunken log viewport and the §4.9.2 leading/trailing slots.
    for (name, bg) in all_surfaces(&p) {
        v.push(pairing(
            format!(
                "accent.text on {name} — heading/link/checkmark/chevron (§2.5, §4.7.1, §4.9.2)"
            ),
            p.accent_text,
            bg,
            t,
        ));
    }

    // status.success — "completion states, positive status chips"
    // (§2.5, §4.6.2 `✓ parallel` chip).
    for (name, bg) in all_surfaces(&p) {
        v.push(pairing(
            format!("status.success on {name} — completion status text/chip (§2.5, §4.6.2)"),
            p.status_success,
            bg,
            t,
        ));
    }

    // status.warning — "unsaved indicator dot, warning icons,
    // non-selected special item tint" (§2.5, §4.4.1 slot 2, §4.2.1).
    // The dirty dot only ever rides Active (overlay) or Inactive (raised)
    // tab fills (§4.4.2), never the pressed `surface.active`.
    for (name, bg) in [
        ("surface.sunken", p.surface_sunken),
        ("surface.base", p.surface_base),
        ("surface.raised", p.surface_raised),
        ("surface.code", p.surface_code),
        ("surface.overlay", p.surface_overlay),
        ("surface.hover", p.surface_hover),
    ] {
        v.push(pairing(
            format!("status.warning on {name} — unsaved dot / warning icon (§2.5, §4.4.1, §4.2.1)"),
            p.status_warning,
            bg,
            t,
        ));
    }

    // status.danger as *validation error text* on the surface §3.2
    // actually gives it: the Focused/Invalid input background is
    // `surface.base`, which is where the error message beside a rejected
    // field is drawn.
    v.push(pairing(
        "status.danger on surface.base — validation error text on the invalid-input background (§2.5, §3.2 Focused/Invalid)",
        p.status_danger,
        p.surface_base,
        t,
    ));

    // accent.primary as focus ring / active border (§3.1 Focused for
    // Secondary, Ghost and Danger; §4.4.2 active tab top bar;
    // §4.9.2 keyboard-focused row inset outline; §4.3.2 UIF-PANE-001
    // active splitter stroke) against every surface it can land on —
    // including surface.active, which the G-7 retune made possible by
    // darkening both the accent and that surface.
    for (name, bg) in all_surfaces(&p) {
        v.push(pairing(
            format!("accent.primary as focus ring / active border on {name} (§3.1, §4.4.2, §4.9.2, §4.3.2)"),
            p.accent_primary,
            bg,
            c,
        ));
    }

    // border.strong as the floating-card perimeter (§4.5.1, §4.6.1,
    // §4.7.1, §4.8.1) and the §3.1 Secondary rest border, against the
    // surfaces a card or control can present it over — surface.active
    // included, which is the Ghost pressed border and the latched
    // navbar trigger's outline (§4.2.2) and one of the five G-7 fixes.
    for (name, bg) in [
        ("surface.sunken", p.surface_sunken),
        ("surface.base", p.surface_base),
        ("surface.raised", p.surface_raised),
        ("surface.code", p.surface_code),
        ("surface.overlay", p.surface_overlay),
        ("surface.hover", p.surface_hover),
        ("surface.active", p.surface_active),
    ] {
        v.push(pairing(
            format!("border.strong perimeter on {name} — floating card / secondary border (§3.1, §4.5.1, §4.6.1, §4.8.1)"),
            p.border_strong,
            bg,
            c,
        ));
    }

    // The inverted editor surface (§4.6.2): black buffer text on the
    // white editor canvas, and the spec's RGB(180, 0, 0) error text on
    // the same canvas.
    v.push(pairing(
        "editor.inverted.fg on editor.inverted.bg — the editor buffer (§2.5, §4.6.2)",
        e.text,
        e.background,
        t,
    ));
    v.push(pairing(
        "editor.inverted.err on editor.inverted.bg — validation/save-error text (§2.5, §4.6.2)",
        e.error,
        e.background,
        t,
    ));

    v
}

#[test]
fn test_every_mandated_pairing_clears_its_threshold() {
    let pairings = mandated_pairings();
    assert!(
        pairings.len() > 50,
        "the gate table looks suspiciously short: only {} pairings",
        pairings.len()
    );
    for p in &pairings {
        assert_clears(p);
    }
}
// ---------------------------------------------------------------------------
// G-7 — the five pairings that used to fail, and now clear
// ---------------------------------------------------------------------------

/// A contract-mandated pairing that could **not** clear the rule its role
/// carries under the spec's literal token values, and does now.
struct RetunedPairing {
    /// The contract that mandates the pairing.
    contract: &'static str,
    fg: Color,
    bg: Color,
    /// The rule the role carries.
    min: f32,
    /// What the pairing measured before the retune — the shortfall
    /// **G-7** recorded, kept so the size of the fix stays visible.
    was: f32,
    /// What it measures now, pinned to three decimals.
    now: f32,
}

/// The five contract-mandated pairings **G-7** recorded as unable to
/// clear their rule, and the values that fixed them.
///
/// The fix is decision **A** in `doc/planning/iced-pro-tasks.md`: nudge
/// exactly three tokens — [`accent_primary`](Palette::accent_primary)
/// darker, [`surface_active`](Palette::surface_active) darker and
/// [`status_danger`](Palette::status_danger) lighter — because no single
/// token can absorb all five. `accent.primary` in particular is boxed in
/// from both sides: pairing 1 wants its luminance at or below `0.183333`
/// while pairing 4, taken alone, wants it at or above `0.197944`. The
/// lower bound exceeds the upper one, so the surface has to move too.
///
/// Each row is pinned rather than merely asserted to pass, so a later
/// drift that pushes one of these back under its threshold fails here
/// and names the pairing.
fn retuned_pairings() -> Vec<RetunedPairing> {
    let p = palette();
    let t = MIN_TEXT_CONTRAST;
    let c = MIN_CHROME_CONTRAST;
    vec![
        RetunedPairing {
            contract: "Selection Foreground Lock — text.strong on accent.primary (§2.5 rule 3, §3.1 Primary, §4.3.2 selected tree row)",
            fg: p.text_strong,
            bg: p.accent_primary,
            min: t,
            was: 4.4669,
            now: 4.5648,
        },
        RetunedPairing {
            contract: "status.danger on surface.overlay — input-dialog validation slot (§4.5.1) and modal error banner (§4.6.1)",
            fg: p.status_danger,
            bg: p.surface_overlay,
            min: t,
            was: 4.1473,
            now: 4.5718,
        },
        RetunedPairing {
            contract: "status.danger on surface.raised — Danger button resting foreground (§3.1 Danger Rest)",
            fg: p.status_danger,
            bg: p.surface_raised,
            min: t,
            was: 4.3800,
            now: 4.8283,
        },
        RetunedPairing {
            contract: "accent.primary on surface.active — Secondary pressed 1px accent border (§3.1 Pressed)",
            fg: p.accent_primary,
            bg: p.surface_active,
            min: c,
            was: 2.8441,
            now: 3.0750,
        },
        RetunedPairing {
            contract: "border.strong on surface.active — Ghost pressed border (§3.1) and latched navbar trigger outline (§4.2.2 UIF-NAV-001)",
            fg: p.border_strong,
            bg: p.surface_active,
            min: c,
            was: 2.8775,
            now: 3.1793,
        },
    ]
}

/// The retune's required headroom above each threshold, so nothing sits
/// knife-edge on the rule it just cleared.
const RETUNE_HEADROOM: f32 = 0.05;

#[test]
fn test_the_five_pairings_that_used_to_fail_now_clear_their_rule() {
    let retuned = retuned_pairings();
    assert_eq!(
        retuned.len(),
        5,
        "the G-7 retuned-pairing ledger changed size"
    );

    for r in &retuned {
        let ratio = contrast_ratio(r.fg, r.bg);
        assert!(
            (ratio - r.now).abs() < RATIO_EPS,
            "{}: measures {ratio:.4}, not the pinned {:.4} — re-derive the retune record",
            r.contract,
            r.now,
        );
        assert!(
            ratio >= r.min,
            "{}: {ratio:.4}:1 has fallen back below the {:.1}:1 its role requires \
             ({} before the retune) — the retune has been undone",
            r.contract,
            r.min,
            r.was,
        );
        assert!(
            ratio - r.min >= RETUNE_HEADROOM,
            "{}: only {:.4} of headroom over the {:.1}:1 rule, below the {:.2} the \
             retune promised — nothing may sit knife-edge",
            r.contract,
            ratio - r.min,
            r.min,
            RETUNE_HEADROOM,
        );
        assert!(
            ratio > r.was,
            "{}: {ratio:.4} is not an improvement on the pre-retune {}",
            r.contract,
            r.was,
        );

        let predicate_holds = if r.min >= MIN_TEXT_CONTRAST {
            meets_text_contrast(r.fg, r.bg)
        } else {
            meets_chrome_contrast(r.fg, r.bg)
        };
        assert!(
            predicate_holds,
            "{}: the role's own predicate disagrees with the measured {ratio:.4}:1",
            r.contract,
        );
    }
}

#[test]
fn test_pairings_the_spec_never_places_on_hot_fills_are_recorded_not_assumed() {
    // Nothing in §3.1, §3.2, §4.4.2 or §4.9.2 puts `text.disabled` or
    // `status.warning` on `surface.hover` or `surface.active`, so these
    // are NOT contract violations today. They are recorded because some
    // of them sit under the text threshold, and a future contract that
    // placed them there would be a real accessibility defect.
    //
    // `clears_text_rule` records which side of 4.5:1 each lands on now.
    // The G-7 retune darkened `surface.active`, which pushed
    // `status.warning` on it from `4.3777` to `4.8368` — over the line.
    // That is not a regression and it is not avoidable: holding that
    // pairing under 4.5:1 needs `L(surface.active) > 0.030402`, while
    // pairings 4 and 5 cap it at `0.025662`. The two requirements are
    // mutually unsatisfiable, so the honest record is the classification
    // rather than a blanket "must stay under".
    let p = palette();
    let cases: [(&str, Color, Color, f32, bool); 4] = [
        (
            "text.disabled on surface.hover",
            p.text_disabled,
            p.surface_hover,
            4.4734,
            false,
        ),
        (
            "text.disabled on surface.active",
            p.text_disabled,
            p.surface_active,
            4.1743,
            false,
        ),
        (
            "status.warning on surface.active",
            p.status_warning,
            p.surface_active,
            4.8368,
            true,
        ),
        (
            "status.danger on surface.active",
            p.status_danger,
            p.surface_active,
            3.6221,
            false,
        ),
    ];

    for (name, fg, bg, expected, clears) in cases {
        let ratio = contrast_ratio(fg, bg);
        assert!(
            (ratio - expected).abs() < RATIO_EPS,
            "{name}: measured {ratio:.4}, expected {expected:.4}"
        );
        assert_eq!(
            meets_text_contrast(fg, bg),
            clears,
            "{name}: {ratio:.4}:1 is on the wrong side of the 4.5:1 text rule for how it is recorded"
        );
    }

    // The tab close glyph goes `status.danger` over `surface.hover`
    // (§4.4.2). As a glyph rather than body text it is chrome, and as
    // chrome it clears 3.0 — which is why it is not in the G-7 ledger
    // even though it fails the text threshold.
    let glyph = contrast_ratio(p.status_danger, p.surface_hover);
    assert!(
        (glyph - 3.8817).abs() < RATIO_EPS,
        "status.danger glyph on surface.hover measures {glyph:.4}"
    );
    assert!(
        glyph < MIN_TEXT_CONTRAST,
        "it fails the *text* rule at {glyph:.4}:1"
    );
    assert!(
        meets_chrome_contrast(p.status_danger, p.surface_hover),
        "and passes the *chrome* rule, which is how a 14px glyph is allowed to read"
    );
}

// ---------------------------------------------------------------------------
// UIF-TOK-006 rule 2 — scope: resting dividers are not active chrome
// ---------------------------------------------------------------------------

#[test]
fn test_border_subtle_is_a_resting_divider_and_rule_two_does_not_reach_it() {
    let p = palette();

    // Measured against every surface it can rest on, border.subtle never
    // approaches 3.0: it tops out at 1.3732 against surface.sunken and
    // falls to 1.0203 against surface.hover, where it is nearly invisible.
    let mut best = 0.0_f32;
    for (name, bg) in all_surfaces(&p) {
        let ratio = contrast_ratio(p.border_subtle, bg);
        assert!(
            ratio < MIN_CHROME_CONTRAST,
            "border.subtle against {name} measures {ratio:.4}:1, which would clear the 3.0:1 chrome rule"
        );
        best = best.max(ratio);
    }
    assert!(
        (best - 1.3732).abs() < RATIO_EPS,
        "border.subtle's best case anywhere in the ladder is 1.3732:1, got {best:.4}"
    );
    assert!(
        (contrast_ratio(p.border_subtle, p.surface_base) - 1.3495).abs() < RATIO_EPS,
        "against surface.base — the pane-split line's home — it is 1.3495:1"
    );

    // The scope call, stated rather than implied: rule 2 governs *active*
    // indicators. border.subtle is the resting stroke — pane splits at
    // rest, menu separators, resting input borders, inactive tab borders
    // — and the instant a divider becomes active the spec replaces it
    // with accent.primary (UIF-PANE-001) or border.strong (§3.1 Ghost
    // Pressed). Applying 3.0:1 to it would fail every divider in the
    // interface for a token that was never an indicator.
    assert!(
        meets_chrome_contrast(p.border_strong, p.surface_overlay),
        "the *active* perimeter does clear rule 2 where it is actually specified"
    );
    assert!(
        meets_chrome_contrast(p.accent_primary, p.surface_base),
        "the *active* focus ring does clear rule 2 on the canvas"
    );
}

// ---------------------------------------------------------------------------
// UIF-TOK-006 rule 3 — the Selection Foreground Lock
// ---------------------------------------------------------------------------

#[test]
fn test_no_semantic_status_colour_may_render_on_accent_primary() {
    let p = palette();

    // Rule 3 forbids `danger`, `warning` and `success` as foregrounds on
    // accent.primary outright. The measured ratios are shown so a future
    // reader can see *why* the rule exists rather than taking it on faith.
    let forbidden: [(&str, Color, f32); 3] = [
        ("status.danger", p.status_danger, 1.1779),
        ("status.warning", p.status_warning, 1.5729),
        ("status.success", p.status_success, 3.4844),
    ];

    for (name, fg, expected) in forbidden {
        let ratio = contrast_ratio(fg, p.accent_primary);
        assert!(
            (ratio - expected).abs() < RATIO_EPS,
            "{name} on accent.primary measures {ratio:.4}, expected {expected:.4}"
        );
        assert!(
            !meets_text_contrast(fg, p.accent_primary),
            "{name} on accent.primary is {ratio:.4}:1 — forbidden, and unreadable as text anyway"
        );
    }

    // The rule is not derivable from contrast alone: `status.success`
    // would clear the 3.0:1 chrome bar on the accent fill. It is
    // forbidden because a red/green/gold *status* riding on the single
    // colour the interface uses to say "act here" collides semantically,
    // not because the maths objects.
    assert!(
        meets_chrome_contrast(p.status_success, p.accent_primary),
        "status.success would pass rule 2 on the accent fill — rule 3 is a semantic rule, not a numeric one"
    );

    // And the lock has exactly one occupant.
    assert!(
        contrast_ratio(p.text_strong, p.accent_primary)
            > contrast_ratio(p.text_primary, p.accent_primary),
        "text.strong is the mandated lock foreground and must beat text.primary on the accent fill"
    );
    assert!(
        !meets_text_contrast(p.text_primary, p.accent_primary),
        "text.primary on accent.primary is 3.0192:1 — the lock exists because the *default* text role fails here"
    );
}
#[test]
fn test_selection_foreground_lock_is_recorded_at_its_measured_value() {
    let p = palette();
    let locked = contrast_ratio(p.text_strong, p.accent_primary);

    // The value the lock pins now. G-1 resolved the spec's two
    // near-identical printed figures — 4.45:1 is accent.primary vs
    // surface.base, 4.46:1 is text.strong on accent.primary — but both
    // described the *literal* token values. The G-7 retune darkened the
    // accent, so neither printed figure describes this code any more;
    // the lock measures 4.5648 and the accent-vs-base pairing 4.3585.
    assert!(
        (locked - 4.5648).abs() < RATIO_EPS,
        "text.strong on accent.primary measures {locked:.4}, expected 4.5648"
    );
    assert!(
        (contrast_ratio(p.accent_primary, p.surface_base) - 4.3585).abs() < RATIO_EPS,
        "the *other* 4.4x figure is accent.primary vs surface.base, not the lock"
    );

    // What the lock does achieve: it is the best foreground the palette
    // offers on the accent fill, and it clears the 3.0:1 chrome bar.
    assert!(
        locked >= MIN_CHROME_CONTRAST,
        "the lock clears the chrome threshold at {locked:.4}:1"
    );
    assert!(
        locked
            > contrast_ratio(p.text_primary, p.accent_primary)
                .max(contrast_ratio(p.text_muted, p.accent_primary))
                .max(contrast_ratio(p.text_disabled, p.accent_primary)),
        "text.strong is the strongest text-role foreground available on the accent fill"
    );

    // What the retune achieved: before it, the lock was 4.4669:1 — a
    // selected tree row is 13px (§4.3.2, type.ui_compact), so the
    // mandated pairing sat 0.0331 under rule 1 and rule 1 was
    // unsatisfiable at the literal token values. It now clears with
    // 0.0648 to spare, which is what makes rule 3 a lock worth having
    // rather than a mandate to ship unreadable text.
    assert!(
        locked >= MIN_TEXT_CONTRAST,
        "the Selection Foreground Lock is {locked:.4}:1 against a {:.1}:1 rule",
        MIN_TEXT_CONTRAST,
    );
    assert!(
        (locked - MIN_TEXT_CONTRAST - 0.0648).abs() < RATIO_EPS,
        "the headroom over rule 1 is 0.0648, got {:.4}",
        locked - MIN_TEXT_CONTRAST,
    );
}

// ---------------------------------------------------------------------------
// Spec reconciliation — §2.5's printed contrast column vs the recomputation
// ---------------------------------------------------------------------------

/// The role a §2.5 row plays, which decides what threshold it owes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    /// A background surface. It carries no contrast duty of its own; the
    /// only honest assertion is that its ratio against base is a *valid*
    /// WCAG ratio, i.e. at least `1.0`.
    Background,
    /// A resting divider. Explicitly outside rule 2's "active" scope, so
    /// the assertion is that it stays *below* 3.0:1.
    RestingBorder,
    /// Active chrome — a focus ring or floating perimeter. Owes 3.0:1.
    ActiveChrome,
    /// A text role. Owes 4.5:1.
    Text,
    /// A status colour used as text or a chip. Owes 4.5:1.
    Status,
}

/// One row of `doc/planning/ui-framework.md` §2.5, transcribed with the
/// figure the spec prints and the role its usage contract implies.
struct SpecRow {
    token: &'static str,
    color: Color,
    /// The `Luminance Contrast vs color.surface.base` column, verbatim.
    printed: f32,
    role: Role,
    /// Whether G-5 (plus the two truncations found here) says this
    /// printed figure is materially wrong.
    printed_is_wrong: bool,
}

/// The eighteen numeric rows of §2.5.
///
/// `color.surface.scrim` is excluded: its contrast column is `—`,
/// because a translucent token has no contrast until it is composited
/// (see [`test_a_translucent_token_is_meaningless_until_it_is_composited`]).
/// `color.editor.inverted.*` is excluded too: its printed `21.00:1` is
/// the editor's own black-on-white pair, not a ratio against
/// `surface.base`, and it is asserted in the mandated table instead.
fn spec_rows() -> Vec<SpecRow> {
    let p = palette();
    vec![
        SpecRow {
            token: "color.surface.base",
            color: p.surface_base,
            printed: 1.00,
            role: Role::Background,
            printed_is_wrong: false,
        },
        SpecRow {
            token: "color.surface.sunken",
            color: p.surface_sunken,
            printed: 0.98,
            role: Role::Background,
            printed_is_wrong: true,
        },
        SpecRow {
            token: "color.surface.raised",
            color: p.surface_raised,
            printed: 1.06,
            role: Role::Background,
            printed_is_wrong: false,
        },
        SpecRow {
            token: "color.surface.code",
            color: p.surface_code,
            printed: 1.08,
            role: Role::Background,
            printed_is_wrong: false,
        },
        SpecRow {
            token: "color.surface.overlay",
            color: p.surface_overlay,
            printed: 1.12,
            role: Role::Background,
            printed_is_wrong: false,
        },
        SpecRow {
            token: "color.surface.hover",
            color: p.surface_hover,
            printed: 1.32,
            role: Role::Background,
            printed_is_wrong: false,
        },
        SpecRow {
            token: "color.surface.active",
            color: p.surface_active,
            printed: 1.56,
            role: Role::Background,
            printed_is_wrong: true,
        },
        SpecRow {
            token: "color.border.subtle",
            color: p.border_subtle,
            printed: 1.36,
            role: Role::RestingBorder,
            printed_is_wrong: true,
        },
        SpecRow {
            token: "color.border.strong",
            color: p.border_strong,
            printed: 3.92,
            role: Role::ActiveChrome,
            printed_is_wrong: true,
        },
        SpecRow {
            token: "color.accent.primary",
            color: p.accent_primary,
            printed: 4.45,
            role: Role::ActiveChrome,
            printed_is_wrong: true,
        },
        SpecRow {
            token: "color.accent.text",
            color: p.accent_text,
            printed: 10.12,
            role: Role::Text,
            printed_is_wrong: true,
        },
        SpecRow {
            token: "color.text.primary",
            color: p.text_primary,
            printed: 12.63,
            role: Role::Text,
            printed_is_wrong: true,
        },
        SpecRow {
            token: "color.text.strong",
            color: p.text_strong,
            printed: 19.86,
            role: Role::Text,
            printed_is_wrong: true,
        },
        SpecRow {
            token: "color.text.muted",
            color: p.text_muted,
            printed: 7.61,
            role: Role::Text,
            printed_is_wrong: false,
        },
        SpecRow {
            token: "color.text.disabled",
            color: p.text_disabled,
            printed: 5.86,
            role: Role::Text,
            printed_is_wrong: true,
        },
        SpecRow {
            token: "color.status.success",
            color: p.status_success,
            printed: 15.40,
            role: Role::Status,
            printed_is_wrong: true,
        },
        SpecRow {
            token: "color.status.warning",
            color: p.status_warning,
            printed: 5.50,
            role: Role::Status,
            printed_is_wrong: true,
        },
        SpecRow {
            token: "color.status.danger",
            color: p.status_danger,
            printed: 4.66,
            role: Role::Status,
            printed_is_wrong: true,
        },
    ]
}

#[test]
fn test_spec_rows_recompute_to_the_intended_role_threshold() {
    let p = palette();
    for row in spec_rows() {
        let computed = contrast_ratio(row.color, p.surface_base);
        match row.role {
            Role::Background => assert!(
                computed >= 1.0,
                "{}: a WCAG ratio cannot be below 1.0; the spec's printed {} for surface.sunken \
                 put the darker surface in the numerator (G-5)",
                row.token,
                row.printed
            ),
            Role::RestingBorder => assert!(
                computed < MIN_CHROME_CONTRAST,
                "{}: a resting divider measured {computed:.4}:1, inside rule 2's active-chrome scope",
                row.token
            ),
            Role::ActiveChrome => assert!(
                computed >= MIN_CHROME_CONTRAST,
                "{}: active chrome must clear 3.0:1 vs base, measured {computed:.4}",
                row.token
            ),
            Role::Text | Role::Status => assert!(
                computed >= MIN_TEXT_CONTRAST,
                "{}: a text/status role must clear 4.5:1 vs base, measured {computed:.4}",
                row.token
            ),
        }
    }
}

#[test]
fn test_spec_printed_figures_split_into_rounded_and_materially_wrong() {
    let p = palette();
    let rows = spec_rows();
    let wrong: Vec<&SpecRow> = rows
        .iter()
        .filter(|r| (contrast_ratio(r.color, p.surface_base) - r.printed).abs() > PRINT_EPS)
        .collect();

    for row in &rows {
        let computed = contrast_ratio(row.color, p.surface_base);
        let is_wrong = (computed - row.printed).abs() > PRINT_EPS;
        assert_eq!(
            is_wrong,
            row.printed_is_wrong,
            "{}: recomputed {computed:.4} vs printed {} (delta {:.4}) is classified as {}",
            row.token,
            row.printed,
            (computed - row.printed).abs(),
            if is_wrong { "wrong" } else { "rounded" },
        );
    }

    // G-5 listed eight materially wrong rows; INC-05 added `surface.active`
    // and `text.strong` as truncations that still miss the printed digits,
    // making ten. The G-7 retune moves `accent.primary` and `status.danger`
    // away from the figures the spec printed for their literal values, so
    // twelve of eighteen are now wrong against this code.
    assert_eq!(
        wrong.len(),
        12,
        "expected twelve materially wrong §2.5 figures, got: {:?}",
        wrong.iter().map(|r| r.token).collect::<Vec<_>>()
    );
    assert_eq!(
        rows.len() - wrong.len(),
        6,
        "expected six exact-or-rounded §2.5 figures"
    );
}

// ---------------------------------------------------------------------------
// Rule 1's size band and the typography floor
// ---------------------------------------------------------------------------

#[test]
fn test_rule_one_governs_every_rung_below_eighteen_pixels() {
    let rungs: [(&str, f32); 6] = [
        ("type.caption", typography::CAPTION.size),
        ("type.ui_compact", typography::UI_COMPACT.size),
        ("type.body", typography::BODY.size),
        ("type.title_sm", typography::TITLE_SM.size),
        ("type.title_md", typography::TITLE_MD.size),
        ("type.title_lg", typography::TITLE_LG.size),
    ];

    // Rule 1 is the rule for *normal* text, and the whole point of
    // MIN_SIZE is that nothing escapes it: the smallest text in the
    // interface is the most contrast-hungry, and it still owes 4.5:1.
    for (name, size) in rungs {
        assert!(
            size >= typography::MIN_SIZE,
            "{name} at {size}px sits below the 12px floor"
        );
        let normal_text = size < NORMAL_TEXT_MAX_PX;
        if normal_text {
            assert!(
                matches!(
                    name,
                    "type.caption" | "type.ui_compact" | "type.body" | "type.title_sm"
                ),
                "{name} unexpectedly classified as normal text"
            );
        } else {
            assert!(
                matches!(name, "type.title_md" | "type.title_lg"),
                "{name} at {size}px unexpectedly classified as large text"
            );
        }
    }

    // CAPTION is the rung that carries the floor rule, and it is squarely
    // inside rule 1's band: 12px text measured at the 4.5:1 threshold.
    let p = palette();
    assert!(
        meets_text_contrast(p.text_disabled, p.surface_base),
        "even the dimmest text role on the root canvas clears 4.5:1 at the floor rung"
    );
}
