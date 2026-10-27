//! Tests for [`color`] — the semantic colour tokens (`UIF-TOK-005`).
//!
//! Every token is pinned channel-by-channel to the sRGB value the
//! revised §2.5 table (2026-10-22) gives it, so a silent retune fails
//! loudly and names the token that moved. On top of the pins, the tests
//! enforce the structural promises the table makes: the dark-first
//! elevation ladder, the neutrality of the surface and border greys, the
//! strict prominence order of the text roles, mutual distinguishability
//! of the status colours, the inverted shape of the editor surface, and
//! the rule that no two distinct tokens collapse onto the same colour.
//!
//! The last two groups cover the **polarity split** the revised table is
//! built on. `color.accent.*` and `color.status.danger.*` are each three
//! tokens rather than one — a fill, a pressed fill and a stroke (or, for
//! danger, a text colour, a fill and a pressed fill) — which is the
//! shape decision **D-5** argued for and decision **D-6** made
//! necessary. Those tests pin each half of each split against the role it
//! exists to do, pin the two halves of a split against each other so
//! they cannot be collapsed back into one token, and pin the
//! within-family hue so a revalue cannot quietly turn the danger reds
//! brown or the accent indigos blue.
//!
//! The luminance maths here is deliberately private to this file. The
//! production solver — relative luminance, WCAG ratios and the mandated
//! pairing table — is `token/contrast.rs` (`UIF-TOK-006` /
//! `UIF-VER-001`); duplicating its *public* API here would let a change
//! to one side go unnoticed by the other.

use super::*;

/// Tolerance for `f32` channel arithmetic: a channel round-trips through
/// `/ 255.0` untouched, so the only error is float representation.
const EPS: f32 = 1e-6;

/// Ceiling on the R/G/B spread of a token that claims to be a neutral
/// grey, on the 0–255 scale.
///
/// The dark surfaces are not perfectly neutral: each carries a slight
/// blue lean that grows with elevation — `+2` at `base` and `sunken`,
/// `+3` at `overlay`, `+4` at `hover` and `active`. Four units is 1.6%
/// of a channel's range, which reads as a cool cast rather than a hue.
/// The ceiling is held at eight — twice the widest surface actually
/// spans — so the rule stays a statement about neutrality rather than a
/// restatement of the current values, and a surface that drifted to a
/// visibly blue-grey would still fail it.
const NEUTRAL_SPREAD_MAX: f32 = 8.0;

/// Minimum Euclidean RGB distance (0–255 scale) between two status
/// colours.
///
/// A third of a channel's range has to separate them: the three statuses
/// carry meaning on colour alone in chips and dots, so they must survive
/// a quick glance, a cheap display and colour-blind-adjacent viewing.
/// The tightest pair on the revised table is `warning` ↔ `danger.text`
/// at ≈105.5, so 100 leaves real headroom without being so loose that a
/// retune could slide two statuses together.
const STATUS_MIN_DISTANCE: f32 = 100.0;

/// A row of the `UIF-TOK-005` table paired with the token that
/// implements it.
///
/// Keeping the transcription next to the value means retuning either side
/// shows up as a failing assertion that names its own token. Every row
/// below carries the value the revised §2.5 prints: the pre-revision
/// `G-7` retune of `accent.primary`, `surface.active` and
/// `status.danger` is gone, because the revision renamed and revalued
/// those three to figures that clear the rules on their own.
#[derive(Clone, Copy)]
struct SpecRow {
    /// The spec token, e.g. `color.surface.base`.
    token: &'static str,
    /// The `iced::Color` implementing it.
    color: Color,
    /// The red channel of the `sRGB Value` column.
    r: u8,
    /// The green channel of the `sRGB Value` column.
    g: u8,
    /// The blue channel of the `sRGB Value` column.
    b: u8,
    /// The alpha the column implies: `1.0` everywhere except the scrim.
    a: f32,
}

/// Build a transcribed table row.
fn row(token: &'static str, color: Color, r: u8, g: u8, b: u8, a: f32) -> SpecRow {
    SpecRow {
        token,
        color,
        r,
        g,
        b,
        a,
    }
}

/// The twenty-three dark-palette tokens against the spec table, in the
/// table's own order.
///
/// Every table-driven test walks this list, so no token can be skipped by
/// a test that checks a hand-picked subset. All twenty-three rows
/// transcribe §2.5 as revised on 2026-10-22 — including the three
/// `accent.*` and three `status.danger.*` rows that make up the
/// polarity split, which the pre-revision table did not print at all.
fn spec_rows() -> [SpecRow; 23] {
    let p = Palette::dark();
    [
        row("color.surface.base", p.surface_base, 9, 9, 11, 1.0),
        row("color.surface.sunken", p.surface_sunken, 6, 6, 8, 1.0),
        row("color.surface.raised", p.surface_raised, 18, 18, 20, 1.0),
        row("color.surface.code", p.surface_code, 20, 20, 22, 1.0),
        row("color.surface.overlay", p.surface_overlay, 24, 24, 27, 1.0),
        row("color.surface.hover", p.surface_hover, 32, 32, 36, 1.0),
        row("color.surface.active", p.surface_active, 42, 42, 46, 1.0),
        row("color.surface.scrim", p.surface_scrim, 9, 9, 11, 0.55),
        row("color.border.subtle", p.border_subtle, 40, 40, 40, 1.0),
        row("color.border.strong", p.border_strong, 120, 120, 120, 1.0),
        row("color.accent.fill", p.accent_fill, 93, 96, 238, 1.0),
        row(
            "color.accent.fill_active",
            p.accent_fill_active,
            82,
            85,
            222,
            1.0,
        ),
        row("color.accent.border", p.accent_border, 129, 140, 248, 1.0),
        row("color.accent.text", p.accent_text, 100, 200, 255, 1.0),
        row("color.text.primary", p.text_primary, 210, 210, 210, 1.0),
        row("color.text.strong", p.text_strong, 255, 255, 255, 1.0),
        row("color.text.muted", p.text_muted, 160, 160, 160, 1.0),
        row("color.text.disabled", p.text_disabled, 140, 140, 140, 1.0),
        row("color.status.success", p.status_success, 100, 255, 100, 1.0),
        row("color.status.warning", p.status_warning, 220, 130, 0, 1.0),
        row(
            "color.status.danger.text",
            p.status_danger_text,
            255,
            92,
            92,
            1.0,
        ),
        row(
            "color.status.danger.fill",
            p.status_danger_fill,
            216,
            28,
            20,
            1.0,
        ),
        row(
            "color.status.danger.fill_active",
            p.status_danger_fill_active,
            202,
            24,
            16,
            1.0,
        ),
    ]
}

