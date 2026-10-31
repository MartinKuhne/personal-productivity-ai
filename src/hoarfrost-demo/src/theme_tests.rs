//! Unit tests for [`crate::theme`].

use hoarfrost::token::color::Palette;
use iced::theme::Base;

use super::*;

#[test]
fn theme_carries_the_demo_name() {
    assert_eq!(demo_theme().name(), DEMO_THEME_NAME);
}

#[test]
fn every_iced_role_comes_straight_from_the_framework_palette() {
    let pro = Palette::dark();
    let mapped = demo_palette(&pro);

    assert_eq!(mapped.background, pro.surface_base);
    assert_eq!(mapped.text, pro.text_primary);
    assert_eq!(mapped.primary, pro.accent_fill);
    assert_eq!(mapped.success, pro.status_success);
    assert_eq!(mapped.warning, pro.status_warning);
    assert_eq!(mapped.danger, pro.status_danger_text);
}

#[test]
fn mapping_is_deterministic() {
    assert_eq!(
        demo_palette(&Palette::dark()).primary,
        demo_palette(&Palette::dark()).primary
    );
}
