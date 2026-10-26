//! Tests for navigation bar and trigger styling (`UIF-NAV-001`, `UIF-NAV-003`).

use super::*;
use crate::token::color::Palette;

#[test]
fn test_navbar_height_constants() {
    assert_eq!(NAVBAR_HEIGHT_STANDARD, 36.0);
    assert_eq!(NAVBAR_HEIGHT_COMPACT, 32.0);
    assert_eq!(TRIGGER_HEIGHT_COMPACT, 28.0);
    assert_eq!(TRIGGER_HEIGHT_STANDARD, 32.0);
}

#[test]
fn test_trigger_style_holds_surface_active_and_border_strong_when_open() {
    let palette = Palette::dark();

    // When menu is open: held active surface + strong border (UIF-NAV-001)
    let open_style = resolve_trigger_style(&palette, true, false, false);
    assert_eq!(open_style.background, palette.surface_active);
    assert_eq!(open_style.border.color, palette.border_strong);
    assert_eq!(open_style.border.width, 1.0);
    assert_eq!(open_style.text_color, palette.text_strong);

    // When hovered (not open): hover surface + subtle border
    let hover_style = resolve_trigger_style(&palette, false, true, false);
    assert_eq!(hover_style.background, palette.surface_hover);
    assert_eq!(hover_style.border.color, palette.border_subtle);
    assert_eq!(hover_style.text_color, palette.text_primary);

    // Resting state: transparent fill
    let rest_style = resolve_trigger_style(&palette, false, false, false);
    assert_eq!(rest_style.background, Color::TRANSPARENT);
    assert_eq!(rest_style.border.width, 0.0);
}

#[test]
fn test_navbar_model_toggle_menu_latch() {
    let mut model = NavBarModel::new()
        .with_brand("fastmd")
        .with_trigger(NavBarMenuTrigger::new("file", "File").with_tooltip("File operations"))
        .with_trigger(NavBarMenuTrigger::new("edit", "Edit").with_tooltip("Edit commands"));

    assert_eq!(model.active_menu_id, None);
    assert!(!model.triggers[0].is_open);
    assert!(!model.triggers[1].is_open);

    // Toggle "file" opens it
    model.toggle_menu("file");
    assert_eq!(model.active_menu_id.as_deref(), Some("file"));
    assert!(model.triggers[0].is_open);
    assert!(!model.triggers[1].is_open);

    // Toggle "file" again closes it (latch behavior)
    model.toggle_menu("file");
    assert_eq!(model.active_menu_id, None);
    assert!(!model.triggers[0].is_open);

    // Toggle "edit" opens edit
    model.toggle_menu("edit");
    assert_eq!(model.active_menu_id.as_deref(), Some("edit"));
    assert!(!model.triggers[0].is_open);
    assert!(model.triggers[1].is_open);

    // Close all
    model.close_menu();
    assert_eq!(model.active_menu_id, None);
    assert!(!model.triggers[1].is_open);
}

#[test]
fn test_navbar_widget_into_element_renders_cleanly() {
    #[allow(dead_code)]
    #[derive(Clone, Debug)]
    enum Msg {
        ToggleMenu(String),
    }

    let model = NavBarModel::new()
        .with_brand("fastmd")
        .with_trigger(NavBarMenuTrigger::new("file", "File").with_tooltip("File menu"))
        .with_trigger(NavBarMenuTrigger::new("help", "Help"));

    let navbar = NavBar::new(&model)
        .compact()
        .on_toggle_menu(Msg::ToggleMenu);

    let _el: Element<'_, Msg> = navbar.into();
}
