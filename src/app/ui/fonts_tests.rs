//! Unit tests for font configuration and fallback loading.

use super::*;
use eframe::egui;

fn setup_test_context() -> egui::Context {
    let ctx = egui::Context::default();
    configure_fonts(&ctx);
    let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
    output.textures_delta.clear();
    ctx
}

#[test]
fn test_arrow_glyph_supported_in_proportional_font() {
    let ctx = setup_test_context();
    assert!(
        is_glyph_supported(&ctx, '→'),
        "Proportional font must support '→' (U+2192 RIGHTWARDS ARROW)"
    );
}

#[test]
fn test_directional_arrows_family_coverage() {
    let ctx = setup_test_context();
    let arrows = [
        ('←', "LEFTWARDS ARROW"),
        ('→', "RIGHTWARDS ARROW"),
        ('↑', "UPWARDS ARROW"),
        ('↓', "DOWNWARDS ARROW"),
        ('↔', "LEFT RIGHT ARROW"),
        ('⇒', "RIGHTWARDS DOUBLE ARROW"),
        ('⇐', "LEFTWARDS DOUBLE ARROW"),
        ('⇔', "LEFT RIGHT DOUBLE ARROW"),
    ];
    for (ch, name) in arrows {
        assert!(
            is_glyph_supported(&ctx, ch),
            "Proportional font must support '{ch}' ({name})"
        );
    }
}

#[test]
fn test_monospace_font_has_fallback_arrows() {
    let ctx = setup_test_context();
    assert!(
        is_monospace_glyph_supported(&ctx, '→'),
        "Monospace font must support '→' (U+2192) via fallback chain"
    );
    assert!(
        is_monospace_glyph_supported(&ctx, '←'),
        "Monospace font must support '←' (U+2190) via fallback chain"
    );
}

#[test]
fn test_markdown_arrow_renders_without_tofu() {
    let ctx = setup_test_context();
    let text = "Step 1 → Step 2 ⇒ Step 3 ← Previous";
    assert!(
        are_glyphs_supported(&ctx, text),
        "Text with arrows must be rendered without tofu or replacement characters"
    );

    ctx.fonts_mut(|f| {
        let font_id = egui::FontId::proportional(14.0);
        let galley = f.layout_no_wrap(text.to_string(), font_id, egui::Color32::WHITE);
        for row in &galley.rows {
            for glyph in &row.glyphs {
                assert_ne!(
                    glyph.chr,
                    '◻',
                    "Rendered glyph '{chr}' must not be fallback tofu square (U+25FB)",
                    chr = glyph.chr
                );
                assert_ne!(
                    glyph.chr,
                    '?',
                    "Rendered glyph '{chr}' must not be question mark replacement",
                    chr = glyph.chr
                );
            }
        }
    });
}

#[test]
fn test_box_drawing_and_math_coverage() {
    let ctx = setup_test_context();
    let symbols = [
        ('─', "BOX DRAWINGS LIGHT HORIZONTAL"),
        ('│', "BOX DRAWINGS LIGHT VERTICAL"),
        ('┌', "BOX DRAWINGS LIGHT DOWN AND RIGHT"),
        ('┘', "BOX DRAWINGS LIGHT UP AND LEFT"),
        ('✓', "CHECK MARK"),
    ];
    for (ch, name) in symbols {
        assert!(
            is_glyph_supported(&ctx, ch),
            "Proportional font must support '{ch}' ({name})"
        );
    }
}

#[test]
fn test_phosphor_icons_still_resolve_in_proportional() {
    let ctx = setup_test_context();
    let phosphor_icons = [
        egui_phosphor::regular::LIGHTNING.chars().next().unwrap(),
        egui_phosphor::regular::ROBOT.chars().next().unwrap(),
        egui_phosphor::regular::STOP.chars().next().unwrap(),
        egui_phosphor::regular::LIST.chars().next().unwrap(),
        egui_phosphor::regular::COPY.chars().next().unwrap(),
    ];
    for icon in phosphor_icons {
        assert!(
            is_glyph_supported(&ctx, icon),
            "Phosphor icon U+{:04X} must resolve in proportional font",
            icon as u32
        );
    }
}

#[test]
fn test_candidate_fallback_font_paths_non_empty() {
    let paths = candidate_fallback_font_paths();
    assert!(
        !paths.is_empty(),
        "Candidate fallback font paths list must not be empty"
    );
}

#[test]
fn test_build_font_definitions_structure() {
    let font_defs = build_font_definitions();
    assert!(
        font_defs.font_data.contains_key("phosphor"),
        "Font definitions must include phosphor icon font"
    );

    let prop_family = font_defs
        .families
        .get(&egui::FontFamily::Proportional)
        .expect("Proportional family must exist");
    assert_eq!(
        prop_family.get(1),
        Some(&"phosphor".to_string()),
        "Phosphor should be at index 1 in Proportional family"
    );

    if font_defs.font_data.contains_key(FALLBACK_FONT_NAME) {
        assert_eq!(
            prop_family.get(2),
            Some(&FALLBACK_FONT_NAME.to_string()),
            "Fallback symbol font should be at index 2 in Proportional family"
        );
        let mono_family = font_defs
            .families
            .get(&egui::FontFamily::Monospace)
            .expect("Monospace family must exist");
        assert_eq!(
            mono_family.get(1),
            Some(&FALLBACK_FONT_NAME.to_string()),
            "Fallback symbol font should be at index 1 in Monospace family"
        );
    }
}
