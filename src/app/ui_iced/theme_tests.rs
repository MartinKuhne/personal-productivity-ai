//! Unit tests for the iced theme mapping ([TASK-008]).
//!
//! All colour values come from [`crate::ui::theme::Palette::fastmd_dark`]
//! ([RUST-006]: pure computation only — no window/process spawn, no
//! filesystem access).

use super::*;
use crate::ui::theme::Palette;

/// One spot-checked palette token: `(token, expected R, G, B)` — the u8
/// values the egui theme declares ([GUD-003] single source of truth).
fn palette_spot_checks(palette: &Palette) -> [(eframe::egui::Color32, u8, u8, u8); 5] {
    [
        (palette.surface, 9, 9, 11),
        (palette.text, 210, 210, 210),
        (palette.accent, 99, 102, 241),
        (palette.warning, 220, 130, 0),
        (palette.error, 244, 15, 2),
    ]
}

#[test]
fn test_to_iced_color_preserves_rgb_spot_checks() {
    let palette = Palette::fastmd_dark();
    for (token, r, g, b) in palette_spot_checks(&palette) {
        let color = to_iced_color(token);
        assert!(
            (color.r - f32::from(r) / 255.0).abs() < f32::EPSILON
                && (color.g - f32::from(g) / 255.0).abs() < f32::EPSILON
                && (color.b - f32::from(b) / 255.0).abs() < f32::EPSILON,
            "token {token:?} must map to iced rgb({r}, {g}, {b}), got \
             ({}, {}, {})",
            color.r * 255.0,
            color.g * 255.0,
            color.b * 255.0
        );
    }
}

#[test]
fn test_to_iced_color_includes_alpha() {
    // egui Color32 is stored PREMULTIPLIED (ecolor color32.rs:31); the helper
    // must convert through `to_srgba_unmultiplied` so iced receives straight
    // (unmultiplied) channels. `from_rgba_unmultiplied(10,20,30,128)` stores
    // premultiplied (5,10,15,128) and must come back as (10,20,30,128).
    let semi = eframe::egui::Color32::from_rgba_unmultiplied(10, 20, 30, 128);
    let color = to_iced_color(semi);
    assert_eq!(color.r, f32::from(10u8) / 255.0);
    assert_eq!(color.g, f32::from(20u8) / 255.0);
    assert_eq!(color.b, f32::from(30u8) / 255.0);
    assert!(
        (color.a - f32::from(128u8) / 255.0).abs() < f32::EPSILON,
        "alpha must survive the conversion, got {}",
        color.a
    );
    // A fully opaque token maps to alpha 1.0.
    assert_eq!(to_iced_color(Palette::fastmd_dark().surface).a, 1.0);
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
            iced_color,
            to_iced_color(token),
            "each iced palette role must carry its token's exact value"
        );
    }
}

#[test]
fn test_scrim_color_dims_surface_token_to_scrim_alpha() {
    let scrim = scrim_color();
    let surface = to_iced_color(Palette::fastmd_dark().surface);

    // RGB must be the untouched `surface` token — the scrim only changes α, so
    // no new colour value enters the palette ([GUD-003]).
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
    assert_eq!(
        theme.palette().primary,
        to_iced_color(Palette::fastmd_dark().accent)
    );
}
