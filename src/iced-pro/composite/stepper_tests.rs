//! Unit tests for `stepper` composite (`UIF-COMP-011`, `UIF-PRIN-005`, `UIF-PRIN-004`).

use super::*;
use crate::token::color::Palette;
use crate::token::contrast::contrast_ratio;

#[test]
fn test_step_status_traits() {
    assert_eq!(StepStatus::Pending, StepStatus::Pending);
    assert_ne!(StepStatus::Pending, StepStatus::Current);
    assert_ne!(StepStatus::Current, StepStatus::Completed);
    assert_ne!(StepStatus::Completed, StepStatus::Error);

    let cloned = StepStatus::Completed;
    assert_eq!(cloned, StepStatus::Completed);
    assert_eq!(format!("{:?}", StepStatus::Error), "Error");
}

#[test]
fn test_stepper_orientation_default_and_traits() {
    assert_eq!(
        StepperOrientation::default(),
        StepperOrientation::Horizontal
    );
    assert_eq!(
        StepperOrientation::Horizontal,
        StepperOrientation::Horizontal
    );
    assert_ne!(StepperOrientation::Horizontal, StepperOrientation::Vertical);
    assert_eq!(format!("{:?}", StepperOrientation::Vertical), "Vertical");
}

#[test]
fn test_step_item_creation_and_disabled_modifier() {
    let item = StepItem::new(1, "Account Details", StepStatus::Pending);
    assert_eq!(item.id, 1);
    assert_eq!(item.label, "Account Details");
    assert_eq!(item.status, StepStatus::Pending);
    assert!(!item.disabled);

    let disabled_item = item.disabled(true);
    assert!(disabled_item.disabled);
    assert_eq!(disabled_item.id, 1);
}

#[test]
fn test_stepper_model_navigation_and_indices() {
    let steps = vec![
        StepItem::new(1, "First", StepStatus::Completed),
        StepItem::new(2, "Second", StepStatus::Current),
        StepItem::new(3, "Third", StepStatus::Pending),
    ];
    let model = StepperModel::new(steps, 2, true);

    assert_eq!(model.current_index(), Some(1));
    assert!(model.can_previous());
    assert!(model.can_next());
    assert!(!model.is_last_step());
    assert_eq!(model.previous_step_id(), Some(1));
    assert_eq!(model.next_step_id(), Some(3));
}

#[test]
fn test_stepper_model_boundary_steps() {
    let steps = vec![
        StepItem::new("a", "Step A", StepStatus::Current),
        StepItem::new("b", "Step B", StepStatus::Pending),
    ];
    let first_step_model = StepperModel::new(steps.clone(), "a", true);
    assert_eq!(first_step_model.current_index(), Some(0));
    assert!(!first_step_model.can_previous());
    assert!(first_step_model.can_next());
    assert!(!first_step_model.is_last_step());
    assert_eq!(first_step_model.previous_step_id(), None);
    assert_eq!(first_step_model.next_step_id(), Some("b"));

    let last_step_model = StepperModel::new(steps, "b", true);
    assert_eq!(last_step_model.current_index(), Some(1));
    assert!(last_step_model.can_previous());
    assert!(!last_step_model.can_next());
    assert!(last_step_model.is_last_step());
    assert_eq!(last_step_model.previous_step_id(), Some("a"));
    assert_eq!(last_step_model.next_step_id(), None);
}

#[test]
fn test_stepper_model_empty_and_missing_current() {
    let empty_model: StepperModel<usize> = StepperModel::new(vec![], 0, false);
    assert_eq!(empty_model.current_index(), None);
    assert!(!empty_model.can_previous());
    assert!(!empty_model.can_next());
    assert!(!empty_model.is_last_step());
    assert_eq!(empty_model.previous_step_id(), None);
    assert_eq!(empty_model.next_step_id(), None);
    assert!(!empty_model.can_select(&0));

    let steps = vec![StepItem::new(1, "One", StepStatus::Pending)];
    let missing_model = StepperModel::new(steps, 999, false);
    assert_eq!(missing_model.current_index(), None);
    assert!(!missing_model.can_previous());
    assert!(!missing_model.can_next());
    assert!(!missing_model.is_last_step());
}

