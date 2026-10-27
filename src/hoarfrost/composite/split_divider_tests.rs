//! Unit tests for `split_divider` composite (`UIF-COMP-016`, `UIF-PANE-001`, `UIF-PRIN-006`).

use iced::{Point, Rectangle, Size};

use crate::binding::snapshot::GestureStart;
use crate::composite::split_divider::{
    SplitAxis, SplitDivider, SplitDividerModel, SplitDividerResize, compute_grab_rect,
    split_divider_geometry, split_divider_layers,
};
use crate::state::{BaseState, VisualState};
use crate::token::color::Palette;
use crate::token::contrast::contrast_ratio;
use crate::token::shape;
use crate::token::size;
use crate::token::space;

#[test]
fn test_split_divider_model_clamping() {
    // Basic model initialization clamped to [0.0, 1.0]
    let model = SplitDividerModel::new(SplitAxis::Horizontal, 0.5);
    assert_eq!(model.axis, SplitAxis::Horizontal);
    assert_eq!(model.ratio, 0.5);
    assert_eq!(model.min_ratio, 0.0);
    assert_eq!(model.max_ratio, 1.0);
    assert!(!model.disabled);

    // Negative ratio clamped to 0.0
    let model_under = SplitDividerModel::new(SplitAxis::Horizontal, -0.2);
    assert_eq!(model_under.ratio, 0.0);

    // Excessive ratio clamped to 1.0
    let model_over = SplitDividerModel::new(SplitAxis::Horizontal, 1.5);
    assert_eq!(model_over.ratio, 1.0);

    // Custom min and max ratios
    let model_bounded = SplitDividerModel::new(SplitAxis::Vertical, 0.1)
        .with_min_ratio(0.2)
        .with_max_ratio(0.8)
        .with_disabled(true);

    assert_eq!(model_bounded.axis, SplitAxis::Vertical);
    assert_eq!(model_bounded.min_ratio, 0.2);
    assert_eq!(model_bounded.max_ratio, 0.8);
    assert_eq!(model_bounded.clamped_ratio(), 0.2);
    assert!(model_bounded.disabled);

    let model_bounded_over = SplitDividerModel::new(SplitAxis::Vertical, 0.95)
        .with_min_ratio(0.2)
        .with_max_ratio(0.8);
    assert_eq!(model_bounded_over.clamped_ratio(), 0.8);
}

#[test]
fn test_split_axis_properties() {
    let horizontal = SplitAxis::Horizontal;
    let vertical = SplitAxis::Vertical;

    assert_ne!(horizontal, vertical);
    assert_eq!(SplitAxis::default(), SplitAxis::Horizontal);
}

#[test]
fn test_mount_safety_zero_size_no_panic_or_divide_by_zero() {
    // 0x0 size bounds inside Length::Fixed(0.0) region
    let zero_bounds = Rectangle::new(Point::ORIGIN, Size::ZERO);
    let palette = Palette::dark();
    let visual = VisualState {
        base: BaseState::Rest,
        focused: false,
    };

    let layers_h = split_divider_layers(zero_bounds, SplitAxis::Horizontal, visual, &palette);
    assert!(
        layers_h.is_empty(),
        "zero bounds must emit empty layers rather than collapsing layout"
    );

    let layers_v = split_divider_layers(zero_bounds, SplitAxis::Vertical, visual, &palette);
    assert!(
        layers_v.is_empty(),
        "zero bounds must emit empty layers rather than collapsing layout"
    );

    let (stroke_rect, pill_rect) = split_divider_geometry(zero_bounds, SplitAxis::Horizontal);
    assert!(stroke_rect.width >= 0.0 && stroke_rect.height >= 0.0);
    assert!(pill_rect.width >= 0.0 && pill_rect.height >= 0.0);

    // Ensure update calculation does not divide by zero when total_length is 0.0
    let mut resize = SplitDividerResize::new(SplitAxis::Horizontal, 0.5, 0.0, 1.0);
    let start = resize.begin(0.5, 100.0, 0.0, 0.0, 1.0);
    assert_eq!(start, GestureStart::Started);

    let live = resize.update(200.0);
    assert!(
        live.is_none(),
        "zero total length must safely yield 0.0 delta and not divide by zero"
    );
    assert_eq!(resize.current_ratio(), 0.5);
}

