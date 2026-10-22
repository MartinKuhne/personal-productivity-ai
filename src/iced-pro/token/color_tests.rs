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
/// retuned ones, not the ones §2.5 prints.
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

/// The nineteen dark-palette tokens against the spec table, in the
/// table's own order.
///
/// Every table-driven test walks this list, so no token can be skipped by
/// a test that checks a hand-picked subset.
fn spec_rows() -> [SpecRow; 19] {
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
