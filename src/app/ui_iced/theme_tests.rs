//! Unit tests for the iced theme mapping.
//!
//! Since INT-1 the values come from the `hoarfrost` framework palette
//! (`UIF-TOK-005`), reached through
//! [`crate::ui::theme::Palette::pro`]. These tests assert **parity with
//! the framework** — the mapped iced role IS the framework token, and
//! the app scrim IS `hoarfrost::surface::zorder::scrim_for(...)` — so a
//! divergence between app and framework fails loudly here.
//!
//! [RUST-006]: pure computation only — no window/process spawn, no
//! filesystem access.

use super::*;
use crate::ui::theme::Palette;
use hoarfrost::surface::zorder;
use hoarfrost::token::color::Palette as ProPalette;

/// One spot-checked palette token: `(token, expected R, G, B)` — the u8
/// values the `UIF-TOK-005` palette declares.
fn palette_spot_checks(palette: &Palette) -> [(iced::Color, u8, u8, u8); 5] {
    [
        (palette.surface, 9, 9, 11),
        (palette.text, 210, 210, 210),
        // UIF-TOK-005 accent.fill (was 99, 102, 241 pre-INT-1).
        (palette.accent, 93, 96, 238),
        (palette.warning, 220, 130, 0),
        // UIF-TOK-005 status.danger.text (was 244, 15, 2 pre-INT-1).
        (palette.error, 255, 92, 92),
    ]
}

#[test]
fn test_palette_tokens_preserve_rgb_spot_checks() {
    let palette = Palette::fastmd_dark();
    for (token, r, g, b) in palette_spot_checks(&palette) {
        assert!(
            (token.r - f32::from(r) / 255.0).abs() < f32::EPSILON
                && (token.g - f32::from(g) / 255.0).abs() < f32::EPSILON
                && (token.b - f32::from(b) / 255.0).abs() < f32::EPSILON,
            "token {token:?} must be iced rgb({r}, {g}, {b}), got \
             ({}, {}, {})",
            token.r * 255.0,
            token.g * 255.0,
            token.b * 255.0
        );
    }
}

#[test]
fn test_palette_tokens_are_opaque() {
    // Every token in `Palette` is fully opaque (α = 1.0), which is what
    // lets the theme mapping pass tokens through untouched.
    let palette = Palette::fastmd_dark();
    assert_eq!(palette.surface.a, 1.0);
    assert_eq!(palette.accent.a, 1.0);
    assert_eq!(palette.error.a, 1.0);
}

/// The iced palette roles carry the framework tokens they are mapped
/// from, and — because the app palette derives from the same source — the
/// same value the app token carries. Both directions are asserted so a
/// divergence in either hop is caught.
#[test]
fn test_iced_theme_palette_roles_and_values() {
    let app = Palette::fastmd_dark();
    let pro = ProPalette::dark();
    let iced_palette = iced_theme_palette(&pro);

    let roles = [
        (
            "background",
            iced_palette.background,
            pro.surface_base,
            app.surface,
        ),
        ("text", iced_palette.text, pro.text_primary, app.text),
        ("primary", iced_palette.primary, pro.accent_fill, app.accent),
        (
            "success",
            iced_palette.success,
            pro.status_success,
            app.success,
        ),
        (
            "warning",
            iced_palette.warning,
            pro.status_warning,
            app.warning,
        ),
        (
            "danger",
            iced_palette.danger,
            pro.status_danger_text,
            app.error,
        ),
    ];
    for (name, iced_color, pro_value, app_value) in roles {
        assert_eq!(
            iced_color, pro_value,
            "iced role `{name}` must carry the hoarfrost source token \
             {pro_value:?}, got {iced_color:?}"
        );
        assert_eq!(
            iced_color, app_value,
            "iced role `{name}` must agree with the app token \
             {app_value:?}, got {iced_color:?}"
        );
    }
}

/// The app scrim IS the framework's `scrim_for` result — not a
/// re-implementation of it.
#[test]
fn test_scrim_color_is_the_framework_scrim() {
    let pro = ProPalette::dark();
    let expected = zorder::scrim_for(&pro);

    assert_eq!(
        scrim_color(),
        expected,
        "the app scrim must be hoarfrost::surface::zorder::scrim_for(&Palette::pro())"
    );
    assert_eq!(
        scrim_color(),
        pro.surface_scrim,
        "the framework scrim must in turn equal the `color.surface.scrim` \
         token (UIF-TOK-005)"
    );
}

/// [RUST-021]: the app must not own a second scrim-alpha literal. The
/// scrim's alpha is read from the framework constant
/// `hoarfrost::surface::zorder::SCRIM_ALPHA`, never restated here — which
/// is why no numeric alpha appears in this file at all.
#[test]
fn test_scrim_alpha_is_the_framework_alpha_not_a_second_literal() {
    assert_eq!(
        scrim_color().a,
        zorder::SCRIM_ALPHA,
        "the app scrim alpha must be hoarfrost::surface::zorder::SCRIM_ALPHA"
    );
}

#[test]
fn test_scrim_color_dims_surface_token_to_scrim_alpha() {
    let scrim = scrim_color();
    let surface = Palette::fastmd_dark().surface;

    // RGB must be the untouched `surface` token — the scrim only changes α, so
    // no new colour value enters the palette.
    assert_eq!(scrim.r, surface.r);
    assert_eq!(scrim.g, surface.g);
    assert_eq!(scrim.b, surface.b);
    assert_eq!(scrim.a, zorder::SCRIM_ALPHA);
}

#[test]
fn test_scrim_alpha_is_translucent_not_opaque() {
    let scrim = scrim_color();

    assert!(
        scrim.a > 0.0 && scrim.a < 1.0,
        "the scrim must be translucent so the background stays legible, got {}",
        scrim.a
    );
}

#[test]
fn test_fastmd_dark_theme_is_registered_custom_theme() {
    let theme = fastmd_dark_theme();
    assert!(
        matches!(theme, iced::Theme::Custom(_)),
        "the FastMD theme must be a Theme::Custom instance"
    );
    assert_eq!(theme.palette().background.r, f32::from(9u8) / 255.0);
    assert_eq!(theme.palette().background.g, f32::from(9u8) / 255.0);
    assert_eq!(theme.palette().background.b, f32::from(11u8) / 255.0);
    assert_eq!(theme.palette().primary, Palette::fastmd_dark().accent);
    assert_eq!(theme.palette().primary, ProPalette::dark().accent_fill);
}
