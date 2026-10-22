//! Tests for [`color`] — the semantic colour tokens (`UIF-TOK-005`).
//!
//! Every token is pinned channel-by-channel to the sRGB value the spec's
//! §2.5 table gives it, so a silent retune fails loudly and names the
//! token that moved. On top of the pins, the tests enforce the structural
//! promises the table makes: the dark-first elevation ladder, the
//! neutrality of the surface and border greys, the strict prominence
//! order of the text roles, mutual distinguishability of the status
//! colours, the inverted shape of the editor surface, and the rule that
//! no two distinct tokens collapse onto the same colour.
//!
//! The luminance maths here is deliberately private to this file. The
//! production solver — relative luminance, WCAG ratios and the mandated
//! pairing table — is `token/contrast.rs` (`UIF-TOK-006` /
//! `UIF-VER-001`) and arrives in a later increment; duplicating it in
//! production now would pre-empt that module's API.
//!
//! The last group of tests covers the `status.danger` split: decision
//! **D-5** added `color.status.danger_fill` because §2.5's single red
//! cannot be both the error-text colour on dark surfaces and the
//! background behind `text.strong` (gap **G-8**). Those tests pin the
//! new token's contrast margin, its hue, its distance from every surface,
//! and the disjointness of the two roles' luminance bounds — so the
//! split cannot quietly collapse back into one token.

use super::*;

/// Tolerance for `f32` channel arithmetic: a channel round-trips through
/// `/ 255.0` untouched, so the only error is float representation.
const EPS: f32 = 1e-6;

/// Ceiling on the R/G/B spread of a token that claims to be a neutral
/// grey, on the 0–255 scale.
///
/// The dark surfaces are not perfectly neutral: each carries a slight
/// blue lean that grows with elevation — `+2` at `base` and `sunken`,
/// `+3` at `overlay`, `+6` at `hover`, `+8` at `active`. Eight units is
/// 3.1% of a channel's range, which reads as a cool cast rather than a
/// hue, and is exactly the spread of the widest offending token. Anything
/// past this ceiling has stopped being grey and started being a colour.
const NEUTRAL_SPREAD_MAX: f32 = 8.0;

/// Minimum Euclidean RGB distance (0–255 scale) between two status
/// colours.
///
/// A third of a channel's range has to separate them: the three statuses
/// carry meaning on colour alone in chips and dots, so they must survive
/// a quick glance, a cheap display and colour-blind-adjacent viewing.
/// The tightest pair on the spec is `warning` ↔ `danger` at ≈117.5, so
/// 100 leaves real headroom without being so loose that a retune could
/// slide two statuses together.
const STATUS_MIN_DISTANCE: f32 = 100.0;

/// A row of the `UIF-TOK-005` table paired with the token that
/// implements it.
///
/// Keeping the transcription next to the value means retuning either
/// side shows up as a failing assertion that names its own token. Three
/// rows deliberately do *not* carry the spec's printed value —
/// `surface.active`, `accent.primary` and `status.danger` are the
/// `G-7` contrast retunes (decision **A** in
/// `doc/planning/iced-pro-tasks.md`), so the numbers below are the
/// retuned ones, not the ones §2.5 prints. One further row,
/// `color.status.danger_fill`, carries a value §2.5 does not print at
/// all: the token decision **D-5** adds to the set.
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

