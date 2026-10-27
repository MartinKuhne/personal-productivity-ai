//! Tests for [`Avatar`] and [`AvatarModel`] (`UIF-ATOM-013`, §3.13).

use super::*;
use crate::test_helpers::Harness;
use crate::token::contrast;

#[test]
fn test_avatar_size_diameters_and_defaults() {
    assert_eq!(AvatarSize::default(), AvatarSize::Sm);
    assert_eq!(AvatarSize::Sm.diameter(), 24.0);
    assert_eq!(AvatarSize::Md.diameter(), 32.0);
    assert_eq!(AvatarSize::Lg.diameter(), 36.0);
}

#[test]
fn test_avatar_shape_radii_and_defaults() {
    assert_eq!(AvatarShape::default(), AvatarShape::Circle);
    assert_eq!(AvatarShape::Circle.radius(), shape::RADIUS_PILL);
    assert_eq!(AvatarShape::RoundedSquare.radius(), shape::RADIUS_CARD);
}

#[test]
fn test_extract_initials_cases() {
    assert_eq!(extract_initials("Jane Doe"), "JD");
    assert_eq!(extract_initials("Admin"), "A");
    assert_eq!(extract_initials("jane doe"), "JD");
    assert_eq!(extract_initials("John Fitzgerald Kennedy"), "JK");
    assert_eq!(extract_initials("  alice   bob  "), "AB");
    assert_eq!(extract_initials(""), "");
    assert_eq!(extract_initials("   "), "");
    assert_eq!(extract_initials("Élise Dupont"), "ÉD");
    assert_eq!(extract_initials("user123"), "U");
    assert_eq!(extract_initials("123 456"), "14");
    assert_eq!(extract_initials("!@#"), "");
}

#[test]
fn test_avatar_model_builders() {
    let model = AvatarModel::new("Jane Doe")
        .with_initials("JJ")
        .with_image_uri("https://example.com/user.png")
        .with_presence(Some(BadgeTone::Success))
        .with_size(AvatarSize::Lg)
        .with_shape(AvatarShape::RoundedSquare);

    assert_eq!(model.display_name, "Jane Doe");
    assert_eq!(model.initials, "JJ");
    assert_eq!(
        model.image_uri.as_deref(),
        Some("https://example.com/user.png")
    );
    assert_eq!(model.presence, Some(BadgeTone::Success));
    assert_eq!(model.size, AvatarSize::Lg);
    assert_eq!(model.shape, AvatarShape::RoundedSquare);
}

#[test]
fn test_fallback_cascade_order() {
    // Stage 1: Image URI present
    let model_img = AvatarModel::new("Jane Doe").with_image_uri("https://example.com/pic.png");
    assert_eq!(model_img.fallback_stage(), FallbackStage::Image);

    // Whitespace/empty image URI falls through
    let model_empty_uri = AvatarModel::new("Jane Doe").with_image_uri("   ");
    assert_eq!(model_empty_uri.fallback_stage(), FallbackStage::Initials);

    // Stage 2: Initials present
    let model_initials = AvatarModel::new("Jane Doe");
    assert_eq!(model_initials.fallback_stage(), FallbackStage::Initials);

    // Stage 3: Glyph when no initials and no image
    let model_glyph = AvatarModel::new("");
    assert_eq!(model_glyph.fallback_stage(), FallbackStage::Glyph);
}

#[test]
fn test_resolve_presence_color_mapping() {
    let palette = Palette::dark();
    assert_eq!(
        resolve_presence_color(BadgeTone::Success, &palette),
        palette.status_success
    );
    assert_eq!(
        resolve_presence_color(BadgeTone::Warning, &palette),
        palette.status_warning
    );
    assert_eq!(
        resolve_presence_color(BadgeTone::Danger, &palette),
        palette.status_danger_text
    );
    assert_eq!(
        resolve_presence_color(BadgeTone::Neutral, &palette),
        palette.text_muted
    );
    assert_eq!(
        resolve_presence_color(BadgeTone::Accent, &palette),
        palette.accent_border
    );
}

#[test]
fn test_initials_contrast_clears_thresholds() {
    let palette = Palette::dark();

    // Initials on accent.fill: spec requires clearing 4.50:1
    let ratio_accent = contrast::contrast_ratio(palette.text_strong, palette.accent_fill);
    assert!(
        ratio_accent >= 4.50,
        "text_strong on accent_fill ({ratio_accent}) must clear 4.50:1"
    );

    // Initials on surface.active: clears 4.50:1
    let ratio_surface = contrast::contrast_ratio(palette.text_strong, palette.surface_active);
    assert!(
        ratio_surface >= 4.50,
        "text_strong on surface_active ({ratio_surface}) must clear 4.50:1"
    );
}

#[test]
fn test_avatar_widget_sizes() {
    for size in [AvatarSize::Sm, AvatarSize::Md, AvatarSize::Lg] {
        let model = AvatarModel::new("Alice").with_size(size);
        let widget = avatar(model);
        let d = size.diameter();
        assert_eq!(widget.size().width, Length::Fixed(d));
        assert_eq!(widget.size().height, Length::Fixed(d));
    }
}

#[test]
fn test_avatar_draw_headless_across_fallback_stages() {
    // 1. Stage Image with presence
    let model_img = AvatarModel::new("Jane Doe")
        .with_image_uri("path/to/img.png")
        .with_presence(Some(BadgeTone::Success))
        .with_size(AvatarSize::Md);
    let mut harness_img = Harness::<(), _>::headless(move || avatar(model_img.clone()).into());
    harness_img.render();

    // 2. Stage Initials with presence
    let model_init = AvatarModel::new("Jane Doe")
        .with_presence(Some(BadgeTone::Warning))
        .with_size(AvatarSize::Sm);
    let mut harness_init = Harness::<(), _>::headless(move || avatar(model_init.clone()).into());
    harness_init.render();

    // 3. Stage Glyph without presence
    let model_glyph = AvatarModel::new("")
        .with_size(AvatarSize::Lg)
        .with_shape(AvatarShape::RoundedSquare);
    let mut harness_glyph = Harness::<(), _>::headless(move || avatar(model_glyph.clone()).into());
    harness_glyph.render();
}

#[test]
fn test_from_avatar_for_element() {
    let model = AvatarModel::new("Test User");
    let av = avatar(model);
    let _element: Element<'_, ()> = av.into();
}