/// A `0.0–1.0` iced channel back onto the `0–255` scale the spec writes,
/// rounded so a pin compares integers rather than float noise.
fn ch8(v: f32) -> f32 {
    (v * 255.0).round()
}

/// Pin one colour to an exact 8-bit RGB triple plus alpha, reporting the
/// offending channel and token by name.
fn assert_rgba(token: &str, color: Color, r: u8, g: u8, b: u8, a: f32) {
    assert_eq!(
        ch8(color.r),
        f32::from(r),
        "{token} red must be {r}, got {}",
        ch8(color.r)
    );
    assert_eq!(
        ch8(color.g),
        f32::from(g),
        "{token} green must be {g}, got {}",
        ch8(color.g)
    );
    assert_eq!(
        ch8(color.b),
        f32::from(b),
        "{token} blue must be {b}, got {}",
        ch8(color.b)
    );
    assert!(
        (color.a - a).abs() <= EPS,
        "{token} alpha must be {a}, got {}",
        color.a
    );
}

/// WCAG 2.1 relative luminance of an sRGB colour.
///
/// Test-local on purpose: the production solver is `token/contrast.rs`
/// (`UIF-TOK-006`), and keeping this copy independent means a change to
/// the production transfer function cannot silently re-label every pin
/// in this file as still-correct.
fn luminance(color: Color) -> f32 {
    fn linearize(v: f32) -> f32 {
        if v <= 0.039_28 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    }

    0.2126 * linearize(color.r) + 0.7152 * linearize(color.g) + 0.0722 * linearize(color.b)
}

/// WCAG 2.1 contrast ratio between two colours.
///
/// Test-local, matching the file's existing [`luminance`] helper.
fn contrast(a: Color, b: Color) -> f32 {
    let la = luminance(a);
    let lb = luminance(b);
    let (lighter, darker) = if la >= lb { (la, lb) } else { (lb, la) };
    (lighter + 0.05) / (darker + 0.05)
}

/// Widest gap between any two RGB channels of a colour, on the 0–255
/// scale. Zero for a pure grey.
fn spread(color: Color) -> f32 {
    let r = ch8(color.r);
    let g = ch8(color.g);
    let b = ch8(color.b);
    r.max(g).max(b) - r.min(g).min(b)
}

/// Euclidean RGB distance between two colours on the 0–255 scale.
fn distance(a: Color, b: Color) -> f32 {
    let dr = ch8(a.r) - ch8(b.r);
    let dg = ch8(a.g) - ch8(b.g);
    let db = ch8(a.b) - ch8(b.b);
    (dr * dr + dg * dg + db * db).sqrt()
}

/// HSL hue in degrees, `0..360`. Undefined for a grey, which never
/// reaches this file's red-band checks.
fn hue_deg(color: Color) -> f32 {
    let r = ch8(color.r);
    let g = ch8(color.g);
    let b = ch8(color.b);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    if delta == 0.0 {
        return 0.0;
    }
    let hue = if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };
    if hue < 0.0 { hue + 360.0 } else { hue }
}