/// The twenty dark-palette tokens against the spec table, in the
/// table's own order.
///
/// Every table-driven test walks this list, so no token can be skipped by
/// a test that checks a hand-picked subset.
///
/// Nineteen rows transcribe §2.5. The last one does not:
/// `color.status.danger_fill` is not in the spec's printed table at all.
/// It is the token decision **D-5** adds to the set because §2.5's
/// single `status.danger` cannot serve both the error-text role and the
/// background-behind-`text.strong` role (gap **G-8**), so this row's
/// pinned value is the ledger's, not the spec's.
fn spec_rows() -> [SpecRow; 20] {
    let p = Palette::dark();
    [
        row("color.surface.base", p.surface_base, 9, 9, 11, 1.0),
        row("color.surface.sunken", p.surface_sunken, 6, 6, 8, 1.0),
        row("color.surface.raised", p.surface_raised, 18, 18, 20, 1.0),
        row("color.surface.code", p.surface_code, 20, 20, 22, 1.0),
        row("color.surface.overlay", p.surface_overlay, 24, 24, 27, 1.0),
        row("color.surface.hover", p.surface_hover, 38, 38, 44, 1.0),
        row("color.surface.active", p.surface_active, 43, 43, 51, 1.0),
        row("color.surface.scrim", p.surface_scrim, 9, 9, 11, 0.55),
        row("color.border.subtle", p.border_subtle, 40, 40, 40, 1.0),
        row("color.border.strong", p.border_strong, 120, 120, 120, 1.0),
        row("color.accent.primary", p.accent_primary, 98, 100, 240, 1.0),
        row("color.accent.text", p.accent_text, 100, 200, 255, 1.0),
        row("color.text.primary", p.text_primary, 210, 210, 210, 1.0),
        row("color.text.strong", p.text_strong, 255, 255, 255, 1.0),
        row("color.text.muted", p.text_muted, 160, 160, 160, 1.0),
        row("color.text.disabled", p.text_disabled, 140, 140, 140, 1.0),
        row("color.status.success", p.status_success, 100, 255, 100, 1.0),
        row("color.status.warning", p.status_warning, 220, 130, 0, 1.0),
        row("color.status.danger", p.status_danger, 255, 28, 2, 1.0),
        row(
            "color.status.danger_fill",
            p.status_danger_fill,
            208,
            23,
            2,
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
/// (`UIF-TOK-006`), which is a later increment.
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
    let ladder = [
        ("color.surface.sunken", p.surface_sunken),
        ("color.surface.base", p.surface_base),
        ("color.surface.raised", p.surface_raised),
        ("color.surface.code", p.surface_code),
        ("color.surface.overlay", p.surface_overlay),
        ("color.surface.hover", p.surface_hover),
        ("color.surface.active", p.surface_active),
    ];

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

/// `sunken` is the only surface below base — the ladder has exactly one
/// step downwards, and it is the one the spec names.
#[test]
fn test_sunken_is_the_only_surface_darker_than_base() {
    let p = Palette::dark();
    let surfaces = [
        ("color.surface.sunken", p.surface_sunken),
        ("color.surface.raised", p.surface_raised),
        ("color.surface.code", p.surface_code),
        ("color.surface.overlay", p.surface_overlay),
        ("color.surface.hover", p.surface_hover),
        ("color.surface.active", p.surface_active),
    ];
    let base = luminance(p.surface_base);

    for (name, surface) in surfaces {
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
/// a failure here on `color.accent.primary` would mean the neutrality
/// list was widened by mistake, not that the accent changed.
#[test]
fn test_surfaces_and_borders_are_neutral_greys() {
    let p = Palette::dark();
    let neutrals = [
        ("color.surface.base", p.surface_base),
        ("color.surface.sunken", p.surface_sunken),
        ("color.surface.raised", p.surface_raised),
        ("color.surface.code", p.surface_code),
        ("color.surface.overlay", p.surface_overlay),
        ("color.surface.hover", p.surface_hover),
        ("color.surface.active", p.surface_active),
        ("color.surface.scrim", p.surface_scrim),
        ("color.border.subtle", p.border_subtle),
        ("color.border.strong", p.border_strong),
    ];

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

/// The five chromatic tokens are outside the neutrality rule: each leads
/// a channel by far more than a grey ever does, which is what makes them
/// carry meaning rather than structure.
#[test]
fn test_accents_and_statuses_are_deliberately_non_neutral() {
    let p = Palette::dark();
    let chromatic = [
        ("color.accent.primary", p.accent_primary),
        ("color.accent.text", p.accent_text),
        ("color.status.success", p.status_success),
        ("color.status.warning", p.status_warning),
        ("color.status.danger", p.status_danger),
        ("color.status.danger_fill", p.status_danger_fill),
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
/// `color.status.danger_fill` is deliberately **not** a fourth entry. It
/// is the same danger signal in a different *role* — a background rather
/// than a text colour — and it sits only ~47 units from `color.status.danger`
/// on purpose, so the two read as one colour at two depths. Requiring the
/// 100-unit signal separation here would demand the fill be a different
/// colour rather than a darker red, which defeats the point of **D-5**.
/// Its own distinctness requirements are the ones that matter for a fill:
/// [`test_status_danger_fill_is_not_a_surface`] and
/// [`test_the_two_danger_roles_have_disjoint_luminance_bounds`].
#[test]
fn test_status_colors_are_mutually_distinguishable() {
    let p = Palette::dark();
    let statuses = [
        ("color.status.success", p.status_success),
        ("color.status.warning", p.status_warning),
        ("color.status.danger", p.status_danger),
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
    let surfaces = [
        p.surface_sunken,
        p.surface_base,
        p.surface_raised,
        p.surface_code,
        p.surface_overlay,
        p.surface_hover,
        p.surface_active,
    ];
    let dimmest_text = luminance(p.text_disabled);
    let brightest_surface = surfaces
        .iter()
        .map(|s| luminance(*s))
        .fold(f32::NEG_INFINITY, f32::max);

    for surface in surfaces {
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
// The `status.danger` split — decision D-5, gap G-8
// ---------------------------------------------------------------------------

/// The `UIF-TOK-006` rule 1 threshold the disjoint-bounds test solves with.
const MIN_TEXT_RULE: f32 = 4.5;

/// Tolerance on a pinned contrast ratio — three decimals, enough to tell
/// `4.9990` from `5.5185`.
const RATIO_EPS: f32 = 1e-3;

/// The margin `status.danger_fill` is held to against `text.strong`.
///
/// Deliberately above the bare `4.5:1` text rule: the reason this token
/// exists at all is that the old pairing was `0.6247` short, so the
/// replacement is pinned with room rather than sat on the line.
const DANGER_FILL_MIN_CONTRAST: f32 = 5.0;

/// Upper edge of the red hue band, in degrees. Orange starts at `30°`;
/// fifteen is well inside red and far from any colour that could be read
/// as a warm brown or amber.
const RED_BAND_MAX_DEG: f32 = 15.0;

/// The smallest luminance step that reads as "clearly darker" rather
/// than a nudge — a quarter of the way from `surface.base` to
/// `surface.active`.
const MIN_VISIBLE_LUMINANCE_STEP: f32 = 0.05;

/// How far the danger fill must stand from every surface on luminance, so
/// it is never mistaken for the layer it is meant to stand out from.
const FILL_VS_SURFACE_MIN_RATIO: f32 = 2.0;

/// WCAG 2.1 contrast ratio between two colours.
///
/// Test-local, matching the file's existing [`luminance`] helper.
fn contrast(a: Color, b: Color) -> f32 {
    let la = luminance(a);
    let lb = luminance(b);
    let (lighter, darker) = if la >= lb { (la, lb) } else { (lb, la) };
    (lighter + 0.05) / (darker + 0.05)
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

/// The background red carries `text.strong` with real margin, not a
/// knife-edge pass.
#[test]
fn test_status_danger_fill_carries_text_strong_with_margin() {
    let p = Palette::dark();
    let ratio = contrast(p.text_strong, p.status_danger_fill);

    assert!(
        ratio >= DANGER_FILL_MIN_CONTRAST,
        "text.strong on color.status.danger_fill measures {ratio:.4}:1, under \
         the {DANGER_FILL_MIN_CONTRAST}:1 this token exists to clear",
    );
    assert!(
        (ratio - 5.5185).abs() < RATIO_EPS,
        "color.status.danger_fill vs text.strong is pinned at 5.5185:1, got \
         {ratio:.4}:1",
    );
    assert!(
        ratio - DANGER_FILL_MIN_CONTRAST > 0.5,
        "headroom above the target is {:.4} — the point of the token is margin, \
         so a drift that eats it must fail loudly",
        ratio - DANGER_FILL_MIN_CONTRAST,
    );
}

/// The fill is still a red, not a brown or an amber: red dominant, green
/// and blue low, hue inside the red band.
#[test]
fn test_status_danger_fill_stays_recognisably_red() {
    let p = Palette::dark();
    let fill = p.status_danger_fill;

    assert!(
        ch8(fill.r) > 150.0,
        "the fill must stay a strong red, red is {}",
        ch8(fill.r)
    );
    assert!(
        ch8(fill.g) < 60.0,
        "green must stay low or the red goes brown, green is {}",
        ch8(fill.g)
    );
    assert!(
        ch8(fill.b) < 60.0,
        "blue must stay low or the red goes purple, blue is {}",
        ch8(fill.b)
    );
    assert!(
        ch8(fill.r) - ch8(fill.g) > 150.0,
        "red must dominate green by a wide margin, got {}",
        ch8(fill.r) - ch8(fill.g)
    );
    assert!(
        ch8(fill.r) - ch8(fill.b) > 150.0,
        "red must dominate blue by a wide margin, got {}",
        ch8(fill.r) - ch8(fill.b)
    );

    let hue = hue_deg(fill);
    assert!(
        (0.0..RED_BAND_MAX_DEG).contains(&hue),
        "color.status.danger_fill hue is {hue:.2}°, outside the red band \
         [0, {RED_BAND_MAX_DEG})",
    );
    assert!(
        (hue - hue_deg(p.status_danger)).abs() < 1.0,
        "the two danger reds must read as the same red: fill {hue:.2}° vs \
         status.danger {:.2}°",
        hue_deg(p.status_danger),
    );
}

/// The step down from `status.danger` is visible, and `status.danger`
/// itself is untouched by the split.
#[test]
fn test_status_danger_fill_is_a_visible_step_darker_and_danger_is_unchanged() {
    let p = Palette::dark();
    let fill_l = luminance(p.status_danger_fill);
    let text_l = luminance(p.status_danger);

    assert!(
        fill_l < text_l,
        "the fill must be darker than the text red: fill {fill_l:.6} vs \
         status.danger {text_l:.6}",
    );
    assert!(
        text_l - fill_l >= MIN_VISIBLE_LUMINANCE_STEP,
        "the step down is only {:.6} of relative luminance — that is a nudge, \
         not the visible escalation D-5 asks for",
        text_l - fill_l,
    );
    assert!(
        fill_l / text_l < 0.7,
        "the fill is at {:.1}% of the text red's luminance; it must read as a \
         third darker, not slightly darker",
        100.0 * fill_l / text_l,
    );

    // The split adds a token; it does not move the existing one.
    assert_rgba("color.status.danger", p.status_danger, 255, 28, 2, 1.0);
}

/// The fill is not a surface and must never read as one: it is far more
/// chromatic than any grey in the ladder, and it stands clear of every
/// surface on luminance too.
#[test]
fn test_status_danger_fill_is_not_a_surface() {
    let p = Palette::dark();
    let fill = p.status_danger_fill;
    let surfaces = [
        ("color.surface.sunken", p.surface_sunken),
        ("color.surface.base", p.surface_base),
        ("color.surface.raised", p.surface_raised),
        ("color.surface.code", p.surface_code),
        ("color.surface.overlay", p.surface_overlay),
        ("color.surface.hover", p.surface_hover),
        ("color.surface.active", p.surface_active),
    ];

    for (name, surface) in surfaces {
        assert!(
            spread(fill) > NEUTRAL_SPREAD_MAX * 10.0,
            "the fill's channel spread is {}, which is not chromatic enough to \
             be unmistakably not a grey",
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
            "color.status.danger_fill vs {name} is only {ratio:.4}:1 — close \
             enough to the surface to be mistaken for one",
        );
    }
}

/// The two roles cannot share one token, and each token sits on its own
/// side of the divide.
///
/// Solving each role for the red's luminance gives two bounds that do not
/// overlap. That is the arithmetic behind gap **G-8** and the reason
/// **D-5** split the token rather than retuning it.
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
        luminance(p.status_danger) >= min_for_text,
        "the text red sits below the text role's floor and would fail as text: \
         {} < {min_for_text:.6}",
        luminance(p.status_danger),
    );

    // And each token fails in the other's role, which is what makes them
    // non-interchangeable rather than merely different.
    assert!(
        contrast(p.text_strong, p.status_danger) < MIN_TEXT_RULE,
        "status.danger must still fail as a background — otherwise the split \
         was unnecessary"
    );
    assert!(
        contrast(p.status_danger_fill, p.surface_overlay) < MIN_TEXT_RULE,
        "status.danger_fill must still fail as error text on the overlay — it \
         is too dark to be the written-error colour"
    );
}
