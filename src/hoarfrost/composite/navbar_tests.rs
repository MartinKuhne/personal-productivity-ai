//! Tests for navigation bar, trigger styling, and ellipsis validation (`UIF-NAV-001`, `UIF-NAV-002`, `UIF-NAV-003`).

use super::*;
use crate::invariant_gate::InvariantCategory;
use crate::microcopy::action_label::PromptBehaviour;
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

#[test]
fn test_navbar_trigger_validate_ellipsis_happy_paths() {
    // Immediate trigger with no ellipsis passes
    let t1 = NavBarMenuTrigger::new("file", "File");
    assert!(t1.validate_ellipsis().is_ok());

    // Explicit immediate trigger with no ellipsis passes
    let t2 = NavBarMenuTrigger::new("save", "Save").with_dialog_prompt(false);
    assert!(t2.validate_ellipsis().is_ok());

    // Dialog trigger with ellipsis passes
    let t3 = NavBarMenuTrigger::new("rename", "Rename...").with_dialog_prompt(true);
    assert!(t3.validate_ellipsis().is_ok());

    // PromptBehaviour builder variant
    let t4 = NavBarMenuTrigger::new("move", "Move...")
        .with_prompt_behaviour(PromptBehaviour::LaunchesPrompt);
    assert!(t4.validate_ellipsis().is_ok());

    // Icon-only trigger with compliant tooltip
    let t5 = NavBarMenuTrigger::new("export", "")
        .with_tooltip("Export as PDF...")
        .with_dialog_prompt(true);
    assert!(t5.validate_ellipsis().is_ok());

    // Icon-only immediate trigger with compliant tooltip
    let t6 = NavBarMenuTrigger::new("close", "")
        .with_tooltip("Close Tab")
        .with_dialog_prompt(false);
    assert!(t6.validate_ellipsis().is_ok());

    // Empty trigger without tooltip passes
    let t7 = NavBarMenuTrigger::new("blank", "");
    assert!(t7.validate_ellipsis().is_ok());
}

#[test]
fn test_navbar_trigger_validate_ellipsis_rejects_missing_ellipsis_for_dialog() {
    let t = NavBarMenuTrigger::new("rename", "Rename").with_dialog_prompt(true);
    let err = t
        .validate_ellipsis()
        .expect_err("should reject missing ellipsis");
    assert_eq!(err.category, InvariantCategory::ActionEllipsis);
    assert_eq!(err.target, "Rename");
    assert!(err.message.contains("opens a secondary prompt"));
}

#[test]
fn test_navbar_trigger_validate_ellipsis_rejects_spurious_ellipsis_for_immediate() {
    // Default immediate
    let t1 = NavBarMenuTrigger::new("save", "Save...");
    let err1 = t1
        .validate_ellipsis()
        .expect_err("should reject spurious ellipsis");
    assert_eq!(err1.category, InvariantCategory::ActionEllipsis);
    assert_eq!(err1.target, "Save...");
    assert!(err1.message.contains("runs immediately"));

    // Explicit immediate
    let t2 = NavBarMenuTrigger::new("save", "Save...").with_dialog_prompt(false);
    let err2 = t2
        .validate_ellipsis()
        .expect_err("should reject spurious ellipsis");
    assert_eq!(err2.category, InvariantCategory::ActionEllipsis);
    assert_eq!(err2.target, "Save...");
}

#[test]
fn test_navbar_trigger_validate_ellipsis_rejects_icon_only_tooltip_violations() {
    // Dialog without ellipsis on icon tooltip
    let t1 = NavBarMenuTrigger::new("settings", "")
        .with_tooltip("Open Settings")
        .with_dialog_prompt(true);
    let err1 = t1
        .validate_ellipsis()
        .expect_err("should reject missing tooltip ellipsis");
    assert_eq!(err1.category, InvariantCategory::ActionEllipsis);
    assert_eq!(err1.target, "Open Settings");
    assert!(err1.message.contains("opens a secondary prompt"));

    // Immediate with spurious ellipsis on icon tooltip
    let t2 = NavBarMenuTrigger::new("save", "")
        .with_tooltip("Save...")
        .with_dialog_prompt(false);
    let err2 = t2
        .validate_ellipsis()
        .expect_err("should reject spurious tooltip ellipsis");
    assert_eq!(err2.category, InvariantCategory::ActionEllipsis);
    assert_eq!(err2.target, "Save...");
    assert!(err2.message.contains("runs immediately"));
}
