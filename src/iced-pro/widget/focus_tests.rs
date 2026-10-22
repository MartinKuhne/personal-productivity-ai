//! Tests for [`focus`] — the adaptive focus-ring rule (`UIF-TOK-006`
//! rule 2, decision **D-6** / gap **G-9**).
//!
//! The rule has four branches and each is pinned separately: the accent
//! wins while it clears `3.0:1`, `text.strong` takes over when it does
//! not, a fill that defeats both returns the higher-contrast candidate,
//! and a transparent background is never measured as a fill. On top of
//! the branches the tests pin the *ordering* — the accent is preferred
//! whenever it is visible, so the rule reproduces the spec's own ring
//! colour everywhere the spec's ring actually shows — and purity.

use super::{resolve_ring_color, resolve_ring_color_for_background};
use crate::token::color::Palette;
use crate::token::contrast::{MIN_CHROME_CONTRAST, contrast_ratio};
use iced::Color;

/// Tolerance on a pinned contrast ratio — three decimals, enough to tell
/// `1.2089` from `3.0000`.
const RATIO_EPS: f32 = 1e-3;

/// The dark palette, taken once so every table reads as token names.
fn palette() -> Palette {
    Palette::dark()
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

/// Every surface in the elevation ladder, plus white — the full set of
/// backgrounds a ring can legitimately be asked to stand out from.
fn ring_backdrops() -> [(&'static str, Color); 8] {
    let p = palette();
    [
        ("surface.sunken", p.surface_sunken),
        ("surface.base", p.surface_base),
        ("surface.raised", p.surface_raised),
        ("surface.code", p.surface_code),
        ("surface.overlay", p.surface_overlay),
        ("surface.hover", p.surface_hover),
        ("surface.active", p.surface_active),
        ("text.strong", p.text_strong),
    ]
}

/// A mid grey whose luminance sits close enough to `accent.primary` to
/// swallow it, and close enough to white to swallow that too — so
/// *neither* candidate clears `3.0:1`.
///
/// `L = 0.4019`. Against the accent: `1.9646:1`. Against white:
/// `2.3236:1`. Both under the chrome floor, which is the case the rule
/// has to decide rather than leave undefined.
fn grey_neither_candidate_clears() -> Color {
    Color::from_rgb8(170, 170, 170)
}

/// A lighter mid grey that also defeats both candidates, but by more on
/// the accent side — the accent ends up the *higher*-contrast candidate
/// even though neither passes.
///
/// `L = 0.5029`. Against the accent: `2.4037:1`. Against white:
/// `1.8991:1`.
fn grey_accent_is_the_higher_loser() -> Color {
    Color::from_rgb8(188, 188, 188)
}

#[test]
fn test_accent_wins_against_every_surface_it_clears() {
    let p = palette();
    for (name, backdrop) in ring_backdrops() {
        let accent = contrast_ratio(p.accent_primary, backdrop);
        let ring = resolve_ring_color(backdrop, &p);
        assert!(
            accent >= MIN_CHROME_CONTRAST,
            "precondition: the accent must clear the chrome floor against {name}, \
             got {accent:.4}:1"
        );
        assert_eq!(
            ring, p.accent_primary,
            "{name}: the accent clears {:.1}:1 at {accent:.4}:1, so it must be \
             the ring — the spec's own ring colour is not displaced where it is \
             already visible",
            MIN_CHROME_CONTRAST,
        );
    }
}

#[test]
fn test_text_strong_wins_when_the_accent_cannot_be_seen() {
    let p = palette();
    let fill = p.status_danger_fill;
    let accent = contrast_ratio(p.accent_primary, fill);
    let strong = contrast_ratio(p.text_strong, fill);

    assert!(
        (accent - 1.2089).abs() < RATIO_EPS,
        "accent.primary on status.danger_fill measures {accent:.4}:1"
    );
    assert!(
        accent < MIN_CHROME_CONTRAST,
        "precondition: the accent must fail against the danger fill, got \
         {accent:.4}:1 — the G-9 case has gone away"
    );
    assert_eq!(
        resolve_ring_color(fill, &p),
        p.text_strong,
        "the fallback must be text.strong at {strong:.4}:1, not an invisible \
         accent at {accent:.4}:1"
    );
    assert!(
        contrast_ratio(p.text_strong, fill) >= MIN_CHROME_CONTRAST,
        "the fallback must itself clear the chrome floor"
    );
}

#[test]
fn test_the_rule_finds_a_visible_ring_whenever_one_exists() {
    let p = palette();
    let fills = [
        p.surface_sunken,
        p.surface_base,
        p.surface_raised,
        p.surface_code,
        p.surface_overlay,
        p.surface_hover,
        p.surface_active,
        p.status_danger,
        p.status_danger_fill,
        p.accent_primary,
        p.text_strong,
    ];
    for fill in fills {
        let ring = resolve_ring_color(fill, &p);
        let ratio = contrast_ratio(ring, fill);
        let best = contrast_ratio(p.accent_primary, fill).max(contrast_ratio(p.text_strong, fill));
        assert!(
            ratio >= MIN_CHROME_CONTRAST || best < MIN_CHROME_CONTRAST,
            "a fill at {} had a visible candidate at {best:.4}:1 but got a ring \
             at {ratio:.4}:1 — the resolver missed a ring it could have drawn",
            rgb8(fill),
        );
    }
}

#[test]
fn test_when_neither_candidate_clears_the_higher_contrast_one_is_returned() {
    let p = palette();
    let fill = grey_neither_candidate_clears();
    let accent = contrast_ratio(p.accent_primary, fill);
    let strong = contrast_ratio(p.text_strong, fill);

    assert!(
        accent < MIN_CHROME_CONTRAST && strong < MIN_CHROME_CONTRAST,
        "precondition: both candidates must fail — accent {accent:.4}:1, \
         strong {strong:.4}:1"
    );
    assert!(
        strong > accent,
        "precondition: text.strong must be the higher-contrast candidate here"
    );
    assert_eq!(
        resolve_ring_color(fill, &p),
        p.text_strong,
        "with both candidates under {:.1}:1 the resolver must still return the \
         less invisible one, not give up",
        MIN_CHROME_CONTRAST,
    );
}

#[test]
fn test_the_failing_branch_can_pick_the_accent_when_it_is_the_higher_contrast_one() {
    let p = palette();
    let fill = grey_accent_is_the_higher_loser();
    let accent = contrast_ratio(p.accent_primary, fill);
    let strong = contrast_ratio(p.text_strong, fill);

    assert!(
        accent < MIN_CHROME_CONTRAST && strong < MIN_CHROME_CONTRAST,
        "precondition: both candidates must fail — accent {accent:.4}:1, \
         strong {strong:.4}:1"
    );
    assert!(
        accent > strong,
        "precondition: the accent must be the higher-contrast candidate here"
    );
    assert_eq!(
        resolve_ring_color(fill, &p),
        p.accent_primary,
        "the tie-break is by contrast, not by token identity — the accent wins \
         whenever it is the better of the two"
    );
}

#[test]
fn test_an_exact_tie_resolves_to_text_strong() {
    let mut p = palette();
    // Collapse the two candidates onto one colour so their contrasts are
    // bit-identical, then pick a fill that leaves both under the floor.
    p.accent_primary = p.text_strong;
    let fill = grey_neither_candidate_clears();

    assert_eq!(
        contrast_ratio(p.accent_primary, fill),
        contrast_ratio(p.text_strong, fill),
        "precondition: the two candidates must tie exactly"
    );
    assert!(
        contrast_ratio(p.text_strong, fill) < MIN_CHROME_CONTRAST,
        "precondition: the tie must be below the chrome floor"
    );
    assert_eq!(
        resolve_ring_color(fill, &p),
        p.text_strong,
        "a tie must resolve to text.strong, the documented preference, rather \
         than being left to chance"
    );
}

#[test]
fn test_a_transparent_background_is_never_resolved_as_a_fill() {
    let p = palette();
    // Resolving against `transparent` would measure a colour that is
    // never painted. The short-circuit returns the spec's own ring
    // colour instead, and never consults the fill rule.
    assert_eq!(
        resolve_ring_color_for_background(Color::TRANSPARENT, &p),
        p.accent_primary,
        "a Ghost button's ring sits on the container's surface, so it must get \
         the spec's accent ring rather than a measurement against `transparent`"
    );
    assert_eq!(
        resolve_ring_color_for_background(Color::TRANSPARENT, &p),
        resolve_ring_color(p.surface_base, &p),
        "the short-circuit must agree with what the rule would choose on the \
         darkest surface a Ghost button can sit on"
    );
    assert_eq!(
        resolve_ring_color(Color::TRANSPARENT, &p),
        resolve_ring_color(Color::BLACK, &p),
        "the raw fill rule ignores alpha, so `transparent` and `black` are \
         indistinguishable to it — a measurement of a colour that is never \
         painted, which is exactly why a widget must hand its background to the \
         background-aware entry point rather than to the fill rule"
    );
}

#[test]
fn test_a_painted_background_delegates_to_the_fill_rule() {
    let p = palette();
    let painted = [
        p.surface_raised,
        p.surface_hover,
        p.surface_active,
        p.status_danger_fill,
        Color {
            a: 0.85,
            ..p.status_danger_fill
        },
        Color {
            a: 0.85,
            ..p.accent_primary
        },
    ];
    for background in painted {
        assert_eq!(
            resolve_ring_color_for_background(background, &p),
            resolve_ring_color(background, &p),
            "any background with a non-zero alpha must go through the fill rule, \
            including the translucent pressed fills"
        );
    }
}

#[test]
fn test_the_ring_is_a_real_ring_against_every_backdrop() {
    let p = palette();
    for (name, backdrop) in ring_backdrops() {
        let ring = resolve_ring_color_for_background(backdrop, &p);
        let ratio = contrast_ratio(ring, backdrop);
        assert!(
            ratio >= MIN_CHROME_CONTRAST,
            "{name}: the ring the shared rule picks measures {ratio:.4}:1, under \
             the {:.1}:1 chrome floor",
            MIN_CHROME_CONTRAST,
        );
        assert_ne!(
            ring, backdrop,
            "{name}: the ring must not be the backdrop's own colour"
        );
    }
}

#[test]
fn test_resolve_ring_color_is_pure_and_total() {
    let p = palette();
    let fills = [
        p.surface_base,
        p.status_danger,
        p.status_danger_fill,
        grey_neither_candidate_clears(),
        grey_accent_is_the_higher_loser(),
        Color::TRANSPARENT,
        Color::BLACK,
        Color::WHITE,
    ];
    for fill in fills {
        let first = resolve_ring_color(fill, &p);
        let second = resolve_ring_color(fill, &p);
        assert_eq!(
            first, second,
            "the rule must be a pure function of its input"
        );
        assert!(
            first.r.is_finite() && first.g.is_finite() && first.b.is_finite(),
            "no resolved channel may be NaN or infinite"
        );
        assert_eq!(
            resolve_ring_color_for_background(fill, &p),
            resolve_ring_color_for_background(fill, &p),
            "the background-aware entry point must be pure too"
        );
    }
}

#[test]
fn test_the_accent_is_preferred_over_an_equal_or_worse_fallback() {
    let p = palette();
    // Wherever the accent clears the floor it must win, even when
    // text.strong would clear it by further. The accent is the
    // interface's designated focus colour; degrading to white is a
    // concession, not a preference.
    let fill = p.surface_active;
    let accent = contrast_ratio(p.accent_primary, fill);
    let strong = contrast_ratio(p.text_strong, fill);
    assert!(accent >= MIN_CHROME_CONTRAST);
    assert!(strong > accent);
    assert_eq!(
        resolve_ring_color(fill, &p),
        p.accent_primary,
        "the accent must win at {accent:.4}:1 even though text.strong clears by \
         more at {strong:.4}:1"
    );
}
