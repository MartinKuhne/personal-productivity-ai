//! Tests for `ui/theme.rs` (UI-002 dark palette, UI-046 inverted editor).

use super::*;
use crate::ui::test_helpers::run_ui_test;

/// UI-002: the canonical dark palette carries the FastMD brand colours
/// (RGB(9, 9, 11) surface, indigo RGB(99, 102, 241) accent).
#[test]
fn test_fastmd_dark_brand_tokens() {
    let palette = Palette::fastmd_dark();
    assert_eq!(palette.surface, egui::Color32::from_rgb(9, 9, 11));
    assert_eq!(palette.accent, egui::Color32::from_rgb(99, 102, 241));
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
    assert_eq!(palette.log_timestamp, egui::Color32::DARK_GRAY);
    assert_eq!(palette.log_category, egui::Color32::LIGHT_BLUE);
}

/// `apply_to_visuals` wires the surface/accent/foreground tokens into the
/// egui `Visuals` slots the app owns.
#[test]
fn test_apply_to_visuals_maps_owned_fields() {
    let palette = Palette::fastmd_dark();
    let mut visuals = egui::Visuals::dark();
    palette.apply_to_visuals(&mut visuals);

    assert_eq!(visuals.panel_fill, palette.surface);
    assert_eq!(visuals.window_fill, palette.surface);
    assert_eq!(visuals.selection.bg_fill, palette.accent);
    assert_eq!(visuals.widgets.noninteractive.fg_stroke.color, palette.text);
    assert_eq!(visuals.widgets.inactive.fg_stroke.color, palette.text);
    assert_eq!(visuals.widgets.active.fg_stroke.color, palette.text_strong);
    assert_eq!(visuals.widgets.hovered.fg_stroke.color, palette.text_strong);
}

/// UI-002 regression (ported from `app/tests.rs`): `install_dark` must force
/// the active theme to Dark even when the host reported light mode, and apply
/// the brand palette to the dark theme explicitly.
#[test]
fn test_install_dark_pins_dark_theme_and_brand_palette() {
    let ctx = egui::Context::default();
    ctx.set_theme(egui::Theme::Light);
    assert_eq!(ctx.theme(), egui::Theme::Light);

    install_dark(&ctx);

    assert_eq!(
        ctx.theme(),
        egui::Theme::Dark,
        "install_dark must force the active theme to Dark"
    );

    let palette = Palette::fastmd_dark();
    let dark_visuals = ctx.style_of(egui::Theme::Dark).visuals.clone();
    assert_eq!(dark_visuals.panel_fill, palette.surface);
    assert_eq!(dark_visuals.window_fill, palette.surface);
    assert_eq!(dark_visuals.selection.bg_fill, palette.accent);
}

/// `install_dark` publishes the palette so `PaletteExt::palette` returns it.
#[test]
fn test_install_dark_publishes_palette_to_context() {
    let ctx = egui::Context::default();
    install_dark(&ctx);
    assert_eq!(ctx.palette(), Palette::fastmd_dark());
}

/// Before `install_dark` runs, `PaletteExt::palette` falls back to the
/// canonical dark palette rather than panicking.
#[test]
fn test_palette_ext_falls_back_without_install() {
    let ctx = egui::Context::default();
    assert_eq!(ctx.palette(), Palette::fastmd_dark());
}

/// `PaletteExt` is implemented for `Ui` as well as `Context`.
#[test]
fn test_palette_ext_on_ui_matches_context() {
    let ctx = egui::Context::default();
    install_dark(&ctx);
    let raw_input = egui::RawInput::default();
    let seen = std::cell::Cell::new(None);
    let _ = run_ui_test(&ctx, raw_input, |ui| {
        seen.set(Some(ui.palette()));
    });
    assert_eq!(seen.get(), Some(Palette::fastmd_dark()));
}

/// UI-046 in one assertion set: the inverted editor palette has black text on
/// a white background with a black border, the default equals the inverted
/// one (so callers don't have to opt in), and per-channel contrast is maximal.
#[test]
fn test_editor_colors_inverted_satisfies_ui046() {
    let colors = EditorColors::inverted();

    assert_eq!(colors.background, egui::Color32::WHITE);
    assert_eq!(colors.text, egui::Color32::BLACK);
    assert_eq!(colors.border, egui::Color32::BLACK);

    assert_eq!(EditorColors::default(), EditorColors::inverted());

    let bg = colors.background;
    let fg = colors.text;
    assert_eq!(bg.r() + fg.r(), 255);
    assert_eq!(bg.g() + fg.g(), 255);
    assert_eq!(bg.b() + fg.b(), 255);
}
