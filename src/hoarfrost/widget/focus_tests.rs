//! Tests for [`focus`] — the adaptive focus-indicator rule
//! (`UIF-TOK-006` rule 3, decision **D-6** / gap **G-9**) and the two
//! placements §3.0.2 hangs off it.
//!
//! The rule has four branches and each is pinned separately: the accent
//! wins while it clears `3.0:1`, `text.strong` takes over when it does
//! not, a fill that defeats both returns the higher-contrast candidate,
//! and a transparent background is never measured as a fill. On top of
//! the branches the tests pin the *ordering* — the accent is preferred
//! whenever it is visible, so the rule reproduces the spec's own ring
//! colour everywhere the spec's ring actually shows — and purity.
//!
//! The second half covers the `1px` **inset outline** §3.0.2 gives menu
//! and tree rows: its stroke weight comes from `border.hairline`, its
//! geometry keeps the whole stroke inside the row's own perimeter while
//! the outer ring's grows outside the control's, and — the claim that
//! matters most — both placements are handed the *same* colour by the
//! *same* rule for the same background. That is what keeps a row's
//! outline from becoming a second, independently tunable rule that
//! quietly disagrees with the ring.
//!
//! Since the 2026-10-22 revision of §2.5 the candidate is
//! `accent.border` rather than `accent.primary`: the split gave the rule
//! a stroke token built for the job, so the fallback now fires on all
//! four saturated fills and on no surface at all.

use super::{
    FocusIndicator, FocusPlacement, resolve_focus_indicator_for_background,
    resolve_inset_outline_for_background, resolve_ring_color, resolve_ring_color_for_background,
};
use crate::token::color::Palette;
use crate::token::contrast::{MIN_CHROME_CONTRAST, contrast_ratio};
use crate::token::shape;
use iced::{Color, Rectangle};

/// Tolerance on a pinned contrast ratio — three decimals, enough to tell
/// `1.7188` from `3.0000`.
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