#[test]
fn test_can_select_non_linear_mode() {
    let steps = vec![
        StepItem::new(10, "Profile", StepStatus::Pending),
        StepItem::new(20, "Billing", StepStatus::Current),
        StepItem::new(30, "Confirm", StepStatus::Pending).disabled(true),
    ];
    let model = StepperModel::new(steps, 20, false);

    assert!(
        model.can_select(&10),
        "non-disabled step can be selected in non-linear mode"
    );
    assert!(
        model.can_select(&20),
        "current step can be selected in non-linear mode"
    );
    assert!(
        !model.can_select(&30),
        "disabled step cannot be selected in non-linear mode"
    );
    assert!(!model.can_select(&999), "unknown step returns false");
}

#[test]
fn test_can_select_linear_mode() {
    let steps = vec![
        StepItem::new(1, "Step 1", StepStatus::Completed),
        StepItem::new(2, "Step 2", StepStatus::Current),
        StepItem::new(3, "Step 3", StepStatus::Pending),
        StepItem::new(4, "Step 4", StepStatus::Pending),
        StepItem::new(5, "Step 5", StepStatus::Pending).disabled(true),
    ];
    let model = StepperModel::new(steps, 2, true);

    // Completed step can be selected
    assert!(model.can_select(&1), "completed step is selectable");
    // Immediate next step (current_index + 1 = index 2 = step 3) can be selected
    assert!(model.can_select(&3), "immediate next step is selectable");
    // Non-completed future step (index 3 = step 4) cannot be selected
    assert!(
        !model.can_select(&4),
        "future steps beyond immediate next cannot be selected"
    );
    // Disabled step cannot be selected even if eligible
    assert!(!model.can_select(&5), "disabled step cannot be selected");
    // Unknown step cannot be selected
    assert!(!model.can_select(&100), "unknown step cannot be selected");
}

#[test]
fn test_can_select_linear_prior_incomplete_step() {
    let steps = vec![
        StepItem::new(1, "Step 1", StepStatus::Error),
        StepItem::new(2, "Step 2", StepStatus::Current),
        StepItem::new(3, "Step 3", StepStatus::Pending),
    ];
    let model = StepperModel::new(steps, 2, true);

    // Step 1 is in Error, not Completed -> cannot be selected in linear mode
    assert!(
        !model.can_select(&1),
        "error status prior step is not completed, so cannot be selected"
    );
    assert!(model.can_select(&3), "immediate next is selectable");
}

#[test]
fn test_tokens_and_geometry() {
    assert_eq!(
        BAR_HEIGHT, 36.0,
        "horizontal stepper height must be 36px (size.control.lg) per §3B.11"
    );
    assert_eq!(
        NODE_SIZE, 24.0,
        "step node dimensions must be 24x24px (size.hit_target.min) per §3B.11"
    );
    assert_eq!(
        NODE_RADIUS, 999.0,
        "step node radius must be radius.pill per §3B.11"
    );
    assert_eq!(
        CONNECTOR_STROKE, 1.0,
        "connector stroke must be 1px hairline per §3B.11"
    );
    assert_eq!(
        CURRENT_BORDER_WIDTH, 2.0,
        "current step border must be 2px (shape.border_focus) per §3B.11"
    );
    assert_eq!(
        LABEL_GAP, 8.0,
        "label gap must be 8px (space.sm) per §3B.11"
    );
    assert_eq!(
        STEP_GAP, 12.0,
        "step gap must be 12px (space.md) per §3B.11"
    );
    assert_eq!(
        FOOTER_BUTTON_HEIGHT, 32.0,
        "footer button height must be 32px (size.control.md)"
    );
}

