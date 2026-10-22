//! Tests for `ui/theme.rs` (UI-002 dark palette, UI-046 inverted editor).
//!
//! Pure token-value tests only ([RUST-006]): every assertion compares the
//! re-typed `iced::Color` tokens against the exact u8 triples the palette
//! declares, keeping the palette the single source of truth.

use super::*;

/// UI-002: the canonical dark palette carries the FastMD brand colours
/// (RGB(9, 9, 11) surface, indigo RGB(99, 102, 241) accent).
#[test]
fn test_fastmd_dark_brand_tokens() {
    let palette = Palette::fastmd_dark();
    assert_eq!(palette.surface, Color::from_rgb8(9, 9, 11));
    assert_eq!(palette.accent, Color::from_rgb8(99, 102, 241));
}

/// Every token must be distinct from its neighbours where the original inline
/// literals differed, so collapsing the palette can't silently merge two
/// shades.
#[test]
fn test_fastmd_dark_preserves_distinct_shades() {
    let palette = Palette::fastmd_dark();
    assert_ne!(palette.code_surface, palette.table_surface);
    assert_ne!(palette.text_muted, palette.text_faint);
    assert_ne!(palette.accent_text_soft, palette.accent_text_subtle);
    assert_ne!(palette.log_timestamp, palette.log_category);
    assert_eq!(palette.log_timestamp, Color::from_rgb8(96, 96, 96));
    assert_eq!(palette.log_category, Color::from_rgb8(173, 216, 230));
}

/// Every palette token is fully opaque (α = 1.0) — the theme mapping
/// relies on this: no palette token carries translucency.
#[test]
fn test_fastmd_dark_tokens_are_opaque() {
    let palette = Palette::fastmd_dark();
    let tokens = [
        palette.surface,
        palette.surface_variant,
        palette.code_surface,
        palette.table_surface,
        palette.pdf_surface,
        palette.border,
        palette.border_strong,
        palette.text,
        palette.text_strong,
        palette.text_value,
        palette.text_muted,
        palette.text_faint,
        palette.accent,
        palette.accent_text,
        palette.accent_text_soft,
        palette.accent_text_subtle,
        palette.success,
        palette.warning,
        palette.error,
        palette.log_timestamp,
        palette.log_category,
    ];
    for token in tokens {
        assert_eq!(token.a, 1.0, "palette token {token:?} must be opaque");
    }
}

/// The u8 round-trip is exact for every declared brand value: the re-typed
/// `iced::Color` tokens normalise 0..255 bytes to f32 / 255.0 without
/// drift.
#[test]
fn test_brand_values_round_trip_through_rgba8() {
    let palette = Palette::fastmd_dark();
    let checks: [(Color, u8, u8, u8); 6] = [
        (palette.surface, 9, 9, 11),
        (palette.text, 210, 210, 210),
        (palette.accent, 99, 102, 241),
        (palette.warning, 220, 130, 0),
        (palette.error, 244, 15, 2),
        (palette.accent_text, 100, 200, 255),
    ];
    for (token, r, g, b) in checks {
        assert_eq!(token, Color::from_rgb8(r, g, b));
        let [tr, tg, tb, ta] = token.into_rgba8();
        assert_eq!((tr, tg, tb, ta), (r, g, b, 255));
    }
}

/// UI-046 in one assertion set: the inverted editor palette has black text on
/// a white background with a black border, the default equals the inverted
/// one (so callers don't have to opt in), and per-channel contrast is maximal.
#[test]
fn test_editor_colors_inverted_satisfies_ui046() {
    let colors = EditorColors::inverted();

    assert_eq!(colors.background, Color::WHITE);
    assert_eq!(colors.text, Color::BLACK);
    assert_eq!(colors.border, Color::BLACK);
    assert_eq!(colors.error, Color::from_rgb8(255, 0, 0));

    assert_eq!(EditorColors::default(), EditorColors::inverted());

    let bg = colors.background;
    let fg = colors.text;
    assert_eq!(bg.r + fg.r, 1.0);
    assert_eq!(bg.g + fg.g, 1.0);
    assert_eq!(bg.b + fg.b, 1.0);
}