/// Every surface in the elevation ladder — the backgrounds a ring is
/// actually asked to stand out from inside the dark chrome.
///
/// White is deliberately not here. `accent.border` measures `2.9829:1`
/// against it, under the chrome floor, so the "accent wins everywhere"
/// sweep cannot include it; the light-background case is pinned on its
/// own by
/// [`test_a_white_backdrop_has_no_candidate_that_clears_the_floor`].
fn ring_backdrops() -> [(&'static str, Color); 7] {
    let p = palette();
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

/// A mid grey whose luminance sits close enough to `accent.border` to
/// swallow it, and close enough to white to swallow that too — so
/// *neither* candidate clears `3.0:1`.
///
/// `L = 0.4020`. Against the accent: `1.2840:1`. Against white:
/// `2.3231:1`. Both under the chrome floor, which is the case the rule
/// has to decide rather than leave undefined.
fn grey_neither_candidate_clears() -> Color {
    Color::from_rgb8(170, 170, 170)
}

/// A lighter mid grey that also defeats both candidates, but by more on
/// the white side — the accent ends up the *higher*-contrast candidate
/// even though neither passes.
///
/// `L = 0.5841`. Against the accent: `1.8013:1`. Against white:
/// `1.6559:1`.
///
/// The grey had to move up from `188` for this split. Against
/// `accent.border`, `RGB(188, 188, 188)` still leaves white ahead
/// (`1.8991:1` against `1.5707:1`), so it exercises the same branch as
/// the darker grey; `201` is the point where the ordering actually
/// flips, which is the only thing this test is for.
fn grey_accent_is_the_higher_loser() -> Color {
    Color::from_rgb8(201, 201, 201)
}

#[test]
fn test_accent_wins_against_every_surface_it_clears() {
    let p = palette();
    for (name, backdrop) in ring_backdrops() {
        let accent = contrast_ratio(p.accent_border, backdrop);
        let ring = resolve_ring_color(backdrop, &p);
        assert!(
            accent >= MIN_CHROME_CONTRAST,
            "precondition: the accent must clear the chrome floor against {name}, \
             got {accent:.4}:1"
        );
        assert_eq!(
            ring, p.accent_border,
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
    let accent = contrast_ratio(p.accent_border, fill);
    let strong = contrast_ratio(p.text_strong, fill);

    assert!(
        (accent - 1.7188).abs() < RATIO_EPS,
        "accent.border on status.danger_fill measures {accent:.4}:1"
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

/// The fallback fires on **all four** saturated fills and on none of
/// them partially: `accent.border` cannot be made out against a
/// saturated fill at all, so every one of them resolves to white.
#[test]
fn test_the_fallback_fires_on_every_saturated_fill() {
    let p = palette();
    let fills = [
        ("accent.fill", p.accent_fill, 1.6098),
        ("accent.fill_active", p.accent_fill_active, 1.8961),
        ("status.danger.fill", p.status_danger_fill, 1.7188),
        (
            "status.danger.fill_active",
            p.status_danger_fill_active,
            1.9291,
        ),
    ];
    for (name, fill, pinned) in fills {
        let accent = contrast_ratio(p.accent_border, fill);
        assert!(
            (accent - pinned).abs() < RATIO_EPS,
            "accent.border on {name} must measure {pinned:.4}:1, got {accent:.4}:1"
        );
        assert!(
            accent < MIN_CHROME_CONTRAST,
            "{name}: the accent cannot clear the floor against the fill, got {accent:.4}:1"
        );
        assert_eq!(
            resolve_ring_color(fill, &p),
            p.text_strong,
            "{name}: must resolve to the text.strong fallback"
        );
        assert!(
            contrast_ratio(p.text_strong, fill) >= MIN_CHROME_CONTRAST,
            "{name}: the fallback must clear the floor at {}",
            contrast_ratio(p.text_strong, fill)
        );
    }
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
        p.status_danger_text,
        p.status_danger_fill,
        p.status_danger_fill_active,
        p.accent_fill,
        p.accent_fill_active,
        p.text_strong,
    ];
    for fill in fills {
        let ring = resolve_ring_color(fill, &p);
        let ratio = contrast_ratio(ring, fill);
        let best = contrast_ratio(p.accent_border, fill).max(contrast_ratio(p.text_strong, fill));
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
    let accent = contrast_ratio(p.accent_border, fill);
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
    let accent = contrast_ratio(p.accent_border, fill);
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
        p.accent_border,
        "the tie-break is by contrast, not by token identity — the accent wins \
         whenever it is the better of the two"
    );
}

/// White is the one backdrop in the set where the rule has no good
/// answer, and it is pinned rather than glossed.
///
/// `accent.border` against white is `2.9829:1` — the spec's own table
/// prints `2.98:1` — and white against itself is `1.0000:1`. Neither
/// candidate reaches the chrome floor, so the resolver returns the
/// higher of the two, which is the accent. On the inverted editor
/// surface that means a focus ring that is technically the best available
/// and still under threshold; the shortfall belongs to the token set on a
/// light background, not to the rule.
#[test]
fn test_a_white_backdrop_has_no_candidate_that_clears_the_floor() {
    let p = palette();
    let accent = contrast_ratio(p.accent_border, Color::WHITE);
    let strong = contrast_ratio(p.text_strong, Color::WHITE);

    assert!(
        (accent - 2.982903).abs() < RATIO_EPS,
        "accent.border on white measures 2.9829, got {accent:.6}"
    );
    assert!(
        accent < MIN_CHROME_CONTRAST,
        "precondition: the accent must fall short on white, got {accent:.6}:1"
    );
    assert!(
        strong < MIN_CHROME_CONTRAST,
        "precondition: white on white cannot clear anything, got {strong:.6}:1"
    );
    assert_eq!(
        resolve_ring_color(Color::WHITE, &p),
        p.accent_border,
        "the resolver must still return the less invisible candidate"
    );
}

#[test]
fn test_an_exact_tie_resolves_to_text_strong() {
    let mut p = palette();
    // Collapse the two candidates onto one colour so their contrasts are
    // bit-identical, then pick a fill that leaves both under the floor.
    p.accent_border = p.text_strong;
    let fill = grey_neither_candidate_clears();

    assert_eq!(
        contrast_ratio(p.accent_border, fill),
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
        p.accent_border,
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
            ..p.accent_fill
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
        p.status_danger_text,
        p.status_danger_fill,
        p.accent_fill,
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
    let accent = contrast_ratio(p.accent_border, fill);
    let strong = contrast_ratio(p.text_strong, fill);
    assert!(accent >= MIN_CHROME_CONTRAST);
    assert!(strong > accent);
    assert_eq!(
        resolve_ring_color(fill, &p),
        p.accent_border,
        "the accent must win at {accent:.4}:1 even though text.strong clears by \
         more at {strong:.4}:1"
    );
}

// ---------------------------------------------------------------------------
// The inset outline — §3.0.2 rule 2's second placement
// ---------------------------------------------------------------------------

/// A row's own box, as wide and tall as a §4.8.1 menu row.
fn row_box() -> Rectangle {
    Rectangle::new((10.0, 40.0).into(), (240.0, 32.0).into())
}

/// The two placements, so every table below reads as placement names.
fn placements() -> [(FocusPlacement, &'static str); 2] {
    [
        (FocusPlacement::Outer, "outer ring"),
        (FocusPlacement::Inset, "inset outline"),
    ]
}

#[test]
fn test_each_placement_carries_its_own_stroke_weight_from_the_token_set() {
    // The weights are the two rungs `shape` defines, not two numbers
    // picked to look different: the ring is the attention stroke and the
    // outline is the structural one.
    assert_eq!(FocusPlacement::Outer.stroke(), shape::BORDER_FOCUS);
    assert_eq!(FocusPlacement::Inset.stroke(), shape::BORDER_HAIRLINE);
    assert_eq!(FocusPlacement::Outer.stroke(), 2.0);
    assert_eq!(FocusPlacement::Inset.stroke(), 1.0);
    assert_ne!(
        FocusPlacement::Outer.stroke(),
        FocusPlacement::Inset.stroke(),
        "if the two placements ever weigh the same, a row's outline stops \
         being distinguishable from a button's ring"
    );
}

#[test]
fn test_the_constructors_take_their_weight_from_their_placement() {
    let color = Color::from_rgb8(11, 22, 33);

    let outer = FocusIndicator::outer(color);
    assert_eq!(outer.color, color);
    assert_eq!(outer.width, FocusPlacement::Outer.stroke());
    assert_eq!(outer.placement, FocusPlacement::Outer);

    let inset = FocusIndicator::inset(color);
    assert_eq!(inset.color, color);
    assert_eq!(inset.width, FocusPlacement::Inset.stroke());
    assert_eq!(inset.placement, FocusPlacement::Inset);
}

/// **The colour rule is shared, not duplicated.**
///
/// Every background in the ladder, and the saturated fills on top of it,
/// must produce the *same colour* for both placements. The only thing
/// `placement` is allowed to change is geometry. If a second colour rule
/// were ever introduced for the inset case it would drift from the ring
/// rule the first time either was retuned, and this is the test that
/// notices.
#[test]
fn test_both_placements_pick_the_identical_colour_for_the_same_background() {
    let p = palette();
    let backgrounds = [
        p.surface_sunken,
        p.surface_base,
        p.surface_raised,
        p.surface_code,
        p.surface_overlay,
        p.surface_hover,
        p.surface_active,
        p.status_danger_fill,
        p.status_danger_fill_active,
        p.accent_fill,
        p.accent_fill_active,
        Color::TRANSPARENT,
        grey_neither_candidate_clears(),
        grey_accent_is_the_higher_loser(),
    ];

    for background in backgrounds {
        let shared = resolve_ring_color_for_background(background, &p);

        for (placement, name) in placements() {
            let indicator = resolve_focus_indicator_for_background(background, &p, placement);
            assert_eq!(
                indicator.color,
                shared,
                "{name} on {} resolved to a different colour than the shared \
                 rule produced — the placements must share one colour rule",
                rgb8(background),
            );
            assert_eq!(
                indicator.width,
                placement.stroke(),
                "{name} must carry its own stroke weight"
            );
            assert_eq!(indicator.placement, placement);
        }
    }
}

#[test]
fn test_the_inset_shorthand_is_the_shared_resolver_at_the_inset_placement() {
    let p = palette();
    for background in [p.surface_hover, p.status_danger_fill, Color::TRANSPARENT] {
        assert_eq!(
            resolve_inset_outline_for_background(background, &p),
            resolve_focus_indicator_for_background(background, &p, FocusPlacement::Inset),
            "the shorthand must not be a second path through the rule"
        );
    }
}

/// The geometry that makes an inset outline an inset outline: the stroke's
/// outer edge lands on the row's own perimeter, so the whole `1px` is
/// inside the row and no neighbour can clip it.
#[test]
fn test_the_inset_outline_stays_inside_the_rows_own_perimeter() {
    let row = row_box();
    let outline = FocusIndicator::inset(Color::WHITE);
    let stroked = outline.bounds(row);

    assert_eq!(
        stroked, row,
        "iced strokes a quad inward from its bounds, so an inset outline \
         takes the row's own box — shrinking it would push the stroke a \
         further stroke-width inside and leave a gap of fill at the \
         perimeter §3.0.2 does not describe"
    );
    assert!(
        stroked.x >= row.x && stroked.y >= row.y,
        "the outline must not reach outside the row"
    );
    assert!(
        stroked.x + stroked.width <= row.x + row.width
            && stroked.y + stroked.height <= row.y + row.height,
        "the outline must not reach outside the row"
    );
    assert_eq!(
        outline.radius(4.0),
        4.0,
        "the outline's outer edge IS the row's edge, so it rides the row's \
         own corner radius rather than inventing one"
    );
}

/// The outer ring is the mirror image: grown outward, with its own larger
/// radius so it stays concentric with what it rings.
#[test]
fn test_the_outer_ring_grows_outside_the_control_it_rings() {
    let control = row_box();
    let ring = FocusIndicator::outer(Color::WHITE);
    let stroked = ring.bounds(control);

    assert_eq!(stroked, control.expand(ring.width));
    assert!(stroked.x < control.x && stroked.y < control.y);
    assert!(stroked.width > control.width && stroked.height > control.height);
    assert_eq!(
        ring.radius(4.0),
        4.0 + ring.width,
        "the ring's outer edge is a stroke-width further out than the \
         control's, so its corner has to be a stroke-width rounder too"
    );
}

#[test]
fn test_the_two_placements_never_land_on_the_same_rectangle() {
    let control = row_box();
    let inset = FocusIndicator::inset(Color::WHITE).bounds(control);
    let outer = FocusIndicator::outer(Color::WHITE).bounds(control);

    assert_ne!(
        inset, outer,
        "one box for both placements would mean a renderer could not tell \
         them apart from the quad alone"
    );
    assert!(
        inset.area() < outer.area(),
        "the inset outline is strictly inside the ring's footprint"
    );
}

/// The two focused cells of §4.9.2, computed rather than configured.
///
/// §4.9.2 prints the normal focused row's outline as `5.35:1`. The
/// solver says **5.4427** — the crate's solver is authoritative over the
/// spec's rounded column, and the discrepancy is pinned here so the
/// record shows which number is real.
#[test]
fn test_the_two_spec_focus_cells_fall_out_of_the_shared_rule() {
    let p = palette();

    // Normal focused row: surface.hover + accent.border.
    let normal = resolve_inset_outline_for_background(p.surface_hover, &p);
    let accent_on_hover = contrast_ratio(p.accent_border, p.surface_hover);
    assert_eq!(
        normal.color, p.accent_border,
        "accent.border clears the chrome floor on surface.hover at \
         {accent_on_hover:.4}:1, so it must win"
    );
    assert!(
        (accent_on_hover - 5.4427).abs() < RATIO_EPS,
        "the normal focused row's outline measures {accent_on_hover:.4}:1, \
         expected the solver's 5.4427 — §4.9.2 prints 5.35, which is the \
         spec's rounding and not the truth"
    );
    assert_eq!(normal.width, shape::BORDER_HAIRLINE);
    assert_eq!(normal.placement, FocusPlacement::Inset);

    // Destructive focused row: status.danger.fill + text.strong.
    let destructive = resolve_inset_outline_for_background(p.status_danger_fill, &p);
    let accent_on_danger = contrast_ratio(p.accent_border, p.status_danger_fill);
    let strong_on_danger = contrast_ratio(p.text_strong, p.status_danger_fill);
    assert!(
        (accent_on_danger - 1.7188).abs() < RATIO_EPS,
        "accent.border on the danger fill measures {accent_on_danger:.4}:1"
    );
    assert!(
        (strong_on_danger - 5.1270).abs() < RATIO_EPS,
        "text.strong on the danger fill measures {strong_on_danger:.4}:1"
    );
    assert_eq!(
        destructive.color, p.text_strong,
        "the accent cannot be seen on the danger fill, so the outline \
         degrades to text.strong exactly as a ring would"
    );
    assert_eq!(destructive.width, shape::BORDER_HAIRLINE);
    assert_eq!(destructive.placement, FocusPlacement::Inset);

    // Same fill, both placements, same colour — the cells differ from a
    // button's ring in geometry only.
    for fill in [p.surface_hover, p.status_danger_fill] {
        assert_eq!(
            resolve_focus_indicator_for_background(fill, &p, FocusPlacement::Outer).color,
            resolve_focus_indicator_for_background(fill, &p, FocusPlacement::Inset).color,
            "{}: the row's outline and the button's ring must agree on colour",
            rgb8(fill),
        );
    }
}

#[test]
fn test_an_unpainted_row_background_short_circuits_the_outline_to_the_accent() {
    let p = palette();
    // A Rest row paints nothing, so its outline belongs to the card, not
    // to `transparent` — the same short-circuit the ring already makes.
    let outline = resolve_inset_outline_for_background(Color::TRANSPARENT, &p);
    assert_eq!(outline.color, p.accent_border);
    assert_eq!(outline.placement, FocusPlacement::Inset);
    assert_eq!(outline.width, shape::BORDER_HAIRLINE);
}

#[test]
fn test_the_inset_outline_clears_the_chrome_floor_against_every_surface() {
    let p = palette();
    for (name, surface) in ring_backdrops() {
        let outline = resolve_inset_outline_for_background(surface, &p);
        let ratio = contrast_ratio(outline.color, surface);
        assert!(
            ratio >= MIN_CHROME_CONTRAST,
            "{name}: the inset outline measures {ratio:.4}:1, under the \
             {:.1}:1 chrome floor",
            MIN_CHROME_CONTRAST,
        );
        assert_ne!(
            outline.color, surface,
            "{name}: the outline must not be \
                    the row's own colour"
        );
    }
}

#[test]
fn test_the_indicator_resolvers_are_pure_and_total() {
    let p = palette();
    for fill in [
        p.surface_base,
        p.surface_hover,
        p.status_danger_fill,
        grey_neither_candidate_clears(),
        Color::TRANSPARENT,
        Color::WHITE,
    ] {
        for (placement, name) in placements() {
            let first = resolve_focus_indicator_for_background(fill, &p, placement);
            let second = resolve_focus_indicator_for_background(fill, &p, placement);
            assert_eq!(first, second, "{name} on {} is not pure", rgb8(fill));
            assert!(
                first.width.is_finite()
                    && first.color.r.is_finite()
                    && first.color.g.is_finite()
                    && first.color.b.is_finite(),
                "{name}: no resolved channel may be NaN or infinite"
            );
            assert!(first.bounds(row_box()).area() > 0.0);
        }
    }
}