#[test]
fn test_resting_visual_tokens_and_geometry() {
    let palette = Palette::dark();
    let bounds = Rectangle::new(Point::new(100.0, 50.0), Size::new(10.0, 500.0));
    let visual = VisualState {
        base: BaseState::Rest,
        focused: false,
    };

    let layers = split_divider_layers(bounds, SplitAxis::Horizontal, visual, &palette);
    assert_eq!(layers.len(), 2);

    let (name_stroke, stroke_layer) = &layers[0];
    let (name_pill, pill_layer) = &layers[1];

    assert_eq!(*name_stroke, "divider-stroke");
    assert_eq!(*name_pill, "grip-pill");

    // 1px stroke in color.border.subtle
    assert_eq!(stroke_layer.bounds.width, 1.0);
    assert_eq!(stroke_layer.bounds.height, 500.0);
    assert_eq!(stroke_layer.fill, palette.border_subtle);

    // 2px x 24px grip pill in color.border.strong
    assert_eq!(pill_layer.bounds.width, space::XXS);
    assert_eq!(pill_layer.bounds.height, size::HIT_TARGET_MIN);
    assert_eq!(pill_layer.fill, palette.border_strong);
    assert_eq!(pill_layer.border.radius, shape::RADIUS_PILL.into());

    // Pill must be centered along the split stroke
    let expected_center_y = bounds.y + (bounds.height - size::HIT_TARGET_MIN) / 2.0;
    assert_eq!(pill_layer.bounds.y, expected_center_y);

    // Grab zone spans 10px (size.splitter.grab)
    let grab_rect = compute_grab_rect(bounds, SplitAxis::Horizontal);
    assert_eq!(grab_rect.width, size::SPLITTER_GRAB);
    assert_eq!(grab_rect.height, bounds.height);
}

#[test]
fn test_vertical_axis_geometry() {
    let palette = Palette::dark();
    let bounds = Rectangle::new(Point::new(50.0, 100.0), Size::new(600.0, 10.0));
    let visual = VisualState {
        base: BaseState::Rest,
        focused: false,
    };

    let layers = split_divider_layers(bounds, SplitAxis::Vertical, visual, &palette);
    assert_eq!(layers.len(), 2);

    let (_, stroke_layer) = &layers[0];
    let (_, pill_layer) = &layers[1];

    // 1px horizontal stroke
    assert_eq!(stroke_layer.bounds.width, 600.0);
    assert_eq!(stroke_layer.bounds.height, 1.0);
    assert_eq!(stroke_layer.fill, palette.border_subtle);

    // 24px x 2px horizontal grip pill
    assert_eq!(pill_layer.bounds.width, size::HIT_TARGET_MIN);
    assert_eq!(pill_layer.bounds.height, space::XXS);
    assert_eq!(pill_layer.fill, palette.border_strong);
    assert_eq!(pill_layer.border.radius, shape::RADIUS_PILL.into());

    let expected_center_x = bounds.x + (bounds.width - size::HIT_TARGET_MIN) / 2.0;
    assert_eq!(pill_layer.bounds.x, expected_center_x);
}

#[test]
fn test_hover_and_drag_promotion_accent_border() {
    let palette = Palette::dark();
    let bounds = Rectangle::new(Point::ORIGIN, Size::new(10.0, 300.0));

    // Hover state
    let hover_visual = VisualState {
        base: BaseState::Hovered,
        focused: false,
    };
    let hover_layers = split_divider_layers(bounds, SplitAxis::Horizontal, hover_visual, &palette);
    assert_eq!(hover_layers[0].1.fill, palette.accent_border);
    assert_eq!(hover_layers[1].1.fill, palette.accent_border);

    // Active drag (Pressed) state
    let drag_visual = VisualState {
        base: BaseState::Pressed,
        focused: false,
    };
    let drag_layers = split_divider_layers(bounds, SplitAxis::Horizontal, drag_visual, &palette);
    assert_eq!(drag_layers[0].1.fill, palette.accent_border);
    assert_eq!(drag_layers[1].1.fill, palette.accent_border);

    // Contrast on surface.raised must clear 3.0:1
    let contrast = contrast_ratio(palette.accent_border, palette.surface_raised);
    assert!(
        contrast >= 3.0,
        "accent_border contrast on surface_raised ({contrast:.2}:1) must clear 3.0:1"
    );
}

