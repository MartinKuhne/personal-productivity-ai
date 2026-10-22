//! Tests for [`contrast`] — the WCAG 2.1 solver that carries
//! `UIF-TOK-006` and the measurement half of `UIF-VER-001`.
//!
//! Four groups, in order:
//!
//! 1. **Solver correctness.** Reference luminances, the piecewise
//!    transfer function on both sides of its knee, ratio symmetry and
//!    bounds, and Porter-Duff source-over compositing — including the
//!    demonstration that a translucent token measured before it is
//!    composited yields a number describing nothing.
//! 2. **The four rule matrices.** `UIF-TOK-006` as revised on
//!    2026-10-22 is four rules, and each one is asserted here as a
//!    *complete* matrix rather than a spot check: rule 1 is 7 text roles ×
//!    7 surfaces, rule 2 is disabled text over all 7, rule 3 is 3 judged
//!    borders × 6 surfaces plus 4 solid fills × 4, and rule 4 is 4 fills
//!    × their locked foreground. A rule that is only sampled where it is
//!    known to pass is not being tested.
//! 3. **Widget-cell contracts.** The `BG`/`FG`/`Border` triples §3.1 and
//!    §3.2 actually specify, cell by cell, so a failure names the button
//!    state rather than a token pair.
//! 4. **Spec reconciliation.** Every numeric cell of the revised §2.5
//!    recomputed and compared with the figure the spec prints.
//!
//! # The two places the revised spec contradicts itself
//!
//! The 2026-10-22 revision fixed every value that ledger **G-7** had to
//! retune by hand, but it left two of its own rules at odds with its own
//! printed values. Both are pinned at their measured figure by a
//! `known_conflict_*` test and neither is tuned away:
//!
//! * **Rule 2's ceiling on `surface.sunken`.** The band is
//!   `[4.00:1, 6.00:1]`. `text.disabled` measures `6.020452:1` on
//!   `surface.sunken` — over the ceiling by `0.0205`, and inside the
//!   band on the other six surfaces. The token cannot satisfy the rule
//!   and stay dimmer than `text.muted`, and the surface cannot lighten
//!   without collapsing the elevation ladder.
//! * **Rule 3's `CoincidentWithFill` floor on `surface.overlay`.** A
//!   `1px` stroke set to the same token as its fill leaves the fill to
//!   carry the whole boundary, and the rule asks that for `3.08:1`.
//!   `status.danger.fill_active` against `surface.overlay` measures
//!   `3.078876:1` — short by `0.0011`, on a case as ordinary as a
//!   pressed destructive row on a floating menu.
//!
//! The second is worth reading carefully, because the spec's own table
//! *prints* `3.08:1` in that cell. The floor was set from the printed
//! rounding of the worst case rather than from the worst case, so the
//! rule is satisfiable only at two decimal places of precision and not at
//! the precision its own solver produces.
//!
//! # The spec's printed numeric columns are wrong in four cells
//!
//! Ledger gap **G-5** says §2.5's numeric columns were hand-collected.
//! Recomputing all `110` of them (22 rows × `L` + four contrast columns)
//! leaves `4` that do not match — `106` correct, `1` last-digit rounding
//! slip and `3` materially wrong:
//!
//! | Row | Column | Printed | Recomputed | Off by |
//! |---|---|---|---|---|
//! | `accent.fill_active` | `vs surface.active` | `2.52` | `2.5274` | `0.0074` — rounds to `2.53` |
//! | `status.danger.fill_active` | `L` | `0.1309` | `0.1325` | `0.0016` |
//! | `status.danger.fill_active` | `vs surface.base` | `3.43` | `3.4575` | `0.0275` |
//! | `status.danger.fill_active` | `vs surface.active` | `2.46` | `2.4842` | `0.0242` |
//!
//! Three of the four are one bad `L` propagating through that row's
//! derived columns; the `accent.fill_active` slip is independent. None
//! of the four changes a pass into a fail or the reverse. Nothing below
//! asserts equality with a printed figure:
//! `test_spec_rows_recompute_to_the_intended_role_threshold` asserts
//! each row clears the threshold its *role* needs, and
//! `test_spec_printed_figures_split_into_rounded_and_materially_wrong`
//! classifies every cell so the wrongness is recorded rather than
//! inherited.

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

/// The coincident-fill floor sits above the plain chrome floor, because a
/// fill that has to do a border's whole job alone is being asked to carry
/// more than a border that is itself visible.
const _: () = assert!(
    MIN_COINCIDENT_FILL_CONTRAST > MIN_CHROME_CONTRAST,
    "a fill carrying a coincident stroke owes more than ordinary chrome"
);