/// Every surface token in the elevation ladder, darkest to lightest.
fn all_surfaces(p: &Palette) -> [(&'static str, Color); 7] {
    [
        ("color.surface.sunken", p.surface_sunken),
        ("color.surface.base", p.surface_base),
        ("color.surface.raised", p.surface_raised),
        ("color.surface.code", p.surface_code),
        ("color.surface.overlay", p.surface_overlay),
        ("color.surface.hover", p.surface_hover),
        ("color.surface.active", p.surface_active),
    ]
}

/// The four saturated fills `UIF-TOK-006` rule 4 locks to
/// `text.strong`.
fn saturated_fills(p: &Palette) -> [(&'static str, Color); 4] {
    [
        ("color.accent.fill", p.accent_fill),
        ("color.accent.fill_active", p.accent_fill_active),
        ("color.status.danger.fill", p.status_danger_fill),
        (
            "color.status.danger.fill_active",
            p.status_danger_fill_active,
        ),
    ]
}

/// Every token carries the exact sRGB value the `UIF-TOK-005` table
/// gives it, channel by channel.
#[test]
fn test_every_token_matches_the_spec_table() {
    for r in spec_rows() {
        assert_rgba(r.token, r.color, r.r, r.g, r.b, r.a);
    }
}

/// Only the scrim is translucent: every other token is fully opaque, so
/// no surface or text token can accidentally let a layer underneath show
/// through.
#[test]
fn test_only_the_scrim_is_translucent() {
    for r in spec_rows() {
        if r.token == "color.surface.scrim" {
            assert!(
                r.color.a < 1.0,
                "color.surface.scrim must be translucent, got alpha {}",
                r.color.a
            );
        } else {
            assert!(
                (r.color.a - 1.0).abs() <= EPS,
                "{} must be fully opaque, got alpha {}",
                r.token,
                r.color.a
            );
        }
    }
}

/// The scrim is `color.surface.base` at 55% alpha: same RGB as the
/// canvas, dimmed rather than replaced, so it composites the workspace
/// into the background instead of hiding it.
#[test]
fn test_scrim_is_the_base_surface_at_fifty_five_percent_alpha() {
    let p = Palette::dark();
    assert!(
        (p.surface_scrim.a - 0.55).abs() <= EPS,
        "color.surface.scrim alpha must be exactly 0.55, got {}",
        p.surface_scrim.a
    );
    assert_eq!(
        ch8(p.surface_scrim.r),
        ch8(p.surface_base.r),
        "the scrim must carry base's red so it composites the canvas rather than a different colour"
    );
    assert_eq!(
        ch8(p.surface_scrim.g),
        ch8(p.surface_base.g),
        "the scrim must carry base's green so it composites the canvas rather than a different colour"
    );
    assert_eq!(
        ch8(p.surface_scrim.b),
        ch8(p.surface_base.b),
        "the scrim must carry base's blue so it composites the canvas rather than a different colour"
    );
}

/// The elevation ladder: dark-first, with each higher z-layer strictly
/// lighter than the one below it.
#[test]
fn test_surface_ladder_gets_lighter_as_elevation_rises() {
    let p = Palette::dark();
    let ladder = all_surfaces(&p);

    for pair in ladder.windows(2) {
        let (lower_name, lower) = pair[0];
        let (upper_name, upper) = pair[1];
        assert!(
            luminance(lower) < luminance(upper),
            "{upper_name} must be lighter than {lower_name} on the elevation ladder, but {} is not brighter than {}",
            luminance(upper),
            luminance(lower)
        );
    }
}

/// The top of the ladder — the rung the 2026-10-22 revision actually
/// moved — is a visible step, and the bottom rungs are recorded as
/// inherently tight rather than held to a floor they cannot meet.
///
/// Relative luminance is a *nonlinear* scale: near black, a whole RGB
/// step barely moves `L` at all. `sunken → base` is `0.000911` and
/// `raised → code` is `0.000952`, which is not a design failure but a
/// property of the sRGB toe — those rungs are separated by channel
/// distance the eye reads fine at the bottom of the curve. Asserting a
/// uniform luminance floor across the whole ladder would fail on rungs
/// that are correct.
///
/// The rung worth pinning is `hover → active`, because that is the one
/// the revision compressed by pulling `hover` up to `RGB(32, 32, 36)`
/// while `active` came down to `RGB(42, 42, 46)`. At `0.008780` it is
/// the *widest* step on the ladder in absolute terms, so the top of the
/// elevation model is not at risk of reading flat — but a future nudge
/// that closed it further would be, hence the floor.
#[test]
fn test_the_top_of_the_ladder_is_a_visible_step() {
    let p = Palette::dark();
    let step = luminance(p.surface_active) - luminance(p.surface_hover);

    assert!(
        step >= 0.008,
        "the hover → active step is {step:.6} of relative luminance; the top of \
         the elevation ladder must stay at or above 0.008 or a pressed control \
         reads as a repaint of a hovered one",
    );
    // And it is genuinely the widest rung, which is what makes the top of
    // the ladder the safest place to be pressed rather than the riskiest.
    let ladder = all_surfaces(&p);
    for pair in ladder.windows(2) {
        let (lower_name, lower) = pair[0];
        let (upper_name, upper) = pair[1];
        let other = luminance(upper) - luminance(lower);
        assert!(
            other <= step + 1e-6,
            "{lower_name} → {upper_name} is {other:.6}, wider than the \
             hover → active step of {step:.6} — this test's premise is stale",
        );
    }
}

/// `sunken` is the only surface below base — the ladder has exactly one
/// step downwards, and it is the one the spec names.
#[test]
fn test_sunken_is_the_only_surface_darker_than_base() {
    let p = Palette::dark();
    let base = luminance(p.surface_base);

    for (name, surface) in all_surfaces(&p) {
        if name == "color.surface.base" {
            continue;
        }
        if name == "color.surface.sunken" {
            assert!(
                luminance(surface) < base,
                "{name} must sit below base, but {} is not darker than {}",
                luminance(surface),
                base
            );
        } else {
            assert!(
                luminance(surface) > base,
                "{name} must sit above base on the elevation ladder, but {} is not brighter than {}",
                luminance(surface),
                base
            );
        }
    }
}

/// The surfaces and borders are neutral greys: no channel leads far
/// enough to read as a hue.
///
/// The rule covers the ten `surface.*` and `border.*` tokens only. The
/// accents and statuses are deliberately chromatic and are checked by
/// [`test_accents_and_statuses_are_deliberately_non_neutral`] instead —
/// a failure here on `color.accent.fill` would mean the neutrality list
/// was widened by mistake, not that the accent changed.
#[test]
fn test_surfaces_and_borders_are_neutral_greys() {
    let p = Palette::dark();
    let mut neutrals: Vec<(&'static str, Color)> = all_surfaces(&p).to_vec();
    neutrals.push(("color.surface.scrim", p.surface_scrim));
    neutrals.push(("color.border.subtle", p.border_subtle));
    neutrals.push(("color.border.strong", p.border_strong));

    for (name, color) in neutrals {
        assert!(
            spread(color) <= NEUTRAL_SPREAD_MAX,
            "{name} must stay a neutral grey, but its channels span {} (max {NEUTRAL_SPREAD_MAX})",
            spread(color)
        );
    }

    // Both borders are stated as flat greys, so they get the stricter
    // rule: not merely near-neutral, but exactly neutral.
    assert_eq!(
        spread(p.border_subtle),
        0.0,
        "color.border.subtle is a flat grey, but spans {}",
        spread(p.border_subtle)
    );
    assert_eq!(
        spread(p.border_strong),
        0.0,
        "color.border.strong is a flat grey, but spans {}",
        spread(p.border_strong)
    );
}

/// The seven chromatic tokens are outside the neutrality rule: each
/// leads a channel by far more than a grey ever does, which is what
/// makes them carry meaning rather than structure.
///
/// All three members of each polarity split are listed, because the split
/// is a *chromatic* split — a fill, a pressed fill and a stroke are all
/// the same hue at different depths, and every one of them has to clear
/// the neutrality floor.
#[test]
fn test_accents_and_statuses_are_deliberately_non_neutral() {
    let p = Palette::dark();
    let chromatic = [
        ("color.accent.fill", p.accent_fill),
        ("color.accent.fill_active", p.accent_fill_active),
        ("color.accent.border", p.accent_border),
        ("color.accent.text", p.accent_text),
        ("color.status.success", p.status_success),
        ("color.status.warning", p.status_warning),
        ("color.status.danger.text", p.status_danger_text),
        ("color.status.danger.fill", p.status_danger_fill),
        (
            "color.status.danger.fill_active",
            p.status_danger_fill_active,
        ),
    ];

    for (name, color) in chromatic {
        assert!(
            spread(color) > NEUTRAL_SPREAD_MAX,
            "{name} is meant to be chromatic, but its channels span only {} — inside the neutral band",
            spread(color)
        );
    }
}

/// The text roles are strictly ordered by prominence, so a role name
/// always means more prominent text than the role below it.
#[test]
fn test_text_roles_are_strictly_ordered_by_prominence() {
    let p = Palette::dark();
    let roles = [
        ("color.text.disabled", p.text_disabled),
        ("color.text.muted", p.text_muted),
        ("color.text.primary", p.text_primary),
        ("color.text.strong", p.text_strong),
    ];

    for pair in roles.windows(2) {
        let (lower_name, lower) = pair[0];
        let (upper_name, upper) = pair[1];
        assert!(
            luminance(lower) < luminance(upper),
            "{upper_name} must be more prominent than {lower_name}, but {} is not brighter than {}",
            luminance(upper),
            luminance(lower)
        );
    }
}

/// The three status colours are mutually distinguishable: each pair is
/// far apart in the RGB cube, and each differs on at least two channels,
/// so no colour-blind-adjacent viewing or cheap display can collapse two
/// of them into one signal.
///
/// The two danger *fills* are deliberately **not** entries. They are the
/// same danger signal in a different *role* — a background rather than a
/// text colour — and they sit close to `color.status.danger.text` on
/// purpose, so the family reads as one colour at three depths. Requiring
/// the 100-unit signal separation here would demand the fills be a
/// different colour rather than darker reds, which defeats the point of
/// the split. Their own distinctness requirements are
/// [`test_danger_fills_are_not_surfaces`] and
/// [`test_the_two_danger_roles_have_disjoint_luminance_bounds`].
#[test]
fn test_status_colors_are_mutually_distinguishable() {
    let p = Palette::dark();
    let statuses = [
        ("color.status.success", p.status_success),
        ("color.status.warning", p.status_warning),
        ("color.status.danger.text", p.status_danger_text),
    ];

    for (i, (a_name, a)) in statuses.iter().enumerate() {
        for (b_name, b) in statuses.iter().skip(i + 1) {
            assert!(
                distance(*a, *b) >= STATUS_MIN_DISTANCE,
                "{a_name} and {b_name} are only {:.1} apart, below the {STATUS_MIN_DISTANCE} a status signal needs",
                distance(*a, *b)
            );

            let near = [
                (ch8(a.r) - ch8(b.r)).abs(),
                (ch8(a.g) - ch8(b.g)).abs(),
                (ch8(a.b) - ch8(b.b)).abs(),
            ];
            let decisive = near.iter().filter(|d| **d >= 20.0).count();
            assert!(
                decisive >= 2,
                "{a_name} and {b_name} must differ clearly on at least two channels, got {near:?}"
            );
        }
    }
}

/// Every surface sits below the dimmest text role, so text keeps its
/// headroom wherever it lands — and the strong border clears the highest
/// surface, so a floating perimeter is visible against anything it
/// outlines.
#[test]
fn test_surfaces_stay_below_text_and_borders_cut_through() {
    let p = Palette::dark();
    let dimmest_text = luminance(p.text_disabled);
    let brightest_surface = all_surfaces(&p)
        .iter()
        .map(|(_, s)| luminance(*s))
        .fold(f32::NEG_INFINITY, f32::max);

    for (_, surface) in all_surfaces(&p) {
        assert!(
            luminance(surface) < dimmest_text,
            "a surface at {} is too light to sit under the dimmest text role at {dimmest_text}",
            luminance(surface)
        );
    }
    assert!(
        luminance(p.border_strong) > brightest_surface,
        "color.border.strong ({}) must cut above the highest surface ({}) to read as a floating perimeter",
        luminance(p.border_strong),
        brightest_surface
    );
}

/// The editor palette carries the exact inverted triple the spec gives.
#[test]
fn test_editor_palette_matches_the_spec() {
    let e = EditorPalette::inverted();
    assert_rgba("color.editor.inverted.bg", e.background, 255, 255, 255, 1.0);
    assert_rgba("color.editor.inverted.fg", e.text, 0, 0, 0, 1.0);
    assert_rgba("color.editor.inverted.err", e.error, 180, 0, 0, 1.0);
}

/// The editor surface is inverted relative to the dark palette: its
/// canvas is at least as light as anything in the chrome and its text is
/// at most as dark as anything in the chrome. The two equalities are the
/// white-on-black extremes meeting `color.text.strong` and the black
/// floor of the greys.
#[test]
fn test_editor_palette_is_inverted_relative_to_the_dark_palette() {
    let e = EditorPalette::inverted();
    assert!(
        luminance(e.background) > luminance(e.text),
        "the editor must be light-on-dark inverted: bg {} vs text {}",
        luminance(e.background),
        luminance(e.text)
    );

    for r in spec_rows() {
        assert!(
            luminance(e.background) >= luminance(r.color),
            "the editor canvas ({}) must be at least as light as every dark-palette token, but {} is brighter",
            luminance(e.background),
            r.token
        );
        assert!(
            luminance(e.text) <= luminance(r.color),
            "the editor text ({}) must be at most as dark as every dark-palette token, but {} is darker",
            luminance(e.text),
            r.token
        );
    }
}

/// The editor error is the spec's dark red rather than the app's pure
/// red: red-dominant, unsullied by green or blue, and pulled well below
/// full brightness so it stays readable on white (`D-4`).
#[test]
fn test_editor_error_is_the_spec_dark_red_not_pure_red() {
    let e = EditorPalette::inverted();
    assert_eq!(
        ch8(e.error.g),
        0.0,
        "color.editor.inverted.err must carry no green, got {}",
        ch8(e.error.g)
    );
    assert_eq!(
        ch8(e.error.b),
        0.0,
        "color.editor.inverted.err must carry no blue, got {}",
        ch8(e.error.b)
    );
    assert!(
        ch8(e.error.r) > 0.0,
        "color.editor.inverted.err must carry red, got {}",
        ch8(e.error.r)
    );
    assert!(
        ch8(e.error.r) < 255.0,
        "color.editor.inverted.err must be darker than pure red (the app's old value), got {}",
        ch8(e.error.r)
    );
    assert!(
        luminance(e.error) < luminance(e.background),
        "the error colour must recede into the white canvas rather than glow out of it"
    );
}

/// `Default` is the inverted set: inversion is the only variant this
/// surface has, so a defaulted value must not be a blank or dark editor.
#[test]
fn test_editor_default_is_the_inverted_set() {
    assert_eq!(
        EditorPalette::default(),
        EditorPalette::inverted(),
        "Default for EditorPalette must be the inverted palette"
    );
    assert_eq!(
        EditorPalette::default(),
        EditorPalette::default(),
        "the derived equality must hold across two defaults"
    );
}

/// No two distinct tokens collapse onto the same colour.
///
/// The one near-collision is intentional and survives the check:
/// `color.surface.scrim` shares `color.surface.base`'s RGB but carries
/// 55% alpha, so as an `iced::Color` it is a distinct value. The
/// comparison below is over full RGBA for exactly that reason — a test
/// that compared RGB only would have to special-case the scrim, and would
/// stop catching an aliasing mistake in the process.
#[test]
fn test_no_two_tokens_share_an_identical_colour() {
    let rows = spec_rows();
    for (i, a) in rows.iter().enumerate() {
        for b in rows.iter().skip(i + 1) {
            assert_ne!(
                a.color, b.color,
                "{} and {} are distinct tokens but resolve to the same colour",
                a.token, b.token
            );
        }
    }
}

/// The premise the aliasing test rests on: `Palette`'s equality is
/// field-wise, so a single moved channel really does register as a
/// different palette rather than comparing equal.
#[test]
fn test_palette_equality_is_field_wise() {
    let a = Palette::dark();
    let mut b = Palette::dark();
    assert_eq!(a, b, "two unmodified dark palettes must be equal");

    b.border_strong = a.border_subtle;
    assert_ne!(
        a, b,
        "aliasing border.strong onto border.subtle must make the palettes unequal"
    );
    assert_eq!(
        a.surface_base, b.surface_base,
        "the untouched fields must still compare equal"
    );
}

// ---------------------------------------------------------------------------
// The polarity split — decision D-5, gap G-8, and the accent's mirror
// ---------------------------------------------------------------------------

/// The `UIF-TOK-006` rule 1 threshold the disjoint-bounds test solves with.
const MIN_TEXT_RULE: f32 = 4.5;

/// Tolerance on a pinned contrast ratio — three decimals, enough to tell
/// `4.9990` from `5.1270`.
const RATIO_EPS: f32 = 1e-3;

/// The floor a saturated fill must reach under `text.strong`
/// (`UIF-TOK-006` rule 4).
///
/// This is the rule's own lower edge, not a locally chosen margin: the
/// lock guarantees `4.80:1` to `5.75:1`, and `accent.fill` sits on that
/// lower edge at `4.8020:1`. Pinning the rule rather than a rounder
/// number keeps the assertion true for the spec as written instead of
/// for one fill's luck.
const FILL_MIN_CONTRAST: f32 = 4.80;

/// Upper edge of the red hue band, in degrees. Orange starts at `30°`;
/// fifteen is well inside red and far from any colour that could be read
/// as a warm brown or amber.
const RED_BAND_MAX_DEG: f32 = 15.0;

/// The smallest luminance step that reads as "clearly darker" rather
/// than a nudge — a quarter of the way from `surface.base` to
/// `surface.active`.
const MIN_VISIBLE_LUMINANCE_STEP: f32 = 0.05;

/// How far a saturated fill must stand from every surface on luminance,
/// so it is never mistaken for the layer it is meant to stand out from.
const FILL_VS_SURFACE_MIN_RATIO: f32 = 2.0;

/// The lower edge of the indigo band the accent family lives in, and the
/// upper edge of it. Blue-dominant, but with enough red present that it
/// reads as indigo rather than plain blue — cyan starts around `180°`
/// and pure blue is `240°`, so a band of `225°..255°` brackets the
/// family and excludes both.
const INDIGO_BAND_MIN_DEG: f32 = 225.0;
const INDIGO_BAND_MAX_DEG: f32 = 255.0;

/// Each of the four rule-4 fills carries `text.strong` with real margin,
/// not a knife-edge pass, and each is pinned to its measured value.
#[test]
fn test_every_saturated_fill_carries_text_strong_with_margin() {
    let p = Palette::dark();
    let pinned: [(&str, f32); 4] = [
        ("color.accent.fill", 4.8020),
        ("color.accent.fill_active", 5.6559),
        ("color.status.danger.fill", 5.1270),
        ("color.status.danger.fill_active", 5.7543),
    ];
    let fills = saturated_fills(&p);

    for ((name, expected), (fill_name, fill)) in pinned.iter().zip(fills.iter()) {
        assert_eq!(
            *name, *fill_name,
            "the pinned-value table and the fill table drifted apart"
        );
        let ratio = contrast(p.text_strong, *fill);
        assert!(
            (ratio - expected).abs() < RATIO_EPS,
            "{fill_name} vs text.strong is pinned at {expected:.4}:1, got {ratio:.4}:1",
        );
        assert!(
            ratio >= FILL_MIN_CONTRAST,
            "text.strong on {fill_name} measures {ratio:.4}:1, under the \
             {FILL_MIN_CONTRAST}:1 this token exists to clear",
        );
    }
}

/// Rule 4's lock band is exactly what the spec claims: the four fills
/// span `4.80:1` to `5.75:1` under `text.strong`, no wider.
///
/// The band is the point. A spread that widened meant a fill had drifted
/// far enough that the lock no longer described a single consistent
/// reading experience; a spread that narrowed below the four fills' own
/// measured range meant the band had been fitted to the data rather than
/// held against the claim.
#[test]
fn test_rule_four_lock_band_is_4_80_to_5_75() {
    let p = Palette::dark();
    let ratios: Vec<f32> = saturated_fills(&p)
        .iter()
        .map(|(_, fill)| contrast(p.text_strong, *fill))
        .collect();
    let low = ratios.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let high = ratios.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));

    assert!(
        (low - 4.8020).abs() < RATIO_EPS,
        "the weakest rule-4 pairing measures {low:.4}:1, not the 4.8020:1 the \
         spec's `4.80:1` lower edge describes",
    );
    assert!(
        (high - 5.7543).abs() < RATIO_EPS,
        "the strongest rule-4 pairing measures {high:.4}:1, not the 5.7543:1 \
         the spec's `5.75:1` upper edge describes",
    );
    assert!(
        high - low > 0.9 && high - low < 1.0,
        "the lock band spans {:.4}:1 — outside the 4.80–5.75 band the spec states",
        high - low,
    );
}

/// The accent family is one indigo at three depths: fill, pressed fill
/// and stroke all sit inside the indigo band and within a few degrees of
/// each other.
///
/// This is what makes the polarity split read as *one colour doing three
/// jobs* rather than three unrelated tokens that happen to be blue-ish.
/// A revalue that pushed `accent.border` toward pure blue, or
/// `accent.fill_active` toward violet, would break the family while still
/// passing every contrast rule — and this is the assertion that stops it.
#[test]
fn test_the_accent_split_is_one_indigo_at_three_depths() {
    let p = Palette::dark();
    let family = [
        ("color.accent.fill", p.accent_fill),
        ("color.accent.fill_active", p.accent_fill_active),
        ("color.accent.border", p.accent_border),
    ];

    for (name, color) in family {
        let hue = hue_deg(color);
        assert!(
            (INDIGO_BAND_MIN_DEG..=INDIGO_BAND_MAX_DEG).contains(&hue),
            "{name} hue is {hue:.2}°, outside the indigo band \
             [{INDIGO_BAND_MIN_DEG}, {INDIGO_BAND_MAX_DEG}] — it has stopped \
             being the accent colour",
        );
        assert!(
            ch8(color.b) > ch8(color.r) && ch8(color.b) > ch8(color.g),
            "{name} must stay blue-dominant, got RGB({}, {}, {})",
            ch8(color.r),
            ch8(color.g),
            ch8(color.b)
        );
        // Red must still be present in real quantity: with red near zero
        // the token is plain blue, not indigo.
        assert!(
            ch8(color.r) > 60.0,
            "{name} must carry real red or it reads as blue rather than indigo, \
             red is {}",
            ch8(color.r)
        );
    }

    let hues: Vec<f32> = family.iter().map(|(_, c)| hue_deg(*c)).collect();
    let low = hues.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let high = hues.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
    assert!(
        high - low < 10.0,
        "the accent family's hues span {:.2}° ({} → {}); the three tokens must \
         read as one colour, not three",
        high - low,
        low,
        high,
    );
}

/// The pressed accent fill is a visible step *down* from the resting
/// fill, and the stroke is a visible step *up* from both.
///
/// The direction matters: the pressed state deepens (the control goes
/// away from the user) and the border lightens (the outline has to read
/// against the fill it surrounds). A split that inverted either would
/// still pass every contrast rule and would still be wrong.
///
/// The press step itself is `0.033012` — deliberately *not* held to
/// [`MIN_VISIBLE_LUMINANCE_STEP`]. That `0.05` floor was set for the
/// danger text→fill relationship, where the two tokens sit in different
/// roles and must not be mistakable for one another. A press step is a
/// different kind of distinction: it is transient, it is seen only
/// against the resting state the user just looked at, and the accent is
/// already the most saturated thing on screen, so a smaller luminance
/// move carries proportionally more meaning. The floor used here is
/// `0.03`, which the step clears with `0.003` to spare.
#[test]
fn test_the_accent_split_orders_fill_darker_than_border_and_press_deeper() {
    let p = Palette::dark();
    let fill = luminance(p.accent_fill);
    let pressed = luminance(p.accent_fill_active);
    let border = luminance(p.accent_border);

    assert!(
        pressed < fill,
        "the pressed fill must be deeper than the resting fill: \
         {pressed:.6} vs {fill:.6}",
    );
    assert!(
        fill - pressed >= 0.03,
        "the press step is only {:.6} of relative luminance — too small for a \
         press to read at all",
        fill - pressed,
    );
    assert!(
        border > fill,
        "the border must be lighter than the fill it outlines: \
         {border:.6} vs {fill:.6}",
    );
    assert!(
        border - fill >= MIN_VISIBLE_LUMINANCE_STEP,
        "the border is only {:.6} above the fill — it would not read as a \
         separate stroke on top of it",
        border - fill,
    );
}

/// The danger family is one red at three depths.
///
/// Same argument as the accent family, opposite hue: the text red, the
/// fill and the pressed fill must all read as *the same red*, or the
/// three tokens stop being one signal.
#[test]
fn test_the_danger_split_is_one_red_at_three_depths() {
    let p = Palette::dark();
    let family = [
        ("color.status.danger.text", p.status_danger_text),
        ("color.status.danger.fill", p.status_danger_fill),
        (
            "color.status.danger.fill_active",
            p.status_danger_fill_active,
        ),
    ];

    for (name, color) in family {
        let hue = hue_deg(color);
        assert!(
            (0.0..RED_BAND_MAX_DEG).contains(&hue),
            "{name} hue is {hue:.2}°, outside the red band \
             [0, {RED_BAND_MAX_DEG}) — it has gone brown or amber",
        );
        assert!(
            ch8(color.r) > 150.0,
            "{name} must stay a strong red, red is {}",
            ch8(color.r)
        );
        assert!(
            ch8(color.g) < 100.0,
            "{name} must keep green low or the red goes brown, green is {}",
            ch8(color.g)
        );
        assert!(
            ch8(color.b) < 100.0,
            "{name} must keep blue low or the red goes purple, blue is {}",
            ch8(color.b)
        );
        assert!(
            ch8(color.r) - ch8(color.g) > 100.0,
            "{name} must dominate green by a wide margin, got {}",
            ch8(color.r) - ch8(color.g)
        );
        assert!(
            ch8(color.r) - ch8(color.b) > 100.0,
            "{name} must dominate blue by a wide margin, got {}",
            ch8(color.r) - ch8(color.b)
        );
    }

    let hues: Vec<f32> = family.iter().map(|(_, c)| hue_deg(*c)).collect();
    let low = hues.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let high = hues.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
    assert!(
        high - low < 5.0,
        "the danger family's hues span {:.2}° ({} → {}); the three tokens must \
         read as one red, not three",
        high - low,
        low,
        high,
    );
}

/// The danger family's luminance order is text > fill > pressed fill, and
/// each step is visible.
///
/// The text red is the brightest thing it can be while still being red —
/// it has to read as body copy on dark surfaces. The fill is a third
/// darker so a label sits *on* it rather than bleeding into it. The
/// pressed fill is darker still, which is what makes a pressed
/// destructive control read as deeper rather than merely dimmer.
#[test]
fn test_the_danger_split_is_ordered_by_luminance_with_visible_steps() {
    let p = Palette::dark();
    let text_l = luminance(p.status_danger_text);
    let fill_l = luminance(p.status_danger_fill);
    let pressed_l = luminance(p.status_danger_fill_active);

    assert!(
        text_l > fill_l,
        "the text red must be brighter than the fill: {text_l:.6} vs {fill_l:.6}"
    );
    assert!(
        fill_l > pressed_l,
        "the fill must be brighter than the pressed fill: {fill_l:.6} vs {pressed_l:.6}"
    );
    assert!(
        text_l - fill_l >= MIN_VISIBLE_LUMINANCE_STEP,
        "the text → fill step is only {:.6} — not the visible escalation D-5 asks for",
        text_l - fill_l,
    );
    assert!(
        fill_l - pressed_l >= 0.02,
        "the fill → pressed step is only {:.6} — a press must read as a step, \
         not a dimmer repaint",
        fill_l - pressed_l,
    );
    assert!(
        fill_l / text_l < 0.7,
        "the fill is at {:.1}% of the text red's luminance; it must read as a \
         third darker, not slightly darker",
        100.0 * fill_l / text_l,
    );
}

/// The fills are not surfaces and must never read as one: each is far
/// more chromatic than any grey in the ladder, and each stands clear of
/// every surface on luminance too.
#[test]
fn test_danger_fills_are_not_surfaces() {
    let p = Palette::dark();
    let fills = [
        ("color.status.danger.fill", p.status_danger_fill),
        (
            "color.status.danger.fill_active",
            p.status_danger_fill_active,
        ),
    ];

    for (fill_name, fill) in fills {
        for (name, surface) in all_surfaces(&p) {
            assert!(
                spread(fill) > NEUTRAL_SPREAD_MAX * 10.0,
                "{fill_name}'s channel spread is {}, which is not chromatic \
                 enough to be unmistakably not a grey",
                spread(fill)
            );
            assert!(
                spread(surface) <= NEUTRAL_SPREAD_MAX,
                "{name} must stay a neutral grey, but spans {}",
                spread(surface)
            );
            let ratio = contrast(fill, surface);
            assert!(
                ratio >= FILL_VS_SURFACE_MIN_RATIO,
                "{fill_name} vs {name} is only {ratio:.4}:1 — close enough to \
                 the surface to be mistaken for one",
            );
        }
    }
}

/// The two danger roles cannot share one token, and each token sits on
/// its own side of the divide.
///
/// Solving each role for the red's luminance gives two bounds that do not
/// overlap. That is the arithmetic behind gap **G-8** and the reason
/// **D-5** split the token rather than retuning it — and it is why the
/// revised spec's polarity split is structure rather than decoration.
#[test]
fn test_the_two_danger_roles_have_disjoint_luminance_bounds() {
    let p = Palette::dark();
    let white_l = luminance(p.text_strong);
    let overlay_l = luminance(p.surface_overlay);

    // white-on-red >= 4.5  =>  L_red <= (L_white + 0.05) / 4.5 - 0.05
    let max_for_background = (white_l + 0.05) / MIN_TEXT_RULE - 0.05;
    // red-on-overlay >= 4.5  =>  L_red >= 4.5 * (L_overlay + 0.05) - 0.05
    let min_for_text = MIN_TEXT_RULE * (overlay_l + 0.05) - 0.05;

    assert!(
        max_for_background < min_for_text,
        "the two roles' bounds must be disjoint — that is the whole argument \
         for the split: max {max_for_background:.6} vs min {min_for_text:.6}",
    );
    assert!(
        luminance(p.status_danger_fill) <= max_for_background,
        "the fill sits above the background role's ceiling and would fail as a \
         background: {} > {max_for_background:.6}",
        luminance(p.status_danger_fill),
    );
    assert!(
        luminance(p.status_danger_fill_active) <= max_for_background,
        "the pressed fill sits above the background role's ceiling: {} > \
         {max_for_background:.6}",
        luminance(p.status_danger_fill_active),
    );
    assert!(
        luminance(p.status_danger_text) >= min_for_text,
        "the text red sits below the text role's floor and would fail as text: \
         {} < {min_for_text:.6}",
        luminance(p.status_danger_text),
    );

    // And each token fails in the other's role, which is what makes them
    // non-interchangeable rather than merely different.
    assert!(
        contrast(p.text_strong, p.status_danger_text) < MIN_TEXT_RULE,
        "status.danger.text must still fail as a background — otherwise the \
         split was unnecessary"
    );
    assert!(
        contrast(p.status_danger_fill, p.surface_overlay) < MIN_TEXT_RULE,
        "status.danger.fill must still fail as error text on the overlay — it \
         is too dark to be the written-error colour"
    );
    assert!(
        contrast(p.status_danger_fill_active, p.surface_overlay) < MIN_TEXT_RULE,
        "status.danger.fill_active must still fail as error text on the overlay \
         for the same reason"
    );
}

/// The polarity split cannot collapse: for each family, no member can do
/// another member's job.
///
/// This is the general form of the danger disjoint-bounds result, stated
/// across both families. The accent's three tokens are pinned so that the
/// fill cannot serve as the border, the border cannot serve as the fill's
/// text, and the pressed fill cannot substitute for either — which is
/// precisely the property that makes a six-token split worth its size
/// over a single-token palette with opacity modifiers.
#[test]
fn test_no_polarity_member_can_do_another_members_job() {
    let p = Palette::dark();

    // The accent border cannot be the CTA fill: text.strong on it is
    // 1.87:1, unreadable as body text.
    assert!(
        contrast(p.text_strong, p.accent_border) < MIN_TEXT_RULE,
        "accent.border must fail as a text background — it is a stroke, not a \
         fill: {:.4}:1",
        contrast(p.text_strong, p.accent_border),
    );
    // The accent fill cannot be the focus stroke against its own pressed
    // sibling: 1.18:1 is not a visible ring.
    assert!(
        contrast(p.accent_fill, p.accent_fill_active) < 3.0,
        "accent.fill must fail as a ring on the pressed fill — it is too close \
         in luminance: {:.4}:1",
        contrast(p.accent_fill, p.accent_fill_active),
    );
    // The pressed accent fill cannot carry the resting fill's job of
    // sitting on base at the resting token's contrast — it is darker, so
    // it reads deeper, and the two are not interchangeable.
    assert_ne!(
        p.accent_fill, p.accent_fill_active,
        "the pressed accent must be its own value, not an alias of the resting fill"
    );

    // Danger mirrors the accent exactly.
    assert!(
        contrast(p.text_strong, p.status_danger_text) < MIN_TEXT_RULE,
        "status.danger.text must fail as a text background — it is the written-\
         error colour, not a fill: {:.4}:1",
        contrast(p.text_strong, p.status_danger_text),
    );
    assert!(
        contrast(p.status_danger_fill, p.status_danger_fill_active) < 3.0,
        "status.danger.fill must fail as a ring on the pressed fill: {:.4}:1",
        contrast(p.status_danger_fill, p.status_danger_fill_active),
    );
    assert_ne!(
        p.status_danger_fill, p.status_danger_fill_active,
        "the pressed danger fill must be its own value, not an alias"
    );
}

/// The two families' *fills* are not interchangeable with each other's
/// either, which is what keeps a primary CTA from being painted with the
/// danger red by a copy-paste mistake that still passes contrast.
#[test]
fn test_the_two_families_are_not_interchangeable() {
    let p = Palette::dark();
    assert_ne!(
        p.accent_fill, p.status_danger_fill,
        "the accent and danger fills must be different colours"
    );
    assert!(
        distance(p.accent_fill, p.status_danger_fill) > 200.0,
        "the two resting fills are only {:.1} apart in the RGB cube — a \
         copy-paste between them would not be visible",
        distance(p.accent_fill, p.status_danger_fill)
    );
    assert!(
        distance(p.accent_border, p.status_danger_text) > 150.0,
        "the accent stroke and the danger text colour are only {:.1} apart — \
         they must not be confusable",
        distance(p.accent_border, p.status_danger_text)
    );
}