#[test]
fn test_disabled_divider_visual_and_immutability() {
    let palette = Palette::dark();
    let bounds = Rectangle::new(Point::ORIGIN, Size::new(10.0, 300.0));
    let disabled_visual = VisualState {
        base: BaseState::Disabled,
        focused: false,
    };

    let layers = split_divider_layers(bounds, SplitAxis::Horizontal, disabled_visual, &palette);
    // Stroke is border.subtle, grip pill is muted in border.subtle
    assert_eq!(layers[0].1.fill, palette.border_subtle);
    assert_eq!(layers[1].1.fill, palette.border_subtle);

    // Interaction returns None when disabled
    let model = SplitDividerModel::new(SplitAxis::Horizontal, 0.4).with_disabled(true);
    let divider: SplitDivider<'_, ()> = SplitDivider::new(model);
    assert!(divider.model.disabled);
}

#[test]
fn test_drag_lifecycle_change_commit_cancel() {
    let mut resize = SplitDividerResize::new(SplitAxis::Horizontal, 0.3, 0.1, 0.9);
    assert!(!resize.is_active());
    assert_eq!(resize.start_ratio(), 0.3);
    assert_eq!(resize.current_ratio(), 0.3);

    // Begin drag at x = 300.0 with total width = 1000.0
    let start = resize.begin(0.3, 300.0, 1000.0, 0.1, 0.9);
    assert_eq!(start, GestureStart::Started);
    assert!(resize.is_active());

    // Pointer moves 50px right -> delta ratio = +0.05 -> new ratio = 0.35
    let live = resize.update(350.0);
    let ratio = live.as_ref().map(|l| *l.value()).unwrap_or_default();
    assert!((ratio - 0.35).abs() < 1e-5);
    assert!((resize.current_ratio() - 0.35).abs() < 1e-5);

    // Duplicate move with same coordinate -> coalesced, returns None
    let live_dup = resize.update(350.0);
    assert!(live_dup.is_none());

    // Commit release
    let commit = resize.commit();
    let committed = commit.as_ref().map(|c| *c.value()).unwrap_or_default();
    assert!((committed - 0.35).abs() < 1e-5);
    assert!(!resize.is_active());

    // A second commit after settling produces None
    assert!(resize.commit().is_none());

    // New drag test for Cancel
    let mut resize2 = SplitDividerResize::new(SplitAxis::Horizontal, 0.4, 0.1, 0.9);
    let _ = resize2.begin(0.4, 400.0, 1000.0, 0.1, 0.9);
    let _ = resize2.update(500.0);
    assert_eq!(resize2.current_ratio(), 0.5);

    let cancel = resize2.cancel();
    assert_eq!(cancel.as_ref().map(|c| c.restored().ratio), Some(0.4));
    assert!(!resize2.is_active());
    assert_eq!(resize2.current_ratio(), 0.4);
}

#[test]
fn test_zero_overshoot_clamping_from_last_clamped_position() {
    let mut resize = SplitDividerResize::new(SplitAxis::Horizontal, 0.5, 0.1, 0.8);
    let _ = resize.begin(0.5, 500.0, 1000.0, 0.1, 0.8);

    // Move 500px right (to x = 1000.0), which would ask for ratio 1.0, but clamps to 0.8
    let live1 = resize.update(1000.0);
    assert_eq!(live1.as_ref().map(|l| *l.value()), Some(0.8));
    assert_eq!(resize.current_ratio(), 0.8);

    // Move further right to x = 1500.0 -> remains clamped at 0.8
    let live2 = resize.update(1500.0);
    assert!(live2.is_none());
    assert_eq!(resize.current_ratio(), 0.8);

    // Now reverse by just 10px (from 1500.0 to 1490.0) -> delta = -10px (-0.01 ratio)
    // Because clamping applies from the last clamped position (0.8), it immediately moves to 0.79!
    let live3 = resize.update(1490.0);
    assert_eq!(live3.as_ref().map(|l| *l.value()), Some(0.79));
    assert_eq!(resize.current_ratio(), 0.79);
}