#[test]
fn test_redundant_cues_contrast_and_glyphs() {
    let palette = Palette::dark();

    // 1. Completed: BG: accent.fill, FG: text.strong (>= 4.5:1), glyph '✓'
    assert_eq!(GLYPH_COMPLETED, "✓");
    let completed_contrast = contrast_ratio(palette.text_strong, palette.accent_fill);
    assert!(
        completed_contrast >= 4.5,
        "Completed step contrast ({:.2}:1) must clear 4.5:1 WCAG AA (nominal 4.80:1)",
        completed_contrast
    );

    // 2. Current: BG: surface.active, Border: 2px accent.border, FG: text.strong (>= 4.5:1), number
    let current_fg_contrast = contrast_ratio(palette.text_strong, palette.surface_active);
    assert!(
        current_fg_contrast >= 4.5,
        "Current step text contrast ({:.2}:1) must clear 4.5:1 (nominal 14.29:1)",
        current_fg_contrast
    );
    let current_border_contrast = contrast_ratio(palette.accent_border, palette.surface_active);
    assert!(
        current_border_contrast >= 3.0,
        "Current step border contrast ({:.2}:1) must clear 3.0:1 active chrome (nominal 4.79:1)",
        current_border_contrast
    );

    // 3. Pending: BG: surface.raised, Border: 1px border.strong, FG: text.muted (>= 4.5:1), number
    let pending_fg_contrast = contrast_ratio(palette.text_muted, palette.surface_raised);
    assert!(
        pending_fg_contrast >= 4.5,
        "Pending step text contrast ({:.2}:1) must clear 4.5:1 (nominal 7.16:1)",
        pending_fg_contrast
    );
    let pending_border_contrast = contrast_ratio(palette.border_strong, palette.surface_raised);
    assert!(
        pending_border_contrast >= 3.0,
        "Pending step border contrast ({:.2}:1) must clear 3.0:1 (nominal 4.24:1)",
        pending_border_contrast
    );

    // 4. Error: BG: status.danger.fill, FG: text.strong (>= 4.5:1), glyph '!'
    assert_eq!(GLYPH_ERROR, "!");
    let error_contrast = contrast_ratio(palette.text_strong, palette.status_danger_fill);
    assert!(
        error_contrast >= 4.5,
        "Error step contrast ({:.2}:1) must clear 4.5:1 (nominal 5.13:1)",
        error_contrast
    );
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum StepperMsg {
    Select(usize),
    Next(usize),
    Previous(usize),
    Finish,
    Cancel,
}

#[test]
fn test_widget_creation_and_builder_chain() {
    let steps = vec![
        StepItem::new(1, "Account", StepStatus::Completed),
        StepItem::new(2, "Profile", StepStatus::Current),
        StepItem::new(3, "Finish", StepStatus::Pending),
    ];
    let model = StepperModel::new(steps, 2, true);

    let widget: Stepper<'_, StepperMsg, usize> = stepper(model.clone())
        .orientation(StepperOrientation::Vertical)
        .on_step_select(|c| StepperMsg::Select(c.into_value()))
        .on_next(|c| StepperMsg::Next(c.into_value()))
        .on_previous(|c| StepperMsg::Previous(c.into_value()))
        .on_finish(|_c| StepperMsg::Finish)
        .on_cancel(|_c| StepperMsg::Cancel);

    assert_eq!(widget.model(), &model);
    assert_eq!(widget.orientation_value(), StepperOrientation::Vertical);

    // Conversion into Element
    let _element: Element<'_, StepperMsg> = widget.into();
}

#[test]
fn test_horizontal_stepper_element_conversion() {
    let steps = vec![
        StepItem::new("a", "Alpha", StepStatus::Completed),
        StepItem::new("b", "Beta", StepStatus::Current),
    ];
    let model = StepperModel::new(steps, "b", false);

    let element: Element<'_, StepperMsg> = stepper(model)
        .orientation(StepperOrientation::Horizontal)
        .on_step_select(|_| StepperMsg::Select(1))
        .into();

    let _ = element;
}

#[test]
fn test_wizard_footer_non_shifting_step_zero() {
    let steps = vec![
        StepItem::new(1, "Step 1", StepStatus::Current),
        StepItem::new(2, "Step 2", StepStatus::Pending),
    ];
    let model = StepperModel::new(steps, 1, true);

    let config = WizardFooterConfig::new()
        .on_previous(|c| StepperMsg::Previous(c.into_value()))
        .on_next(|c| StepperMsg::Next(c.into_value()))
        .on_finish(|_| StepperMsg::Finish)
        .on_cancel(|_| StepperMsg::Cancel);

    // On step 0, can_previous() is false, so [← Previous] renders disabled rather than disappearing
    assert!(!model.can_previous());
    assert!(model.can_next());
    assert!(!model.is_last_step());

    let _footer_element: Element<'_, StepperMsg> = wizard_footer(&model, config);
}

#[test]
fn test_wizard_footer_final_step() {
    let steps = vec![
        StepItem::new(1, "Step 1", StepStatus::Completed),
        StepItem::new(2, "Step 2", StepStatus::Current),
    ];
    let model = StepperModel::new(steps, 2, true);

    let config = WizardFooterConfig::new()
        .on_previous(|c| StepperMsg::Previous(c.into_value()))
        .on_finish(|_| StepperMsg::Finish);

    assert!(model.can_previous());
    assert!(!model.can_next());
    assert!(model.is_last_step());

    let _footer_element: Element<'_, StepperMsg> = wizard_footer(&model, config);
}