/// The disabled-text band must straddle the active-text floor, otherwise
/// rule 2 would not be a relaxation of rule 1 at all — it would be a
/// stricter, narrower thing and disabled text would be over-constrained.
const _: () = assert!(
    DISABLED_TEXT_MIN_CONTRAST < MIN_TEXT_CONTRAST
        && DISABLED_TEXT_MAX_CONTRAST > MIN_TEXT_CONTRAST,
    "the disabled band must bracket the active-text floor"
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
        ("accent.fill", p.accent_fill),
        ("accent.fill_active", p.accent_fill_active),
        ("accent.border", p.accent_border),
        ("accent.text", p.accent_text),
        ("text.primary", p.text_primary),
        ("text.strong", p.text_strong),
        ("text.muted", p.text_muted),
        ("text.disabled", p.text_disabled),
        ("status.danger_text", p.status_danger_text),
        ("status.danger_fill", p.status_danger_fill),
        ("status.danger_fill_active", p.status_danger_fill_active),
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
            ("accent.fill", p.accent_fill),
            ("accent.fill_active", p.accent_fill_active),
            ("accent.border", p.accent_border),
            ("accent.text", p.accent_text),
            ("text.primary", p.text_primary),
            ("text.strong", p.text_strong),
            ("text.muted", p.text_muted),
            ("text.disabled", p.text_disabled),
            ("status.danger_text", p.status_danger_text),
            ("status.danger_fill", p.status_danger_fill),
            ("status.danger_fill_active", p.status_danger_fill_active),
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
// UIF-TOK-006 rule 1 — active foreground text over the whole ladder
// ---------------------------------------------------------------------------

/// Every token rule 1 names as an *active* foreground.
///
/// `text.disabled` is deliberately absent: rule 2 lifts it out of rule 1
/// and puts it in a band instead. Listing it here would assert the floor
/// the revision explicitly removed.
fn active_text_roles(p: &Palette) -> [(&'static str, Color); 7] {
    [
        ("text.primary", p.text_primary),
        ("text.strong", p.text_strong),
        ("text.muted", p.text_muted),
        ("accent.text", p.accent_text),
        ("status.success", p.status_success),
        ("status.warning", p.status_warning),
        ("status.danger.text", p.status_danger_text),
    ]
}

/// Rule 1, asserted as the complete 7 × 7 matrix.
///
/// Every active text role against every surface in the ladder, including
/// the two interaction surfaces (`hover`, `active`) a text role is least
/// likely to be sampled on and most likely to fail against — they are the
/// lightest backgrounds in the set, so they give the smallest ratio.
#[test]
fn test_rule_one_is_the_full_seven_by_seven_text_matrix() {
    let p = palette();
    let mut worst = (f32::MAX, String::new());
    let mut checked = 0usize;

    for (role, fg) in active_text_roles(&p) {
        for (surface, bg) in all_surfaces(&p) {
            let ratio = contrast_ratio(fg, bg);
            assert!(
                ratio >= MIN_TEXT_CONTRAST,
                "rule 1: {role} on {surface} measures {ratio:.4}:1, below the \
                 {MIN_TEXT_CONTRAST:.1}:1 normal-text floor"
            );
            if ratio < worst.0 {
                worst = (ratio, format!("{role} on {surface}"));
            }
            checked += 1;
        }
    }

    assert_eq!(checked, 49, "rule 1 must cover 7 roles x 7 surfaces");
    assert!(
        (worst.0 - 4.722259).abs() < RATIO_EPS,
        "the binding case across the whole matrix is {}: expected 4.7223, got {:.4}",
        worst.1,
        worst.0
    );
    assert_eq!(
        worst.1, "status.danger.text on surface.active",
        "the binding case must be the darkest-red role on the lightest surface"
    );
}

/// The matrix's binding case is pinned separately from the sweep so a
/// change that moves the worst case *up* still shows up as a changed
/// number rather than a silently looser margin.
#[test]
fn test_rule_one_binding_pair_is_pinned_at_its_measured_value() {
    let p = palette();
    let ratio = contrast_ratio(p.status_danger_text, p.surface_active);
    assert!(
        (ratio - 4.722259).abs() < RATIO_EPS,
        "status.danger.text on surface.active is the tightest pairing rule 1 makes: expected \
         4.7223, got {ratio:.6}"
    );
    assert!(
        ratio - MIN_TEXT_CONTRAST > 0.22,
        "the binding case must still leave real headroom, not just clear the floor"
    );
}

// ---------------------------------------------------------------------------
// UIF-TOK-006 rule 2 — the disabled-text band
// ---------------------------------------------------------------------------

/// Rule 2 asserted over all seven surfaces, with the one breach
/// delegated to [`test_known_conflict_disabled_text_breaches_the_band_ceiling_on_sunken`].
///
/// Both bounds are asserted. A test that only checked the floor would let
/// disabled text drift up into the active range, which is precisely the
/// failure the band exists to prevent — a disabled control that reads as
/// live text is worse than one that reads as absent.
#[test]
fn test_rule_two_keeps_disabled_text_inside_its_band_on_six_surfaces() {
    let p = palette();
    let mut inside = Vec::new();

    for (surface, bg) in all_surfaces(&p) {
        let ratio = contrast_ratio(p.text_disabled, bg);
        if surface == "surface.sunken" {
            continue;
        }
        assert!(
            (DISABLED_TEXT_MIN_CONTRAST..=DISABLED_TEXT_MAX_CONTRAST).contains(&ratio),
            "rule 2: text.disabled on {surface} measures {ratio:.4}:1, outside the \
             [{DISABLED_TEXT_MIN_CONTRAST:.2}:1, {DISABLED_TEXT_MAX_CONTRAST:.2}:1] band"
        );
        inside.push(ratio);
    }

    assert_eq!(inside.len(), 6, "six of the seven surfaces must be inside");
    let lo = inside.iter().cloned().fold(f32::INFINITY, f32::min);
    let hi = inside.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    assert!(
        (lo - 4.250954).abs() < RATIO_EPS,
        "the low end of the band is surface.active at 4.2510, got {lo:.6}"
    );
    assert!(
        (hi - 5.916513).abs() < RATIO_EPS,
        "the high end of the band is surface.base at 5.9165, got {hi:.6}"
    );
    assert!(
        meets_disabled_text_band(p.text_disabled, p.surface_base),
        "the helper must agree with the inline band check on a surface that passes"
    );
}

/// **Known conflict.** Rule 2's `6.00:1` ceiling does not hold on
/// `surface.sunken`.
///
/// `text.disabled` (`RGB(140, 140, 140)`) against `surface.sunken`
/// (`RGB(6, 6, 8)`) measures `6.020452:1` — `0.020452` over the
/// ceiling. The token is fine everywhere else; the surface is simply dark
/// enough that a grey held dim against a *near-black* background crosses
/// a ceiling calibrated against `base` and `active`, the two cells the
/// rule actually prints.
///
/// Neither side can move without breaking something the rule also wants:
///
/// * Darkening `text.disabled` to clear `6.00:1` on sunken pushes it to
///   about `RGB(138, 138, 138)`, which narrows the gap to the
///   `text.muted` it is supposed to sit clearly below.
/// * Lightening `surface.sunken` to clear it collapses the `sunken →
///   base` step from `0.000911` toward nothing, and the whole point of
///   sunken is that it reads as a hole cut into the canvas.
///
/// So the value is left exactly as §2.5 specifies and the breach is
/// pinned here. If the band is ever re-derived, this assertion is the
/// thing that has to change deliberately rather than by accident.
#[test]
fn test_known_conflict_disabled_text_breaches_the_band_ceiling_on_sunken() {
    let p = palette();
    let ratio = contrast_ratio(p.text_disabled, p.surface_sunken);

    assert!(
        (ratio - 6.020452).abs() < RATIO_EPS,
        "pinned value: text.disabled on surface.sunken must measure exactly 6.0205, got {ratio:.6}"
    );
    assert!(
        ratio > DISABLED_TEXT_MAX_CONTRAST,
        "this is a genuine breach, not a rounding question: {ratio:.6} > \
         {DISABLED_TEXT_MAX_CONTRAST:.2}"
    );
    assert!(
        !meets_disabled_text_band(p.text_disabled, p.surface_sunken),
        "the band helper must report the breach"
    );
    assert!(
        ratio - DISABLED_TEXT_MAX_CONTRAST < 0.021,
        "and it must still be a hair over, not a structural miss: over by {:.6}",
        ratio - DISABLED_TEXT_MAX_CONTRAST
    );
    // The same token clears the band on every other surface, which is
    // what makes this a property of the (token, surface) pair rather than
    // of the token.
    for (surface, bg) in all_surfaces(&p) {
        if surface != "surface.sunken" {
            assert!(
                meets_disabled_text_band(p.text_disabled, bg),
                "text.disabled must clear the band on {surface} — only sunken is in conflict"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// UIF-TOK-006 rule 3 — the four-class chrome model
// ---------------------------------------------------------------------------

/// The four classes carry exactly the thresholds §3's table states, and
/// only one of them is exempt.
#[test]
fn test_chrome_class_thresholds_are_exactly_what_the_rule_prints() {
    assert_eq!(
        ChromeClass::Judged.minimum_contrast(),
        Some(MIN_CHROME_CONTRAST)
    );
    assert_eq!(
        ChromeClass::CoincidentWithFill.minimum_contrast(),
        Some(MIN_COINCIDENT_FILL_CONTRAST)
    );
    assert_eq!(
        ChromeClass::UnpaintedBackground.minimum_contrast(),
        Some(MIN_CHROME_CONTRAST),
        "an unpainted background is deferred, not exempt — once something is painted the \
         ordinary chrome floor applies to it"
    );
    assert_eq!(
        ChromeClass::RestingDivider.minimum_contrast(),
        None,
        "the resting divider is the only exempt class"
    );

    assert!(!ChromeClass::Judged.is_exempt());
    assert!(!ChromeClass::CoincidentWithFill.is_exempt());
    assert!(!ChromeClass::UnpaintedBackground.is_exempt());
    assert!(
        ChromeClass::RestingDivider.is_exempt(),
        "only RestingDivider reports exempt"
    );
}

/// The class-aware helper must agree with the plain helpers, so there is
/// no way for a call site to get a different answer by choosing a
/// different entry point.
#[test]
fn test_meets_chrome_contrast_as_agrees_with_the_plain_helpers() {
    let p = palette();
    let pairs = [
        (
            "border.strong vs surface.base",
            p.border_strong,
            p.surface_base,
        ),
        (
            "accent.border vs surface.overlay",
            p.accent_border,
            p.surface_overlay,
        ),
        (
            "status.danger.text vs surface.active",
            p.status_danger_text,
            p.surface_active,
        ),
        (
            "border.subtle vs surface.sunken",
            p.border_subtle,
            p.surface_sunken,
        ),
    ];

    for (label, a, b) in pairs {
        assert_eq!(
            meets_chrome_contrast_as(ChromeClass::Judged, a, b),
            meets_chrome_contrast(a, b),
            "Judged must be identical to the plain chrome check for {label}"
        );
        assert_eq!(
            meets_chrome_contrast_as(ChromeClass::CoincidentWithFill, a, b),
            meets_coincident_fill_contrast(a, b),
            "CoincidentWithFill must be identical to the coincident-fill check for {label}"
        );
    }

    // Exemption means "not measured", which the helper reports as true —
    // and that must be visibly different from the measurement failing.
    let subtle_on_active = contrast_ratio(p.border_subtle, p.surface_active);
    assert!(
        meets_chrome_contrast_as(
            ChromeClass::RestingDivider,
            p.border_subtle,
            p.surface_active
        ),
        "an exempt class returns true"
    );
    assert!(
        !meets_chrome_contrast(p.border_subtle, p.surface_active),
        "and the same pair genuinely fails when it *is* judged: {subtle_on_active:.4}:1"
    );
}

/// Rule 3's `Judged` border set, over the slice of ladder the rule names.
#[test]
fn test_rule_three_judged_borders_clear_every_surface_in_scope() {
    let p = palette();
    // "base through active" — sunken is excluded because no judged
    // border is specified to sit on it.
    let scope: Vec<(&'static str, Color)> = all_surfaces(&p)
        .into_iter()
        .filter(|(name, _)| *name != "surface.sunken")
        .collect();
    let judged = [
        ("border.strong", p.border_strong),
        ("accent.border", p.accent_border),
        ("status.danger.text", p.status_danger_text),
    ];

    let mut worst = (f32::MAX, String::new());
    for (role, stroke) in judged {
        for (surface, bg) in &scope {
            let ratio = contrast_ratio(stroke, *bg);
            assert!(
                ratio >= MIN_CHROME_CONTRAST,
                "rule 3 (Judged): {role} on {surface} measures {ratio:.4}:1, below the \
                 {MIN_CHROME_CONTRAST:.1}:1 chrome floor"
            );
            if ratio < worst.0 {
                worst = (ratio, format!("{role} on {surface}"));
            }
        }
    }

    assert_eq!(scope.len(), 6, "base through active is six surfaces");
    assert!(
        (worst.0 - 3.237672).abs() < RATIO_EPS,
        "the binding judged pairing is {}: expected 3.2377, got {:.4}",
        worst.1,
        worst.0
    );
    assert_eq!(worst.1, "border.strong on surface.active");
}

/// Rule 3's solid-fill set, over the slice of ladder the rule names.
#[test]
fn test_rule_three_solid_fills_clear_every_surface_in_scope() {
    let p = palette();
    // "base through overlay" — hover and active are excluded because a
    // solid CTA or destructive fill is never painted on top of another
    // interaction fill.
    let scope: Vec<(&'static str, Color)> = all_surfaces(&p)
        .into_iter()
        .filter(|(name, _)| {
            matches!(
                *name,
                "surface.base" | "surface.raised" | "surface.code" | "surface.overlay"
            )
        })
        .collect();
    let fills = [
        ("accent.fill", p.accent_fill),
        ("accent.fill_active", p.accent_fill_active),
        ("status.danger.fill", p.status_danger_fill),
        ("status.danger.fill_active", p.status_danger_fill_active),
    ];

    let mut worst = (f32::MAX, String::new());
    for (role, fill) in fills {
        for (surface, bg) in &scope {
            let ratio = contrast_ratio(fill, *bg);
            assert!(
                ratio >= MIN_CHROME_CONTRAST,
                "rule 3 (fill): {role} on {surface} measures {ratio:.4}:1, below the \
                 {MIN_CHROME_CONTRAST:.1}:1 floor"
            );
            if ratio < worst.0 {
                worst = (ratio, format!("{role} on {surface}"));
            }
        }
    }

    assert_eq!(scope.len(), 4, "base through overlay is four surfaces");
    assert!(
        (worst.0 - 3.078876).abs() < RATIO_EPS,
        "the binding fill pairing is {}: expected 3.0789, got {:.4}",
        worst.1,
        worst.0
    );
    assert_eq!(worst.1, "status.danger.fill_active on surface.overlay");
}

/// **Known conflict.** The `CoincidentWithFill` floor of `3.08:1` is
/// not reachable at full precision by the worst case in its own set.
///
/// The class covers a `1px` perimeter stroke set to the *same* token as
/// the solid fill it surrounds, where the fill alone has to separate the
/// control from the surface behind it. Over its three members and four
/// in-scope surfaces the worst pairing is
/// `status.danger.fill_active` against `surface.overlay` at
/// `3.078876:1` — `0.001124` short of the floor.
///
/// The floor was set from the printed value of that very cell: §2.5
/// prints `3.08:1`, and `3.078876` rounds to `3.08`. So the rule is
/// satisfiable at two decimal places and unsatisfiable at the precision
/// the solver that produced it works in. A pressed destructive row on a
/// floating menu is an ordinary, everyday case, not a corner.
///
/// The colour is left exactly as specified. Raising
/// `status.danger.fill_active` to clear `3.08:1` against overlay would
/// move a token the spec pins for its pressed-depth semantics in order
/// to rescue a threshold that was read off its own rounded output.
#[test]
fn test_known_conflict_coincident_danger_fill_active_falls_short_of_the_floor_on_overlay() {
    let p = palette();
    let ratio = contrast_ratio(p.status_danger_fill_active, p.surface_overlay);

    assert!(
        (ratio - 3.078876).abs() < RATIO_EPS,
        "pinned value: status.danger.fill_active on surface.overlay must measure exactly \
         3.0789, got {ratio:.6}"
    );
    assert!(
        ratio < MIN_COINCIDENT_FILL_CONTRAST,
        "this is a real miss against the coincident-fill floor: {ratio:.6} < \
         {MIN_COINCIDENT_FILL_CONTRAST:.2}"
    );
    assert!(
        !meets_coincident_fill_contrast(p.status_danger_fill_active, p.surface_overlay),
        "the coincident-fill helper must report the miss"
    );
    assert!(
        MIN_COINCIDENT_FILL_CONTRAST - ratio < 0.0012,
        "and by a hair, not a structural gap: short by {:.6}",
        MIN_COINCIDENT_FILL_CONTRAST - ratio
    );
    // It clears the *plain* chrome floor, which is what makes the class
    // distinction matter: judged as ordinary border it passes, judged as
    // coincident it does not.
    assert!(
        meets_chrome_contrast(p.status_danger_fill_active, p.surface_overlay),
        "the same pairing passes at 3.00:1 — the 0.08 difference is the whole conflict"
    );
    // Every other member of the coincident set clears the floor, so the
    // conflict is specific to this fill on this surface.
    for (fill_name, fill) in [
        ("accent.fill", p.accent_fill),
        ("accent.fill_active", p.accent_fill_active),
    ] {
        for (surface, bg) in all_surfaces(&p) {
            if matches!(
                surface,
                "surface.base" | "surface.raised" | "surface.code" | "surface.overlay"
            ) {
                assert!(
                    meets_coincident_fill_contrast(fill, bg),
                    "{fill_name} must clear the coincident floor on {surface}"
                );
            }
        }
    }
}

/// `border.subtle` is a `RestingDivider`: exempt, and far below the
/// floor it is exempt from.
///
/// The exemption is recorded against a token that would fail by a wide
/// margin, not against one that would have squeaked through. Its whole
/// spread across the ladder is `1.0314` to `1.3732`, so no reading of
/// the chrome rule could reach it.
#[test]
fn test_rule_three_resting_divider_is_exempt_and_nowhere_near_the_floor() {
    let p = palette();
    let mut hi = 0.0f32;
    let mut lo = f32::INFINITY;

    for (surface, bg) in all_surfaces(&p) {
        let ratio = contrast_ratio(p.border_subtle, bg);
        assert!(
            ratio < MIN_CHROME_CONTRAST,
            "border.subtle on {surface} must be well under the chrome floor, got {ratio:.4}:1"
        );
        assert!(
            meets_chrome_contrast_as(ChromeClass::RestingDivider, p.border_subtle, bg),
            "the exempt class must not fail on {surface}"
        );
        hi = hi.max(ratio);
        lo = lo.min(ratio);
    }

    assert!(
        (hi - 1.373162).abs() < RATIO_EPS,
        "its best case is surface.sunken at 1.3732, got {hi:.6}"
    );
    assert!(
        (lo - 1.031386).abs() < RATIO_EPS,
        "its worst case is surface.active at 1.0314, got {lo:.6}"
    );
    assert!(
        hi < 1.4,
        "a resting hairline tops out under 1.4:1 — it is structure, not signal"
    );
}

// ---------------------------------------------------------------------------
// UIF-TOK-006 rule 4 — the selection / solid-fill foreground lock
// ---------------------------------------------------------------------------

/// Every solid fill locked to [`Palette::text_strong`], with the span
/// the rule prints.
#[test]
fn test_rule_four_locks_every_solid_fill_to_text_strong() {
    let p = palette();
    let fills = [
        ("accent.fill", p.accent_fill),
        ("accent.fill_active", p.accent_fill_active),
        ("status.danger.fill", p.status_danger_fill),
        ("status.danger.fill_active", p.status_danger_fill_active),
    ];

    let mut lo = f32::INFINITY;
    let mut hi = 0.0f32;
    for (name, fill) in fills {
        let ratio = contrast_ratio(fill, p.text_strong);
        assert!(
            ratio >= 4.80,
            "rule 4: text.strong on {name} measures {ratio:.4}:1, under the 4.80:1 the lock \
             guarantees — and under the {MIN_TEXT_CONTRAST:.1}:1 text rule as well"
        );
        lo = lo.min(ratio);
        hi = hi.max(ratio);
    }

    assert!(
        (lo - 4.801988).abs() < RATIO_EPS,
        "the low end of the locked span is accent.fill at 4.8020, got {lo:.6}"
    );
    assert!(
        (hi - 5.754297).abs() < RATIO_EPS,
        "the high end of the locked span is status.danger.fill_active at 5.7543, got {hi:.6}"
    );
    // The rule says "yielding 4.80:1 to 5.75:1". Read as a floor that
    // is exact; read as a *ceiling* it would be breached by 0.0043,
    // because 5.754297 rounds to 5.75. The span is a description of
    // the observed range, not a tolerance, and pinning it here keeps
    // that reading explicit rather than assumed.
    assert!(
        hi > 5.75 && hi < 5.76,
        "the top of the span prints as 5.75 but measures {hi:.6} — the printed range is \
         descriptive, not a ceiling"
    );
}

/// The prohibition half of rule 4: a foreground-only token cannot be
/// read on a solid fill, which is *why* the rule forbids it.
///
/// None of the three clears even the `3.00:1` chrome floor on either
/// fill, let alone the `4.50:1` a label owes. The rule is protective
/// rather than arbitrary, and the measured spread is the evidence.
#[test]
fn test_rule_four_foreground_only_tokens_cannot_read_on_solid_fills() {
    let p = palette();
    let forbidden = [
        ("status.danger.text", p.status_danger_text),
        ("status.warning", p.status_warning),
        ("accent.text", p.accent_text),
    ];
    let fills = [
        ("accent.fill", p.accent_fill),
        ("status.danger.fill", p.status_danger_fill),
    ];

    let mut best = 0.0f32;
    for (role, fg) in forbidden {
        for (fill_name, fill) in fills {
            let ratio = contrast_ratio(fg, fill);
            assert!(
                ratio < MIN_CHROME_CONTRAST,
                "rule 4 forbids {role} on {fill_name}, and it could not read there anyway: \
                 {ratio:.4}:1 is under even the chrome floor"
            );
            assert!(
                !meets_text_contrast(fg, fill),
                "{role} on {fill_name} must fail the text rule outright"
            );
            best = best.max(ratio);
        }
    }

    assert!(
        (best - 2.745998).abs() < RATIO_EPS,
        "the best available forbidden pairing is accent.text on status.danger.fill at 2.7460, \
         got {best:.6}"
    );
}

// ---------------------------------------------------------------------------
// Widget-cell contracts — §3.1 buttons, §3.2 inputs, the editor
// ---------------------------------------------------------------------------

/// Each `BG`/`FG`/`Border` triple the widget matrices actually specify,
/// as a named cell.
///
/// The rule matrices above prove a token can never fail against a
/// surface. This proves the specific cells the product draws clear the
/// figure the spec prints beside them, so a regression names
/// "Danger/Pressed" rather than a token pair.
fn widget_cell_pairings() -> Vec<Pairing> {
    let p = palette();
    let e = EditorPalette::inverted();
    let t = MIN_TEXT_CONTRAST;
    let c = MIN_CHROME_CONTRAST;

    vec![
        // §3.1 Primary
        pairing(
            "§3.1 Primary/Rest FG on BG — text.strong on accent.fill",
            p.text_strong,
            p.accent_fill,
            t,
        ),
        pairing(
            "§3.1 Primary/Pressed FG on BG — text.strong on accent.fill_active",
            p.text_strong,
            p.accent_fill_active,
            t,
        ),
        pairing(
            "§3.1 Primary/Disabled FG on BG — text.disabled on surface.raised",
            p.text_disabled,
            p.surface_raised,
            DISABLED_TEXT_MIN_CONTRAST,
        ),
        // §3.1 Secondary
        pairing(
            "§3.1 Secondary/Rest FG on BG — text.primary on surface.raised",
            p.text_primary,
            p.surface_raised,
            t,
        ),
        pairing(
            "§3.1 Secondary/Rest border — border.strong on surface.raised",
            p.border_strong,
            p.surface_raised,
            c,
        ),
        pairing(
            "§3.1 Secondary/Hovered border — border.strong on surface.hover",
            p.border_strong,
            p.surface_hover,
            c,
        ),
        pairing(
            "§3.1 Secondary/Pressed border — accent.border on surface.active",
            p.accent_border,
            p.surface_active,
            c,
        ),
        // §3.1 Ghost
        pairing(
            "§3.1 Ghost/Hovered FG on BG — text.strong on surface.hover",
            p.text_strong,
            p.surface_hover,
            t,
        ),
        pairing(
            "§3.1 Ghost/Pressed border — border.strong on surface.active",
            p.border_strong,
            p.surface_active,
            c,
        ),
        // §3.1 Danger
        pairing(
            "§3.1 Danger/Rest FG on BG — status.danger.text on surface.raised",
            p.status_danger_text,
            p.surface_raised,
            t,
        ),
        pairing(
            "§3.1 Danger/Hovered FG on BG — text.strong on status.danger.fill",
            p.text_strong,
            p.status_danger_fill,
            t,
        ),
        pairing(
            "§3.1 Danger/Pressed FG on BG — text.strong on status.danger.fill_active",
            p.text_strong,
            p.status_danger_fill_active,
            t,
        ),
        pairing(
            "§3.1 Danger/Disabled FG on BG — text.disabled on surface.base",
            p.text_disabled,
            p.surface_base,
            DISABLED_TEXT_MIN_CONTRAST,
        ),
        // §3.2 Text inputs
        pairing(
            "§3.2 Rest value — text.primary on surface.code",
            p.text_primary,
            p.surface_code,
            t,
        ),
        pairing(
            "§3.2 Rest icon — text.muted on surface.code",
            p.text_muted,
            p.surface_code,
            t,
        ),
        pairing(
            "§3.2 Rest placeholder — text.disabled on surface.code",
            p.text_disabled,
            p.surface_code,
            DISABLED_TEXT_MIN_CONTRAST,
        ),
        pairing(
            "§3.2 Hovered border — border.strong on surface.code",
            p.border_strong,
            p.surface_code,
            c,
        ),
        pairing(
            "§3.2 Focused border — accent.border on surface.base",
            p.accent_border,
            p.surface_base,
            c,
        ),
        pairing(
            "§3.2 Invalid border — status.danger.text on surface.base",
            p.status_danger_text,
            p.surface_base,
            c,
        ),
        pairing(
            "§3.2 Disabled FG — text.disabled on surface.raised",
            p.text_disabled,
            p.surface_raised,
            DISABLED_TEXT_MIN_CONTRAST,
        ),
        // The inverted editor surface
        pairing("§2.5 editor.inverted — fg on bg", e.text, e.background, t),
        pairing("§2.5 editor.inverted — err on bg", e.error, e.background, t),
    ]
}

#[test]
fn test_every_specified_widget_cell_clears_its_threshold() {
    let cells = widget_cell_pairings();
    assert!(
        cells.len() >= 20,
        "the widget-cell table must cover every BG/FG/Border triple §3.1, §3.2 and the editor \
         print, got {}",
        cells.len()
    );
    for cell in &cells {
        assert_clears(cell);
    }
}

/// The editor's error red is chosen for white, not for the dark ladder.
///
/// §2.5 prints `7.00:1` for it; `RGB(180, 0, 0)` on white measures
/// `7.1413:1`. The printed figure is low by `0.14` — another G-5
/// transcription slip, outside the 22-row numeric table this file
/// reconciles, and recorded here so the editor cell is not the one place
/// a printed number gets taken at face value. Pure red would land near
/// `4.00:1` and fail the text rule outright, which is the actual reason
/// for the darker red.
#[test]
fn test_the_editor_error_red_clears_white_with_room_and_the_printed_figure_is_low() {
    let e = EditorPalette::inverted();
    let ratio = contrast_ratio(e.error, e.background);

    assert!(
        (ratio - 7.141254).abs() < RATIO_EPS,
        "editor err on white measures 7.1413, got {ratio:.6}"
    );
    assert!(
        ratio > 7.00,
        "the spec prints 7.00:1; the colour is better than that, not worse"
    );
    assert!(
        meets_text_contrast(e.error, e.background),
        "the editor error colour clears the text rule on white"
    );
    assert!(
        !meets_text_contrast(Color::from_rgb8(255, 0, 0), e.background),
        "pure red on white is about 4.0:1 and would fail — which is why the spec's darker red is \
         the right choice"
    );
}

/// Pairings the spec never draws, recorded so their measured values are
/// on file without being asserted as contracts.
///
/// Rule 2 states that a disabled control on a floating surface must
/// never paint an opaque hover or active fill beneath `text.disabled`.
/// That is a behavioural rule about widget code, not something the token
/// set can enforce, so it is recorded here rather than asserted: the
/// numbers show that even the forbidden pairing would stay inside the
/// band, which means the rule is protecting legibility *semantics*
/// (a disabled row must not look interactive) rather than compensating
/// for an unreadable combination.
#[test]
fn test_pairings_the_spec_never_places_are_recorded_not_assumed() {
    let p = palette();

    let on_hover = contrast_ratio(p.text_disabled, p.surface_hover);
    let on_active = contrast_ratio(p.text_disabled, p.surface_active);
    assert!(
        (on_hover - 4.828015).abs() < RATIO_EPS,
        "text.disabled on surface.hover records as 4.8280, got {on_hover:.6}"
    );
    assert!(
        (on_active - 4.250954).abs() < RATIO_EPS,
        "text.disabled on surface.active records as 4.2510, got {on_active:.6}"
    );
    assert!(
        meets_disabled_text_band(p.text_disabled, p.surface_hover)
            && meets_disabled_text_band(p.text_disabled, p.surface_active),
        "the forbidden pairings are legible; the rule forbids them for meaning, not readability"
    );

    // A judged border placed on the one surface outside its declared
    // scope, recorded so widening the scope later is a decision with
    // numbers attached rather than a guess.
    let strong_on_sunken = contrast_ratio(p.border_strong, p.surface_sunken);
    assert!(
        (strong_on_sunken - 4.585382).abs() < RATIO_EPS,
        "border.strong on surface.sunken records as 4.5854, got {strong_on_sunken:.6}"
    );
}

// ---------------------------------------------------------------------------
// Spec reconciliation — every numeric cell of the revised §2.5
// ---------------------------------------------------------------------------

/// Which `UIF-TOK-006` threshold a §2.5 row's colour is judged under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    /// Rule 1 active foreground text: `>= 4.50:1` on every surface.
    Text,
    /// Rule 3 `Judged` stroke: `>= 3.00:1` over `base` through `active`.
    JudgedBorder,
    /// Rule 3 solid fill: `>= 3.00:1` over `base` through `overlay`.
    SolidFill,
    /// Rule 3 `RestingDivider`: exempt.
    Exempt,
    /// A background surface — no foreground threshold of its own.
    Surface,
}

impl Role {
    /// The floor this role owes, or `None` when the role is exempt.
    fn floor(self) -> Option<f32> {
        match self {
            Self::Text => Some(MIN_TEXT_CONTRAST),
            Self::JudgedBorder | Self::SolidFill => Some(MIN_CHROME_CONTRAST),
            Self::Exempt | Self::Surface => None,
        }
    }
}

/// One numeric row of §2.5, with the figures the spec prints beside it.
struct SpecRow {
    token: &'static str,
    role: Role,
    printed_l: &'static str,
    printed_vs_base: &'static str,
    printed_vs_overlay: &'static str,
    printed_vs_active: &'static str,
    printed_vs_strong: &'static str,
}

/// Build a row.
fn row(
    token: &'static str,
    role: Role,
    printed_l: &'static str,
    printed_vs_base: &'static str,
    printed_vs_overlay: &'static str,
    printed_vs_active: &'static str,
    printed_vs_strong: &'static str,
) -> SpecRow {
    SpecRow {
        token,
        role,
        printed_l,
        printed_vs_base,
        printed_vs_overlay,
        printed_vs_active,
        printed_vs_strong,
    }
}

/// The 22 numeric rows of `doc/planning/ui-framework.md` §2.5 as
/// revised 2026-10-22, transcribed exactly as printed.
///
/// `surface.scrim` and `color.editor.inverted.*` are excluded: neither
/// carries the five-column numeric shape this table reconciles (the scrim
/// is translucent and the editor row is a multi-line cell).
fn spec_rows() -> Vec<SpecRow> {
    use Role::*;
    vec![
        row(
            "surface_sunken",
            Surface,
            "0.0019",
            "1.02",
            "1.14",
            "1.42",
            "20.24",
        ),
        row(
            "surface_base",
            Surface,
            "0.0028",
            "1.00",
            "1.12",
            "1.39",
            "19.90",
        ),
        row(
            "surface_raised",
            Surface,
            "0.0061",
            "1.06",
            "1.06",
            "1.31",
            "18.71",
        ),
        row(
            "surface_code",
            Surface,
            "0.0071",
            "1.08",
            "1.04",
            "1.29",
            "18.40",
        ),
        row(
            "surface_overlay",
            Surface,
            "0.0093",
            "1.12",
            "1.00",
            "1.24",
            "17.72",
        ),
        row(
            "surface_hover",
            Surface,
            "0.0147",
            "1.23",
            "1.09",
            "1.14",
            "16.24",
        ),
        row(
            "surface_active",
            Surface,
            "0.0235",
            "1.39",
            "1.24",
            "1.00",
            "14.29",
        ),
        row(
            "border_subtle",
            Exempt,
            "0.0212",
            "1.35",
            "1.20",
            "1.03",
            "14.74",
        ),
        row(
            "border_strong",
            JudgedBorder,
            "0.1878",
            "4.51",
            "4.01",
            "3.24",
            "4.42",
        ),
        row(
            "accent_fill",
            SolidFill,
            "0.1687",
            "4.14",
            "3.69",
            "2.98",
            "4.80",
        ),
        row(
            "accent_fill_active",
            SolidFill,
            "0.1356",
            "3.52",
            "3.13",
            "2.52",
            "5.66",
        ),
        row(
            "accent_border",
            JudgedBorder,
            "0.3020",
            "6.67",
            "5.94",
            "4.79",
            "2.98",
        ),
        row(
            "accent_text",
            Text,
            "0.5124",
            "10.66",
            "9.49",
            "7.66",
            "1.87",
        ),
        row(
            "text_primary",
            Text,
            "0.6445",
            "13.16",
            "11.72",
            "9.45",
            "1.51",
        ),
        row(
            "text_strong",
            Text,
            "1.0000",
            "19.90",
            "17.72",
            "14.29",
            "1.00",
        ),
        row("text_muted", Text, "0.3515", "7.61", "6.78", "5.47", "2.61"),
        row(
            "text_disabled",
            Exempt,
            "0.2623",
            "5.92",
            "5.27",
            "4.25",
            "3.36",
        ),
        row(
            "status_success",
            Text,
            "0.7515",
            "15.19",
            "13.52",
            "10.91",
            "1.31",
        ),
        row(
            "status_warning",
            Text,
            "0.3118",
            "6.86",
            "6.10",
            "4.93",
            "2.90",
        ),
        row(
            "status_danger_text",
            JudgedBorder,
            "0.2969",
            "6.57",
            "5.85",
            "4.72",
            "3.03",
        ),
        row(
            "status_danger_fill",
            SolidFill,
            "0.1548",
            "3.88",
            "3.46",
            "2.79",
            "5.13",
        ),
        row(
            "status_danger_fill_active",
            SolidFill,
            "0.1309",
            "3.43",
            "3.08",
            "2.46",
            "5.75",
        ),
    ]
}

/// A row's colour looked up by its §2.5 token name.
fn token_color(p: &Palette, token: &str) -> Color {
    match token {
        "surface_sunken" => p.surface_sunken,
        "surface_base" => p.surface_base,
        "surface_raised" => p.surface_raised,
        "surface_code" => p.surface_code,
        "surface_overlay" => p.surface_overlay,
        "surface_hover" => p.surface_hover,
        "surface_active" => p.surface_active,
        "border_subtle" => p.border_subtle,
        "border_strong" => p.border_strong,
        "accent_fill" => p.accent_fill,
        "accent_fill_active" => p.accent_fill_active,
        "accent_border" => p.accent_border,
        "accent_text" => p.accent_text,
        "text_primary" => p.text_primary,
        "text_strong" => p.text_strong,
        "text_muted" => p.text_muted,
        "text_disabled" => p.text_disabled,
        "status_success" => p.status_success,
        "status_warning" => p.status_warning,
        "status_danger_text" => p.status_danger_text,
        "status_danger_fill" => p.status_danger_fill,
        "status_danger_fill_active" => p.status_danger_fill_active,
        other => panic!("no such §2.5 token: {other}"),
    }
}

/// One printed cell that does not match the recomputed figure.
struct WrongCell {
    token: &'static str,
    column: &'static str,
    printed: &'static str,
    recomputed: f32,
}

/// Every cell in the reconciliation table whose printed figure differs
/// from the recomputed one at the printed precision.
fn wrong_cells() -> Vec<WrongCell> {
    let p = palette();
    let refs: [(&str, Color, &str); 5] = [
        ("L", Color::from_rgb8(0, 0, 0), "luminance"),
        ("vs surface.base", p.surface_base, "base"),
        ("vs surface.overlay", p.surface_overlay, "overlay"),
        ("vs surface.active", p.surface_active, "active"),
        ("vs text.strong", p.text_strong, "strong"),
    ];
    let printed_for = |r: &SpecRow, i: usize| -> &'static str {
        match i {
            0 => r.printed_l,
            1 => r.printed_vs_base,
            2 => r.printed_vs_overlay,
            3 => r.printed_vs_active,
            _ => r.printed_vs_strong,
        }
    };

    let mut out = Vec::new();
    for r in spec_rows() {
        let color = token_color(&p, r.token);
        for (i, (column, _, _)) in refs.iter().enumerate() {
            let recomputed = if i == 0 {
                relative_luminance(color)
            } else {
                let reference = match i {
                    1 => p.surface_base,
                    2 => p.surface_overlay,
                    3 => p.surface_active,
                    _ => p.text_strong,
                };
                contrast_ratio(color, reference)
            };
            let printed = printed_for(&r, i);
            let rounded = if i == 0 {
                format!("{recomputed:.4}")
            } else {
                format!("{recomputed:.2}")
            };
            if rounded != printed {
                out.push(WrongCell {
                    token: r.token,
                    column,
                    printed,
                    recomputed,
                });
            }
        }
    }
    out
}

/// Every row recomputes to a figure that satisfies its own role.
///
/// This is the assertion that matters: the printed column is compared for
/// record, but what is *checked* is the threshold the role carries.
#[test]
fn test_spec_rows_recompute_to_the_intended_role_threshold() {
    let p = palette();
    let rows = spec_rows();
    assert_eq!(rows.len(), 22, "§2.5 carries 22 numeric rows");

    for r in &rows {
        let color = token_color(&p, r.token);
        let Some(floor) = r.role.floor() else {
            continue;
        };
        // Judge each row against the surfaces its role's scope names, and
        // require the binding (lowest) case in that scope to clear.
        let scope: Vec<(&'static str, Color)> = match r.role {
            Role::Text => all_surfaces(&p).into_iter().collect(),
            Role::JudgedBorder => all_surfaces(&p)
                .into_iter()
                .filter(|(n, _)| *n != "surface.sunken")
                .collect(),
            Role::SolidFill => all_surfaces(&p)
                .into_iter()
                .filter(|(n, _)| {
                    matches!(
                        *n,
                        "surface.base" | "surface.raised" | "surface.code" | "surface.overlay"
                    )
                })
                .collect(),
            _ => Vec::new(),
        };
        let binding = scope
            .iter()
            .map(|(n, bg)| (contrast_ratio(color, *bg), *n))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .expect("every judged role has a non-empty scope");
        assert!(
            binding.0 >= floor,
            "{}: binding case {} measures {:.4}:1, below the {:.1}:1 its role carries",
            r.token,
            binding.1,
            binding.0,
            floor
        );
    }
}

/// All 110 numeric cells classified: correct, a last-digit rounding slip,
/// or materially wrong.
///
/// A cell counts as a **rounding slip** when it is off by less than one
/// unit of the printed precision — the digits are simply the wrong way
/// round. Anything larger is **materially wrong**. The threshold is the
/// printed unit itself, so the classifier is a statement about the data
/// rather than a tuned constant.
#[test]
fn test_spec_printed_figures_split_into_rounded_and_materially_wrong() {
    let total_cells = spec_rows().len() * 5;
    let wrong = wrong_cells();

    let mut slips = Vec::new();
    let mut material = Vec::new();
    for c in &wrong {
        let printed: f32 = c.printed.parse().expect("printed figures parse as numbers");
        let unit = if c.column == "L" { 1e-4 } else { 1e-2 };
        if (c.recomputed - printed).abs() < unit {
            slips.push(c);
        } else {
            material.push(c);
        }
    }

    assert_eq!(
        total_cells, 110,
        "22 rows x (L + four contrast columns) = 110 numeric cells"
    );
    assert_eq!(
        wrong.len(),
        4,
        "exactly 4 of 110 printed cells fail to match; got {}: {:?}",
        wrong.len(),
        wrong
            .iter()
            .map(|c| format!(
                "{} {} printed {} got {:.4}",
                c.token, c.column, c.printed, c.recomputed
            ))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        slips.len(),
        1,
        "one last-digit rounding slip expected; got {:?}",
        slips.iter().map(|c| c.token).collect::<Vec<_>>()
    );
    assert_eq!(
        material.len(),
        3,
        "three materially wrong cells expected; got {:?}",
        material.iter().map(|c| c.token).collect::<Vec<_>>()
    );
    assert_eq!(
        total_cells - wrong.len(),
        106,
        "106 of 110 printed cells are correct"
    );
}

/// The four wrong cells are two independent transcription errors, one of
/// which propagated.
#[test]
fn test_the_four_wrong_cells_trace_to_two_transcription_errors() {
    let wrong = wrong_cells();
    let ids: Vec<String> = wrong
        .iter()
        .map(|c| format!("{}.{}", c.token, c.column))
        .collect();

    // Error 1: a bad printed `L` on status.danger.fill_active, which its
    // two derived contrast columns inherit.
    assert!(
        ids.contains(&"status_danger_fill_active.L".to_string()),
        "the bad luminance must be present: {ids:?}"
    );
    assert!(
        ids.contains(&"status_danger_fill_active.vs surface.base".to_string()),
        "vs base inherits the bad L: {ids:?}"
    );
    assert!(
        ids.contains(&"status_danger_fill_active.vs surface.active".to_string()),
        "vs active inherits the bad L: {ids:?}"
    );
    // Its `vs surface.overlay` cell prints 3.08 and recomputes to
    // 3.0789, which rounds to 3.08 — correct, and the cell the
    // CoincidentWithFill floor was read off.
    assert!(
        !ids.contains(&"status_danger_fill_active.vs surface.overlay".to_string()),
        "vs overlay must round correctly — that is the cell the 3.08 floor came from"
    );

    // Error 2: an independent last-digit slip on accent.fill_active.
    assert!(
        ids.contains(&"accent_fill_active.vs surface.active".to_string()),
        "the independent rounding slip must be present: {ids:?}"
    );

    assert_eq!(
        wrong.len(),
        4,
        "no other cell may be wrong; the full set is {ids:?}"
    );
}

/// Rule 1's scope is a size statement, not a colour one: it applies to
/// every rung under 18px, and the type scale is almost entirely under it.
#[test]
fn test_rule_one_governs_every_rung_below_eighteen_pixels() {
    let rungs = [
        typography::CAPTION,
        typography::UI_COMPACT,
        typography::BODY,
        typography::TITLE_SM,
        typography::TITLE_MD,
        typography::TITLE_LG,
    ];
    let under = rungs.iter().filter(|r| r.size < NORMAL_TEXT_MAX_PX).count();

    assert_eq!(
        under, 4,
        "only TITLE_MD and TITLE_LG escape rule 1's normal-text band"
    );
    let smallest = rungs.iter().map(|r| r.size).fold(f32::MAX, f32::min);
    assert!(
        smallest >= typography::MIN_SIZE,
        "the absolute text floor keeps every rung inside a size band rule 1 measures"
    );
}