#[test]
fn test_activation_gate_suppresses_duplicate_press_starts() {
    let mut resize = SplitDividerResize::new(SplitAxis::Horizontal, 0.5, 0.0, 1.0);
    let start1 = resize.begin(0.5, 100.0, 1000.0, 0.0, 1.0);
    assert_eq!(start1, GestureStart::Started);

    // While in flight, second begin is refused by ActivationGate
    let start2 = resize.begin(0.5, 100.0, 1000.0, 0.0, 1.0);
    assert_eq!(start2, GestureStart::AlreadyOpen);
    assert_eq!(resize.gate().suppressed(), 1);

    let _ = resize.commit();
    assert!(!resize.gate().is_pending());

    // After settle, next begin is permitted
    let start3 = resize.begin(0.5, 100.0, 1000.0, 0.0, 1.0);
    assert_eq!(start3, GestureStart::Started);
}

#[test]
fn test_pre_rank_capture_registration() {
    let resize = SplitDividerResize::new(SplitAxis::Horizontal, 0.5, 0.0, 1.0);
    let capture = resize.capture("workspace-split");
    assert_eq!(capture.id().as_str(), "workspace-split");
    assert_eq!(
        capture.kind(),
        crate::binding::pre_rank::DragKind::SplitDivider
    );
}

#[test]
fn test_contrast_compliance_against_surface_raised() {
    let p = Palette::dark();

    // Resting stroke in border.subtle (1.27:1)
    let subtle_contrast = contrast_ratio(p.border_subtle, p.surface_raised);
    assert!(
        (1.25..=1.30).contains(&subtle_contrast),
        "border_subtle on surface_raised should be ~1.27:1, got {subtle_contrast:.2}:1"
    );

    // Grip pill in border.strong (4.24:1 >= 3.0:1)
    let pill_contrast = contrast_ratio(p.border_strong, p.surface_raised);
    assert!(
        pill_contrast >= 4.0,
        "border_strong on surface_raised must clear 4.0:1, got {pill_contrast:.2}:1"
    );

    // Promoted in accent.border (6.27:1 >= 3.0:1)
    let accent_contrast = contrast_ratio(p.accent_border, p.surface_raised);
    assert!(
        accent_contrast >= 6.0,
        "accent_border on surface_raised must clear 6.0:1, got {accent_contrast:.2}:1"
    );
}

#[test]
fn test_gallery_split_divider_snapshot_quads() {
    let p = Palette::dark();
    let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(10.0, 400.0));
    let snapshots = crate::gallery::split_divider_snapshot_quads(SplitAxis::Horizontal, bounds, &p);

    assert_eq!(snapshots.len(), 4);
    assert_eq!(snapshots[0].0, "resting");
    assert_eq!(snapshots[1].0, "hovered");
    assert_eq!(snapshots[2].0, "dragged");
    assert_eq!(snapshots[3].0, "disabled");

    // Resting: stroke is border.subtle, pill is border.strong
    let resting_quads = &snapshots[0].1;
    assert_eq!(resting_quads.len(), 2);
    assert_eq!(resting_quads[0].layer, "divider-stroke");
    assert_eq!(resting_quads[0].fill, "border.subtle");
    assert_eq!(resting_quads[1].layer, "grip-pill");
    assert_eq!(resting_quads[1].fill, "border.strong");

    // Hovered: stroke is accent.border, pill is accent.border
    let hovered_quads = &snapshots[1].1;
    assert_eq!(hovered_quads[0].fill, "accent.border");
    assert_eq!(hovered_quads[1].fill, "accent.border");

    // Dragged: stroke is accent.border, pill is accent.border
    let dragged_quads = &snapshots[2].1;
    assert_eq!(dragged_quads[0].fill, "accent.border");
    assert_eq!(dragged_quads[1].fill, "accent.border");

    // Disabled: stroke is border.subtle, pill is border.subtle
    let disabled_quads = &snapshots[3].1;
    assert_eq!(disabled_quads[0].fill, "border.subtle");
    assert_eq!(disabled_quads[1].fill, "border.subtle");
}

#[test]
fn test_element_conversion() {
    let model = SplitDividerModel::new(SplitAxis::Horizontal, 0.5);
    let divider: SplitDivider<'_, ()> = SplitDivider::new(model);
    let _element: iced::Element<'_, ()> = divider.into();
}
