//! Unit tests for the iced theme mapping.
//!
//! All colour values come from [`crate::ui::theme::Palette::fastmd_dark`]
//! ([RUST-006]: pure computation only — no window/process spawn, no
//! filesystem access).

use super::*;
use crate::ui::theme::Palette;

/// One spot-checked palette token: `(token, expected R, G, B)` — the u8
/// values the palette declares as the single source of truth.
fn palette_spot_checks(palette: &Palette) -> [(iced::Color, u8, u8, u8); 5] {
    [
        (palette.surface, 9, 9, 11),
        (palette.text, 210, 210, 210),
        (palette.accent, 99, 102, 241),
        (palette.warning, 220, 130, 0),
        (palette.error, 244, 15, 2),
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

#[test]
fn test_iced_theme_palette_roles_and_values() {
    let palette = Palette::fastmd_dark();
    let iced_palette = iced_theme_palette(&palette);

    let roles = [
        (iced_palette.background, palette.surface),
        (iced_palette.text, palette.text),
        (iced_palette.primary, palette.accent),
        (iced_palette.success, palette.success),
        (iced_palette.warning, palette.warning),
        (iced_palette.danger, palette.error),
    ];
    for (iced_color, token) in roles {
        assert_eq!(
            iced_color, token,
            "each iced palette role must carry its token's exact value"
        );
    }
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
    assert_eq!(scrim.a, SCRIM_ALPHA);
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
}
